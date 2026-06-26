#!/usr/bin/env python3
"""Run a fail-closed matrix of llama-server live-VRAM cancellation probes.

This wraps `llama_cpp_live_vram_server_cancel_probe.py` for the integrated
server path. Cases run sequentially because they share GPU and host memory
resources. Each case keeps its own probe artifacts and the matrix writes a
small aggregate summary that can be checked into evidence docs.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


EXAMPLE_CONFIG = {
    "defaults": {
        "ctx_size": 8192,
        "parallel_slots": 2,
        "page_tokens": 64,
        "current_token": 4096,
        "hot_window_tokens": 32,
        "prefetch_window_tokens": 32,
        "max_queued_pages": 32,
        "n_predict": 512,
        "prompt_repeat": 80,
        "host_memory_pressure_mib": 1024,
        "concurrent_followup_during_cancel": True,
        "require_flattened_flash_consumer": True,
        "require_live_offloaded_stream_count": 2,
    },
    "cases": [
        {
            "id": "qwen25-15b-strict-server-cancel",
            "model": "/path/to/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            "model_id": "qwen2.5-1.5b-strict-server-cancel",
            "iterations": 10,
            "max_iteration_seconds": 15,
            "max_followup_seconds": 10,
            "max_server_rss_growth_mib": 1024,
            "prompt": "Validate live KV page restore during shared-server cancellation. ",
        }
    ],
}

SCALAR_FLAGS = {
    "model_id": "--model-id",
    "host": "--host",
    "port": "--port",
    "ctx_size": "--ctx-size",
    "parallel_slots": "--parallel-slots",
    "gpu_layers": "--gpu-layers",
    "kv_gpu_layers": "--kv-gpu-layers",
    "page_tokens": "--page-tokens",
    "current_token": "--current-token",
    "hot_window_tokens": "--hot-window-tokens",
    "prefetch_window_tokens": "--prefetch-window-tokens",
    "max_queued_pages": "--max-queued-pages",
    "max_page_segments": "--max-page-segments",
    "graph_extra_nodes": "--graph-extra-nodes",
    "cache_type_k": "--cache-type-k",
    "cache_type_v": "--cache-type-v",
    "n_predict": "--n-predict",
    "cancel_after_bytes": "--cancel-after-bytes",
    "iterations": "--iterations",
    "warmup_iterations": "--warmup-iterations",
    "host_memory_pressure_mib": "--host-memory-pressure-mib",
    "max_server_rss_growth_mib": "--max-server-rss-growth-mib",
    "max_rss_tail_growth_kib": "--max-rss-tail-growth-kib",
    "rss_tail_window": "--rss-tail-window",
    "max_retained_page_pool_mib": "--max-retained-page-pool-mib",
    "max_iteration_seconds": "--max-iteration-seconds",
    "max_followup_seconds": "--max-followup-seconds",
    "require_live_offloaded_stream_count": "--require-live-offloaded-stream-count",
    "startup_timeout": "--startup-timeout",
    "request_timeout": "--request-timeout",
    "shutdown_timeout": "--shutdown-timeout",
    "direct_peak_vram_sample_interval_ms": "--direct-peak-vram-sample-interval-ms",
    "prompt": "--prompt",
    "prompt_repeat": "--prompt-repeat",
}

BOOLEAN_FLAGS = {
    "kv_unified": "--kv-unified",
    "concurrent_followup_during_cancel": "--concurrent-followup-during-cancel",
    "enable_event_trace": "--enable-event-trace",
    "disable_qatq_traces": "--disable-qatq-traces",
    "native_baseline": "--native-baseline",
    "require_flattened_flash_consumer": "--require-flattened-flash-consumer",
    "require_backend_memory_diagnostics": "--require-backend-memory-diagnostics",
    "sample_direct_peak_vram": "--sample-direct-peak-vram",
    "require_direct_peak_vram_counter": "--require-direct-peak-vram-counter",
}

METADATA_KEYS = {"comparison_group"}
GATE_KEYS = {
    "max_backend_accelerator_self_mib",
    "max_backend_accelerator_context_mib",
    "max_backend_accelerator_compute_mib",
    "max_projected_device_memory_mib",
}
COMPARISON_GATE_KEYS = {
    "max_backend_accelerator_context_ratio",
    "max_direct_peak_vram_ratio",
    "max_followup_p95_ratio",
    "max_iteration_p95_ratio",
    "max_projected_device_memory_ratio",
    "max_rss_growth_ratio",
    "max_rss_tail_growth_delta_kib",
    "max_rss_tail_growth_ratio",
    "min_predicted_per_second_p05_ratio",
    "min_predicted_per_second_p50_ratio",
    "min_predicted_per_second_p95_ratio",
    "require_direct_peak_vram_counters",
}

KNOWN_CONFIG_KEYS = {
    "id",
    "model",
    *SCALAR_FLAGS.keys(),
    *BOOLEAN_FLAGS.keys(),
    *METADATA_KEYS,
    *GATE_KEYS,
}


@dataclass(frozen=True)
class CasePlan:
    case_id: str
    comparison_group: str | None
    work_dir: Path
    command: list[str]
    gates: dict[str, int | float]


@dataclass(frozen=True)
class CaseResult:
    case_id: str
    comparison_group: str | None
    status: str
    returncode: int
    elapsed_seconds: float
    work_dir: Path
    summary_path: Path
    stdout_path: Path
    stderr_path: Path
    failure: str
    probe_summary: dict[str, Any]
    gates: dict[str, int | float]
    gate_failures: list[str]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", help="JSON config with defaults and cases")
    parser.add_argument("--write-example-config", help="Write an example config and exit")
    parser.add_argument(
        "--probe-runner",
        default="scripts/llama_cpp_live_vram_server_cancel_probe.py",
    )
    parser.add_argument(
        "--llama-server",
        default="/private/tmp/qatq-llama-live-work/build/bin/llama-server",
    )
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-server-cancel-matrix")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--max-cases", type=int, default=0)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.write_example_config:
        write_json(Path(args.write_example_config), EXAMPLE_CONFIG)
        return 0

    require(args.config, "--config is required unless --write-example-config is used")
    require(args.timeout > 0, "--timeout must be positive")
    require(args.max_cases >= 0, "--max-cases must be non-negative")

    config = load_config(Path(args.config))
    comparison_gates = extract_comparison_gates(config.get("comparison_gates", {}))
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    cases = config["cases"]
    if args.max_cases:
        cases = cases[: args.max_cases]
    plans = [
        build_case_plan(
            args=args,
            defaults=config.get("defaults", {}),
            case=case,
            work_dir=work_dir / sanitise_case_id(str(case["id"])),
        )
        for case in cases
    ]
    plan = {
        "format": "qatq-live-vram-server-cancel-matrix-plan-v1",
        "config": str(Path(args.config)),
        "probe_runner": args.probe_runner,
        "llama_server": args.llama_server,
        "dry_run": args.dry_run,
        "timeout_seconds": args.timeout,
        "cases": [
            {
                "id": case_plan.case_id,
                "comparison_group": case_plan.comparison_group,
                "work_dir": str(case_plan.work_dir),
                "command": case_plan.command,
                "gates": case_plan.gates,
            }
            for case_plan in plans
        ],
    }
    write_json(work_dir / "server-cancel-matrix-plan.json", plan)

    results: list[CaseResult] = []
    for case_plan in plans:
        results.append(run_case(case_plan, timeout=args.timeout))
        if results[-1].status == "fail":
            break

    summary = build_summary(work_dir, args, results, comparison_gates)
    write_json(work_dir / "summary.json", summary)
    write_markdown(work_dir / "summary.md", summary)
    print((work_dir / "summary.md").read_text(encoding="utf-8"))
    return 0 if summary["status"] in {"pass", "dry-run"} else 1


def load_config(path: Path) -> dict[str, Any]:
    try:
        config = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid JSON config {path}: {exc}") from exc
    require(isinstance(config, dict), "config must be a JSON object")
    defaults = config.get("defaults", {})
    require(isinstance(defaults, dict), "config defaults must be an object")
    comparison_gates = config.get("comparison_gates", {})
    require(isinstance(comparison_gates, dict), "config comparison_gates must be an object")
    unknown_comparison_gates = sorted(set(comparison_gates) - COMPARISON_GATE_KEYS)
    require(
        not unknown_comparison_gates,
        f"comparison_gates contains unknown keys: {', '.join(unknown_comparison_gates)}",
    )
    cases = config.get("cases")
    require(isinstance(cases, list) and cases, "config must contain a non-empty cases array")
    for scope, values in (("defaults", defaults),):
        unknown = sorted(set(values) - KNOWN_CONFIG_KEYS)
        require(not unknown, f"{scope} contains unknown keys: {', '.join(unknown)}")
    for index, case in enumerate(cases, start=1):
        require(isinstance(case, dict), f"case {index} must be an object")
        require(isinstance(case.get("id"), str) and case["id"], f"case {index} missing id")
        require(isinstance(case.get("model"), str) and case["model"], f"case {case['id']} missing model")
        unknown = sorted(set(case) - KNOWN_CONFIG_KEYS)
        require(not unknown, f"case {case['id']} contains unknown keys: {', '.join(unknown)}")
    return config


def build_case_plan(
    *,
    args: argparse.Namespace,
    defaults: dict[str, Any],
    case: dict[str, Any],
    work_dir: Path,
) -> CasePlan:
    values = {**defaults, **case}
    command = [
        sys.executable,
        args.probe_runner,
        "--llama-server",
        args.llama_server,
        "--model",
        str(values["model"]),
        "--work-dir",
        str(work_dir),
    ]
    for key, flag in SCALAR_FLAGS.items():
        if key in values:
            command.extend([flag, str(values[key])])
    for key, flag in BOOLEAN_FLAGS.items():
        if bool(values.get(key, False)):
            command.append(flag)
    command.append("--keep-work-dir")
    if args.dry_run:
        command.append("--dry-run")
    comparison_group = values.get("comparison_group")
    return CasePlan(
        case_id=str(case["id"]),
        comparison_group=str(comparison_group) if comparison_group else None,
        work_dir=work_dir,
        command=command,
        gates=extract_case_gates(values),
    )


def run_case(case_plan: CasePlan, *, timeout: int) -> CaseResult:
    case_plan.work_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = case_plan.work_dir / "matrix-probe-stdout.log"
    stderr_path = case_plan.work_dir / "matrix-probe-stderr.log"
    summary_path = case_plan.work_dir / "summary.json"
    started = time.monotonic()
    failure = ""
    try:
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
            "w",
            encoding="utf-8",
        ) as stderr:
            completed = subprocess.run(
                case_plan.command,
                check=False,
                text=True,
                stdout=stdout,
                stderr=stderr,
                timeout=timeout,
            )
        returncode = completed.returncode
    except subprocess.TimeoutExpired:
        returncode = 124
        failure = f"case exceeded timeout of {timeout}s"
    elapsed = time.monotonic() - started
    probe_summary = load_probe_summary(summary_path)
    status = str(probe_summary.get("status") or ("fail" if returncode else "pass"))
    if returncode != 0:
        status = "fail"
    if status not in {"pass", "dry-run"} and not failure:
        failure = "; ".join(str(item) for item in probe_summary_failures(probe_summary))
    gate_failures = (
        []
        if status == "dry-run"
        else evaluate_case_gates(case_plan.case_id, probe_summary, case_plan.gates)
    )
    if gate_failures:
        status = "fail"
        failure = "; ".join([failure, *gate_failures]).strip("; ")
    return CaseResult(
        case_id=case_plan.case_id,
        comparison_group=case_plan.comparison_group,
        status=status,
        returncode=returncode,
        elapsed_seconds=elapsed,
        work_dir=case_plan.work_dir,
        summary_path=summary_path,
        stdout_path=stdout_path,
        stderr_path=stderr_path,
        failure=failure,
        probe_summary=probe_summary,
        gates=case_plan.gates,
        gate_failures=gate_failures,
    )


def load_probe_summary(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def probe_summary_failures(summary: dict[str, Any]) -> list[Any]:
    checks = summary.get("checks")
    if isinstance(checks, dict) and isinstance(checks.get("failures"), list):
        return checks["failures"]
    return []


def build_summary(
    work_dir: Path,
    args: argparse.Namespace,
    results: list[CaseResult],
    comparison_gates: dict[str, int | float],
) -> dict[str, Any]:
    statuses = [result.status for result in results]
    status = "dry-run" if statuses and all(item == "dry-run" for item in statuses) else "pass"
    failures = [result for result in results if result.status not in {"pass", "dry-run"}]
    if failures:
        status = "fail"
    comparisons = build_comparisons(results)
    comparison_gate_failures = (
        []
        if args.dry_run
        else evaluate_comparison_gates(comparisons, comparison_gates)
    )
    if comparison_gate_failures:
        status = "fail"
    return {
        "format": "qatq-live-vram-server-cancel-matrix-summary-v1",
        "status": status,
        "dry_run": args.dry_run,
        "work_dir": str(work_dir),
        "total_cases": len(results),
        "passed": sum(1 for result in results if result.status == "pass"),
        "dry_run_cases": sum(1 for result in results if result.status == "dry-run"),
        "failed": len(failures),
        "cases": [summarise_case(result) for result in results],
        "comparisons": comparisons,
        "comparison_gates": comparison_gates,
        "comparison_gate_failures": comparison_gate_failures,
        "boundary": (
            "Sequential llama-server cancellation matrix for QATQ live-VRAM "
            "native page-streaming evidence. It proves only the configured "
            "models, prompts, dtypes, page sizes, and latency/RSS gates."
        ),
    }


def summarise_case(result: CaseResult) -> dict[str, Any]:
    checks = result.probe_summary.get("checks")
    page_counts = {}
    if isinstance(checks, dict) and isinstance(checks.get("page_segment_counts"), dict):
        page_counts = checks["page_segment_counts"]
    persistent_page_source_stats = {}
    if isinstance(checks, dict) and isinstance(checks.get("persistent_page_source_stats"), dict):
        persistent_page_source_stats = checks["persistent_page_source_stats"]
    latency = result.probe_summary.get("latency_checks")
    memory = result.probe_summary.get("memory_checks")
    followup_completion_metrics = result.probe_summary.get("followup_completion_metrics")
    backend_memory = result.probe_summary.get("backend_memory")
    backend_memory = backend_memory if isinstance(backend_memory, dict) else {}
    direct_peak_vram_counter = result.probe_summary.get("direct_peak_vram_counter")
    direct_peak_vram_counter = (
        direct_peak_vram_counter if isinstance(direct_peak_vram_counter, dict) else {}
    )
    accelerator_memory = select_accelerator_memory(backend_memory)
    return {
        "id": result.case_id,
        "comparison_group": result.comparison_group,
        "status": result.status,
        "mode": result.probe_summary.get("mode"),
        "qatq_traces_enabled": result.probe_summary.get("qatq_traces_enabled"),
        "returncode": result.returncode,
        "elapsed_seconds": result.elapsed_seconds,
        "work_dir": str(result.work_dir),
        "summary": str(result.summary_path),
        "stdout": str(result.stdout_path),
        "stderr": str(result.stderr_path),
        "failure": result.failure,
        "gates": result.gates,
        "gate_failures": result.gate_failures,
        "iterations_completed": result.probe_summary.get("iterations_completed"),
        "iterations_requested": result.probe_summary.get("iterations_requested"),
        "host_memory_pressure_mib": result.probe_summary.get("host_memory_pressure_mib"),
        "rss_growth_kib": memory.get("growth_kib") if isinstance(memory, dict) else None,
        "rss_after_last_minus_first_kib": memory.get("rss_after_last_minus_first_kib")
        if isinstance(memory, dict)
        else None,
        "rss_tail_range_kib": memory.get("rss_tail_range_kib") if isinstance(memory, dict) else None,
        "rss_tail_growth_kib": memory.get("rss_tail_growth_kib") if isinstance(memory, dict) else None,
        "rss_tail_gate_growth_kib": memory.get("rss_tail_gate_growth_kib")
        if isinstance(memory, dict)
        else None,
        "rss_tail_window": memory.get("rss_tail_window") if isinstance(memory, dict) else None,
        "max_rss_tail_growth_kib": memory.get("max_rss_tail_growth_kib")
        if isinstance(memory, dict)
        else None,
        "iteration_latency": latency.get("iteration_duration_seconds")
        if isinstance(latency, dict)
        else None,
        "followup_latency": latency.get("followup_duration_seconds")
        if isinstance(latency, dict)
        else None,
        "followup_completion_metrics": followup_completion_metrics
        if isinstance(followup_completion_metrics, dict)
        else {},
        "flattened_flash_consumers": page_counts.get(
            "consumer.backend_scheduled_flattened_flash_attention",
        ),
        "live_offloaded_segments": page_counts.get("live_offloaded_segments"),
        "persistent_page_source_stats": persistent_page_source_stats,
        "max_retained_bytes": persistent_page_source_stats.get("max_retained_bytes"),
        "sample_direct_peak_vram": result.probe_summary.get("sample_direct_peak_vram"),
        "require_direct_peak_vram_counter": result.probe_summary.get(
            "require_direct_peak_vram_counter",
        ),
        "direct_peak_vram_counter_available": direct_peak_vram_counter.get("available"),
        "direct_peak_vram_mib": direct_peak_vram_counter.get("peak_memory_mib"),
        "direct_peak_vram_backend": direct_peak_vram_counter.get("backend"),
        "backend_memory": backend_memory,
        "backend_accelerator": accelerator_memory.get("backend"),
        "backend_accelerator_self_mib": accelerator_memory.get("self"),
        "backend_accelerator_context_mib": accelerator_memory.get("context"),
        "backend_accelerator_compute_mib": accelerator_memory.get("compute"),
        "projected_device_memory_mib": backend_memory.get("projected_device_memory_mib"),
    }


def select_accelerator_memory(backend_memory: dict[str, Any]) -> dict[str, Any]:
    breakdown = backend_memory.get("memory_breakdown_mib")
    if not isinstance(breakdown, dict):
        return {}
    for backend, values in breakdown.items():
        if backend == "Host" or not isinstance(values, dict):
            continue
        return {"backend": backend, **values}
    return {}


def extract_case_gates(values: dict[str, Any]) -> dict[str, int | float]:
    gates: dict[str, int | float] = {}
    for key in GATE_KEYS:
        if key not in values:
            continue
        value = values[key]
        require(
            isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0,
            f"{key} must be a non-negative number",
        )
        gates[key] = value
    return gates


def evaluate_case_gates(
    case_id: str,
    probe_summary: dict[str, Any],
    gates: dict[str, int | float],
) -> list[str]:
    if not gates:
        return []
    case = summarise_probe_summary(case_id, probe_summary)
    gate_paths = {
        "max_backend_accelerator_self_mib": ["backend_accelerator_self_mib"],
        "max_backend_accelerator_context_mib": ["backend_accelerator_context_mib"],
        "max_backend_accelerator_compute_mib": ["backend_accelerator_compute_mib"],
        "max_projected_device_memory_mib": ["projected_device_memory_mib"],
    }
    failures: list[str] = []
    for gate, maximum in sorted(gates.items()):
        value = nested_number(case, gate_paths[gate])
        if value is None:
            failures.append(f"{case_id}: {gate} requires a numeric backend memory value")
            continue
        if value > maximum:
            failures.append(f"{case_id}: {gate} exceeded: {value} > {maximum}")
    return failures


def extract_comparison_gates(value: Any) -> dict[str, int | float]:
    require(isinstance(value, dict), "comparison_gates must be an object")
    gates: dict[str, int | float] = {}
    for key in sorted(COMPARISON_GATE_KEYS):
        if key not in value:
            continue
        gate_value = value[key]
        require(
            isinstance(gate_value, (int, float)) and not isinstance(gate_value, bool) and gate_value >= 0,
            f"comparison_gates.{key} must be a non-negative number",
        )
        gates[key] = gate_value
    return gates


def evaluate_comparison_gates(
    comparisons: list[dict[str, Any]],
    gates: dict[str, int | float],
) -> list[str]:
    if not gates:
        return []
    failures: list[str] = []
    for comparison in comparisons:
        group = str(comparison.get("comparison_group", ""))
        qatq_case = str(comparison.get("qatq_case", ""))
        label = f"{group}/{qatq_case}".strip("/")
        if comparison.get("status") != "pass":
            failures.append(f"{label}: comparison status is {comparison.get('status')}")
            continue
        for key, gate_value in sorted(gates.items()):
            if key == "require_direct_peak_vram_counters":
                if gate_value > 0 and not bool(comparison.get("direct_peak_vram_counters_available")):
                    failures.append(f"{label}: require_direct_peak_vram_counters violated")
                continue
            ratio_key = comparison_gate_ratio_key(key)
            ratio_value = comparison.get(ratio_key)
            if not isinstance(ratio_value, (int, float)):
                failures.append(f"{label}: {key} requires numeric {ratio_key}")
                continue
            if key.startswith("min_") and ratio_value < gate_value:
                failures.append(f"{label}: {key} violated: {ratio_value} < {gate_value}")
            if key.startswith("max_") and ratio_value > gate_value:
                failures.append(f"{label}: {key} violated: {ratio_value} > {gate_value}")
    return failures


def comparison_gate_ratio_key(gate_key: str) -> str:
    mapping = {
        "max_backend_accelerator_context_ratio": "backend_accelerator_context_ratio",
        "max_direct_peak_vram_ratio": "direct_peak_vram_ratio",
        "max_followup_p95_ratio": "followup_p95_ratio",
        "max_iteration_p95_ratio": "iteration_p95_ratio",
        "max_projected_device_memory_ratio": "projected_device_memory_ratio",
        "max_rss_growth_ratio": "rss_growth_ratio",
        "max_rss_tail_growth_delta_kib": "rss_tail_growth_delta_kib",
        "max_rss_tail_growth_ratio": "rss_tail_growth_ratio",
        "min_predicted_per_second_p05_ratio": "predicted_per_second_p05_ratio",
        "min_predicted_per_second_p50_ratio": "predicted_per_second_p50_ratio",
        "min_predicted_per_second_p95_ratio": "predicted_per_second_p95_ratio",
    }
    return mapping[gate_key]


def summarise_probe_summary(case_id: str, probe_summary: dict[str, Any]) -> dict[str, Any]:
    synthetic = CaseResult(
        case_id=case_id,
        comparison_group=None,
        status=str(probe_summary.get("status", "")),
        returncode=0,
        elapsed_seconds=0.0,
        work_dir=Path("."),
        summary_path=Path("."),
        stdout_path=Path("."),
        stderr_path=Path("."),
        failure="",
        probe_summary=probe_summary,
        gates={},
        gate_failures=[],
    )
    return summarise_case(synthetic)


def build_comparisons(results: list[CaseResult]) -> list[dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for result in results:
        if not result.comparison_group:
            continue
        grouped.setdefault(result.comparison_group, []).append(summarise_case(result))

    comparisons: list[dict[str, Any]] = []
    for group, cases in sorted(grouped.items()):
        native = next(
            (case for case in cases if case.get("mode") == "native-baseline" and case["status"] == "pass"),
            None,
        )
        qatq_cases = [
            case for case in cases if case.get("mode") == "qatq-live-vram" and case["status"] == "pass"
        ]
        if native is None or not qatq_cases:
            comparisons.append(
                {
                    "comparison_group": group,
                    "status": "incomplete",
                    "reason": "requires one passing native-baseline case and one passing qatq-live-vram case",
                }
            )
            continue
        for qatq in qatq_cases:
            comparisons.append(
                {
                    "comparison_group": group,
                    "status": "pass",
                    "native_case": native["id"],
                    "qatq_case": qatq["id"],
                    "iteration_p95_ratio": ratio(
                        nested_number(qatq, ["iteration_latency", "p95"]),
                        nested_number(native, ["iteration_latency", "p95"]),
                    ),
                    "followup_p95_ratio": ratio(
                        nested_number(qatq, ["followup_latency", "p95"]),
                        nested_number(native, ["followup_latency", "p95"]),
                    ),
                    "predicted_per_second_p50_ratio": ratio(
                        nested_number(
                            qatq,
                            ["followup_completion_metrics", "predicted_per_second", "p50"],
                        ),
                        nested_number(
                            native,
                            ["followup_completion_metrics", "predicted_per_second", "p50"],
                        ),
                    ),
                    "predicted_per_second_p05_ratio": ratio(
                        nested_number(
                            qatq,
                            ["followup_completion_metrics", "predicted_per_second", "p05"],
                        ),
                        nested_number(
                            native,
                            ["followup_completion_metrics", "predicted_per_second", "p05"],
                        ),
                    ),
                    "predicted_per_second_p95_ratio": ratio(
                        nested_number(
                            qatq,
                            ["followup_completion_metrics", "predicted_per_second", "p95"],
                        ),
                        nested_number(
                            native,
                            ["followup_completion_metrics", "predicted_per_second", "p95"],
                        ),
                    ),
                    "rss_growth_ratio": ratio(qatq.get("rss_growth_kib"), native.get("rss_growth_kib")),
                    "rss_tail_growth_ratio": zero_equal_ratio(
                        qatq.get("rss_tail_gate_growth_kib"),
                        native.get("rss_tail_gate_growth_kib"),
                    ),
                    "rss_tail_growth_delta_kib": difference(
                        qatq.get("rss_tail_gate_growth_kib"),
                        native.get("rss_tail_gate_growth_kib"),
                    ),
                    "backend_accelerator_context_ratio": ratio(
                        qatq.get("backend_accelerator_context_mib"),
                        native.get("backend_accelerator_context_mib"),
                    ),
                    "projected_device_memory_ratio": ratio(
                        qatq.get("projected_device_memory_mib"),
                        native.get("projected_device_memory_mib"),
                    ),
                    "direct_peak_vram_ratio": ratio(
                        qatq.get("direct_peak_vram_mib"),
                        native.get("direct_peak_vram_mib"),
                    ),
                    "direct_peak_vram_counters_available": bool(
                        qatq.get("direct_peak_vram_counter_available")
                    )
                    and bool(native.get("direct_peak_vram_counter_available")),
                    "qatq_live_offloaded_segments": qatq.get("live_offloaded_segments"),
                    "qatq_flattened_flash_consumers": qatq.get("flattened_flash_consumers"),
                }
            )
    return comparisons


def nested_number(value: dict[str, Any], path: list[str]) -> int | float | None:
    cursor: Any = value
    for key in path:
        if not isinstance(cursor, dict):
            return None
        cursor = cursor.get(key)
    return cursor if isinstance(cursor, (int, float)) else None


def ratio(numerator: Any, denominator: Any) -> float | None:
    if not isinstance(numerator, (int, float)):
        return None
    if not isinstance(denominator, (int, float)) or denominator == 0:
        return None
    return float(numerator) / float(denominator)


def zero_equal_ratio(numerator: Any, denominator: Any) -> float | None:
    if not isinstance(numerator, (int, float)):
        return None
    if not isinstance(denominator, (int, float)):
        return None
    if denominator == 0:
        return 1.0 if numerator == 0 else None
    return float(numerator) / float(denominator)


def difference(value: Any, baseline: Any) -> float | None:
    if not isinstance(value, (int, float)):
        return None
    if not isinstance(baseline, (int, float)):
        return None
    return float(value) - float(baseline)


def write_markdown(path: Path, summary: dict[str, Any]) -> None:
    lines = [
        "# QATQ Live VRAM Server Cancellation Matrix",
        "",
        f"- status: `{summary['status']}`",
        f"- dry run: `{summary['dry_run']}`",
        f"- total cases: `{summary['total_cases']}`",
        f"- passed: `{summary['passed']}`",
        f"- dry-run cases: `{summary['dry_run_cases']}`",
        f"- failed: `{summary['failed']}`",
        "",
        "| case | status | iterations | predicted tok/s p50 | live offloaded segments | flattened Flash consumers | max retained MiB | backend self MiB | backend KV MiB | backend compute MiB | RSS growth KiB | RSS tail growth KiB | RSS tail gate growth KiB | RSS tail range KiB | RSS tail gate KiB |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for case in summary["cases"]:
        iterations = format_count_pair(
            case.get("iterations_completed"),
            case.get("iterations_requested"),
        )
        case_label = str(case["id"])
        if case.get("mode"):
            case_label += f" ({case['mode']})"
        lines.append(
            "| "
            + " | ".join(
                [
                    case_label,
                    f"`{case['status']}`",
                    iterations,
                    format_optional(
                        nested_number(
                            case,
                            ["followup_completion_metrics", "predicted_per_second", "p50"],
                        )
                    ),
                    format_optional(case.get("live_offloaded_segments")),
                    format_optional(case.get("flattened_flash_consumers")),
                    format_mib(case.get("max_retained_bytes")),
                    format_optional(case.get("backend_accelerator_self_mib")),
                    format_optional(case.get("backend_accelerator_context_mib")),
                    format_optional(case.get("backend_accelerator_compute_mib")),
                    format_optional(case.get("rss_growth_kib")),
                    format_optional(case.get("rss_tail_growth_kib")),
                    format_optional(case.get("rss_tail_gate_growth_kib")),
                    format_optional(case.get("rss_tail_range_kib")),
                    format_optional(case.get("max_rss_tail_growth_kib")),
                ]
            )
            + " |"
        )
    failures = [case for case in summary["cases"] if case.get("failure")]
    if failures:
        lines.extend(["", "## Failures", ""])
        for case in failures:
            lines.append(f"- `{case['id']}`: {case['failure']}")
    comparisons = summary.get("comparisons", [])
    if isinstance(comparisons, list) and comparisons:
        lines.extend(
            [
                "",
                "## Native Comparisons",
                "",
                "| group | QATQ case | status | iteration p95 ratio | follow-up p95 ratio | predicted tok/s p05 ratio | predicted tok/s p50 ratio | predicted tok/s p95 ratio | backend KV ratio | projected device ratio | direct peak VRAM ratio | RSS growth ratio | RSS tail growth ratio | RSS tail delta KiB | QATQ live offloaded segments |",
                "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
            ]
        )
        for comparison in comparisons:
            if not isinstance(comparison, dict):
                continue
            lines.append(
                "| "
                + " | ".join(
                    [
                        str(comparison.get("comparison_group", "")),
                        str(comparison.get("qatq_case", "")),
                        f"`{comparison.get('status', '')}`",
                        format_ratio(comparison.get("iteration_p95_ratio")),
                        format_ratio(comparison.get("followup_p95_ratio")),
                        format_ratio(comparison.get("predicted_per_second_p05_ratio")),
                        format_ratio(comparison.get("predicted_per_second_p50_ratio")),
                        format_ratio(comparison.get("predicted_per_second_p95_ratio")),
                        format_ratio(comparison.get("backend_accelerator_context_ratio")),
                        format_ratio(comparison.get("projected_device_memory_ratio")),
                        format_ratio(comparison.get("direct_peak_vram_ratio")),
                        format_ratio(comparison.get("rss_growth_ratio")),
                        format_ratio(comparison.get("rss_tail_growth_ratio")),
                        format_optional(comparison.get("rss_tail_growth_delta_kib")),
                        format_optional(comparison.get("qatq_live_offloaded_segments")),
                    ]
                )
                + " |"
            )
    comparison_gate_failures = summary.get("comparison_gate_failures", [])
    if isinstance(comparison_gate_failures, list) and comparison_gate_failures:
        lines.extend(["", "## Comparison Gate Failures", ""])
        for failure in comparison_gate_failures:
            lines.append(f"- {failure}")
    lines.extend(["", summary["boundary"], ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def format_count_pair(completed: Any, requested: Any) -> str:
    if completed is None and requested is None:
        return ""
    return f"{format_optional(completed)}/{format_optional(requested)}"


def format_optional(value: Any) -> str:
    return "" if value is None else str(value)


def format_mib(value: Any) -> str:
    return "" if isinstance(value, bool) or not isinstance(value, (int, float)) else f"{value / (1024 * 1024):.2f}"


def format_ratio(value: Any) -> str:
    return "" if not isinstance(value, (int, float)) else f"{float(value):.3f}"


def sanitise_case_id(case_id: str) -> str:
    allowed = []
    for char in case_id:
        if char.isalnum() or char in {"-", "_", "."}:
            allowed.append(char)
        else:
            allowed.append("-")
    cleaned = "".join(allowed).strip("-._")
    return cleaned or "case"


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def require(condition: Any, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
