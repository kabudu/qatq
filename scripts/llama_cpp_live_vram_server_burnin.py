#!/usr/bin/env python3
"""Repeat a llama-server live-VRAM matrix as a bounded burn-in gate.

The matrix runner proves one configured native/QATQ policy pass. This wrapper
repeats that pass, fails on the first failed run, and optionally checks that
selected per-case metrics do not drift wildly across runs.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class BurnInRun:
    index: int
    status: str
    returncode: int
    elapsed_seconds: float
    work_dir: Path
    summary_path: Path
    stdout_path: Path
    stderr_path: Path
    failure: str
    summary: dict[str, Any]
    timed_out: bool = False
    timeout_seconds: int = 0
    cleanup_signal: str | None = None
    cleanup_escalated: bool = False


@dataclass(frozen=True)
class BurnInPreparedConfig:
    original_path: Path
    effective_path: Path
    config: dict[str, Any]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True, help="Matrix config JSON")
    parser.add_argument(
        "--matrix-runner",
        default="scripts/llama_cpp_live_vram_server_cancel_matrix.py",
    )
    parser.add_argument(
        "--probe-runner",
        default="scripts/llama_cpp_live_vram_server_cancel_probe.py",
    )
    parser.add_argument(
        "--llama-server",
        default="/private/tmp/qatq-llama-live-work/build/bin/llama-server",
    )
    parser.add_argument(
        "--model-root",
        default="",
        help=(
            "Optional directory used to resolve every case model to "
            "<model-root>/<original model filename> before running."
        ),
    )
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-server-burnin")
    parser.add_argument("--runs", type=int, default=2)
    parser.add_argument("--timeout", type=int, default=3600, help="Per-case matrix timeout in seconds")
    parser.add_argument(
        "--run-timeout",
        type=int,
        default=0,
        help=(
            "Whole matrix-run timeout in seconds. Default 0 derives a bounded "
            "ceiling from --timeout and the configured case count."
        ),
    )
    parser.add_argument("--max-cases", type=int, default=0)
    parser.add_argument("--max-total-seconds", type=float, default=0.0)
    parser.add_argument(
        "--min-passed-elapsed-seconds",
        type=float,
        default=0.0,
        help=(
            "Fail unless completed passing runs accumulate at least this many "
            "wall-clock seconds. Use 3600 for the sustained one-hour gate and "
            "28800 or higher for overnight soak evidence."
        ),
    )
    parser.add_argument(
        "--require-backend-memory-diagnostics",
        action="store_true",
        help=(
            "Fail unless every completed matrix case has projected device "
            "memory plus non-host accelerator memory breakdown."
        ),
    )
    parser.add_argument(
        "--require-soak-memory-metrics",
        action="store_true",
        help=(
            "Fail unless every completed matrix case exports RSS growth, "
            "steady-state RSS tail growth, and RSS tail range metrics."
        ),
    )
    parser.add_argument("--max-rss-growth-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-rss-tail-growth-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-backend-kv-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-projected-device-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-direct-peak-vram-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--preflight-only", action="store_true")
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    validate_args(args)
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    prepared_config = prepare_config(args, work_dir)
    args.config = str(prepared_config.effective_path)

    plan = build_plan(args, work_dir, prepared_config)
    write_json(work_dir / "server-burnin-plan.json", plan)
    preflight = build_preflight_report(args, work_dir, prepared_config)
    write_json(work_dir / "preflight.json", preflight)
    write_preflight_markdown(work_dir / "preflight.md", preflight)
    if preflight["status"] != "pass":
        print((work_dir / "preflight.md").read_text(encoding="utf-8"))
        return 1
    if args.preflight_only:
        summary = build_preflight_only_summary(args, work_dir, prepared_config, preflight)
        write_json(work_dir / "summary.json", summary)
        write_markdown(work_dir / "summary.md", summary)
        print((work_dir / "summary.md").read_text(encoding="utf-8"))
        return 0

    runs: list[BurnInRun] = []
    started = time.monotonic()
    for index in range(1, args.runs + 1):
        if args.max_total_seconds > 0.0 and time.monotonic() - started >= args.max_total_seconds:
            runs.append(
                BurnInRun(
                    index=index,
                    status="fail",
                    returncode=124,
                    elapsed_seconds=0.0,
                    work_dir=work_dir / f"run-{index:03d}",
                    summary_path=work_dir / f"run-{index:03d}" / "summary.json",
                    stdout_path=work_dir / f"run-{index:03d}" / "matrix-stdout.log",
                    stderr_path=work_dir / f"run-{index:03d}" / "matrix-stderr.log",
                    failure=f"burn-in exceeded max total seconds before run {index}",
                    summary={},
                    timed_out=False,
                    timeout_seconds=args.run_timeout or derived_run_timeout(args),
                )
            )
            break
        run_timeout = run_timeout_for_remaining_total(args, started)
        run = run_matrix(args, index, work_dir / f"run-{index:03d}", run_timeout=run_timeout)
        runs.append(run)
        if run.status == "fail":
            break

    summary = build_summary(args, work_dir, runs)
    summary["preflight"] = preflight
    summary["config"] = str(prepared_config.effective_path)
    summary["original_config"] = str(prepared_config.original_path)
    write_json(work_dir / "summary.json", summary)
    write_markdown(work_dir / "summary.md", summary)
    print((work_dir / "summary.md").read_text(encoding="utf-8"))
    return 0 if summary["status"] in {"pass", "dry-run"} else 1


def validate_args(args: argparse.Namespace) -> None:
    require(args.runs > 0, "--runs must be positive")
    require(args.timeout > 0, "--timeout must be positive")
    require(args.run_timeout >= 0, "--run-timeout must be non-negative")
    require(args.max_cases >= 0, "--max-cases must be non-negative")
    require(args.max_total_seconds >= 0.0, "--max-total-seconds must be non-negative")
    require(args.min_passed_elapsed_seconds >= 0.0, "--min-passed-elapsed-seconds must be non-negative")
    require(args.max_rss_growth_jitter_ratio >= 0.0, "--max-rss-growth-jitter-ratio must be non-negative")
    require(
        args.max_rss_tail_growth_jitter_ratio >= 0.0,
        "--max-rss-tail-growth-jitter-ratio must be non-negative",
    )
    require(args.max_backend_kv_jitter_ratio >= 0.0, "--max-backend-kv-jitter-ratio must be non-negative")
    require(
        args.max_projected_device_jitter_ratio >= 0.0,
        "--max-projected-device-jitter-ratio must be non-negative",
    )
    require(
        args.max_direct_peak_vram_jitter_ratio >= 0.0,
        "--max-direct-peak-vram-jitter-ratio must be non-negative",
    )
    if args.model_root:
        require(Path(args.model_root).is_dir(), f"--model-root is not a directory: {args.model_root}")


def prepare_config(args: argparse.Namespace, work_dir: Path) -> BurnInPreparedConfig:
    original_path = Path(args.config)
    config = load_burnin_config(original_path)
    if args.max_cases:
        config = {**config, "cases": config["cases"][: args.max_cases]}
    if args.model_root:
        model_root = Path(args.model_root)
        resolved_cases = []
        for case in config["cases"]:
            model_path = Path(str(case["model"]))
            resolved = model_root / model_path.name
            resolved_cases.append({**case, "model": str(resolved)})
        config = {**config, "cases": resolved_cases}
    effective_path = work_dir / "server-burnin-effective-config.json"
    write_json(effective_path, config)
    return BurnInPreparedConfig(original_path=original_path, effective_path=effective_path, config=config)


def load_burnin_config(path: Path) -> dict[str, Any]:
    try:
        config = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise SystemExit(f"config file missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid JSON config {path}: {exc}") from exc
    require(isinstance(config, dict), "config must be a JSON object")
    defaults = config.get("defaults", {})
    require(isinstance(defaults, dict), "config defaults must be an object")
    cases = config.get("cases")
    require(isinstance(cases, list) and cases, "config must contain a non-empty cases array")
    for index, case in enumerate(cases, start=1):
        require(isinstance(case, dict), f"case {index} must be an object")
        require(isinstance(case.get("id"), str) and case["id"], f"case {index} missing id")
        require(isinstance(case.get("model"), str) and case["model"], f"case {case['id']} missing model")
    return config


def build_plan(
    args: argparse.Namespace,
    work_dir: Path,
    prepared_config: BurnInPreparedConfig,
) -> dict[str, Any]:
    return {
        "format": "qatq-live-vram-server-burnin-plan-v1",
        "config": str(prepared_config.effective_path),
        "original_config": str(prepared_config.original_path),
        "model_root": args.model_root,
        "matrix_runner": args.matrix_runner,
        "probe_runner": args.probe_runner,
        "llama_server": args.llama_server,
        "work_dir": str(work_dir),
        "runs": args.runs,
        "timeout_seconds": args.timeout,
        "run_timeout_seconds": args.run_timeout,
        "max_cases": args.max_cases,
        "max_total_seconds": args.max_total_seconds,
        "min_passed_elapsed_seconds": args.min_passed_elapsed_seconds,
        "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
        "require_soak_memory_metrics": args.require_soak_memory_metrics,
        "max_rss_growth_jitter_ratio": args.max_rss_growth_jitter_ratio,
        "max_rss_tail_growth_jitter_ratio": args.max_rss_tail_growth_jitter_ratio,
        "max_backend_kv_jitter_ratio": args.max_backend_kv_jitter_ratio,
        "max_projected_device_jitter_ratio": args.max_projected_device_jitter_ratio,
        "max_direct_peak_vram_jitter_ratio": args.max_direct_peak_vram_jitter_ratio,
        "dry_run": args.dry_run,
        "preflight_only": args.preflight_only,
        "run_work_dirs": [str(work_dir / f"run-{index:03d}") for index in range(1, args.runs + 1)],
    }


def build_preflight_report(
    args: argparse.Namespace,
    work_dir: Path,
    prepared_config: BurnInPreparedConfig,
) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []

    def add_check(name: str, passed: bool, detail: str, *, required: bool = True) -> None:
        checks.append(
            {
                "name": name,
                "status": "pass" if passed else "skip" if not required else "fail",
                "required": required,
                "detail": detail,
            }
        )

    add_check(
        "config",
        prepared_config.original_path.is_file() and prepared_config.effective_path.is_file(),
        f"original={prepared_config.original_path}; effective={prepared_config.effective_path}",
    )
    add_check(
        "matrix-runner",
        Path(args.matrix_runner).is_file(),
        args.matrix_runner,
    )
    add_check(
        "probe-runner",
        Path(args.probe_runner).is_file(),
        args.probe_runner,
    )
    executable_required = not args.dry_run
    add_check(
        "llama-server-executable",
        os.access(args.llama_server, os.X_OK),
        args.llama_server,
        required=executable_required,
    )
    model_required = not args.dry_run
    cases = prepared_config.config.get("cases", [])
    missing_models: list[str] = []
    if isinstance(cases, list):
        for case in cases:
            if not isinstance(case, dict):
                continue
            model = str(case.get("model", ""))
            if not Path(model).is_file():
                missing_models.append(f"{case.get('id', '<unknown>')}={model}")
    add_check(
        "model-files",
        not missing_models,
        "all selected models exist" if not missing_models else "; ".join(missing_models[:16]),
        required=model_required,
    )
    add_check(
        "duration-gate",
        args.min_passed_elapsed_seconds > 0.0,
        f"min_passed_elapsed_seconds={format_metric(args.min_passed_elapsed_seconds)}",
        required=not args.dry_run and args.preflight_only,
    )
    add_check(
        "soak-memory-gate",
        args.require_soak_memory_metrics,
        f"require_soak_memory_metrics={str(args.require_soak_memory_metrics).lower()}",
        required=not args.dry_run and args.preflight_only,
    )
    add_check(
        "backend-memory-gate",
        args.require_backend_memory_diagnostics,
        f"require_backend_memory_diagnostics={str(args.require_backend_memory_diagnostics).lower()}",
        required=not args.dry_run and args.preflight_only,
    )

    failures = [check for check in checks if check["required"] and check["status"] == "fail"]
    return {
        "format": "qatq-live-vram-server-burnin-preflight-v1",
        "status": "fail" if failures else "pass",
        "dry_run": args.dry_run,
        "preflight_only": args.preflight_only,
        "work_dir": str(work_dir),
        "config": str(prepared_config.effective_path),
        "original_config": str(prepared_config.original_path),
        "model_root": args.model_root,
        "selected_cases": len(cases) if isinstance(cases, list) else 0,
        "checks": checks,
        "failures": failures,
        "boundary": (
            "Preflight validates local runner inputs and required burn-in gates. "
            "It does not prove runtime correctness or memory stability."
        ),
    }


def build_preflight_only_summary(
    args: argparse.Namespace,
    work_dir: Path,
    prepared_config: BurnInPreparedConfig,
    preflight: dict[str, Any],
) -> dict[str, Any]:
    return {
        "format": "qatq-live-vram-server-burnin-summary-v1",
        "status": "pass",
        "dry_run": args.dry_run,
        "preflight_only": True,
        "work_dir": str(work_dir),
        "runs_requested": args.runs,
        "runs_completed": 0,
        "passed": 0,
        "dry_run_runs": 0,
        "failed": 0,
        "runs": [],
        "preflight": preflight,
        "sustained_runtime": {
            "passed_elapsed_seconds": 0.0,
            "required_passed_elapsed_seconds": args.min_passed_elapsed_seconds,
            "passing_runs": 0,
            "completed_runs": 0,
        },
        "sustained_runtime_failures": [],
        "aggregate_case_metrics": {},
        "aggregate_gate_failures": [],
        "backend_memory_diagnostics": {},
        "backend_memory_diagnostic_failures": [],
        "soak_memory_metrics": {},
        "soak_memory_metric_failures": [],
        "gates": {
            "min_passed_elapsed_seconds": args.min_passed_elapsed_seconds,
            "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
            "require_soak_memory_metrics": args.require_soak_memory_metrics,
            "max_rss_growth_jitter_ratio": args.max_rss_growth_jitter_ratio,
            "max_rss_tail_growth_jitter_ratio": args.max_rss_tail_growth_jitter_ratio,
            "max_backend_kv_jitter_ratio": args.max_backend_kv_jitter_ratio,
            "max_projected_device_jitter_ratio": args.max_projected_device_jitter_ratio,
            "max_direct_peak_vram_jitter_ratio": args.max_direct_peak_vram_jitter_ratio,
        },
        "boundary": (
            "Preflight-only burn-in summary. It proves the selected runner inputs "
            "and gates are present, not live runtime correctness."
        ),
        "config": str(prepared_config.effective_path),
        "original_config": str(prepared_config.original_path),
    }


def run_timeout_for_remaining_total(args: argparse.Namespace, started: float) -> float:
    run_timeout = float(args.run_timeout or derived_run_timeout(args))
    if args.max_total_seconds <= 0.0:
        return run_timeout
    remaining = args.max_total_seconds - (time.monotonic() - started)
    return max(0.001, min(run_timeout, remaining))


def run_matrix(
    args: argparse.Namespace,
    index: int,
    work_dir: Path,
    *,
    run_timeout: float | None = None,
) -> BurnInRun:
    work_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = work_dir / "matrix-stdout.log"
    stderr_path = work_dir / "matrix-stderr.log"
    summary_path = work_dir / "summary.json"
    command = [
        sys.executable,
        args.matrix_runner,
        "--config",
        args.config,
        "--probe-runner",
        args.probe_runner,
        "--llama-server",
        args.llama_server,
        "--work-dir",
        str(work_dir),
        "--timeout",
        str(args.timeout),
        "--keep-work-dir",
    ]
    if args.max_cases:
        command.extend(["--max-cases", str(args.max_cases)])
    if args.dry_run:
        command.append("--dry-run")
    run_timeout = float(run_timeout if run_timeout is not None else args.run_timeout or derived_run_timeout(args))
    started = time.monotonic()
    failure = ""
    proc = subprocess.Popen(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    cleanup_signal: str | None = None
    cleanup_escalated = False
    try:
        stdout, stderr = proc.communicate(timeout=run_timeout)
        returncode = proc.returncode if proc.returncode is not None else -1
    except subprocess.TimeoutExpired:
        timed_out = True
        cleanup_signal, cleanup_escalated = terminate_process_group(proc)
        stdout, stderr = proc.communicate()
        returncode = 124
        failure = (
            f"run {index} exceeded timeout of {format_seconds(run_timeout)}s; "
            f"cleanup={cleanup_signal}"
        )
    stdout_path.write_text(timeout_output_to_text(stdout), encoding="utf-8")
    stderr_text = timeout_output_to_text(stderr)
    if timed_out:
        stderr_text += (
            f"\nserver burn-in matrix run exceeded timeout of {format_seconds(run_timeout)}s; "
            f"cleanup={cleanup_signal}; escalated={str(cleanup_escalated).lower()}\n"
        )
    stderr_path.write_text(stderr_text, encoding="utf-8")
    elapsed = time.monotonic() - started
    summary, summary_failure = load_matrix_summary(summary_path)
    status = str(summary.get("status") or ("fail" if returncode else "pass"))
    if returncode != 0:
        status = "fail"
    if summary_failure:
        status = "fail"
        failure = summary_failure if not failure else f"{failure}; {summary_failure}"
    if status not in {"pass", "dry-run"} and not failure:
        failure = summarise_matrix_failure(summary)
    return BurnInRun(
        index=index,
        status=status,
        returncode=returncode,
        elapsed_seconds=elapsed,
        work_dir=work_dir,
        summary_path=summary_path,
        stdout_path=stdout_path,
        stderr_path=stderr_path,
        failure=failure,
        summary=summary,
        timed_out=timed_out,
        timeout_seconds=run_timeout,
        cleanup_signal=cleanup_signal,
        cleanup_escalated=cleanup_escalated,
    )


def build_summary(args: argparse.Namespace, work_dir: Path, runs: list[BurnInRun]) -> dict[str, Any]:
    run_summaries = [summarise_run(run) for run in runs]
    failures = [run for run in runs if run.status not in {"pass", "dry-run"}]
    aggregate = aggregate_case_metrics(runs)
    aggregate_failures = [] if args.dry_run else evaluate_aggregate_gates(args, aggregate)
    sustained_runtime = aggregate_sustained_runtime(args, runs)
    sustained_runtime_failures = [] if args.dry_run else evaluate_sustained_runtime(args, sustained_runtime)
    backend_memory_diagnostics = aggregate_backend_memory_diagnostics(runs)
    backend_memory_failures = (
        []
        if args.dry_run or not args.require_backend_memory_diagnostics
        else evaluate_backend_memory_diagnostics(backend_memory_diagnostics)
    )
    soak_memory_metrics = aggregate_soak_memory_metrics(runs)
    soak_memory_failures = (
        []
        if args.dry_run or not getattr(args, "require_soak_memory_metrics", False)
        else evaluate_soak_memory_metrics(soak_memory_metrics)
    )
    status = "dry-run" if runs and all(run.status == "dry-run" for run in runs) else "pass"
    if (
        failures
        or aggregate_failures
        or sustained_runtime_failures
        or backend_memory_failures
        or soak_memory_failures
        or len(runs) != args.runs
    ):
        status = "fail"
    return {
        "format": "qatq-live-vram-server-burnin-summary-v1",
        "status": status,
        "dry_run": args.dry_run,
        "work_dir": str(work_dir),
        "runs_requested": args.runs,
        "runs_completed": len(runs),
        "passed": sum(1 for run in runs if run.status == "pass"),
        "dry_run_runs": sum(1 for run in runs if run.status == "dry-run"),
        "failed": len(failures),
        "runs": run_summaries,
        "sustained_runtime": sustained_runtime,
        "sustained_runtime_failures": sustained_runtime_failures,
        "aggregate_case_metrics": aggregate,
        "aggregate_gate_failures": aggregate_failures,
        "backend_memory_diagnostics": backend_memory_diagnostics,
        "backend_memory_diagnostic_failures": backend_memory_failures,
        "soak_memory_metrics": soak_memory_metrics,
        "soak_memory_metric_failures": soak_memory_failures,
        "gates": {
            "min_passed_elapsed_seconds": getattr(args, "min_passed_elapsed_seconds", 0.0),
            "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
            "require_soak_memory_metrics": getattr(args, "require_soak_memory_metrics", False),
            "max_rss_growth_jitter_ratio": args.max_rss_growth_jitter_ratio,
            "max_rss_tail_growth_jitter_ratio": getattr(args, "max_rss_tail_growth_jitter_ratio", 0.0),
            "max_backend_kv_jitter_ratio": args.max_backend_kv_jitter_ratio,
            "max_projected_device_jitter_ratio": args.max_projected_device_jitter_ratio,
            "max_direct_peak_vram_jitter_ratio": args.max_direct_peak_vram_jitter_ratio,
        },
        "boundary": (
            "Bounded burn-in repetition for the configured llama-server live-VRAM "
            "matrix. It proves only the selected matrix config, run count, "
            "elapsed-duration gate, exported memory metrics, and aggregate "
            "jitter gates."
        ),
    }


def derived_run_timeout(args: argparse.Namespace) -> int:
    multiplier = args.max_cases if args.max_cases else 16
    return max(args.timeout, args.timeout * multiplier)


def summarise_run(run: BurnInRun) -> dict[str, Any]:
    return {
        "index": run.index,
        "status": run.status,
        "returncode": run.returncode,
        "elapsed_seconds": run.elapsed_seconds,
        "work_dir": str(run.work_dir),
        "summary": str(run.summary_path),
        "stdout": str(run.stdout_path),
        "stderr": str(run.stderr_path),
        "failure": run.failure,
        "total_cases": run.summary.get("total_cases"),
        "passed": run.summary.get("passed"),
        "comparison_gate_failures": run.summary.get("comparison_gate_failures", []),
        "timed_out": run.timed_out,
        "timeout_seconds": run.timeout_seconds,
        "cleanup_signal": run.cleanup_signal,
        "cleanup_escalated": run.cleanup_escalated,
    }


def aggregate_sustained_runtime(args: argparse.Namespace, runs: list[BurnInRun]) -> dict[str, Any]:
    passed_elapsed = sum(run.elapsed_seconds for run in runs if run.status == "pass")
    return {
        "passed_elapsed_seconds": passed_elapsed,
        "required_passed_elapsed_seconds": getattr(args, "min_passed_elapsed_seconds", 0.0),
        "passing_runs": sum(1 for run in runs if run.status == "pass"),
        "completed_runs": len(runs),
    }


def evaluate_sustained_runtime(args: argparse.Namespace, sustained_runtime: dict[str, Any]) -> list[str]:
    required = getattr(args, "min_passed_elapsed_seconds", 0.0)
    if required <= 0.0:
        return []
    elapsed = sustained_runtime.get("passed_elapsed_seconds")
    if not isinstance(elapsed, (int, float)) or elapsed < required:
        return [
            "sustained runtime below required passed elapsed seconds: "
            f"{format_metric(elapsed)} < {format_metric(required)}"
        ]
    return []


def aggregate_case_metrics(runs: list[BurnInRun]) -> dict[str, Any]:
    values: dict[str, dict[str, list[float]]] = {}
    for run in runs:
        if run.status not in {"pass", "dry-run"}:
            continue
        cases = run.summary.get("cases")
        if not isinstance(cases, list):
            continue
        for case in cases:
            if not isinstance(case, dict):
                continue
            case_id = case.get("id")
            if not isinstance(case_id, str):
                continue
            case_values = values.setdefault(case_id, {})
            add_metric(case_values, "rss_growth_kib", case.get("rss_growth_kib"))
            add_metric(case_values, "rss_tail_growth_kib", case.get("rss_tail_growth_kib"))
            add_metric(case_values, "rss_tail_range_kib", case.get("rss_tail_range_kib"))
            add_metric(case_values, "backend_accelerator_context_mib", case.get("backend_accelerator_context_mib"))
            add_metric(case_values, "projected_device_memory_mib", case.get("projected_device_memory_mib"))
            add_metric(case_values, "direct_peak_vram_mib", case.get("direct_peak_vram_mib"))
    return {
        case_id: {
            metric: metric_stats(samples)
            for metric, samples in sorted(case_values.items())
        }
        for case_id, case_values in sorted(values.items())
    }


def aggregate_soak_memory_metrics(runs: list[BurnInRun]) -> dict[str, Any]:
    total_cases = 0
    with_rss_growth = 0
    with_tail_growth = 0
    with_tail_range = 0
    missing_rss_growth: list[str] = []
    missing_tail_growth: list[str] = []
    missing_tail_range: list[str] = []
    for run in runs:
        if run.status not in {"pass", "dry-run"}:
            continue
        cases = run.summary.get("cases")
        if not isinstance(cases, list):
            continue
        for case in cases:
            if not isinstance(case, dict):
                continue
            total_cases += 1
            case_label = f"run-{run.index}:{case.get('id', '<unknown>')}"
            if isinstance(case.get("rss_growth_kib"), (int, float)):
                with_rss_growth += 1
            else:
                missing_rss_growth.append(case_label)
            if isinstance(case.get("rss_tail_growth_kib"), (int, float)):
                with_tail_growth += 1
            else:
                missing_tail_growth.append(case_label)
            if isinstance(case.get("rss_tail_range_kib"), (int, float)):
                with_tail_range += 1
            else:
                missing_tail_range.append(case_label)
    return {
        "available": (
            total_cases > 0
            and with_rss_growth == total_cases
            and with_tail_growth == total_cases
            and with_tail_range == total_cases
        ),
        "total_cases": total_cases,
        "cases_with_rss_growth": with_rss_growth,
        "cases_with_rss_tail_growth": with_tail_growth,
        "cases_with_rss_tail_range": with_tail_range,
        "missing_rss_growth": missing_rss_growth[:16],
        "missing_rss_tail_growth": missing_tail_growth[:16],
        "missing_rss_tail_range": missing_tail_range[:16],
        "missing_lists_truncated": (
            len(missing_rss_growth) > 16
            or len(missing_tail_growth) > 16
            or len(missing_tail_range) > 16
        ),
    }


def evaluate_soak_memory_metrics(metrics: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    total_cases = metrics.get("total_cases")
    if not isinstance(total_cases, int) or total_cases <= 0:
        return ["soak memory metrics gate requires at least one completed matrix case"]
    rss_growth = metrics.get("cases_with_rss_growth")
    if rss_growth != total_cases:
        failures.append(f"soak memory metrics missing rss_growth_kib ({rss_growth}/{total_cases})")
    tail_growth = metrics.get("cases_with_rss_tail_growth")
    if tail_growth != total_cases:
        failures.append(f"soak memory metrics missing rss_tail_growth_kib ({tail_growth}/{total_cases})")
    tail_range = metrics.get("cases_with_rss_tail_range")
    if tail_range != total_cases:
        failures.append(f"soak memory metrics missing rss_tail_range_kib ({tail_range}/{total_cases})")
    return failures


def aggregate_backend_memory_diagnostics(runs: list[BurnInRun]) -> dict[str, Any]:
    total_cases = 0
    cases_with_projected = 0
    cases_with_accelerator_breakdown = 0
    missing_projected: list[str] = []
    missing_breakdown: list[str] = []
    for run in runs:
        if run.status not in {"pass", "dry-run"}:
            continue
        cases = run.summary.get("cases")
        if not isinstance(cases, list):
            continue
        for case in cases:
            if not isinstance(case, dict):
                continue
            total_cases += 1
            case_label = f"run-{run.index}:{case.get('id', '<unknown>')}"
            if isinstance(case.get("projected_device_memory_mib"), (int, float)):
                cases_with_projected += 1
            else:
                missing_projected.append(case_label)
            backend_memory = case.get("backend_memory")
            memory_breakdown = (
                backend_memory.get("memory_breakdown_mib")
                if isinstance(backend_memory, dict)
                else None
            )
            if isinstance(memory_breakdown, dict) and any(key != "Host" for key in memory_breakdown):
                cases_with_accelerator_breakdown += 1
            else:
                missing_breakdown.append(case_label)
    return {
        "available": (
            total_cases > 0
            and cases_with_projected == total_cases
            and cases_with_accelerator_breakdown == total_cases
        ),
        "total_cases": total_cases,
        "cases_with_projected_device_memory": cases_with_projected,
        "cases_with_accelerator_breakdown": cases_with_accelerator_breakdown,
        "missing_projected_device_memory": missing_projected[:16],
        "missing_accelerator_breakdown": missing_breakdown[:16],
        "missing_lists_truncated": len(missing_projected) > 16 or len(missing_breakdown) > 16,
    }


def evaluate_backend_memory_diagnostics(diagnostics: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    total_cases = diagnostics.get("total_cases")
    if not isinstance(total_cases, int) or total_cases <= 0:
        return ["backend memory diagnostics gate requires at least one completed matrix case"]
    projected = diagnostics.get("cases_with_projected_device_memory")
    if projected != total_cases:
        failures.append(
            "backend memory diagnostics missing projected_device_memory_mib "
            f"({projected}/{total_cases})"
        )
    breakdown = diagnostics.get("cases_with_accelerator_breakdown")
    if breakdown != total_cases:
        failures.append(
            "backend memory diagnostics missing non-host accelerator breakdown "
            f"({breakdown}/{total_cases})"
        )
    return failures


def add_metric(case_values: dict[str, list[float]], key: str, value: Any) -> None:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        case_values.setdefault(key, []).append(float(value))


def metric_stats(samples: list[float]) -> dict[str, Any]:
    if not samples:
        return {"count": 0, "min": None, "max": None, "jitter_ratio": None}
    minimum = min(samples)
    maximum = max(samples)
    return {
        "count": len(samples),
        "min": minimum,
        "max": maximum,
        "values": samples,
        "jitter_ratio": None if minimum == 0 else maximum / minimum,
    }


def evaluate_aggregate_gates(args: argparse.Namespace, aggregate: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    gate_map = {
        "rss_growth_kib": args.max_rss_growth_jitter_ratio,
        "rss_tail_growth_kib": getattr(args, "max_rss_tail_growth_jitter_ratio", 0.0),
        "backend_accelerator_context_mib": args.max_backend_kv_jitter_ratio,
        "projected_device_memory_mib": args.max_projected_device_jitter_ratio,
        "direct_peak_vram_mib": args.max_direct_peak_vram_jitter_ratio,
    }
    for case_id, metrics in aggregate.items():
        if not isinstance(metrics, dict):
            continue
        for metric, gate in gate_map.items():
            if gate <= 0.0:
                continue
            stats = metrics.get(metric)
            if (
                not isinstance(stats, dict)
                or stats.get("count", 0) < 2
                or not isinstance(stats.get("jitter_ratio"), (int, float))
            ):
                failures.append(f"{case_id}: {metric} jitter gate requires at least two non-zero samples")
                continue
            if stats["jitter_ratio"] > gate:
                failures.append(
                    f"{case_id}: {metric} jitter ratio exceeded: "
                    f"{stats['jitter_ratio']} > {gate}"
                )
    return failures


def summarise_matrix_failure(summary: dict[str, Any]) -> str:
    failures: list[str] = []
    for item in summary.get("comparison_gate_failures", []):
        failures.append(str(item))
    for case in summary.get("cases", []):
        if isinstance(case, dict) and case.get("failure"):
            failures.append(f"{case.get('id')}: {case.get('failure')}")
    return "; ".join(failures)


def terminate_process_group(proc: subprocess.Popen[str]) -> tuple[str, bool]:
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        try:
            proc.wait(timeout=5.0)
            return "SIGTERM", False
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait(timeout=5.0)
            return "SIGKILL", True
    except ProcessLookupError:
        return "exited", False


def timeout_output_to_text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def load_matrix_summary(path: Path) -> tuple[dict[str, Any], str]:
    if not path.is_file():
        return {}, f"matrix summary missing: {path}"
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return {}, f"matrix summary is invalid JSON: {path}: {exc}"
    if not isinstance(value, dict):
        return {}, f"matrix summary is not a JSON object: {path}"
    expected_format = "qatq-live-vram-server-cancel-matrix-summary-v1"
    if value.get("format") != expected_format:
        return value, (
            "matrix summary has unexpected format: "
            f"{value.get('format')!r}; expected {expected_format}"
        )
    status = value.get("status")
    if status not in {"pass", "dry-run", "fail"}:
        return value, f"matrix summary has invalid status: {status!r}"
    cases = value.get("cases")
    if not isinstance(cases, list):
        return value, "matrix summary missing cases array"
    return value, ""


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def markdown_cell(value: Any) -> str:
    return str(value).replace("\n", " ").replace("|", "\\|")


def write_preflight_markdown(path: Path, preflight: dict[str, Any]) -> None:
    lines = [
        "# QATQ Live VRAM Server Burn-In Preflight",
        "",
        f"- status: `{preflight['status']}`",
        f"- dry run: `{preflight['dry_run']}`",
        f"- preflight only: `{preflight['preflight_only']}`",
        f"- selected cases: `{preflight['selected_cases']}`",
        f"- config: `{preflight['config']}`",
        f"- original config: `{preflight['original_config']}`",
        "",
        "| check | status | required | detail |",
        "| --- | --- | --- | --- |",
    ]
    for check in preflight["checks"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    markdown_cell(check["name"]),
                    f"`{markdown_cell(check['status'])}`",
                    f"`{str(check['required']).lower()}`",
                    markdown_cell(check["detail"]),
                ]
            )
            + " |"
        )
    failures = preflight.get("failures", [])
    if isinstance(failures, list) and failures:
        lines.extend(["", "## Failures", ""])
        for failure in failures:
            if isinstance(failure, dict):
                lines.append(f"- {failure.get('name', '<unknown>')}: {failure.get('detail', '')}")
            else:
                lines.append(f"- {failure}")
    lines.extend(["", preflight["boundary"], ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def write_markdown(path: Path, summary: dict[str, Any]) -> None:
    lines = [
        "# QATQ Live VRAM Server Burn-In",
        "",
        f"- status: `{summary['status']}`",
        f"- dry run: `{summary['dry_run']}`",
        f"- runs completed: `{summary['runs_completed']}` / `{summary['runs_requested']}`",
        f"- passed: `{summary['passed']}`",
        f"- failed: `{summary['failed']}`",
        "",
        "| run | status | cases | timed out | cleanup | elapsed seconds | summary |",
        "| ---: | --- | ---: | --- | --- | ---: | --- |",
    ]
    for run in summary["runs"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    str(run["index"]),
                    f"`{run['status']}`",
                    str(run.get("total_cases", "")),
                    str(run.get("timed_out", False)).lower(),
                    str(run.get("cleanup_signal") or ""),
                    f"{float(run['elapsed_seconds']):.3f}",
                    str(run["summary"]),
                ]
            )
            + " |"
        )
    sustained_runtime = summary.get("sustained_runtime", {})
    if isinstance(sustained_runtime, dict):
        lines.extend(
            [
                "",
                "## Sustained Runtime",
                "",
                f"- passed elapsed seconds: `{format_metric(sustained_runtime.get('passed_elapsed_seconds'))}`",
                f"- required passed elapsed seconds: `{format_metric(sustained_runtime.get('required_passed_elapsed_seconds'))}`",
                f"- passing runs: `{sustained_runtime.get('passing_runs', 0)}`",
            ]
        )
    sustained_failures = summary.get("sustained_runtime_failures", [])
    if isinstance(sustained_failures, list) and sustained_failures:
        lines.extend(["", "## Sustained Runtime Failures", ""])
        for failure in sustained_failures:
            lines.append(f"- {failure}")
    aggregate = summary.get("aggregate_case_metrics", {})
    if isinstance(aggregate, dict) and aggregate:
        lines.extend(
            [
                "",
                "## Aggregate Case Metrics",
                "",
                "| case | metric | count | min | max | jitter ratio |",
                "| --- | --- | ---: | ---: | ---: | ---: |",
            ]
        )
        for case_id, metrics in aggregate.items():
            if not isinstance(metrics, dict):
                continue
            for metric, stats in metrics.items():
                if not isinstance(stats, dict):
                    continue
                lines.append(
                    "| "
                    + " | ".join(
                        [
                            str(case_id),
                            str(metric),
                            str(stats.get("count", "")),
                            format_metric(stats.get("min")),
                            format_metric(stats.get("max")),
                            format_metric(stats.get("jitter_ratio")),
                        ]
                    )
                    + " |"
                )
    aggregate_failures = summary.get("aggregate_gate_failures", [])
    if isinstance(aggregate_failures, list) and aggregate_failures:
        lines.extend(["", "## Aggregate Gate Failures", ""])
        for failure in aggregate_failures:
            lines.append(f"- {failure}")
    backend_memory = summary.get("backend_memory_diagnostics", {})
    if isinstance(backend_memory, dict):
        lines.extend(
            [
                "",
                "## Backend Memory Diagnostics",
                "",
                f"- available: `{backend_memory.get('available', False)}`",
                f"- total cases: `{backend_memory.get('total_cases', 0)}`",
                f"- projected device memory cases: `{backend_memory.get('cases_with_projected_device_memory', 0)}`",
                f"- accelerator breakdown cases: `{backend_memory.get('cases_with_accelerator_breakdown', 0)}`",
            ]
        )
    backend_failures = summary.get("backend_memory_diagnostic_failures", [])
    if isinstance(backend_failures, list) and backend_failures:
        lines.extend(["", "## Backend Memory Diagnostic Failures", ""])
        for failure in backend_failures:
            lines.append(f"- {failure}")
    soak_memory = summary.get("soak_memory_metrics", {})
    if isinstance(soak_memory, dict):
        lines.extend(
            [
                "",
                "## Soak Memory Metrics",
                "",
                f"- available: `{soak_memory.get('available', False)}`",
                f"- total cases: `{soak_memory.get('total_cases', 0)}`",
                f"- RSS growth cases: `{soak_memory.get('cases_with_rss_growth', 0)}`",
                f"- RSS tail growth cases: `{soak_memory.get('cases_with_rss_tail_growth', 0)}`",
                f"- RSS tail range cases: `{soak_memory.get('cases_with_rss_tail_range', 0)}`",
            ]
        )
    soak_failures = summary.get("soak_memory_metric_failures", [])
    if isinstance(soak_failures, list) and soak_failures:
        lines.extend(["", "## Soak Memory Metric Failures", ""])
        for failure in soak_failures:
            lines.append(f"- {failure}")
    preflight = summary.get("preflight", {})
    if isinstance(preflight, dict) and preflight:
        lines.extend(
            [
                "",
                "## Preflight",
                "",
                f"- status: `{preflight.get('status')}`",
                f"- selected cases: `{preflight.get('selected_cases', 0)}`",
                f"- config: `{preflight.get('config', '')}`",
            ]
        )
        failures = preflight.get("failures", [])
        if isinstance(failures, list) and failures:
            lines.extend(["", "### Preflight Failures", ""])
            for failure in failures:
                if isinstance(failure, dict):
                    lines.append(f"- {failure.get('name', '<unknown>')}: {failure.get('detail', '')}")
                else:
                    lines.append(f"- {failure}")
    lines.extend(["", summary["boundary"], ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def format_metric(value: Any) -> str:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return f"{float(value):.6g}"
    if value is None:
        return ""
    return str(value)


def format_seconds(value: float) -> str:
    if float(value).is_integer():
        return str(int(value))
    return f"{value:.3f}".rstrip("0").rstrip(".")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
