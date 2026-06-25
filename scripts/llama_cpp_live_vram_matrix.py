#!/usr/bin/env python3
"""Run a fail-closed matrix of real llama.cpp live-VRAM evidence cases.

The matrix runner wraps `llama_cpp_live_vram_evidence.py` so broad GPU evidence
is repeatable instead of being a pile of one-off shell commands. Each case still
runs the strict per-case gates: Metal presence, output preservation, CPU-KV
baseline, frontier selection, event-trace verification, runtime reclaim, exact
restore, and zstd/lz4 compression comparisons. `--require-live-paging` promotes
the same matrix to the stricter page-granular live-paging conformance gate.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


EXAMPLE_CONFIG = {
    "cases": [
        {
            "id": "qwen25-15b-daily-driver",
            "model": "/path/to/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            "model_id": "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            "sweep_kv_gpu_layers": [14, 21, 24],
            "short_prompt": "Summarise a messy project handover into concrete next actions, risks, and validation checks.",
            "deep_prompt_seed": "Summarise this live runtime migration handover with exactness constraints, rollback steps, security boundaries, operator metrics, latency risk, and reproducibility notes. ",
        }
    ]
}


@dataclass(frozen=True)
class CaseResult:
    case_id: str
    iteration: int
    model_id: str
    status: str
    work_dir: Path
    summary_path: Path
    selected_layers: str
    total_pages: int
    offloaded_pages: int
    compressed_pages: int
    resident_pages: int
    pass_through_pages: int
    verified_restores: int
    raw_bytes: int
    qatq_bytes: int
    zstd_bytes: int
    lz4_bytes: int
    reclaimable_gpu_bytes: int
    event_trace_events: int
    event_trace_passed: bool
    attention_trace_events: int
    attention_trace_layers: int
    mlx_layers_checked: int
    mlx_heads_checked: int
    mlx_qatq_store_ratio: float
    mlx_streaming_time_ratio: float
    latency_samples: int
    full_p95_decode_us: float
    mixed_p95_decode_us: float
    full_p99_decode_us: float
    mixed_p99_decode_us: float
    mixed_p95_regression: float
    mixed_p99_regression: float
    deep_latency_samples: int
    deep_full_p95_decode_us: float
    deep_mixed_p95_decode_us: float
    deep_full_p99_decode_us: float
    deep_mixed_p99_decode_us: float
    deep_mixed_p95_regression: float
    deep_mixed_p99_regression: float
    elapsed_seconds: float
    failure: str


@dataclass(frozen=True)
class TokenLatencyStats:
    full_samples: int
    mixed_samples: int
    full_p95: float
    mixed_p95: float
    full_p99: float
    mixed_p99: float
    p95_regression: float
    p99_regression: float


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", help="JSON config containing a top-level `cases` array")
    parser.add_argument("--write-example-config", help="Write an example matrix JSON config and exit")
    parser.add_argument("--evidence-runner", default="scripts/llama_cpp_live_vram_evidence.py")
    parser.add_argument("--llama-simple", default="/private/tmp/qatq-llama.cpp/build/bin/llama-simple")
    parser.add_argument(
        "--llama-cpp-source",
        default="",
        help=(
            "Optional patched llama.cpp source checkout to pass through to the "
            "evidence runner. Use this when --llama-simple is built outside "
            "the runner's inferred <checkout>/build/bin layout, such as the "
            "reproducible build-qatq bootstrap path."
        ),
    )
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-matrix")
    parser.add_argument("--timeout", type=int, default=1200)
    parser.add_argument("--max-cases", type=int, default=0)
    parser.add_argument("--iterations", type=int, default=1, help="Repeat every case this many times")
    parser.add_argument(
        "--override-short-predict",
        type=int,
        default=0,
        help=(
            "Override every case's short_predict value for this matrix run. "
            "Useful for collecting enough generated-token samples for p95/p99 latency gates."
        ),
    )
    parser.add_argument(
        "--override-deep-predict",
        type=int,
        default=0,
        help=(
            "Override every case's deep_predict value for this matrix run. "
            "Useful when diagnosing p95/p99 tail stability over a longer "
            "long-context generated-token sample window."
        ),
    )
    parser.add_argument(
        "--override-max-queued-pages",
        type=int,
        default=-1,
        help=(
            "Override every case's max_queued_pages value for this matrix run. "
            "Use this to tune the live-VRAM latency/reclaim frontier without "
            "editing local fixture configs."
        ),
    )
    parser.add_argument("--skip-event-trace", action="store_true")
    parser.add_argument("--skip-attention-trace", action="store_true")
    parser.add_argument(
        "--skip-attention-page-segments-trace",
        action="store_true",
        help=(
            "Pass --skip-attention-page-segments-trace through to each evidence "
            "run. Use this for latency diagnostics that should not pay for the "
            "native page-streaming proof trace."
        ),
    )
    parser.add_argument("--require-live-paging", action="store_true")
    parser.add_argument(
        "--require-native-page-streaming",
        action="store_true",
        help=(
            "Require each case to pass the future production gate where the "
            "runtime attention graph consumes pages through a native non-concat "
            "streaming path."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-contract-probe",
        action="store_true",
        help=(
            "Ask each case to run only the executable segmented K/Q/V contract "
            "probe. A case passes when llama.cpp reaches the native backend "
            "boundary and stops with the documented contract-probe "
            "message."
        ),
    )
    parser.add_argument(
        "--gpu-page-staging",
        action="store_true",
        help="Pass --gpu-page-staging through to the per-case evidence runner.",
    )
    parser.add_argument(
        "--native-page-streaming-attention",
        action="store_true",
        help=(
            "Pass --native-page-streaming-attention through to each evidence run. "
            "This is also enabled automatically when --require-native-page-streaming is set."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-attention-ggml",
        action="store_true",
        help=(
            "Pass --native-page-streaming-attention-ggml through to each evidence "
            "run. This is the accelerator-schedulable segmented KQ/V path and is "
            "enabled automatically when --require-native-page-streaming is set."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-attention-backend-op",
        action="store_true",
        help=(
            "Pass --native-page-streaming-attention-backend-op through to each "
            "evidence run. This backend-scheduled path is enabled automatically "
            "when --require-native-page-streaming is set."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-flatten-flash",
        action="store_true",
        help=(
            "Pass --native-page-streaming-flatten-flash through to each evidence "
            "run so eligible backend-op page tables can use llama.cpp's "
            "backend-scheduled Flash Attention path."
        ),
    )
    parser.add_argument(
        "--attention-page-segments-live-offloaded-only",
        action="store_true",
        help=(
            "Pass --attention-page-segments-live-offloaded-only through to each "
            "evidence run. Use this for latency gates that need native cold-page "
            "proof without serialising every resident fast-path diagnostic row."
        ),
    )
    parser.add_argument(
        "--aggregate-codec-gate",
        action="store_true",
        help="Pass --aggregate-codec-gate through to each evidence run.",
    )
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument(
        "--prune-bulk-artifacts",
        action="store_true",
        help=(
            "Pass --prune-bulk-artifacts through to each evidence run so large "
            "generated export directories are deleted after aggregate evidence "
            "has been written."
        ),
    )
    parser.add_argument("--allow-failures", action="store_true")
    parser.add_argument(
        "--require-stable-reclaim",
        action="store_true",
        help="Fail if repeated passing runs for the same case report different reclaimable GPU bytes.",
    )
    parser.add_argument(
        "--require-stable-qc-bytes",
        action="store_true",
        help="Fail if repeated passing runs for the same case report different QATQ/zstd/lz4 byte totals.",
    )
    parser.add_argument(
        "--max-elapsed-jitter-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional per-case elapsed-time stability gate. A value of 0 disables "
            "the gate; otherwise (max-min)/min must be <= this ratio across "
            "passing repeated runs."
        ),
    )
    parser.add_argument(
        "--min-token-latency-samples",
        type=int,
        default=0,
        help=(
            "Minimum short full-GPU and mixed-KV decode-token samples required "
            "when enforcing token latency regression gates. Use 0 to disable."
        ),
    )
    parser.add_argument(
        "--max-mixed-token-p95-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p95 per-token decode latency regression gate comparing "
            "short mixed-KV against short full-GPU. A value of 0 disables it; "
            "0.25 means p95 may be at most 25%% slower."
        ),
    )
    parser.add_argument(
        "--max-mixed-token-p99-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p99 per-token decode latency regression gate comparing "
            "short mixed-KV against short full-GPU. A value of 0 disables it."
        ),
    )
    parser.add_argument(
        "--deep-latency-baseline",
        action="store_true",
        help=(
            "Run an additional deep full-GPU baseline for every case so the "
            "matrix can compare long-context deep mixed-KV token latency "
            "against a deep full-GPU baseline."
        ),
    )
    parser.add_argument(
        "--min-deep-token-latency-samples",
        type=int,
        default=0,
        help=(
            "Minimum deep full-GPU and deep mixed-KV decode-token samples "
            "required when enforcing long-context token latency gates. Use 0 "
            "to disable."
        ),
    )
    parser.add_argument(
        "--max-deep-mixed-token-p95-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p95 decode-token latency regression gate comparing deep "
            "mixed-KV against deep full-GPU. A value of 0 disables it."
        ),
    )
    parser.add_argument(
        "--max-deep-mixed-token-p99-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p99 decode-token latency regression gate comparing deep "
            "mixed-KV against deep full-GPU. A value of 0 disables it."
        ),
    )
    parser.add_argument(
        "--host-memory-pressure-mib",
        type=int,
        default=0,
        help=(
            "Allocate and page-touch this many MiB in the matrix runner while "
            "cases execute. This is a bounded local stress knob for unified-memory "
            "systems; 0 disables it."
        ),
    )
    parser.add_argument(
        "--ignore-config-gates",
        action="store_true",
        help=(
            "Ignore optional top-level matrix_gates from the JSON config. Use "
            "this only for exploratory diagnostics; production-shaped configs "
            "should normally carry their own latency and stability gates."
        ),
    )
    args = parser.parse_args()

    require(
        not (args.require_live_paging and args.skip_event_trace),
        "--require-live-paging requires event trace emission",
    )
    require(
        not args.require_native_page_streaming or args.require_live_paging,
        "--require-native-page-streaming requires --require-live-paging",
    )
    require(
        not (args.require_native_page_streaming and args.skip_attention_page_segments_trace),
        "--require-native-page-streaming requires the attention page-segments trace",
    )
    require(args.max_elapsed_jitter_ratio >= 0.0, "--max-elapsed-jitter-ratio must be non-negative")
    require(args.host_memory_pressure_mib >= 0, "--host-memory-pressure-mib must be non-negative")
    require(args.host_memory_pressure_mib <= 8192, "--host-memory-pressure-mib is capped at 8192 MiB")

    if args.write_example_config:
        path = Path(args.write_example_config)
        path.write_text(json.dumps(EXAMPLE_CONFIG, indent=2) + "\n", encoding="utf-8")
        print(path)
        return 0

    require(args.config, "--config is required unless --write-example-config is used")
    config = load_json(Path(args.config))
    if not args.ignore_config_gates:
        apply_config_gates(args, config)
    validate_gate_args(args)
    cases = config.get("cases")
    require(isinstance(cases, list) and cases, "config must contain a non-empty `cases` array")
    require(args.iterations > 0, "--iterations must be positive")
    require(args.override_short_predict >= 0, "--override-short-predict must be non-negative")
    require(args.override_deep_predict >= 0, "--override-deep-predict must be non-negative")
    require(args.override_max_queued_pages >= -1, "--override-max-queued-pages must be -1 or non-negative")
    if args.max_cases > 0:
        cases = cases[: args.max_cases]

    root = Path.cwd()
    runner = require_file(root / args.evidence_runner, "--evidence-runner")
    llama_simple = require_file(Path(args.llama_simple), "--llama-simple")
    llama_cpp_source = require_file(Path(args.llama_cpp_source), "--llama-cpp-source") if args.llama_cpp_source else None
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)
    host_pressure = allocate_host_memory_pressure(args.host_memory_pressure_mib)

    results: list[CaseResult] = []
    validated_cases = [validate_case(raw_case) for raw_case in cases]
    for iteration in range(1, args.iterations + 1):
        for case in validated_cases:
            result = run_case(
                root=root,
                runner=runner,
                llama_simple=llama_simple,
                llama_cpp_source=llama_cpp_source,
                kv_bench=args.qatq_kv_bench,
                matrix_work_dir=work_dir,
                timeout=args.timeout,
                skip_event_trace=args.skip_event_trace,
                skip_attention_trace=args.skip_attention_trace,
                skip_attention_page_segments_trace=args.skip_attention_page_segments_trace,
                require_live_paging=args.require_live_paging,
                require_native_page_streaming=args.require_native_page_streaming,
                native_page_streaming_contract_probe=args.native_page_streaming_contract_probe,
                gpu_page_staging=args.gpu_page_staging,
                native_page_streaming_attention=args.native_page_streaming_attention
                or args.native_page_streaming_attention_ggml
                or args.native_page_streaming_attention_backend_op
                or args.require_native_page_streaming,
                native_page_streaming_attention_ggml=args.native_page_streaming_attention_ggml
                or args.native_page_streaming_attention_backend_op
                or args.require_native_page_streaming,
                native_page_streaming_attention_backend_op=args.native_page_streaming_attention_backend_op
                or args.require_native_page_streaming,
                native_page_streaming_flatten_flash=args.native_page_streaming_flatten_flash,
                attention_page_segments_live_offloaded_only=args.attention_page_segments_live_offloaded_only,
                aggregate_codec_gate=args.aggregate_codec_gate,
                prune_bulk_artifacts=args.prune_bulk_artifacts,
                deep_latency_baseline=args.deep_latency_baseline,
                override_short_predict=args.override_short_predict,
                override_deep_predict=args.override_deep_predict,
                override_max_queued_pages=args.override_max_queued_pages,
                case=case,
                iteration=iteration,
            )
            results.append(result)

    summary = build_matrix_summary(
        results,
        work_dir,
        require_live_paging=args.require_live_paging,
        require_native_page_streaming=args.require_native_page_streaming,
        native_page_streaming_contract_probe=args.native_page_streaming_contract_probe,
        gpu_page_staging=args.gpu_page_staging,
        native_page_streaming_attention=args.native_page_streaming_attention
        or args.native_page_streaming_attention_ggml
        or args.native_page_streaming_attention_backend_op
        or args.require_native_page_streaming,
        native_page_streaming_attention_ggml=args.native_page_streaming_attention_ggml
        or args.native_page_streaming_attention_backend_op
        or args.require_native_page_streaming,
        native_page_streaming_attention_backend_op=args.native_page_streaming_attention_backend_op
        or args.require_native_page_streaming,
        native_page_streaming_flatten_flash=args.native_page_streaming_flatten_flash,
        attention_page_segments_live_offloaded_only=args.attention_page_segments_live_offloaded_only,
        aggregate_codec_gate=args.aggregate_codec_gate,
        require_stable_reclaim=args.require_stable_reclaim,
        require_stable_qc_bytes=args.require_stable_qc_bytes,
        max_elapsed_jitter_ratio=args.max_elapsed_jitter_ratio,
        min_token_latency_samples=args.min_token_latency_samples,
        max_mixed_token_p95_regression_ratio=args.max_mixed_token_p95_regression_ratio,
        max_mixed_token_p99_regression_ratio=args.max_mixed_token_p99_regression_ratio,
        deep_latency_baseline=args.deep_latency_baseline,
        min_deep_token_latency_samples=args.min_deep_token_latency_samples,
        max_deep_mixed_token_p95_regression_ratio=args.max_deep_mixed_token_p95_regression_ratio,
        max_deep_mixed_token_p99_regression_ratio=args.max_deep_mixed_token_p99_regression_ratio,
        host_memory_pressure_mib=args.host_memory_pressure_mib,
        prune_bulk_artifacts=args.prune_bulk_artifacts,
    )
    summary_path = work_dir / "summary.md"
    summary_path.write_text(summary, encoding="utf-8")
    print(summary)

    failed = [result for result in results if result.status != "pass"]
    stability_failures = evaluate_stability_gates(
        results,
        require_stable_reclaim=args.require_stable_reclaim,
        require_stable_qc_bytes=args.require_stable_qc_bytes,
        max_elapsed_jitter_ratio=args.max_elapsed_jitter_ratio,
        min_token_latency_samples=args.min_token_latency_samples,
        max_mixed_token_p95_regression_ratio=args.max_mixed_token_p95_regression_ratio,
        max_mixed_token_p99_regression_ratio=args.max_mixed_token_p99_regression_ratio,
        min_deep_token_latency_samples=args.min_deep_token_latency_samples,
        max_deep_mixed_token_p95_regression_ratio=args.max_deep_mixed_token_p95_regression_ratio,
        max_deep_mixed_token_p99_regression_ratio=args.max_deep_mixed_token_p99_regression_ratio,
    )
    if stability_failures:
        failure_text = "\n".join(stability_failures)
        (work_dir / "stability-failures.txt").write_text(failure_text + "\n", encoding="utf-8")
        print("\n## Stability Gate Failures\n", file=sys.stderr)
        print(failure_text, file=sys.stderr)
    if (failed or stability_failures) and not args.allow_failures:
        return 1
    if host_pressure is not None:
        host_pressure[0] ^= 0
    return 0


def validate_case(raw: dict) -> dict:
    require(isinstance(raw, dict), "each case must be an object")
    for key in ("id", "model", "model_id", "sweep_kv_gpu_layers", "short_prompt", "deep_prompt_seed"):
        require(key in raw, f"case is missing required key `{key}`")
    require(re.match(r"^[A-Za-z0-9_.-]+$", str(raw["id"])) is not None, "case id must be filesystem-safe")
    require(isinstance(raw["sweep_kv_gpu_layers"], list) and raw["sweep_kv_gpu_layers"], "sweep_kv_gpu_layers must be a non-empty array")
    for value in raw["sweep_kv_gpu_layers"]:
        require(isinstance(value, int) and value >= 0, "sweep_kv_gpu_layers values must be non-negative integers")
    require_file(Path(raw["model"]), f"--model for case {raw['id']}")
    return raw


CONFIG_GATE_KEYS = {
    "require_stable_reclaim",
    "require_stable_qc_bytes",
    "max_elapsed_jitter_ratio",
    "deep_latency_baseline",
    "min_token_latency_samples",
    "max_mixed_token_p95_regression_ratio",
    "max_mixed_token_p99_regression_ratio",
    "min_deep_token_latency_samples",
    "max_deep_mixed_token_p95_regression_ratio",
    "max_deep_mixed_token_p99_regression_ratio",
    "host_memory_pressure_mib",
}


def apply_config_gates(args: argparse.Namespace, config: dict) -> None:
    gates = config.get("matrix_gates")
    if gates is None:
        return
    require(isinstance(gates, dict), "matrix_gates must be an object")
    unknown = sorted(set(gates) - CONFIG_GATE_KEYS)
    require(not unknown, "matrix_gates contains unknown keys: " + ", ".join(unknown))

    args.require_stable_reclaim = args.require_stable_reclaim or config_bool(
        gates, "require_stable_reclaim"
    )
    args.require_stable_qc_bytes = args.require_stable_qc_bytes or config_bool(
        gates, "require_stable_qc_bytes"
    )
    args.deep_latency_baseline = args.deep_latency_baseline or config_bool(
        gates, "deep_latency_baseline"
    )
    args.max_elapsed_jitter_ratio = config_float_default(
        args.max_elapsed_jitter_ratio,
        gates,
        "max_elapsed_jitter_ratio",
    )
    args.min_token_latency_samples = config_int_default(
        args.min_token_latency_samples,
        gates,
        "min_token_latency_samples",
    )
    args.max_mixed_token_p95_regression_ratio = config_float_default(
        args.max_mixed_token_p95_regression_ratio,
        gates,
        "max_mixed_token_p95_regression_ratio",
    )
    args.max_mixed_token_p99_regression_ratio = config_float_default(
        args.max_mixed_token_p99_regression_ratio,
        gates,
        "max_mixed_token_p99_regression_ratio",
    )
    args.min_deep_token_latency_samples = config_int_default(
        args.min_deep_token_latency_samples,
        gates,
        "min_deep_token_latency_samples",
    )
    args.max_deep_mixed_token_p95_regression_ratio = config_float_default(
        args.max_deep_mixed_token_p95_regression_ratio,
        gates,
        "max_deep_mixed_token_p95_regression_ratio",
    )
    args.max_deep_mixed_token_p99_regression_ratio = config_float_default(
        args.max_deep_mixed_token_p99_regression_ratio,
        gates,
        "max_deep_mixed_token_p99_regression_ratio",
    )
    args.host_memory_pressure_mib = config_int_default(
        args.host_memory_pressure_mib,
        gates,
        "host_memory_pressure_mib",
    )


def config_bool(gates: dict, key: str) -> bool:
    if key not in gates:
        return False
    value = gates[key]
    require(isinstance(value, bool), f"matrix_gates.{key} must be a boolean")
    return value


def config_int_default(current: int, gates: dict, key: str) -> int:
    if current != 0 or key not in gates:
        return current
    value = gates[key]
    require(
        isinstance(value, int) and not isinstance(value, bool),
        f"matrix_gates.{key} must be an integer",
    )
    return value


def config_float_default(current: float, gates: dict, key: str) -> float:
    if current != 0.0 or key not in gates:
        return current
    value = gates[key]
    require(
        isinstance(value, (int, float)) and not isinstance(value, bool),
        f"matrix_gates.{key} must be a number",
    )
    return float(value)


def validate_gate_args(args: argparse.Namespace) -> None:
    require(args.min_token_latency_samples >= 0, "--min-token-latency-samples must be non-negative")
    require(
        args.max_mixed_token_p95_regression_ratio >= 0.0,
        "--max-mixed-token-p95-regression-ratio must be non-negative",
    )
    require(
        args.max_mixed_token_p99_regression_ratio >= 0.0,
        "--max-mixed-token-p99-regression-ratio must be non-negative",
    )
    require(args.min_deep_token_latency_samples >= 0, "--min-deep-token-latency-samples must be non-negative")
    require(
        args.max_deep_mixed_token_p95_regression_ratio >= 0.0,
        "--max-deep-mixed-token-p95-regression-ratio must be non-negative",
    )
    require(
        args.max_deep_mixed_token_p99_regression_ratio >= 0.0,
        "--max-deep-mixed-token-p99-regression-ratio must be non-negative",
    )
    require(
        args.deep_latency_baseline
        or (
            args.min_deep_token_latency_samples == 0
            and args.max_deep_mixed_token_p95_regression_ratio == 0.0
            and args.max_deep_mixed_token_p99_regression_ratio == 0.0
        ),
        "deep latency gates require --deep-latency-baseline",
    )
    require(args.host_memory_pressure_mib >= 0, "--host-memory-pressure-mib must be non-negative")
    require(args.host_memory_pressure_mib <= 8192, "--host-memory-pressure-mib is capped at 8192 MiB")


def run_case(
    *,
    root: Path,
    runner: Path,
    llama_simple: Path,
    llama_cpp_source: Path | None,
    kv_bench: str,
    matrix_work_dir: Path,
    timeout: int,
    skip_event_trace: bool,
    skip_attention_trace: bool,
    skip_attention_page_segments_trace: bool,
    require_live_paging: bool,
    require_native_page_streaming: bool,
    native_page_streaming_contract_probe: bool,
    gpu_page_staging: bool,
    native_page_streaming_attention: bool,
    native_page_streaming_attention_ggml: bool,
    native_page_streaming_attention_backend_op: bool,
    native_page_streaming_flatten_flash: bool,
    attention_page_segments_live_offloaded_only: bool,
    aggregate_codec_gate: bool,
    prune_bulk_artifacts: bool,
    deep_latency_baseline: bool,
    override_short_predict: int,
    override_deep_predict: int,
    override_max_queued_pages: int,
    case: dict,
    iteration: int,
) -> CaseResult:
    case_id = str(case["id"])
    case_work_dir = matrix_work_dir / f"{case_id}-iter{iteration:02d}"
    command = [
        sys.executable,
        str(runner),
        "--llama-simple",
        str(llama_simple),
        "--qatq-kv-bench",
        kv_bench,
        "--model",
        str(case["model"]),
        "--model-id",
        str(case["model_id"]),
        "--work-dir",
        str(case_work_dir),
        "--sweep-kv-gpu-layers",
        ",".join(str(value) for value in case["sweep_kv_gpu_layers"]),
        "--short-prompt",
        str(case["short_prompt"]),
        "--deep-prompt-seed",
        str(case["deep_prompt_seed"]),
        "--timeout",
        str(timeout),
    ]
    if llama_cpp_source is not None:
        command.extend(["--llama-cpp-source", str(llama_cpp_source)])
    if skip_event_trace:
        command.append("--skip-event-trace")
    if skip_attention_trace:
        command.append("--skip-attention-trace")
    if skip_attention_page_segments_trace or case.get("skip_attention_page_segments_trace"):
        command.append("--skip-attention-page-segments-trace")
    if require_live_paging:
        command.append("--require-live-paging")
    if require_native_page_streaming:
        command.append("--require-native-page-streaming")
    probe_enabled = native_page_streaming_contract_probe or case.get("native_page_streaming_contract_probe")
    if probe_enabled:
        command.append("--native-page-streaming-contract-probe")
    if gpu_page_staging or case.get("gpu_page_staging"):
        command.append("--gpu-page-staging")
    if native_page_streaming_attention_ggml or case.get("native_page_streaming_attention_ggml"):
        command.append("--native-page-streaming-attention-ggml")
    if native_page_streaming_attention_backend_op or case.get("native_page_streaming_attention_backend_op"):
        command.append("--native-page-streaming-attention-backend-op")
    elif native_page_streaming_attention or case.get("native_page_streaming_attention"):
        command.append("--native-page-streaming-attention")
    if native_page_streaming_flatten_flash or case.get("native_page_streaming_flatten_flash"):
        command.append("--native-page-streaming-flatten-flash")
    if attention_page_segments_live_offloaded_only or case.get("attention_page_segments_live_offloaded_only"):
        command.append("--attention-page-segments-live-offloaded-only")
    if aggregate_codec_gate or case.get("aggregate_codec_gate"):
        command.append("--aggregate-codec-gate")
    if prune_bulk_artifacts or case.get("prune_bulk_artifacts"):
        command.append("--prune-bulk-artifacts")
    if deep_latency_baseline or case.get("deep_latency_baseline"):
        command.append("--deep-latency-baseline")
    optional_args = {
        "short_predict": "--short-predict",
        "deep_predict": "--deep-predict",
        "deep_repeat": "--deep-repeat",
        "deep_prompt_suffix": "--deep-prompt-suffix",
        "max_mixed_decode_regression": "--max-mixed-decode-regression",
        "min_gpu_saved_ratio": "--min-gpu-saved-ratio",
        "restore_bytes_per_token": "--restore-bytes-per-token",
        "cache_type_k": "--cache-type-k",
        "cache_type_v": "--cache-type-v",
        "gpu_layers": "--gpu-layers",
        "page_tokens": "--page-tokens",
        "n_kv_pad_tokens": "--n-kv-pad-tokens",
        "current_token": "--current-token",
        "hot_window_tokens": "--hot-window-tokens",
        "prefetch_window_tokens": "--prefetch-window-tokens",
        "next_required": "--next-required",
        "max_queued_pages": "--max-queued-pages",
        "native_page_streaming_transient_pool_max_bytes": "--native-page-streaming-transient-pool-max-bytes",
        "live_page_self_test_tokens": "--live-page-self-test-tokens",
        "live_restore_slot_pressure_self_test_tokens": "--live-restore-slot-pressure-self-test-tokens",
        "live_restore_slot_pressure_max_bytes": "--live-restore-slot-pressure-max-bytes",
        "attention_persistent_page_source_max_pages": "--attention-persistent-page-source-max-pages",
        "attention_persistent_page_source_max_source_pages": "--attention-persistent-page-source-max-source-pages",
        "attention_persistent_page_source_max_source_bytes": "--attention-persistent-page-source-max-source-bytes",
        "attention_persistent_page_source_max_retained_bytes": "--attention-persistent-page-source-max-retained-bytes",
        "attention_fixture_max_layers": "--attention-fixture-max-layers",
        "mlx_python": "--mlx-python",
        "mlx_qatq_bin": "--mlx-qatq-bin",
        "mlx_min_layers_checked": "--mlx-min-layers-checked",
        "mlx_min_heads_checked": "--mlx-min-heads-checked",
        "mlx_max_streaming_slowdown": "--mlx-max-streaming-slowdown",
    }
    for key, flag in optional_args.items():
        if key in case:
            if key == "short_predict" and override_short_predict > 0:
                value = override_short_predict
            elif key == "deep_predict" and override_deep_predict > 0:
                value = override_deep_predict
            elif key == "max_queued_pages" and override_max_queued_pages >= 0:
                value = override_max_queued_pages
            else:
                value = case[key]
            command.extend([flag, str(value)])
    if "short_predict" not in case and override_short_predict > 0:
        command.extend(["--short-predict", str(override_short_predict)])
    if "deep_predict" not in case and override_deep_predict > 0:
        command.extend(["--deep-predict", str(override_deep_predict)])
    if "max_queued_pages" not in case and override_max_queued_pages >= 0:
        command.extend(["--max-queued-pages", str(override_max_queued_pages)])
    if case.get("skip_cpu_kv_baseline"):
        command.append("--skip-cpu-kv-baseline")
    if case.get("skip_attention_event_trace"):
        command.append("--skip-attention-event-trace")
    if case.get("skip_attention_page_tensor_self_test"):
        command.append("--skip-attention-page-tensor-self-test")
    if case.get("mlx_streaming_attention_gate"):
        command.append("--mlx-streaming-attention-gate")

    case_work_dir.mkdir(parents=True, exist_ok=True)
    (case_work_dir / "matrix-command.txt").write_text(shell_join(command) + "\n", encoding="utf-8")
    started = time.time()
    try:
        completed = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=timeout + 60)
    except subprocess.TimeoutExpired as exc:
        elapsed = time.time() - started
        log_text = timeout_output_to_text(exc.stdout) + timeout_output_to_text(exc.stderr)
        (case_work_dir / "matrix-run.log").write_text(log_text, encoding="utf-8")
        return empty_result(
            case_id=case_id,
            iteration=iteration,
            model_id=str(case["model_id"]),
            status="fail",
            work_dir=case_work_dir,
            elapsed_seconds=elapsed,
            failure=augment_failure_with_stage_status(
                case_work_dir,
                f"matrix case timed out after {timeout + 60}s",
            ),
        )
    elapsed = time.time() - started
    (case_work_dir / "matrix-run.log").write_text(completed.stdout + completed.stderr, encoding="utf-8")
    if completed.returncode != 0:
        return empty_result(
            case_id=case_id,
            iteration=iteration,
            model_id=str(case["model_id"]),
            status="fail",
            work_dir=case_work_dir,
            elapsed_seconds=elapsed,
            failure=augment_failure_with_stage_status(
                case_work_dir,
                (completed.stderr or completed.stdout or "").strip(),
            ),
        )
    if probe_enabled:
        probe_path = case_work_dir / "native-page-streaming-contract-probe.json"
        try:
            probe = load_json(probe_path)
            require(
                probe.get("passed") is True and probe.get("native_live_vram_claimed") is False,
                f"contract probe did not pass fail-closed checks in {probe_path}",
            )
        except Exception as exc:
            return empty_result(
                case_id=case_id,
                iteration=iteration,
                model_id=str(case["model_id"]),
                status="fail",
                work_dir=case_work_dir,
                elapsed_seconds=elapsed,
                failure=f"contract probe completed but result parsing failed: {exc}",
            )
        return empty_result(
            case_id=case_id,
            iteration=iteration,
            model_id=str(case["model_id"]),
            status="pass",
            work_dir=case_work_dir,
            elapsed_seconds=elapsed,
            failure="",
        )

    evidence_path = case_work_dir / ("live-paging-evidence.json" if require_live_paging else "runtime-reclaim-evidence.json")
    summary_path = case_work_dir / "summary.md"
    try:
        evidence = load_json(evidence_path)
        summary = summary_path.read_text(encoding="utf-8")
        selected_layers = parse_selected_layers(summary)
        residency = evidence.get("residency_estimate") or {}
        event_trace = evidence.get("event_trace_report") or {}
        attention_trace = load_optional_json(case_work_dir / "attention-trace-summary.json")
        mlx_report = load_optional_json(case_work_dir / "mlx-streaming-attention.json")
        mlx_streaming = (mlx_report or {}).get("streaming") or {}
        mlx_materialized = (mlx_report or {}).get("materialized") or {}
        mlx_store = (mlx_report or {}).get("qatq_store") or {}
        mlx_materialized_seconds = float(mlx_materialized.get("seconds", 0.0) or 0.0)
        mlx_streaming_time_ratio = (
            float(mlx_streaming.get("seconds", 0.0) or 0.0) / mlx_materialized_seconds
            if mlx_materialized_seconds > 0.0
            else 0.0
        )
        latency = parse_token_latency_stats(
            case_work_dir / "tokens.csv",
            baseline_run="short-full-gpu",
            candidate_run="short-mixed-kv",
        )
        deep_latency = parse_token_latency_stats(
            case_work_dir / "tokens.csv",
            baseline_run="deep-full-gpu",
            candidate_run="deep-mixed-kv",
        )
        return CaseResult(
            case_id=case_id,
            iteration=iteration,
            model_id=str(case["model_id"]),
            status="pass",
            work_dir=case_work_dir,
            summary_path=summary_path,
            selected_layers=selected_layers,
            total_pages=int(evidence.get("total_pages", 0)),
            offloaded_pages=int(evidence.get("offloaded_pages", 0)),
            compressed_pages=int(evidence.get("compressed_pages", 0)),
            resident_pages=int(evidence.get("resident_pages", 0)),
            pass_through_pages=int(evidence.get("pass_through_pages", 0)),
            verified_restores=int(evidence.get("verified_restores", 0)),
            raw_bytes=int(evidence.get("raw_bytes", 0)),
            qatq_bytes=int(evidence.get("qatq_candidate_bytes", 0)),
            zstd_bytes=int(evidence.get("zstd_bytes", 0)),
            lz4_bytes=int(evidence.get("lz4_bytes", 0)),
            reclaimable_gpu_bytes=int(residency.get("reclaimable_gpu_bytes", 0)),
            event_trace_events=int(event_trace.get("events", 0)),
            event_trace_passed=event_trace.get("passed") is True,
            attention_trace_events=int(attention_trace.get("events", 0)),
            attention_trace_layers=int(attention_trace.get("layers", 0)),
            mlx_layers_checked=int((mlx_report or {}).get("layers_checked", 0)),
            mlx_heads_checked=int((mlx_report or {}).get("heads_checked", 0)),
            mlx_qatq_store_ratio=float(mlx_store.get("compression_ratio", 0.0) or 0.0),
            mlx_streaming_time_ratio=mlx_streaming_time_ratio,
            latency_samples=min(latency.full_samples, latency.mixed_samples),
            full_p95_decode_us=latency.full_p95,
            mixed_p95_decode_us=latency.mixed_p95,
            full_p99_decode_us=latency.full_p99,
            mixed_p99_decode_us=latency.mixed_p99,
            mixed_p95_regression=latency.p95_regression,
            mixed_p99_regression=latency.p99_regression,
            deep_latency_samples=min(deep_latency.full_samples, deep_latency.mixed_samples),
            deep_full_p95_decode_us=deep_latency.full_p95,
            deep_mixed_p95_decode_us=deep_latency.mixed_p95,
            deep_full_p99_decode_us=deep_latency.full_p99,
            deep_mixed_p99_decode_us=deep_latency.mixed_p99,
            deep_mixed_p95_regression=deep_latency.p95_regression,
            deep_mixed_p99_regression=deep_latency.p99_regression,
            elapsed_seconds=elapsed,
            failure="",
        )
    except Exception as exc:
        return empty_result(
            case_id=case_id,
            iteration=iteration,
            model_id=str(case["model_id"]),
            status="fail",
            work_dir=case_work_dir,
            elapsed_seconds=elapsed,
            failure=f"case completed but evidence parsing failed: {exc}",
        )


def empty_result(
    *,
    case_id: str,
    iteration: int,
    model_id: str,
    status: str,
    work_dir: Path,
    elapsed_seconds: float,
    failure: str,
) -> CaseResult:
    return CaseResult(
        case_id=case_id,
        iteration=iteration,
        model_id=model_id,
        status=status,
        work_dir=work_dir,
        summary_path=work_dir / "summary.md",
        selected_layers="",
        total_pages=0,
        offloaded_pages=0,
        compressed_pages=0,
        resident_pages=0,
        pass_through_pages=0,
        verified_restores=0,
        raw_bytes=0,
        qatq_bytes=0,
        zstd_bytes=0,
        lz4_bytes=0,
        reclaimable_gpu_bytes=0,
        event_trace_events=0,
        event_trace_passed=False,
        attention_trace_events=0,
        attention_trace_layers=0,
        mlx_layers_checked=0,
        mlx_heads_checked=0,
        mlx_qatq_store_ratio=0.0,
        mlx_streaming_time_ratio=0.0,
        latency_samples=0,
        full_p95_decode_us=0.0,
        mixed_p95_decode_us=0.0,
        full_p99_decode_us=0.0,
        mixed_p99_decode_us=0.0,
        mixed_p95_regression=0.0,
        mixed_p99_regression=0.0,
        deep_latency_samples=0,
        deep_full_p95_decode_us=0.0,
        deep_mixed_p95_decode_us=0.0,
        deep_full_p99_decode_us=0.0,
        deep_mixed_p99_decode_us=0.0,
        deep_mixed_p95_regression=0.0,
        deep_mixed_p99_regression=0.0,
        elapsed_seconds=elapsed_seconds,
        failure=failure,
    )


def augment_failure_with_stage_status(work_dir: Path, failure: str) -> str:
    stage_path = work_dir / "stage-status.json"
    if not stage_path.exists():
        return failure
    try:
        status = load_json(stage_path)
    except Exception as exc:
        return f"{failure}\nstage_status_path={stage_path}\nstage_status_parse_error={exc}"
    stages = status.get("stages")
    if not isinstance(stages, dict) or not stages:
        return f"{failure}\nstage_status_path={stage_path}"
    latest = max(
        (stage for stage in stages.values() if isinstance(stage, dict)),
        key=lambda stage: float(stage.get("timestamp_unix", 0.0) or 0.0),
        default=None,
    )
    if latest is None:
        return f"{failure}\nstage_status_path={stage_path}"
    stage_name = latest.get("stage", "")
    stage_status = latest.get("status", "")
    stage_log = latest.get("log", "")
    return (
        f"{failure}\n"
        f"stage_status_path={stage_path}\n"
        f"latest_stage={stage_name}\n"
        f"latest_stage_status={stage_status}\n"
        f"latest_stage_log={stage_log}"
    )


def timeout_output_to_text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def parse_token_latency_stats(path: Path, *, baseline_run: str, candidate_run: str) -> TokenLatencyStats:
    if not path.exists():
        return empty_latency_stats()
    full_decode_us: list[float] = []
    mixed_decode_us: list[float] = []
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            if row.get("row_type") != "decode-token":
                continue
            run_name = row.get("run_name", "")
            if run_name not in {baseline_run, candidate_run}:
                continue
            token = (row.get("generated_token") or "").strip()
            if token == "":
                continue
            try:
                batch_tokens = int(row.get("batch_tokens", "") or "0")
            except ValueError:
                continue
            if batch_tokens != 1:
                continue
            try:
                decode_us = float(row.get("decode_us", "") or "0")
            except ValueError:
                continue
            if decode_us <= 0.0 or not math.isfinite(decode_us):
                continue
            if run_name == baseline_run:
                full_decode_us.append(decode_us)
            else:
                mixed_decode_us.append(decode_us)
    full_p95 = percentile_nearest_rank(full_decode_us, 95.0)
    mixed_p95 = percentile_nearest_rank(mixed_decode_us, 95.0)
    full_p99 = percentile_nearest_rank(full_decode_us, 99.0)
    mixed_p99 = percentile_nearest_rank(mixed_decode_us, 99.0)
    return TokenLatencyStats(
        full_samples=len(full_decode_us),
        mixed_samples=len(mixed_decode_us),
        full_p95=full_p95,
        mixed_p95=mixed_p95,
        full_p99=full_p99,
        mixed_p99=mixed_p99,
        p95_regression=ratio_regression(mixed_p95, full_p95),
        p99_regression=ratio_regression(mixed_p99, full_p99),
    )


def empty_latency_stats() -> TokenLatencyStats:
    return TokenLatencyStats(
        full_samples=0,
        mixed_samples=0,
        full_p95=0.0,
        mixed_p95=0.0,
        full_p99=0.0,
        mixed_p99=0.0,
        p95_regression=0.0,
        p99_regression=0.0,
    )


def percentile_nearest_rank(values: list[float], percentile: float) -> float:
    if not values:
        return 0.0
    sorted_values = sorted(values)
    rank = math.ceil((percentile / 100.0) * len(sorted_values))
    index = min(max(rank - 1, 0), len(sorted_values) - 1)
    return sorted_values[index]


def ratio_regression(candidate: float, baseline: float) -> float:
    if baseline <= 0.0:
        return 0.0
    return candidate / baseline - 1.0


def build_matrix_summary(
    results: list[CaseResult],
    work_dir: Path,
    *,
    require_live_paging: bool,
    require_native_page_streaming: bool,
    native_page_streaming_contract_probe: bool,
    gpu_page_staging: bool,
    native_page_streaming_attention: bool,
    native_page_streaming_attention_ggml: bool,
    native_page_streaming_attention_backend_op: bool,
    native_page_streaming_flatten_flash: bool,
    attention_page_segments_live_offloaded_only: bool,
    aggregate_codec_gate: bool,
    require_stable_reclaim: bool,
    require_stable_qc_bytes: bool,
    max_elapsed_jitter_ratio: float,
    min_token_latency_samples: int,
    max_mixed_token_p95_regression_ratio: float,
    max_mixed_token_p99_regression_ratio: float,
    deep_latency_baseline: bool,
    min_deep_token_latency_samples: int,
    max_deep_mixed_token_p95_regression_ratio: float,
    max_deep_mixed_token_p99_regression_ratio: float,
    host_memory_pressure_mib: int,
    prune_bulk_artifacts: bool,
) -> str:
    passed = sum(1 for result in results if result.status == "pass")
    failed = len(results) - passed
    out = "# llama.cpp Live VRAM Matrix\n\n"
    out += "Generated by `scripts/llama_cpp_live_vram_matrix.py`.\n\n"
    out += "## Summary\n\n"
    out += f"- work dir: `{work_dir}`\n"
    out += f"- evidence gate: `{'live-paging' if require_live_paging else 'runtime-reclaim'}`\n"
    out += f"- native page-streaming gate: `{'enabled' if require_native_page_streaming else 'disabled'}`\n"
    out += f"- native page-streaming contract probe: `{'enabled' if native_page_streaming_contract_probe else 'case/default'}`\n"
    out += f"- native page-streaming attention flag: `{'enabled' if native_page_streaming_attention else 'case/default'}`\n"
    out += f"- native page-streaming ggml backend: `{'enabled' if native_page_streaming_attention_ggml else 'case/default'}`\n"
    out += f"- native page-streaming backend op: `{'enabled' if native_page_streaming_attention_backend_op else 'case/default'}`\n"
    out += f"- native page-streaming flattened Flash Attention: `{'enabled' if native_page_streaming_flatten_flash else 'case/default'}`\n"
    out += f"- page-segment live-offloaded-only trace: `{'enabled' if attention_page_segments_live_offloaded_only else 'case/default'}`\n"
    out += f"- aggregate codec gate: `{'enabled' if aggregate_codec_gate else 'case/default'}`\n"
    out += f"- GPU page staging: `{'enabled' if gpu_page_staging else 'case/default'}`\n"
    out += f"- stable reclaim gate: `{'enabled' if require_stable_reclaim else 'disabled'}`\n"
    out += f"- stable codec byte gate: `{'enabled' if require_stable_qc_bytes else 'disabled'}`\n"
    out += f"- elapsed jitter gate: `{max_elapsed_jitter_ratio:.6f}`\n"
    out += f"- token latency sample gate: `{min_token_latency_samples}`\n"
    out += f"- mixed p95 token regression gate: `{max_mixed_token_p95_regression_ratio:.6f}`\n"
    out += f"- mixed p99 token regression gate: `{max_mixed_token_p99_regression_ratio:.6f}`\n"
    out += f"- deep latency baseline: `{'enabled' if deep_latency_baseline else 'disabled'}`\n"
    out += f"- deep token latency sample gate: `{min_deep_token_latency_samples}`\n"
    out += f"- deep mixed p95 token regression gate: `{max_deep_mixed_token_p95_regression_ratio:.6f}`\n"
    out += f"- deep mixed p99 token regression gate: `{max_deep_mixed_token_p99_regression_ratio:.6f}`\n"
    out += f"- host memory pressure: `{host_memory_pressure_mib} MiB`\n"
    out += f"- bulk artifact pruning: `{'enabled' if prune_bulk_artifacts else 'disabled'}`\n"
    out += f"- cases: `{len(results)}`\n"
    out += f"- unique case ids: `{len({result.case_id for result in results})}`\n"
    out += f"- passed: `{passed}`\n"
    out += f"- failed: `{failed}`\n\n"
    out += "## Cases\n\n"
    out += "| case | iter | status | selected KV GPU layers | exact restores | offloaded QATQ pages | resident pages | pass-through | event trace | attention reads | MLX layers | MLX heads | MLX QATQ ratio | MLX stream/materialised | reclaimable GPU MiB | QATQ MiB | zstd MiB | lz4 MiB | elapsed s |\n"
    out += "| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"
    for result in results:
        event_trace = f"{'pass' if result.event_trace_passed else 'fail'}/{result.event_trace_events}"
        mlx_qatq_ratio = f"{result.mlx_qatq_store_ratio:.6f}" if result.mlx_qatq_store_ratio > 0.0 else ""
        mlx_time_ratio = f"{result.mlx_streaming_time_ratio:.3f}" if result.mlx_streaming_time_ratio > 0.0 else ""
        out += (
            f"| {result.case_id} | {result.iteration} | {result.status} | {result.selected_layers or '-'} | "
            f"{result.verified_restores}/{result.total_pages} | "
            f"{result.compressed_pages}/{result.offloaded_pages} | "
            f"{result.resident_pages} | {result.pass_through_pages} | {event_trace} | "
            f"{result.attention_trace_events}/{result.attention_trace_layers} layers | "
            f"{result.mlx_layers_checked or ''} | {result.mlx_heads_checked or ''} | "
            f"{mlx_qatq_ratio} | {mlx_time_ratio} | "
            f"{mib(result.reclaimable_gpu_bytes):.2f} | {mib(result.qatq_bytes):.2f} | "
            f"{mib(result.zstd_bytes):.2f} | {mib(result.lz4_bytes):.2f} | "
            f"{result.elapsed_seconds:.2f} |\n"
        )
    out += "\n## Stability\n\n"
    out += "| case | runs | failures | elapsed min/max s | reclaimable GPU min/max MiB | QATQ min/max MiB |\n"
    out += "| --- | ---: | ---: | ---: | ---: | ---: |\n"
    for case_id in sorted({result.case_id for result in results}):
        case_results = [result for result in results if result.case_id == case_id]
        pass_results = [result for result in case_results if result.status == "pass"]
        failures_count = len(case_results) - len(pass_results)
        if pass_results:
            elapsed_values = [result.elapsed_seconds for result in pass_results]
            reclaim_values = [mib(result.reclaimable_gpu_bytes) for result in pass_results]
            qatq_values = [mib(result.qatq_bytes) for result in pass_results]
            elapsed_range = f"{min(elapsed_values):.2f}/{max(elapsed_values):.2f}"
            reclaim_range = f"{min(reclaim_values):.2f}/{max(reclaim_values):.2f}"
            qatq_range = f"{min(qatq_values):.2f}/{max(qatq_values):.2f}"
        else:
            elapsed_range = "-"
            reclaim_range = "-"
            qatq_range = "-"
        out += f"| {case_id} | {len(case_results)} | {failures_count} | {elapsed_range} | {reclaim_range} | {qatq_range} |\n"
    out += "\n## Token Latency\n\n"
    out += "| case | iter | samples | full p95 us | mixed p95 us | p95 regression | full p99 us | mixed p99 us | p99 regression |\n"
    out += "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"
    for result in results:
        out += (
            f"| {result.case_id} | {result.iteration} | {result.latency_samples} | "
            f"{result.full_p95_decode_us:.0f} | {result.mixed_p95_decode_us:.0f} | "
            f"{result.mixed_p95_regression:.6f} | {result.full_p99_decode_us:.0f} | "
            f"{result.mixed_p99_decode_us:.0f} | {result.mixed_p99_regression:.6f} |\n"
        )
    if deep_latency_baseline or any(result.deep_latency_samples > 0 for result in results):
        out += "\n## Deep Token Latency\n\n"
        out += "| case | iter | samples | deep full p95 us | deep mixed p95 us | p95 regression | deep full p99 us | deep mixed p99 us | p99 regression |\n"
        out += "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"
        for result in results:
            out += (
                f"| {result.case_id} | {result.iteration} | {result.deep_latency_samples} | "
                f"{result.deep_full_p95_decode_us:.0f} | {result.deep_mixed_p95_decode_us:.0f} | "
                f"{result.deep_mixed_p95_regression:.6f} | {result.deep_full_p99_decode_us:.0f} | "
                f"{result.deep_mixed_p99_decode_us:.0f} | {result.deep_mixed_p99_regression:.6f} |\n"
            )
    failures = [result for result in results if result.status != "pass"]
    if failures:
        out += "\n## Failures\n\n"
        for result in failures:
            out += f"### {result.case_id}\n\n"
            out += fenced(result.failure[-4000:] or "unknown failure")
            out += "\n"
    out += "\n## Claim Boundary\n\n"
    if passed > 0:
        if require_live_paging:
            out += "- Supported by passing cases: page-staged runtime KV placement reduced persistent GPU K/V residency while preserving deterministic continuations.\n"
        else:
            out += "- Supported by passing cases: whole-tensor/layer-granularity runtime KV placement reduced GPU KV allocation while preserving deterministic continuations.\n"
        out += "- Supported by passing cases: QATQ restored exported KV pages exactly and beat raw, zstd, and lz4 on the same page boundaries.\n"
    else:
        out += "- Not supported: no configured case passed the evidence gate.\n"
    if require_live_paging and failed == 0:
        out += "- Supported by passing cases: strict page-staging proof gate passed with page-granular runtime reclaim evidence.\n"
    elif require_live_paging:
        out += "- Not supported: strict live-paging proof gate did not pass for every configured case.\n"
        out += "- Current failures must be fixed before claiming transparent token-page live VRAM reduction.\n"
    else:
        out += "- Not supported: transparent live token-page eviction and restore inside the attention loop.\n"
        out += "- Use `--require-live-paging` to turn this matrix into the future page-granular conformance gate.\n"
    if any(result.mlx_heads_checked > 0 for result in results):
        out += "- Supported by passing MLX-gated cases: external page-bounded streaming attention matched the materialised MLX reference over the reported layer/head coverage using QATQ-restored pages.\n"
    if require_native_page_streaming and failed == 0:
        out += "- Supported by passing cases: native non-concat page-streaming attention correctness passed for every configured case.\n"
        out += "- Boundary: production live paging additionally requires the adapter audit gate plus broader latency-tail, memory-pressure, and soak evidence, not only staged backend-op correctness.\n"
    else:
        out += "- Not supported: native non-concat page-streaming attention inside llama.cpp remains the production gate.\n"
    return out


def allocate_host_memory_pressure(mib_count: int) -> bytearray | None:
    if mib_count == 0:
        return None
    byte_count = mib_count * 1024 * 1024
    pressure = bytearray(byte_count)
    page_size = 4096
    for index in range(0, byte_count, page_size):
        pressure[index] = (index // page_size) & 0xFF
    pressure[-1] ^= 0x5A
    return pressure


def evaluate_stability_gates(
    results: list[CaseResult],
    *,
    require_stable_reclaim: bool,
    require_stable_qc_bytes: bool,
    max_elapsed_jitter_ratio: float,
    min_token_latency_samples: int,
    max_mixed_token_p95_regression_ratio: float,
    max_mixed_token_p99_regression_ratio: float,
    min_deep_token_latency_samples: int,
    max_deep_mixed_token_p95_regression_ratio: float,
    max_deep_mixed_token_p99_regression_ratio: float,
) -> list[str]:
    failures: list[str] = []
    enforce_latency = (
        min_token_latency_samples > 0
        or max_mixed_token_p95_regression_ratio > 0.0
        or max_mixed_token_p99_regression_ratio > 0.0
    )
    if enforce_latency:
        required_samples = max(min_token_latency_samples, 1)
        for result in results:
            if result.status != "pass":
                continue
            if result.latency_samples < required_samples:
                failures.append(
                    f"{result.case_id}: token latency samples {result.latency_samples} "
                    f"below required {required_samples}"
                )
                continue
            if (
                max_mixed_token_p95_regression_ratio > 0.0
                and result.mixed_p95_regression > max_mixed_token_p95_regression_ratio
            ):
                failures.append(
                    f"{result.case_id}: mixed p95 token decode regression "
                    f"{result.mixed_p95_regression:.6f} exceeded "
                    f"{max_mixed_token_p95_regression_ratio:.6f}"
                )
            if (
                max_mixed_token_p99_regression_ratio > 0.0
                and result.mixed_p99_regression > max_mixed_token_p99_regression_ratio
            ):
                failures.append(
                    f"{result.case_id}: mixed p99 token decode regression "
                    f"{result.mixed_p99_regression:.6f} exceeded "
                    f"{max_mixed_token_p99_regression_ratio:.6f}"
                )
    enforce_deep_latency = (
        min_deep_token_latency_samples > 0
        or max_deep_mixed_token_p95_regression_ratio > 0.0
        or max_deep_mixed_token_p99_regression_ratio > 0.0
    )
    if enforce_deep_latency:
        required_samples = max(min_deep_token_latency_samples, 1)
        for result in results:
            if result.status != "pass":
                continue
            if result.deep_latency_samples < required_samples:
                failures.append(
                    f"{result.case_id}: deep token latency samples {result.deep_latency_samples} "
                    f"below required {required_samples}"
                )
                continue
            if (
                max_deep_mixed_token_p95_regression_ratio > 0.0
                and result.deep_mixed_p95_regression > max_deep_mixed_token_p95_regression_ratio
            ):
                failures.append(
                    f"{result.case_id}: deep mixed p95 token decode regression "
                    f"{result.deep_mixed_p95_regression:.6f} exceeded "
                    f"{max_deep_mixed_token_p95_regression_ratio:.6f}"
                )
            if (
                max_deep_mixed_token_p99_regression_ratio > 0.0
                and result.deep_mixed_p99_regression > max_deep_mixed_token_p99_regression_ratio
            ):
                failures.append(
                    f"{result.case_id}: deep mixed p99 token decode regression "
                    f"{result.deep_mixed_p99_regression:.6f} exceeded "
                    f"{max_deep_mixed_token_p99_regression_ratio:.6f}"
                )
    for case_id in sorted({result.case_id for result in results}):
        case_results = [result for result in results if result.case_id == case_id]
        pass_results = [result for result in case_results if result.status == "pass"]
        if len(pass_results) < 2:
            continue
        if require_stable_reclaim:
            values = {result.reclaimable_gpu_bytes for result in pass_results}
            if len(values) != 1:
                failures.append(f"{case_id}: reclaimable GPU bytes changed across passing runs: {sorted(values)}")
        if require_stable_qc_bytes:
            byte_tuples = {
                (result.qatq_bytes, result.zstd_bytes, result.lz4_bytes, result.raw_bytes)
                for result in pass_results
            }
            if len(byte_tuples) != 1:
                rendered = ", ".join(
                    f"qatq={qatq},zstd={zstd},lz4={lz4},raw={raw}"
                    for qatq, zstd, lz4, raw in sorted(byte_tuples)
                )
                failures.append(f"{case_id}: codec byte totals changed across passing runs: {rendered}")
        if max_elapsed_jitter_ratio > 0.0:
            elapsed_values = [result.elapsed_seconds for result in pass_results if result.elapsed_seconds > 0.0]
            if len(elapsed_values) >= 2:
                min_elapsed = min(elapsed_values)
                max_elapsed = max(elapsed_values)
                jitter = (max_elapsed - min_elapsed) / min_elapsed
                if jitter > max_elapsed_jitter_ratio:
                    failures.append(
                        f"{case_id}: elapsed jitter {jitter:.6f} exceeded {max_elapsed_jitter_ratio:.6f} "
                        f"({min_elapsed:.2f}s/{max_elapsed:.2f}s)"
                    )
    return failures


def parse_selected_layers(summary: str) -> str:
    match = re.search(r"selected mixed KV GPU layers: `([^`]+)`", summary)
    if match:
        return match.group(1)
    match = re.search(r"Selected fastest passing frontier point: `([^`]+)`", summary)
    return match.group(1) if match else ""


def require_file(path: Path, label: str) -> Path:
    require(path.exists(), f"{label} path does not exist: {path}")
    return path


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def load_optional_json(path: Path) -> dict:
    if not path.exists():
        return {}
    return load_json(path)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def shell_join(command: list[str]) -> str:
    return " ".join(sh_quote(part) for part in command)


def sh_quote(value: str) -> str:
    if not value:
        return "''"
    if all(ch.isalnum() or ch in "/._-:=+" for ch in value):
        return value
    return "'" + value.replace("'", "'\\''") + "'"


def fenced(text: str) -> str:
    return "```text\n" + text.replace("```", "'''") + "\n```\n"


def mib(value: int) -> float:
    return value / (1024 * 1024)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        raise SystemExit(130)
