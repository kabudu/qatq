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
    parser.add_argument("--max-rss-growth-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-backend-kv-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-projected-device-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--max-direct-peak-vram-jitter-ratio", type=float, default=0.0)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    validate_args(args)
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    plan = build_plan(args, work_dir)
    write_json(work_dir / "server-burnin-plan.json", plan)

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
        run = run_matrix(args, index, work_dir / f"run-{index:03d}")
        runs.append(run)
        if run.status == "fail":
            break

    summary = build_summary(args, work_dir, runs)
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
    require(args.max_rss_growth_jitter_ratio >= 0.0, "--max-rss-growth-jitter-ratio must be non-negative")
    require(args.max_backend_kv_jitter_ratio >= 0.0, "--max-backend-kv-jitter-ratio must be non-negative")
    require(
        args.max_projected_device_jitter_ratio >= 0.0,
        "--max-projected-device-jitter-ratio must be non-negative",
    )
    require(
        args.max_direct_peak_vram_jitter_ratio >= 0.0,
        "--max-direct-peak-vram-jitter-ratio must be non-negative",
    )


def build_plan(args: argparse.Namespace, work_dir: Path) -> dict[str, Any]:
    return {
        "format": "qatq-live-vram-server-burnin-plan-v1",
        "config": str(Path(args.config)),
        "matrix_runner": args.matrix_runner,
        "probe_runner": args.probe_runner,
        "llama_server": args.llama_server,
        "work_dir": str(work_dir),
        "runs": args.runs,
        "timeout_seconds": args.timeout,
        "run_timeout_seconds": args.run_timeout,
        "max_cases": args.max_cases,
        "max_total_seconds": args.max_total_seconds,
        "max_rss_growth_jitter_ratio": args.max_rss_growth_jitter_ratio,
        "max_backend_kv_jitter_ratio": args.max_backend_kv_jitter_ratio,
        "max_projected_device_jitter_ratio": args.max_projected_device_jitter_ratio,
        "max_direct_peak_vram_jitter_ratio": args.max_direct_peak_vram_jitter_ratio,
        "dry_run": args.dry_run,
        "run_work_dirs": [str(work_dir / f"run-{index:03d}") for index in range(1, args.runs + 1)],
    }


def run_matrix(args: argparse.Namespace, index: int, work_dir: Path) -> BurnInRun:
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
    run_timeout = args.run_timeout or derived_run_timeout(args)
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
            f"run {index} exceeded timeout of {run_timeout}s; "
            f"cleanup={cleanup_signal}"
        )
    stdout_path.write_text(timeout_output_to_text(stdout), encoding="utf-8")
    stderr_text = timeout_output_to_text(stderr)
    if timed_out:
        stderr_text += (
            f"\nserver burn-in matrix run exceeded timeout of {run_timeout}s; "
            f"cleanup={cleanup_signal}; escalated={str(cleanup_escalated).lower()}\n"
        )
    stderr_path.write_text(stderr_text, encoding="utf-8")
    elapsed = time.monotonic() - started
    summary = load_json(summary_path)
    status = str(summary.get("status") or ("fail" if returncode else "pass"))
    if returncode != 0:
        status = "fail"
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
    status = "dry-run" if runs and all(run.status == "dry-run" for run in runs) else "pass"
    if failures or aggregate_failures or len(runs) != args.runs:
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
        "aggregate_case_metrics": aggregate,
        "aggregate_gate_failures": aggregate_failures,
        "gates": {
            "max_rss_growth_jitter_ratio": args.max_rss_growth_jitter_ratio,
            "max_backend_kv_jitter_ratio": args.max_backend_kv_jitter_ratio,
            "max_projected_device_jitter_ratio": args.max_projected_device_jitter_ratio,
            "max_direct_peak_vram_jitter_ratio": args.max_direct_peak_vram_jitter_ratio,
        },
        "boundary": (
            "Bounded burn-in repetition for the configured llama-server live-VRAM "
            "matrix. It proves only the selected matrix config, run count, and "
            "aggregate jitter gates."
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
            if not isinstance(stats, dict) or not isinstance(stats.get("jitter_ratio"), (int, float)):
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


def load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


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
    lines.extend(["", summary["boundary"], ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def format_metric(value: Any) -> str:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return f"{float(value):.6g}"
    if value is None:
        return ""
    return str(value)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
