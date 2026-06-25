#!/usr/bin/env python3
"""Run fail-closed llama.cpp live-VRAM evidence for QATQ.

This runner is intentionally scoped to the current QATQ llama.cpp adapter
granularity: whole-tensor/layer KV placement via `--qatq-kv-gpu-layers`.
It proves reduced runtime GPU KV allocation, behaviour preservation for a
deterministic continuation, exact QATQ replay, and compression wins over
zstd/lz4. It does not claim token-page live eviction.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import os
import secrets
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


DEFAULT_RUNTIME_COMMIT = "7992aa7c8e21ea2eb7a5e4802da56eec7b376036"
DEFAULT_ADAPTER_VERSION = "qatq-kv-export-7992aa7c8"
DEFAULT_MODEL = os.environ.get("QATQ_LLAMA_MODEL_QWEN25_CODER_3B", "")
DEFAULT_SHORT_PROMPT = (
    "Write a concise Rust function that validates a QATQ live KV manifest, "
    "rejects duplicate tensor files, and returns a clear error."
)
DEFAULT_DEEP_PROMPT_SEED = (
    "Review this Rust live KV-cache migration design for exact restore, "
    "adversarial manifest handling, GPU residency accounting, timeout budgets, "
    "cancellation, duplicate page keys, compression evidence against zstd and "
    "lz4, and deterministic task continuation. "
)


@dataclass(frozen=True)
class RunSpec:
    name: str
    export_dir: Path
    output_manifest: Path
    token_timings: Path
    prompt: str
    predict: int
    kv_gpu_layers: int | None
    no_kv_offload: bool = False
    event_trace: Path | None = None
    attention_trace: Path | None = None
    attention_event_trace: Path | None = None
    attention_materialized_source_trace: Path | None = None
    attention_page_composed_source_trace: Path | None = None
    attention_persistent_page_source_trace: Path | None = None
    attention_page_segments_trace: Path | None = None
    native_page_streaming_contract: bool = False
    native_page_streaming_attention: bool = False
    native_page_streaming_attention_backend_op: bool = False
    native_page_streaming_transient_pool_max_bytes: int = 256 * 1024 * 1024
    attention_persistent_page_source_max_pages: int = 4096
    attention_persistent_page_source_max_source_pages: int = 16
    attention_persistent_page_source_max_source_bytes: int = 256 * 1024 * 1024
    attention_persistent_page_source_max_retained_bytes: int = 1024 * 1024 * 1024
    attention_fixture_dir: Path | None = None
    attention_fixture_max_layers: int = 1
    attention_page_tensor_self_test: Path | None = None
    attention_page_tensor_self_test_limit: int = 16
    live_page_self_test_tokens: int = 0
    live_physical_page_alloc_self_test_tokens: int = 0
    live_restore_slot_pressure_self_test_tokens: int = 0
    live_restore_slot_pressure_max_bytes: int = 1
    live_persistent_page_pool_trace: Path | None = None
    live_persistent_page_pool_self_test_pages: int = 0
    native_page_streaming_attention_ggml: bool = False


@dataclass(frozen=True)
class FrontierResult:
    layers: int
    output_passed: bool
    total_us: int
    full_gpu_us: int
    cpu_kv_us: int | None
    decode_regression: float
    faster_than_cpu_kv: bool | None
    gpu_context_bytes: int
    total_context_bytes: int
    saved_ratio: float
    gpu_resident_tensors: int
    total_tensors: int
    passed: bool
    failure: str
    compare_json: dict | None


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


def write_stage_status(work_dir: Path, stage: str, status: str, **fields) -> dict:
    """Record the latest state of a long-running evidence stage.

    The evidence runner is often used for expensive GPU jobs. When a run fails
    or times out, this structured status is the stable breadcrumb trail that
    tells the matrix runner and operator which stage was active and what files
    existed at the failure boundary.
    """
    work_dir.mkdir(parents=True, exist_ok=True)
    timestamp = time.time()
    record = {
        "format": "qatq-live-vram-stage-event-v1",
        "stage": stage,
        "status": status,
        "timestamp_unix": timestamp,
        **fields,
    }
    events_path = work_dir / "stage-events.jsonl"
    with events_path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")

    status_path = work_dir / "stage-status.json"
    status = {
        "format": "qatq-live-vram-stage-status-v1",
        "updated_at_unix": timestamp,
        "stages": {},
    }
    if status_path.exists():
        try:
            loaded = load_json(status_path)
            if isinstance(loaded.get("stages"), dict):
                status["stages"] = loaded["stages"]
        except Exception:
            status["previous_status_parse_error"] = True
    status["stages"][stage] = record
    tmp_path = status_path.with_suffix(".json.tmp")
    tmp_path.write_text(json.dumps(status, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp_path.replace(status_path)
    return record


def artifact_state(path: Path | None) -> dict | None:
    if path is None:
        return None
    state = {"path": str(path), "exists": path.exists()}
    if path.exists() and path.is_file():
        state["bytes"] = path.stat().st_size
    return state


def run_spec_artifacts(spec: RunSpec, log_path: Path) -> dict:
    artifacts = {
        "log": artifact_state(log_path),
        "export_manifest": artifact_state(spec.export_dir / "manifest.json"),
        "output_manifest": artifact_state(spec.output_manifest),
        "token_timings": artifact_state(spec.token_timings),
    }
    optional = {
        "event_trace": spec.event_trace,
        "attention_trace": spec.attention_trace,
        "attention_event_trace": spec.attention_event_trace,
        "attention_materialized_source_trace": spec.attention_materialized_source_trace,
        "attention_page_composed_source_trace": spec.attention_page_composed_source_trace,
        "attention_persistent_page_source_trace": spec.attention_persistent_page_source_trace,
        "attention_page_segments_trace": spec.attention_page_segments_trace,
        "attention_page_tensor_self_test": spec.attention_page_tensor_self_test,
        "live_persistent_page_pool_trace": spec.live_persistent_page_pool_trace,
    }
    for name, path in optional.items():
        if path is not None:
            artifacts[name] = artifact_state(path)
    if spec.attention_fixture_dir is not None:
        artifacts["attention_fixture_manifest"] = artifact_state(
            spec.attention_fixture_dir / "attention-fixture.json"
        )
    return artifacts


def timeout_output_to_text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-simple", default="/private/tmp/qatq-llama.cpp/build/bin/llama-simple")
    parser.add_argument(
        "--llama-cpp-source",
        default="",
        help=(
            "Path to the patched llama.cpp source checkout. When omitted, the "
            "runner infers it from --llama-simple if the binary lives under "
            "<checkout>/build/bin/llama-simple."
        ),
    )
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--model", default=DEFAULT_MODEL, help="Path to local GGUF model")
    parser.add_argument("--model-id", default="", help="Stable model label for evidence")
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-evidence")
    parser.add_argument("--runtime-commit", default=DEFAULT_RUNTIME_COMMIT)
    parser.add_argument("--adapter-version", default=DEFAULT_ADAPTER_VERSION)
    parser.add_argument("--gpu-layers", type=int, default=99)
    parser.add_argument("--mixed-kv-gpu-layers", type=int, default=18)
    parser.add_argument(
        "--sweep-kv-gpu-layers",
        default="",
        help="Comma-separated mixed KV GPU layer counts to test before selecting the fastest passing point.",
    )
    parser.add_argument("--cache-type-k", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--cache-type-v", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--short-prompt", default=DEFAULT_SHORT_PROMPT)
    parser.add_argument(
        "--deep-prompt-seed",
        default=DEFAULT_DEEP_PROMPT_SEED,
        help="Prompt fragment repeated for the deep prefill/export run.",
    )
    parser.add_argument(
        "--deep-prompt-suffix",
        default="",
        help=(
            "Optional suffix appended after the repeated deep prompt seed. Use "
            "this to turn a long-context prefill into an open continuation for "
            "deep latency sampling."
        ),
    )
    parser.add_argument("--short-predict", type=int, default=32)
    parser.add_argument("--deep-predict", type=int, default=1)
    parser.add_argument("--deep-repeat", type=int, default=80)
    parser.add_argument(
        "--deep-latency-baseline",
        action="store_true",
        help=(
            "Run a deep full-GPU baseline with the same deep prompt and "
            "prediction count, compare it against deep mixed-KV output, and "
            "include both runs in tokens.csv for long-context latency gates."
        ),
    )
    parser.add_argument(
        "--page-tokens",
        type=int,
        default=0,
        help="Ask patched llama.cpp to split exported KV tensors into token pages of this size.",
    )
    parser.add_argument(
        "--n-kv-pad-tokens",
        type=int,
        default=0,
        help=(
            "Set LLAMA_QATQ_N_KV_PAD_TOKENS for patched llama.cpp runs. "
            "Use this only with live page-staging/native backend-op evidence "
            "to tune graph reserve shape; 0 leaves llama.cpp's default policy."
        ),
    )
    parser.add_argument(
        "--gpu-page-staging",
        action="store_true",
        help=(
            "Ask patched llama.cpp to keep canonical KV tensors off GPU and stage "
            "scheduler-resident K/V pages on the accelerator through the persistent "
            "attention page-source adapter."
        ),
    )
    parser.add_argument(
        "--hot-window-tokens",
        type=int,
        default=0,
        help="Hot token window passed to qatq-kv-bench live-VRAM scheduling.",
    )
    parser.add_argument(
        "--prefetch-window-tokens",
        type=int,
        default=32,
        help=(
            "Prefetch lead window passed to qatq-kv-bench live-VRAM scheduling. "
            "Pages needed within hot plus prefetch are kept resident."
        ),
    )
    parser.add_argument(
        "--current-token",
        type=int,
        default=0,
        help="Current token passed to qatq-kv-bench and patched llama.cpp trace scheduling.",
    )
    parser.add_argument(
        "--next-required",
        default="uniform-after-hot",
        choices=["uniform-after-hot", "page-end", "cold-after-hot"],
        help="How qatq-kv-bench assigns next-use tokens to replayed pages.",
    )
    parser.add_argument(
        "--max-queued-pages",
        type=int,
        default=0,
        help=(
            "Maximum pages QATQ may schedule for live offload in one evidence "
            "replay. 0 keeps the qatq-kv-bench default. Use this for "
            "long-context latency tuning where restore/staging pressure must be "
            "bounded by page count."
        ),
    )
    parser.add_argument("--restore-bytes-per-token", type=int, default=67_108_864)
    parser.add_argument("--min-gpu-saved-ratio", type=float, default=0.10)
    parser.add_argument("--max-mixed-decode-regression", type=float, default=0.50)
    parser.add_argument(
        "--min-deep-token-latency-samples",
        type=int,
        default=0,
        help=(
            "Optional generated-token sample gate for deep full-GPU versus deep "
            "mixed-KV latency. Requires --deep-latency-baseline."
        ),
    )
    parser.add_argument(
        "--max-deep-mixed-token-p95-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p95 generated-token latency regression gate for the deep "
            "mixed-KV run versus the deep full-GPU baseline. 0 disables it."
        ),
    )
    parser.add_argument(
        "--max-deep-mixed-token-p99-regression-ratio",
        type=float,
        default=0.0,
        help=(
            "Optional p99 generated-token latency regression gate for the deep "
            "mixed-KV run versus the deep full-GPU baseline. 0 disables it."
        ),
    )
    parser.add_argument(
        "--skip-runtime-reclaim-gate",
        action="store_true",
        help=(
            "Generate evidence without asserting the runtime GPU reclaim gate. "
            "Use only for native attention correctness runs where the current "
            "consumer is CPU-backed and cannot honestly prove GPU page reclaim."
        ),
    )
    parser.add_argument("--skip-cpu-kv-baseline", action="store_true")
    parser.add_argument(
        "--skip-event-trace",
        action="store_true",
        help="Do not ask patched llama.cpp to emit a QATQ event trace for the deep export.",
    )
    parser.add_argument(
        "--skip-attention-trace",
        action="store_true",
        help="Do not ask patched llama.cpp to emit actual attention-path key/value read telemetry for the deep export.",
    )
    parser.add_argument(
        "--skip-attention-page-segments-trace",
        action="store_true",
        help=(
            "Do not ask patched llama.cpp to emit page-segment diagnostics from "
            "get_k/get_v. This keeps latency diagnostics from paying for the "
            "native page-streaming proof trace. It cannot be combined with "
            "--require-native-page-streaming."
        ),
    )
    parser.add_argument(
        "--attention-page-segments-live-offloaded-only",
        action="store_true",
        help=(
            "Ask patched llama.cpp to emit attention page-segment rows only for "
            "live-offloaded cold pages. This keeps strict native proof runs from "
            "charging resident fast-path diagnostic serialization to token "
            "latency while still requiring cold/offloaded rows to be native and "
            "attention-consumed."
        ),
    )
    parser.add_argument(
        "--skip-attention-event-trace",
        action="store_true",
        help="Do not ask patched llama.cpp to emit restore-before-attention lifecycle events from the actual attention path.",
    )
    parser.add_argument(
        "--skip-attention-materialized-source",
        action="store_true",
        help=(
            "Do not ask patched llama.cpp to make the attention graph consume "
            "materialized K/V source tensors and emit a materialized-source trace."
        ),
    )
    parser.add_argument(
        "--skip-attention-page-composed-source",
        action="store_true",
        help=(
            "Do not ask patched llama.cpp to compose attention K/V sources from "
            "bounded materialized token pages and emit a page-composed-source trace."
        ),
    )
    parser.add_argument(
        "--skip-attention-persistent-page-source",
        action="store_true",
        help=(
            "Do not ask patched llama.cpp to copy attention K/V pages into "
            "independently allocated backend page tensors and compose attention "
            "from those copied page tensors."
        ),
    )
    parser.add_argument(
        "--attention-persistent-page-source-max-pages",
        type=int,
        default=4096,
        help="Maximum retained backend page tensors allowed for the persistent page-source proof.",
    )
    parser.add_argument(
        "--attention-persistent-page-source-max-source-pages",
        type=int,
        default=16,
        help="Maximum token pages allowed per attention source before failing closed to avoid graph object exhaustion.",
    )
    parser.add_argument(
        "--attention-persistent-page-source-max-source-bytes",
        type=int,
        default=256 * 1024 * 1024,
        help="Maximum retained page-source bytes allowed for one attention source before failing closed.",
    )
    parser.add_argument(
        "--attention-persistent-page-source-max-retained-bytes",
        type=int,
        default=1024 * 1024 * 1024,
        help="Maximum total retained backend page-source bytes allowed before failing closed.",
    )
    parser.add_argument(
        "--skip-attention-equivalence",
        action="store_true",
        help="Do not ask patched llama.cpp to emit a Q/K/V fixture for QATQ page-bounded attention equivalence.",
    )
    parser.add_argument(
        "--attention-fixture-max-layers",
        type=int,
        default=1,
        help="Maximum llama.cpp layers to capture in the attention fixture.",
    )
    parser.add_argument(
        "--skip-attention-page-tensor-self-test",
        action="store_true",
        help=(
            "Do not ask patched llama.cpp to materialise actual attention-path "
            "K/V page bytes into page-sized non-host backend tensors."
        ),
    )
    parser.add_argument(
        "--attention-page-tensor-self-test-limit",
        type=int,
        default=16,
        help="Maximum attention-path backend page-tensor round-trip events to require from patched llama.cpp.",
    )
    parser.add_argument(
        "--attention-equivalence-tolerance",
        type=float,
        default=1.0e-4,
        help="Tolerance for the real llama.cpp Q/K/V page-bounded attention equivalence gate.",
    )
    parser.add_argument(
        "--attention-max-peak-page-kv-ratio",
        type=float,
        default=0.75,
        help="Require page-bounded streaming attention peak K/V residency to stay below this materialized-KV ratio.",
    )
    parser.add_argument(
        "--mlx-streaming-attention-gate",
        action="store_true",
        help="Run the MLX GPU streaming-attention gate over the deep QATQ page export.",
    )
    parser.add_argument(
        "--mlx-python",
        default=sys.executable,
        help="Python executable with mlx installed for --mlx-streaming-attention-gate.",
    )
    parser.add_argument(
        "--mlx-streaming-attention-script",
        default="scripts/mlx_live_vram_streaming_attention.py",
        help="MLX streaming-attention verifier script.",
    )
    parser.add_argument(
        "--mlx-qatq-bin",
        default="target/release/qatq",
        help="qatq binary used by the MLX verifier to build the compressed page store.",
    )
    parser.add_argument(
        "--mlx-min-layers-checked",
        type=int,
        default=1,
        help="Minimum MLX-verified attention fixture layers required when --mlx-streaming-attention-gate is enabled.",
    )
    parser.add_argument(
        "--mlx-min-heads-checked",
        type=int,
        default=1,
        help="Minimum MLX-verified attention heads required when --mlx-streaming-attention-gate is enabled.",
    )
    parser.add_argument(
        "--mlx-max-streaming-slowdown",
        type=float,
        default=0.0,
        help=(
            "Optional maximum MLX streaming/materialised attention time ratio. "
            "Use 0 to disable because first-run MLX compilation noise can dominate tiny fixtures."
        ),
    )
    parser.add_argument(
        "--require-live-paging",
        action="store_true",
        help=(
            "Require qatq-kv-bench --live-vram-live-paging-gate instead of the "
            "coarse runtime-reclaim gate. This fails until the runtime adapter "
            "proves attention-loop page-granular GPU reclaim."
        ),
    )
    parser.add_argument(
        "--require-native-page-streaming",
        action="store_true",
        help=(
            "Require proof that the runtime attention graph consumes K/V pages "
            "through a native page-streaming path rather than a ggml_concat-composed "
            "page source. This is a staged/native correctness evidence gate, "
            "not the production adapter-readiness gate; production readiness "
            "also requires scripts/llama_cpp_live_vram_adapter_audit.py "
            "--require-live-paging to pass."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-contract-probe",
        action="store_true",
        help=(
            "Run only the executable segmented K/Q/V contract probe. The probe "
            "passes only when patched llama.cpp reaches the intended native "
            "backend boundary and then stops before backend execution with the "
            "documented contract-probe message. It does not claim live VRAM reduction."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-attention",
        action="store_true",
        help=(
            "Ask patched llama.cpp to route the deep attention graph through "
            "the QATQ page-segment consumer instead of concat-composed page "
            "sources. This is a validation path for the runtime consumer; "
            "current adapter builds may use a CPU custom op."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-attention-ggml",
        action="store_true",
        help=(
            "Ask patched llama.cpp to route the deep attention graph through "
            "the accelerator-schedulable ggml segmented KQ/V consumer instead "
            "of the CPU custom-op validation path. Implies "
            "--native-page-streaming-attention."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-attention-backend-op",
        action="store_true",
        help=(
            "Ask patched llama.cpp to route the deep attention graph through "
            "the backend-scheduled QATQ segmented KQ/V op. Implies "
            "--native-page-streaming-attention and "
            "--native-page-streaming-attention-ggml."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-flatten-flash",
        action="store_true",
        help=(
            "Allow eligible native page-streaming backend-op windows to flatten "
            "bounded K/V page tables into llama.cpp's backend-scheduled Flash "
            "Attention route. This remains fail-closed under "
            "--require-native-page-streaming because the verifier still requires "
            "cold/offloaded rows to be consumed natively and rejects concat or "
            "non-native page sources."
        ),
    )
    parser.add_argument(
        "--native-page-streaming-transient-pool-max-bytes",
        type=int,
        default=256 * 1024 * 1024,
        help=(
            "Maximum graph-local transient K/V page-pool bytes allowed for a "
            "native backend-op attention window before failing closed."
        ),
    )
    parser.add_argument(
        "--aggregate-codec-gate",
        action="store_true",
        help=(
            "Allow QATQ-compressed pages that shrink raw KV bytes but do not "
            "beat zstd/lz4 on every individual page, and require QATQ to beat "
            "the best general codec in aggregate instead. The default remains "
            "the stricter per-offloaded-page codec gate."
        ),
    )
    parser.add_argument(
        "--live-page-self-test-tokens",
        type=int,
        default=0,
        help=(
            "Ask patched llama.cpp to snapshot, evict, restore, and verify one "
            "active key page in real backend KV tensor storage during the deep run."
        ),
    )
    parser.add_argument(
        "--live-physical-page-alloc-self-test-tokens",
        type=int,
        default=0,
        help=(
            "Ask patched llama.cpp to allocate one page-sized non-host backend "
            "tensor on the same backend as an active key page, round-trip real "
            "KV bytes through it, and free it during the deep run."
        ),
    )
    parser.add_argument(
        "--live-restore-slot-pressure-self-test-tokens",
        type=int,
        default=0,
        help=(
            "Ask patched llama.cpp to measure one active non-host key page and "
            "prove the runtime restore-slot byte budget rejects it before "
            "allocation during the deep run."
        ),
    )
    parser.add_argument(
        "--live-restore-slot-pressure-max-bytes",
        type=int,
        default=1,
        help="Maximum restore-slot bytes used by --live-restore-slot-pressure-self-test-tokens.",
    )
    parser.add_argument(
        "--live-persistent-page-pool-self-test-pages",
        type=int,
        default=0,
        help=(
            "Ask patched llama.cpp to allocate, byte-verify, and retain this "
            "many page-sized non-host backend K/V tensors until context teardown "
            "during the deep run."
        ),
    )
    parser.add_argument("--timeout", type=int, default=420)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument(
        "--prune-bulk-artifacts",
        action="store_true",
        help=(
            "After writing summary.md, pages.csv, tokens.csv, and JSON reports, "
            "delete bulky per-run export directories. This preserves the audit "
            "trail needed by matrix summaries while keeping local GPU stress "
            "runs from filling the disk."
        ),
    )
    args = parser.parse_args()

    require(
        not (args.require_live_paging and args.skip_event_trace),
        "--require-live-paging requires event trace emission",
    )
    require(
        not (args.require_live_paging and args.skip_runtime_reclaim_gate),
        "--skip-runtime-reclaim-gate cannot be combined with --require-live-paging",
    )
    require(
        not args.require_native_page_streaming or args.mlx_streaming_attention_gate,
        "--require-native-page-streaming requires --mlx-streaming-attention-gate as an external equivalence reference",
    )
    require(
        not (args.require_native_page_streaming and args.skip_attention_page_segments_trace),
        "--require-native-page-streaming requires the attention page-segments trace",
    )
    require(args.page_tokens >= 0, "--page-tokens must be non-negative")
    require(args.n_kv_pad_tokens >= 0, "--n-kv-pad-tokens must be non-negative")
    require(
        args.n_kv_pad_tokens == 0 or args.gpu_page_staging or args.native_page_streaming_attention_backend_op,
        "--n-kv-pad-tokens is only valid for QATQ GPU page-staging/native backend-op runs",
    )
    require(args.current_token >= 0, "--current-token must be non-negative")
    require(args.hot_window_tokens >= 0, "--hot-window-tokens must be non-negative")
    require(args.prefetch_window_tokens >= 0, "--prefetch-window-tokens must be non-negative")
    require(args.max_queued_pages >= 0, "--max-queued-pages must be non-negative")
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
        "deep token latency gates require --deep-latency-baseline",
    )
    require(args.live_page_self_test_tokens >= 0, "--live-page-self-test-tokens must be non-negative")
    require(
        args.live_physical_page_alloc_self_test_tokens >= 0,
        "--live-physical-page-alloc-self-test-tokens must be non-negative",
    )
    require(
        args.live_restore_slot_pressure_self_test_tokens >= 0,
        "--live-restore-slot-pressure-self-test-tokens must be non-negative",
    )
    require(
        args.live_restore_slot_pressure_max_bytes >= 0,
        "--live-restore-slot-pressure-max-bytes must be non-negative",
    )
    require(
        args.live_persistent_page_pool_self_test_pages >= 0,
        "--live-persistent-page-pool-self-test-pages must be non-negative",
    )
    require(
        args.attention_page_tensor_self_test_limit >= 0,
        "--attention-page-tensor-self-test-limit must be non-negative",
    )
    require(
        args.attention_persistent_page_source_max_pages > 0,
        "--attention-persistent-page-source-max-pages must be positive",
    )
    require(
        args.attention_persistent_page_source_max_source_pages > 0,
        "--attention-persistent-page-source-max-source-pages must be positive",
    )
    require(
        args.attention_persistent_page_source_max_source_bytes > 0,
        "--attention-persistent-page-source-max-source-bytes must be positive",
    )
    require(
        args.attention_persistent_page_source_max_retained_bytes > 0,
        "--attention-persistent-page-source-max-retained-bytes must be positive",
    )
    require(
        args.native_page_streaming_transient_pool_max_bytes > 0,
        "--native-page-streaming-transient-pool-max-bytes must be positive",
    )
    require(args.attention_equivalence_tolerance >= 0.0, "--attention-equivalence-tolerance must be non-negative")
    require(args.attention_fixture_max_layers > 0, "--attention-fixture-max-layers must be positive")
    require(
        0.0 < args.attention_max_peak_page_kv_ratio <= 1.0,
        "--attention-max-peak-page-kv-ratio must be in (0, 1]",
    )
    require(
        not (args.mlx_streaming_attention_gate and args.skip_attention_equivalence),
        "--mlx-streaming-attention-gate requires attention fixture capture; remove --skip-attention-equivalence",
    )
    require(args.mlx_min_layers_checked > 0, "--mlx-min-layers-checked must be positive")
    require(args.mlx_min_heads_checked > 0, "--mlx-min-heads-checked must be positive")
    require(args.mlx_max_streaming_slowdown >= 0.0, "--mlx-max-streaming-slowdown must be non-negative")
    if args.native_page_streaming_attention_backend_op:
        args.native_page_streaming_attention_ggml = True
        args.native_page_streaming_attention = True
    elif args.native_page_streaming_attention_ggml:
        args.native_page_streaming_attention = True

    root = Path.cwd()
    model = require_file(args.model, "--model")
    llama_simple = require_file(args.llama_simple, "--llama-simple")
    if args.require_native_page_streaming:
        run_native_adapter_structural_audit(root, args, llama_simple)
    model_id = args.model_id or model.name
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)
    if args.native_page_streaming_contract_probe:
        run_native_contract_structural_audit(root, args, llama_simple)
        run_native_contract_probe(args, llama_simple, model, model_id, work_dir)
        return 0
    kv_bench = ensure_kv_bench(root, Path(args.qatq_kv_bench))
    frontier_layers = parse_layer_sweep(args.sweep_kv_gpu_layers, args.mixed_kv_gpu_layers)

    started = time.time()
    short_full = RunSpec(
        name="short-full-gpu",
        export_dir=work_dir / "short-full-gpu",
        output_manifest=work_dir / "short-full-gpu" / "output-manifest.json",
        token_timings=work_dir / "short-full-gpu" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=None,
    )
    short_cpu_kv = RunSpec(
        name="short-cpu-kv",
        export_dir=work_dir / "short-cpu-kv",
        output_manifest=work_dir / "short-cpu-kv" / "output-manifest.json",
        token_timings=work_dir / "short-cpu-kv" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=None,
        no_kv_offload=True,
    )
    short_materialized_source = RunSpec(
        name="short-full-gpu-materialized-source",
        export_dir=work_dir / "short-full-gpu-materialized-source",
        output_manifest=work_dir / "short-full-gpu-materialized-source" / "output-manifest.json",
        token_timings=work_dir / "short-full-gpu-materialized-source" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=None,
        attention_materialized_source_trace=None
        if args.skip_attention_materialized_source or args.native_page_streaming_attention
        else work_dir / "short-full-gpu-materialized-source" / "materialized-source.jsonl",
    )
    short_page_composed_source = RunSpec(
        name="short-full-gpu-page-composed-source",
        export_dir=work_dir / "short-full-gpu-page-composed-source",
        output_manifest=work_dir / "short-full-gpu-page-composed-source" / "output-manifest.json",
        token_timings=work_dir / "short-full-gpu-page-composed-source" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=None,
        attention_page_composed_source_trace=None
        if args.skip_attention_page_composed_source or args.native_page_streaming_attention
        else work_dir / "short-full-gpu-page-composed-source" / "page-composed-source.jsonl",
    )
    short_persistent_page_source = RunSpec(
        name="short-full-gpu-persistent-page-source",
        export_dir=work_dir / "short-full-gpu-persistent-page-source",
        output_manifest=work_dir / "short-full-gpu-persistent-page-source" / "output-manifest.json",
        token_timings=work_dir / "short-full-gpu-persistent-page-source" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=None,
        attention_persistent_page_source_trace=None
        if args.skip_attention_persistent_page_source or args.native_page_streaming_attention
        else work_dir / "short-full-gpu-persistent-page-source" / "persistent-page-source.jsonl",
        attention_persistent_page_source_max_pages=args.attention_persistent_page_source_max_pages,
        attention_persistent_page_source_max_source_pages=args.attention_persistent_page_source_max_source_pages,
        attention_persistent_page_source_max_source_bytes=args.attention_persistent_page_source_max_source_bytes,
        attention_persistent_page_source_max_retained_bytes=args.attention_persistent_page_source_max_retained_bytes,
    )

    run_llama(args, llama_simple, model, short_full, work_dir / "short-full-gpu.log")
    materialized_source_compare_json = None
    materialized_source_summary = None
    if short_materialized_source.attention_materialized_source_trace is not None:
        run_llama(args, llama_simple, model, short_materialized_source, work_dir / "short-full-gpu-materialized-source.log")
        materialized_source_compare = work_dir / "materialized-source-output-comparison.json"
        run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(short_full.output_manifest),
                "--compare-output-candidate",
                str(short_materialized_source.output_manifest),
                "--compare-output-gate",
                "--output",
                str(materialized_source_compare),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        materialized_source_compare_json = load_json(materialized_source_compare)
        require(materialized_source_compare_json.get("passed") is True, "materialized-source output comparison gate did not pass")
        materialized_source_summary = assert_attention_materialized_source_trace(
            short_materialized_source.attention_materialized_source_trace,
            model_id=model_id,
            output=work_dir / "materialized-source-trace-summary.json",
        )
    page_composed_source_compare_json = None
    page_composed_source_summary = None
    if short_page_composed_source.attention_page_composed_source_trace is not None:
        run_llama(args, llama_simple, model, short_page_composed_source, work_dir / "short-full-gpu-page-composed-source.log")
        page_composed_source_compare = work_dir / "page-composed-source-output-comparison.json"
        run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(short_full.output_manifest),
                "--compare-output-candidate",
                str(short_page_composed_source.output_manifest),
                "--compare-output-gate",
                "--output",
                str(page_composed_source_compare),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        page_composed_source_compare_json = load_json(page_composed_source_compare)
        require(page_composed_source_compare_json.get("passed") is True, "page-composed-source output comparison gate did not pass")
        page_composed_source_summary = assert_attention_page_composed_source_trace(
            short_page_composed_source.attention_page_composed_source_trace,
            model_id=model_id,
            output=work_dir / "page-composed-source-trace-summary.json",
            require_multi_page=False,
        )
    persistent_page_source_compare_json = None
    persistent_page_source_summary = None
    if short_persistent_page_source.attention_persistent_page_source_trace is not None:
        run_llama(
            args,
            llama_simple,
            model,
            short_persistent_page_source,
            work_dir / "short-full-gpu-persistent-page-source.log",
        )
        persistent_page_source_compare = work_dir / "persistent-page-source-output-comparison.json"
        run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(short_full.output_manifest),
                "--compare-output-candidate",
                str(short_persistent_page_source.output_manifest),
                "--compare-output-gate",
                "--output",
                str(persistent_page_source_compare),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        persistent_page_source_compare_json = load_json(persistent_page_source_compare)
        require(
            persistent_page_source_compare_json.get("passed") is True,
            "persistent-page-source output comparison gate did not pass",
        )
        persistent_page_source_summary = assert_attention_persistent_page_source_trace(
            short_persistent_page_source.attention_persistent_page_source_trace,
            model_id=model_id,
            output=work_dir / "persistent-page-source-trace-summary.json",
            require_multi_page=False,
        )
    if not args.skip_cpu_kv_baseline:
        run_llama(args, llama_simple, model, short_cpu_kv, work_dir / "short-cpu-kv.log")

    frontier = []
    for layers in frontier_layers:
        frontier.append(
            run_frontier_candidate(
                args=args,
                llama_simple=llama_simple,
                model=model,
                kv_bench=kv_bench,
                short_full=short_full,
                short_cpu_kv=None if args.skip_cpu_kv_baseline else short_cpu_kv,
                work_dir=work_dir,
                layers=layers,
            )
        )
    passing = [result for result in frontier if result.passed]
    require(passing, "no mixed-KV layer frontier candidate passed output, memory, and performance gates")
    selected = sorted(passing, key=lambda result: (result.total_us, -result.saved_ratio))[0]
    short_mixed = RunSpec(
        name=f"short-mixed-kv-l{selected.layers}",
        export_dir=work_dir / f"short-mixed-kv-l{selected.layers}",
        output_manifest=work_dir / f"short-mixed-kv-l{selected.layers}" / "output-manifest.json",
        token_timings=work_dir / f"short-mixed-kv-l{selected.layers}" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=selected.layers,
    )
    output_json = selected.compare_json or {}
    cpu_output_json = None
    if not args.skip_cpu_kv_baseline:
        cpu_output_compare = work_dir / "cpu-kv-output-comparison.json"
        run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(short_full.output_manifest),
                "--compare-output-candidate",
                str(short_cpu_kv.output_manifest),
                "--compare-output-gate",
                "--output",
                str(cpu_output_compare),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        cpu_output_json = load_json(cpu_output_compare)
        require(cpu_output_json.get("passed") is True, "CPU-KV output comparison gate did not pass")
    assert_performance_gate(output_json, cpu_output_json, args.max_mixed_decode_regression)

    deep_full = None
    if args.deep_latency_baseline:
        deep_full = RunSpec(
            name="deep-full-gpu",
            export_dir=work_dir / "deep-full-gpu",
            output_manifest=work_dir / "deep-full-gpu" / "output-manifest.json",
            token_timings=work_dir / "deep-full-gpu" / "token-timings.csv",
            prompt=args.deep_prompt_seed * args.deep_repeat + args.deep_prompt_suffix,
            predict=args.deep_predict,
            kv_gpu_layers=None,
        )
        run_llama(args, llama_simple, model, deep_full, work_dir / "deep-full-gpu.log")

    deep_persistent_page_source_enabled = not args.skip_attention_persistent_page_source and not args.native_page_streaming_attention
    deep_mixed = RunSpec(
        name=f"deep-mixed-kv-l{selected.layers}",
        export_dir=work_dir / f"deep-mixed-kv-l{selected.layers}",
        output_manifest=work_dir / f"deep-mixed-kv-l{selected.layers}" / "output-manifest.json",
        token_timings=work_dir / f"deep-mixed-kv-l{selected.layers}" / "token-timings.csv",
        prompt=args.deep_prompt_seed * args.deep_repeat + args.deep_prompt_suffix,
        predict=args.deep_predict,
        kv_gpu_layers=selected.layers,
        event_trace=None
        if args.skip_event_trace
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "event-trace.json",
        attention_trace=None
        if args.skip_attention_trace or args.native_page_streaming_attention_backend_op
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "attention-trace.jsonl",
        attention_event_trace=None
        if args.skip_attention_event_trace or args.native_page_streaming_attention_backend_op
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "attention-event-trace.jsonl",
        attention_materialized_source_trace=None
        if args.native_page_streaming_attention
        or args.skip_attention_materialized_source
        or not args.skip_attention_page_composed_source
        or deep_persistent_page_source_enabled
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "materialized-source.jsonl",
        attention_page_composed_source_trace=None
        if args.native_page_streaming_attention or args.skip_attention_page_composed_source or deep_persistent_page_source_enabled
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "page-composed-source.jsonl",
        attention_persistent_page_source_trace=None
        if args.skip_attention_persistent_page_source or args.native_page_streaming_attention
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "persistent-page-source.jsonl",
        attention_page_segments_trace=None
        if args.skip_attention_page_segments_trace
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "page-segments.jsonl",
        attention_persistent_page_source_max_pages=args.attention_persistent_page_source_max_pages,
        attention_persistent_page_source_max_source_pages=args.attention_persistent_page_source_max_source_pages,
        attention_persistent_page_source_max_source_bytes=args.attention_persistent_page_source_max_source_bytes,
        attention_persistent_page_source_max_retained_bytes=args.attention_persistent_page_source_max_retained_bytes,
        attention_fixture_dir=None
        if args.skip_attention_equivalence
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "attention-fixture",
        attention_fixture_max_layers=args.attention_fixture_max_layers,
        attention_page_tensor_self_test=None
        if args.skip_attention_page_tensor_self_test
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "attention-page-tensor-self-test.jsonl",
        attention_page_tensor_self_test_limit=args.attention_page_tensor_self_test_limit,
        live_page_self_test_tokens=args.live_page_self_test_tokens,
        live_physical_page_alloc_self_test_tokens=args.live_physical_page_alloc_self_test_tokens,
        live_restore_slot_pressure_self_test_tokens=args.live_restore_slot_pressure_self_test_tokens,
        live_restore_slot_pressure_max_bytes=args.live_restore_slot_pressure_max_bytes,
        live_persistent_page_pool_trace=None
        if args.live_persistent_page_pool_self_test_pages == 0
        else work_dir / f"deep-mixed-kv-l{selected.layers}" / "persistent-page-pool.jsonl",
        live_persistent_page_pool_self_test_pages=args.live_persistent_page_pool_self_test_pages,
        native_page_streaming_attention=args.native_page_streaming_attention,
        native_page_streaming_attention_ggml=args.native_page_streaming_attention_ggml,
        native_page_streaming_attention_backend_op=args.native_page_streaming_attention_backend_op,
        native_page_streaming_transient_pool_max_bytes=args.native_page_streaming_transient_pool_max_bytes,
    )
    run_llama(args, llama_simple, model, deep_mixed, work_dir / f"deep-mixed-kv-l{selected.layers}.log")
    deep_output_compare_json = None
    if deep_full is not None:
        deep_output_compare = work_dir / "deep-output-comparison.json"
        run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(deep_full.output_manifest),
                "--compare-output-candidate",
                str(deep_mixed.output_manifest),
                "--compare-output-gate",
                "--output",
                str(deep_output_compare),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        deep_output_compare_json = load_json(deep_output_compare)
        require(deep_output_compare_json.get("passed") is True, "deep output comparison gate did not pass")
    attention_trace_summary = None
    if deep_mixed.attention_trace is not None:
        attention_trace_summary = assert_attention_trace(
            deep_mixed.attention_trace,
            model_id=model_id,
            output=work_dir / "attention-trace-summary.json",
        )
    attention_event_trace_report = None
    if deep_mixed.attention_event_trace is not None:
        attention_event_trace_output = work_dir / "attention-event-trace-report.json"
        run(
            [
                str(kv_bench),
                "--live-vram-event-trace-only",
                "--live-vram-event-trace",
                str(deep_mixed.attention_event_trace),
                "--live-vram-event-trace-gate",
                "--output",
                str(attention_event_trace_output),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        attention_event_trace_report = load_json(attention_event_trace_output)
        require(attention_event_trace_report.get("passed") is True, "attention event trace gate did not pass")
    deep_materialized_source_summary = None
    if deep_mixed.attention_materialized_source_trace is not None:
        deep_materialized_source_summary = assert_attention_materialized_source_trace(
            deep_mixed.attention_materialized_source_trace,
            model_id=model_id,
            output=work_dir / "deep-materialized-source-trace-summary.json",
        )
    deep_page_composed_source_summary = None
    if deep_mixed.attention_page_composed_source_trace is not None:
        deep_page_composed_source_summary = assert_attention_page_composed_source_trace(
            deep_mixed.attention_page_composed_source_trace,
            model_id=model_id,
            output=work_dir / "deep-page-composed-source-trace-summary.json",
            require_multi_page=True,
        )
    deep_persistent_page_source_summary = None
    if deep_mixed.attention_persistent_page_source_trace is not None:
        deep_persistent_page_source_summary = assert_attention_persistent_page_source_trace(
            deep_mixed.attention_persistent_page_source_trace,
            model_id=model_id,
            output=work_dir / "deep-persistent-page-source-trace-summary.json",
            require_multi_page=True,
        )
    deep_page_segments_summary = None
    if deep_mixed.attention_page_segments_trace is not None:
        deep_page_segments_summary = assert_attention_page_segments_trace(
            deep_mixed.attention_page_segments_trace,
            model_id=model_id,
            output=work_dir / "deep-page-segments-trace-summary.json",
            require_multi_page=True,
            expect_native_page_streaming=args.native_page_streaming_attention,
            expect_attention_consumed=args.native_page_streaming_attention,
        )
    attention_equivalence = None
    if deep_mixed.attention_fixture_dir is not None:
        attention_equivalence_output = work_dir / "attention-equivalence.json"
        run(
            [
                sys.executable,
                "scripts/llama_cpp_attention_fixture_gate.py",
                "--export-dir",
                str(deep_mixed.export_dir),
                "--attention-fixture-dir",
                str(deep_mixed.attention_fixture_dir),
                "--qatq-kv-bench",
                str(kv_bench),
                "--tolerance",
                str(args.attention_equivalence_tolerance),
                "--max-peak-page-kv-ratio",
                str(args.attention_max_peak_page_kv_ratio),
                "--output",
                str(attention_equivalence_output),
            ],
            cwd=root,
            timeout=args.timeout,
        )
        attention_equivalence = load_json(attention_equivalence_output)
        require(attention_equivalence.get("passed") is True, "attention equivalence gate did not pass")
    mlx_streaming_attention = None
    if args.mlx_streaming_attention_gate:
        require(deep_mixed.attention_fixture_dir is not None, "MLX streaming attention gate requires an attention fixture")
        mlx_streaming_attention = run_mlx_streaming_attention_gate(
            args=args,
            root=root,
            export_dir=deep_mixed.export_dir,
            fixture_dir=deep_mixed.attention_fixture_dir,
            output=work_dir / "mlx-streaming-attention.json",
            store_dir=work_dir / "mlx-qatq-page-store",
        )
    attention_page_tensor_self_test = None
    if deep_mixed.attention_page_tensor_self_test is not None:
        attention_page_tensor_self_test = assert_attention_page_tensor_self_test(
            deep_mixed.attention_page_tensor_self_test,
            output=work_dir / "attention-page-tensor-self-test-summary.json",
        )
    persistent_page_pool = None
    if deep_mixed.live_persistent_page_pool_trace is not None:
        persistent_page_pool = assert_persistent_page_pool_trace(
            deep_mixed.live_persistent_page_pool_trace,
            expected_pages=args.live_persistent_page_pool_self_test_pages,
            output=work_dir / "persistent-page-pool-summary.json",
        )

    evidence_json = work_dir / (
        "live-paging-evidence.json"
        if args.require_live_paging
        else "native-correctness-evidence.json"
        if args.skip_runtime_reclaim_gate
        else "runtime-reclaim-evidence.json"
    )
    require_page_seals = not args.skip_runtime_reclaim_gate
    page_seal_key_hex = secrets.token_hex(32) if require_page_seals else None
    evidence_command = [
        str(kv_bench),
        "--live-vram-export-dir",
        str(deep_mixed.export_dir),
        "--live-vram-runtime-commit",
        args.runtime_commit,
        "--live-vram-adapter-version",
        args.adapter_version,
        "--live-vram-model-id",
        model_id,
        "--live-vram-current-token",
        str(args.current_token),
        "--live-vram-hot-window-tokens",
        str(args.hot_window_tokens),
        "--live-vram-prefetch-window-tokens",
        str(args.prefetch_window_tokens),
        "--live-vram-next-required",
        args.next_required,
        "--live-vram-restore-bytes-per-token",
        str(args.restore_bytes_per_token),
    ]
    if args.max_queued_pages > 0:
        evidence_command.extend(["--live-vram-max-queued-pages", str(args.max_queued_pages)])
    if deep_mixed.event_trace is not None:
        evidence_command.extend(["--live-vram-event-trace", str(deep_mixed.event_trace)])
    if args.require_live_paging:
        evidence_command.extend(
            [
                "--live-vram-live-paging-gate",
                "--live-vram-page-seal-key-hex",
                page_seal_key_hex,
                "--live-vram-require-page-seals",
            ]
        )
    elif not args.skip_runtime_reclaim_gate:
        evidence_command.extend(
            [
                "--live-vram-runtime-reclaim-gate",
                "--live-vram-page-seal-key-hex",
                page_seal_key_hex,
                "--live-vram-require-page-seals",
            ]
        )
    if args.aggregate_codec_gate:
        evidence_command.append("--live-vram-aggregate-codec-gate")
    evidence_command.extend(
        [
            "--live-vram-min-gpu-saved-ratio",
            str(args.min_gpu_saved_ratio),
            "--output",
            str(evidence_json),
        ]
    )
    run(evidence_command, cwd=root, timeout=args.timeout)
    evidence = load_json(evidence_json)
    if args.require_live_paging:
        assert_live_paging_evidence(evidence, args.min_gpu_saved_ratio, args.aggregate_codec_gate)
    elif not args.skip_runtime_reclaim_gate:
        assert_runtime_reclaim_evidence(evidence, args.min_gpu_saved_ratio, args.aggregate_codec_gate)
    if deep_mixed.event_trace is not None:
        assert_event_trace_evidence(evidence)
    native_page_streaming = build_native_page_streaming_status(
        page_composed_source_summary=page_composed_source_summary,
        deep_page_composed_source_summary=deep_page_composed_source_summary,
        persistent_page_source_summary=persistent_page_source_summary,
        deep_persistent_page_source_summary=deep_persistent_page_source_summary,
        deep_page_segments_summary=deep_page_segments_summary,
        attention_equivalence=attention_equivalence,
        mlx_streaming_attention=mlx_streaming_attention,
    )
    (work_dir / "native-page-streaming-status.json").write_text(
        json.dumps(native_page_streaming, indent=2) + "\n",
        encoding="utf-8",
    )
    if args.require_native_page_streaming:
        assert_native_page_streaming_evidence(native_page_streaming)
    pages_csv = work_dir / "pages.csv"
    tokens_csv = work_dir / "tokens.csv"
    write_pages_csv(evidence, pages_csv)
    write_tokens_csv(
        output_json=output_json,
        cpu_output_json=cpu_output_json,
        evidence=evidence,
        selected=selected,
        timing_paths={
            "short-full-gpu": short_full.token_timings,
            "short-mixed-kv": short_mixed.token_timings,
            "short-cpu-kv": short_cpu_kv.token_timings if not args.skip_cpu_kv_baseline else None,
            "deep-full-gpu": deep_full.token_timings if deep_full is not None else None,
            "deep-mixed-kv": deep_mixed.token_timings,
        },
        attention_trace_summary=attention_trace_summary,
        attention_event_trace_report=attention_event_trace_report,
        materialized_source_summary=materialized_source_summary,
        materialized_source_compare_json=materialized_source_compare_json,
        deep_materialized_source_summary=deep_materialized_source_summary,
        page_composed_source_summary=page_composed_source_summary,
        page_composed_source_compare_json=page_composed_source_compare_json,
        deep_page_composed_source_summary=deep_page_composed_source_summary,
        persistent_page_source_summary=persistent_page_source_summary,
        persistent_page_source_compare_json=persistent_page_source_compare_json,
        deep_persistent_page_source_summary=deep_persistent_page_source_summary,
        deep_page_segments_summary=deep_page_segments_summary,
        attention_equivalence=attention_equivalence,
        mlx_streaming_attention=mlx_streaming_attention,
        native_page_streaming=native_page_streaming,
        deep_output_compare_json=deep_output_compare_json,
        attention_page_tensor_self_test=attention_page_tensor_self_test,
        persistent_page_pool=persistent_page_pool,
        output=tokens_csv,
    )
    deep_latency = None
    if args.deep_latency_baseline:
        deep_latency = parse_token_latency_stats(
            tokens_csv,
            baseline_run="deep-full-gpu",
            candidate_run="deep-mixed-kv",
        )
        assert_deep_latency_gate(
            deep_latency,
            min_samples=args.min_deep_token_latency_samples,
            max_p95_regression=args.max_deep_mixed_token_p95_regression_ratio,
            max_p99_regression=args.max_deep_mixed_token_p99_regression_ratio,
            output=work_dir / "deep-latency-gate.json",
        )

    summary = build_summary(
        args,
        model,
        work_dir,
        output_json,
        cpu_output_json,
        evidence,
        time.time() - started,
        frontier,
        selected,
        attention_trace_summary,
        attention_event_trace_report,
        materialized_source_summary,
        materialized_source_compare_json,
        deep_materialized_source_summary,
        page_composed_source_summary,
        page_composed_source_compare_json,
        deep_page_composed_source_summary,
        persistent_page_source_summary,
        persistent_page_source_compare_json,
        deep_persistent_page_source_summary,
        deep_page_segments_summary,
        attention_equivalence,
        mlx_streaming_attention,
        native_page_streaming,
        attention_page_tensor_self_test,
        persistent_page_pool,
        deep_latency,
    )
    summary_path = work_dir / "summary.md"
    summary_path.write_text(summary, encoding="utf-8")
    if args.prune_bulk_artifacts:
        prune_bulk_artifacts(work_dir)
    print(summary)
    return 0


def prune_bulk_artifacts(work_dir: Path) -> None:
    """Remove bulky generated export trees after aggregate evidence is written."""
    pruned: list[str] = []
    for child in sorted(work_dir.iterdir()):
        if not child.is_dir():
            continue
        if child.name == "mlx-qatq-page-store" or child.name.startswith(("short-", "deep-")):
            shutil.rmtree(child)
            pruned.append(child.name)
    if pruned:
        (work_dir / "pruned-artifacts.txt").write_text(
            "\n".join(pruned) + "\n",
            encoding="utf-8",
        )


def run_native_adapter_structural_audit(root: Path, args, llama_simple: Path) -> None:
    source = Path(args.llama_cpp_source) if args.llama_cpp_source else infer_llama_cpp_source(llama_simple)
    require(source is not None, "--require-native-page-streaming requires --llama-cpp-source when --llama-simple is not under <checkout>/build/bin")
    require(source.exists(), f"llama.cpp source path does not exist: {source}")
    audit = root / "scripts" / "llama_cpp_live_vram_adapter_audit.py"
    require(audit.exists(), f"missing adapter audit script: {audit}")
    command = [
        sys.executable,
        "-B",
        str(audit),
        "--llama-cpp",
        str(source),
        "--require-live-paging",
        "--require-runtime-security",
    ]
    completed = subprocess.run(command, cwd=root, text=True, capture_output=True)
    if completed.returncode != 0:
        detail = completed.stdout.strip() or completed.stderr.strip() or f"exit {completed.returncode}"
        raise SystemExit("native page-streaming structural adapter audit failed:\n" + detail)


def run_native_contract_structural_audit(root: Path, args, llama_simple: Path) -> None:
    source = Path(args.llama_cpp_source) if args.llama_cpp_source else infer_llama_cpp_source(llama_simple)
    require(
        source is not None,
        "--native-page-streaming-contract-probe requires --llama-cpp-source when --llama-simple is not under <checkout>/build/bin",
    )
    require(source.exists(), f"llama.cpp source path does not exist: {source}")
    audit = root / "scripts" / "llama_cpp_live_vram_adapter_audit.py"
    require(audit.exists(), f"missing adapter audit script: {audit}")
    completed = subprocess.run(
        [sys.executable, "-B", str(audit), "--llama-cpp", str(source)],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        detail = completed.stdout.strip() or completed.stderr.strip() or f"exit {completed.returncode}"
        raise SystemExit("native contract structural adapter audit failed:\n" + detail)
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"native contract structural adapter audit did not emit JSON: {exc}") from exc
    by_name = {check.get("name"): check.get("passed") is True for check in report.get("checks", [])}
    require(
        by_name.get("export.runner_flags") is True,
        "native contract structural adapter audit did not find the llama-simple contract flag",
    )
    require(
        by_name.get("live.native_segmented_kqv_contract") is True,
        "native contract structural adapter audit did not find the executable segmented K/Q/V contract hook",
    )


def run_native_contract_probe(args, llama_simple: Path, model: Path, model_id: str, work_dir: Path) -> None:
    expected = "QATQ segmented KQV contract probe completed before backend execution"
    probe_dir = work_dir / "native-page-streaming-contract-probe"
    spec = RunSpec(
        name="native-page-streaming-contract-probe",
        export_dir=probe_dir,
        output_manifest=probe_dir / "output-manifest.json",
        token_timings=probe_dir / "token-timings.csv",
        prompt=args.short_prompt,
        predict=1,
        kv_gpu_layers=None,
        native_page_streaming_contract=True,
    )
    log_path = work_dir / "native-page-streaming-contract-probe.log"
    completed = run_llama(
        args,
        llama_simple,
        model,
        spec,
        log_path,
        expect_failure_substring=expected,
    )
    result = {
        "format": "qatq-llama-cpp-native-page-streaming-contract-probe-v1",
        "passed": True,
        "model_id": model_id,
        "probe": spec.name,
        "expected_probe_stop": expected,
        "returncode": completed.returncode,
        "log": str(log_path),
        "native_live_vram_claimed": False,
    }
    output = work_dir / "native-page-streaming-contract-probe.json"
    output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))


def infer_llama_cpp_source(llama_simple: Path) -> Path | None:
    parts = llama_simple.parts
    if len(parts) < 3:
        return None
    if parts[-3:] != ("build", "bin", "llama-simple"):
        return None
    return llama_simple.parents[2]


def run_llama(
    args,
    llama_simple: Path,
    model: Path,
    spec: RunSpec,
    log_path: Path,
    *,
    expect_failure_substring: str | None = None,
) -> subprocess.CompletedProcess[str]:
    spec.export_dir.mkdir(parents=True, exist_ok=True)
    command = [
        str(llama_simple),
        "-m",
        str(model),
        "-ngl",
        str(args.gpu_layers),
        "-n",
        str(spec.predict),
        "--memory-breakdown",
        "--cache-type-k",
        args.cache_type_k,
        "--cache-type-v",
        args.cache_type_v,
        "--qatq-kv-export-dir",
        str(spec.export_dir),
        "--qatq-output-manifest",
        str(spec.output_manifest),
        "--qatq-token-timings",
        str(spec.token_timings),
    ]
    if args.page_tokens > 0:
        command.extend(["--qatq-page-tokens", str(args.page_tokens)])
    if args.gpu_page_staging:
        command.append("--qatq-gpu-page-staging")
    if spec.kv_gpu_layers is not None:
        command.extend(["--qatq-kv-gpu-layers", str(spec.kv_gpu_layers)])
    if spec.no_kv_offload:
        command.append("--no-kv-offload")
    if spec.event_trace is not None:
        command.extend(["--qatq-event-trace", str(spec.event_trace), "--qatq-model-id", args.model_id or model.name])
        command.extend(["--qatq-trace-current-token", str(args.current_token)])
        command.extend(["--qatq-trace-hot-window-tokens", str(args.hot_window_tokens)])
        command.extend(["--qatq-trace-prefetch-window-tokens", str(args.prefetch_window_tokens)])
        command.extend(["--qatq-trace-next-required", args.next_required])
        if args.max_queued_pages > 0:
            command.extend(["--qatq-trace-max-queued-pages", str(args.max_queued_pages)])
    if spec.attention_trace is not None:
        command.extend(["--qatq-attention-trace", str(spec.attention_trace), "--qatq-model-id", args.model_id or model.name])
    if spec.attention_event_trace is not None:
        command.extend(["--qatq-attention-event-trace", str(spec.attention_event_trace), "--qatq-model-id", args.model_id or model.name])
    if spec.attention_materialized_source_trace is not None:
        command.extend(
            [
                "--qatq-attention-materialized-source-trace",
                str(spec.attention_materialized_source_trace),
                "--qatq-model-id",
                args.model_id or model.name,
            ]
        )
    if spec.attention_page_composed_source_trace is not None:
        command.extend(
            [
                "--qatq-attention-page-composed-source-trace",
                str(spec.attention_page_composed_source_trace),
                "--qatq-model-id",
                args.model_id or model.name,
            ]
        )
    if spec.attention_persistent_page_source_trace is not None:
        command.extend(
            [
                "--qatq-attention-persistent-page-source-trace",
                str(spec.attention_persistent_page_source_trace),
                "--qatq-attention-persistent-page-source-max-pages",
                str(spec.attention_persistent_page_source_max_pages),
                "--qatq-attention-persistent-page-source-max-source-pages",
                str(spec.attention_persistent_page_source_max_source_pages),
                "--qatq-attention-persistent-page-source-max-source-bytes",
                str(spec.attention_persistent_page_source_max_source_bytes),
                "--qatq-attention-persistent-page-source-max-retained-bytes",
                str(spec.attention_persistent_page_source_max_retained_bytes),
                "--qatq-model-id",
                args.model_id or model.name,
            ]
        )
    if spec.attention_page_segments_trace is not None:
        command.extend(
            [
                "--qatq-attention-page-segments-trace",
                str(spec.attention_page_segments_trace),
                "--qatq-model-id",
                args.model_id or model.name,
            ]
        )
    if spec.native_page_streaming_contract:
        command.append("--qatq-native-page-streaming-contract")
    if spec.native_page_streaming_attention_backend_op:
        command.append("--qatq-native-page-streaming-attention-backend-op")
    elif spec.native_page_streaming_attention_ggml:
        command.append("--qatq-native-page-streaming-attention-ggml")
    elif spec.native_page_streaming_attention:
        command.append("--qatq-native-page-streaming-attention")
    if args.native_page_streaming_flatten_flash:
        command.append("--qatq-native-page-streaming-flatten-flash")
    if spec.native_page_streaming_attention or spec.native_page_streaming_attention_backend_op:
        command.extend(
            [
                "--qatq-native-page-streaming-transient-pool-max-bytes",
                str(spec.native_page_streaming_transient_pool_max_bytes),
            ]
        )
    if spec.attention_fixture_dir is not None:
        command.extend(
            [
                "--qatq-attention-fixture-dir",
                str(spec.attention_fixture_dir),
                "--qatq-attention-fixture-max-layers",
                str(spec.attention_fixture_max_layers),
            ]
        )
    if spec.attention_page_tensor_self_test is not None:
        command.extend(
            [
                "--qatq-attention-page-tensor-self-test",
                str(spec.attention_page_tensor_self_test),
                "--qatq-attention-page-tensor-self-test-limit",
                str(spec.attention_page_tensor_self_test_limit),
            ]
        )
    if spec.live_page_self_test_tokens > 0:
        command.extend(["--qatq-live-page-self-test", str(spec.live_page_self_test_tokens)])
    if spec.live_physical_page_alloc_self_test_tokens > 0:
        command.extend(
            [
                "--qatq-live-physical-page-alloc-self-test",
                str(spec.live_physical_page_alloc_self_test_tokens),
            ]
        )
    if spec.live_restore_slot_pressure_self_test_tokens > 0:
        command.extend(
            [
                "--qatq-live-restore-slot-pressure-self-test",
                str(spec.live_restore_slot_pressure_self_test_tokens),
                "--qatq-live-restore-slot-pressure-max-bytes",
                str(spec.live_restore_slot_pressure_max_bytes),
            ]
        )
    if spec.live_persistent_page_pool_self_test_pages > 0:
        require(
            spec.live_persistent_page_pool_trace is not None,
            f"{spec.name} requested persistent page pool self-test without a trace path",
        )
        command.extend(
            [
                "--qatq-live-persistent-page-pool-trace",
                str(spec.live_persistent_page_pool_trace),
                "--qatq-live-persistent-page-pool-self-test",
                str(spec.live_persistent_page_pool_self_test_pages),
            ]
        )
    command.append(spec.prompt)
    env = None
    if (
        args.n_kv_pad_tokens > 0
        or args.native_page_streaming_flatten_flash
        or args.attention_page_segments_live_offloaded_only
    ):
        env = os.environ.copy()
    if args.n_kv_pad_tokens > 0:
        env["LLAMA_QATQ_N_KV_PAD_TOKENS"] = str(args.n_kv_pad_tokens)
    if args.native_page_streaming_flatten_flash:
        env["LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH"] = "1"
    if args.attention_page_segments_live_offloaded_only:
        env["LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_LIVE_OFFLOADED_ONLY"] = "1"
    work_dir = log_path.parent
    stage_started = time.time()
    stage_context = {
        "command": shell_join(command),
        "log": str(log_path),
        "timeout_seconds": args.timeout,
        "expected_failure_substring": expect_failure_substring,
        "n_kv_pad_tokens": args.n_kv_pad_tokens,
        "native_page_streaming_flatten_flash": args.native_page_streaming_flatten_flash,
        "attention_page_segments_live_offloaded_only": args.attention_page_segments_live_offloaded_only,
        "kv_gpu_layers": spec.kv_gpu_layers,
        "predict": spec.predict,
    }
    write_stage_status(
        work_dir,
        spec.name,
        "running",
        **stage_context,
        artifacts=run_spec_artifacts(spec, log_path),
    )
    try:
        completed = run(
            command,
            cwd=Path.cwd(),
            timeout=args.timeout,
            capture=True,
            allow_failure=True,
            env=env,
        )
    except subprocess.TimeoutExpired as exc:
        log_text = timeout_output_to_text(exc.stdout) + timeout_output_to_text(exc.stderr)
        if log_text:
            log_path.write_text(log_text, encoding="utf-8")
        write_stage_status(
            work_dir,
            spec.name,
            "timeout",
            **stage_context,
            elapsed_seconds=time.time() - stage_started,
            artifacts=run_spec_artifacts(spec, log_path),
        )
        raise SystemExit(f"{spec.name} timed out after {args.timeout}s; log: {log_path}") from exc

    log_path.write_text(completed.stdout + completed.stderr, encoding="utf-8")
    log_text = completed.stdout + completed.stderr
    try:
        if expect_failure_substring is not None:
            require(
                completed.returncode != 0,
                f"{spec.name} unexpectedly succeeded; expected fail-closed native contract boundary",
            )
            require(
                expect_failure_substring in log_text,
                f"{spec.name} did not fail with expected message: {expect_failure_substring}",
            )
            write_stage_status(
                work_dir,
                spec.name,
                "pass",
                **stage_context,
                elapsed_seconds=time.time() - stage_started,
                returncode=completed.returncode,
                expected_failure_observed=True,
                artifacts=run_spec_artifacts(spec, log_path),
            )
            return completed
        require(
            completed.returncode == 0,
            f"{spec.name} failed with exit code {completed.returncode}; log: {log_path}",
        )
        require("MTL0" in log_text or "Metal" in log_text, f"{spec.name} did not appear to use Metal")
        require(spec.output_manifest.exists(), f"{spec.name} did not write output manifest")
        require(spec.token_timings.exists(), f"{spec.name} did not write token timings")
        require((spec.export_dir / "manifest.json").exists(), f"{spec.name} did not write KV manifest")
        if spec.event_trace is not None:
            require(spec.event_trace.exists(), f"{spec.name} did not write QATQ event trace")
        if spec.attention_trace is not None:
            require(spec.attention_trace.exists(), f"{spec.name} did not write QATQ attention trace")
        if spec.attention_event_trace is not None:
            require(spec.attention_event_trace.exists(), f"{spec.name} did not write QATQ attention event trace")
        if spec.attention_materialized_source_trace is not None:
            require(spec.attention_materialized_source_trace.exists(), f"{spec.name} did not write QATQ materialized source trace")
        if spec.attention_page_composed_source_trace is not None:
            require(spec.attention_page_composed_source_trace.exists(), f"{spec.name} did not write QATQ page-composed source trace")
        if spec.attention_persistent_page_source_trace is not None:
            require(
                spec.attention_persistent_page_source_trace.exists(),
                f"{spec.name} did not write QATQ persistent page source trace",
            )
        if spec.attention_page_segments_trace is not None:
            require(spec.attention_page_segments_trace.exists(), f"{spec.name} did not write QATQ page segment trace")
        if spec.attention_fixture_dir is not None:
            require(
                (spec.attention_fixture_dir / "attention-fixture.json").exists(),
                f"{spec.name} did not write QATQ attention fixture",
            )
        if spec.attention_page_tensor_self_test is not None:
            require(
                spec.attention_page_tensor_self_test.exists(),
                f"{spec.name} did not write QATQ attention page tensor self-test",
            )
        if spec.live_page_self_test_tokens > 0:
            require(
                "QATQ live page self-test passed" in log_text,
                f"{spec.name} did not pass the QATQ live page self-test",
            )
        if spec.live_physical_page_alloc_self_test_tokens > 0:
            require(
                "QATQ live physical page alloc self-test passed" in log_text,
                f"{spec.name} did not pass the QATQ live physical page alloc self-test",
            )
        if spec.live_restore_slot_pressure_self_test_tokens > 0:
            require(
                "QATQ live restore slot pressure self-test rejected" in log_text,
                f"{spec.name} did not pass the QATQ live restore slot pressure self-test",
            )
        if spec.live_persistent_page_pool_self_test_pages > 0:
            require(
                "QATQ live persistent page pool self-test retained" in log_text,
                f"{spec.name} did not pass the QATQ live persistent page pool self-test",
            )
            require(
                spec.live_persistent_page_pool_trace is not None and spec.live_persistent_page_pool_trace.exists(),
                f"{spec.name} did not write QATQ live persistent page pool trace",
            )
    except SystemExit as exc:
        write_stage_status(
            work_dir,
            spec.name,
            "fail",
            **stage_context,
            elapsed_seconds=time.time() - stage_started,
            returncode=completed.returncode,
            failure=str(exc),
            artifacts=run_spec_artifacts(spec, log_path),
        )
        raise

    write_stage_status(
        work_dir,
        spec.name,
        "pass",
        **stage_context,
        elapsed_seconds=time.time() - stage_started,
        returncode=completed.returncode,
        artifacts=run_spec_artifacts(spec, log_path),
    )
    return completed


def run_frontier_candidate(
    *,
    args,
    llama_simple: Path,
    model: Path,
    kv_bench: Path,
    short_full: RunSpec,
    short_cpu_kv: RunSpec | None,
    work_dir: Path,
    layers: int,
) -> FrontierResult:
    spec = RunSpec(
        name=f"short-mixed-kv-l{layers}",
        export_dir=work_dir / f"short-mixed-kv-l{layers}",
        output_manifest=work_dir / f"short-mixed-kv-l{layers}" / "output-manifest.json",
        token_timings=work_dir / f"short-mixed-kv-l{layers}" / "token-timings.csv",
        prompt=args.short_prompt,
        predict=args.short_predict,
        kv_gpu_layers=layers,
    )
    try:
        run_llama(args, llama_simple, model, spec, work_dir / f"short-mixed-kv-l{layers}.log")
        compare_path = work_dir / f"output-comparison-l{layers}.json"
        completed = run(
            [
                str(kv_bench),
                "--compare-output-baseline",
                str(short_full.output_manifest),
                "--compare-output-candidate",
                str(spec.output_manifest),
                "--compare-output-gate",
                "--output",
                str(compare_path),
            ],
            cwd=Path.cwd(),
            timeout=args.timeout,
            allow_failure=True,
        )
        compare_json = load_json(compare_path) if compare_path.exists() else None
        if completed.returncode != 0:
            detail = (completed.stderr or completed.stdout or "").strip()
            return failed_frontier(layers, f"output comparison failed: {detail}", compare_json)
        require(compare_json is not None, f"frontier candidate {layers} did not write comparison JSON")
        output_passed = compare_json.get("passed") is True
        baseline_us = int(compare_json["baseline"]["total_us"])
        mixed_us = int(compare_json["candidate"]["total_us"])
        cpu_us = None
        faster_than_cpu = None
        if short_cpu_kv is not None:
            cpu_manifest = load_json(short_cpu_kv.output_manifest)
            cpu_us = int(cpu_manifest["total_us"])
            faster_than_cpu = mixed_us < cpu_us
        manifest = load_json(spec.export_dir / "manifest.json")
        gpu_context = int(manifest.get("gpu_context_bytes", 0))
        total_context = int(manifest.get("total_context_bytes", 0))
        saved_ratio = ((total_context - gpu_context) / total_context) if total_context > 0 else 0.0
        regression = (mixed_us / baseline_us) - 1.0 if baseline_us > 0 else float("inf")
        failure = []
        if not output_passed:
            failure.append("output drift")
        if regression > args.max_mixed_decode_regression:
            failure.append(f"decode regression {regression:.6f} exceeds {args.max_mixed_decode_regression:.6f}")
        if saved_ratio < args.min_gpu_saved_ratio:
            failure.append(f"GPU saved ratio {saved_ratio:.6f} below {args.min_gpu_saved_ratio:.6f}")
        if faster_than_cpu is False:
            failure.append(f"mixed-KV decode time {mixed_us} us is not faster than all-CPU-KV baseline {cpu_us} us")
        return FrontierResult(
            layers=layers,
            output_passed=output_passed,
            total_us=mixed_us,
            full_gpu_us=baseline_us,
            cpu_kv_us=cpu_us,
            decode_regression=regression,
            faster_than_cpu_kv=faster_than_cpu,
            gpu_context_bytes=gpu_context,
            total_context_bytes=total_context,
            saved_ratio=saved_ratio,
            gpu_resident_tensors=int(manifest.get("gpu_resident_tensors", 0)),
            total_tensors=int(manifest.get("total_tensors", 0)),
            passed=not failure,
            failure="; ".join(failure),
            compare_json=compare_json,
        )
    except SystemExit as exc:
        return failed_frontier(layers, str(exc), None)
    except Exception as exc:
        return failed_frontier(layers, str(exc), None)


def failed_frontier(layers: int, failure: str, compare_json: dict | None) -> FrontierResult:
    return FrontierResult(
        layers=layers,
        output_passed=False,
        total_us=0,
        full_gpu_us=0,
        cpu_kv_us=None,
        decode_regression=float("inf"),
        faster_than_cpu_kv=None,
        gpu_context_bytes=0,
        total_context_bytes=0,
        saved_ratio=0.0,
        gpu_resident_tensors=0,
        total_tensors=0,
        passed=False,
        failure=failure,
        compare_json=compare_json,
    )


def assert_runtime_reclaim_evidence(evidence: dict, min_gpu_saved_ratio: float, aggregate_codec_gate: bool) -> None:
    assert_common_live_vram_evidence(evidence, aggregate_codec_gate)
    residency = evidence.get("residency_estimate") or {}
    require(residency.get("allocation_granularity") == "whole-tensor", "expected whole-tensor allocator evidence")
    assert_reclaim_fields(evidence, min_gpu_saved_ratio)
    assert_metadata_seals(evidence)


def assert_live_paging_evidence(evidence: dict, min_gpu_saved_ratio: float, aggregate_codec_gate: bool) -> None:
    assert_common_live_vram_evidence(evidence, aggregate_codec_gate)
    residency = evidence.get("residency_estimate") or {}
    require(residency.get("allocation_granularity") == "per-page", "live-paging evidence must prove per-page allocator reclaim")
    assert_reclaim_fields(evidence, min_gpu_saved_ratio)
    assert_event_trace_evidence(evidence)
    assert_metadata_seals(evidence)


def assert_metadata_seals(evidence: dict) -> None:
    offloaded_pages = int(evidence.get("offloaded_pages", 0))
    require(int(evidence.get("sealed_pages", 0)) == offloaded_pages, "not every offloaded page has a metadata seal")
    pages = evidence.get("pages")
    require(isinstance(pages, list), "live-VRAM evidence is missing page rows")
    for page in pages:
        if page.get("schedule") != "offload":
            require(page.get("metadata_seal") is None, "resident page unexpectedly has a metadata seal")
            continue
        seal = page.get("metadata_seal")
        require(isinstance(seal, dict), "offloaded page is missing metadata seal")
        require(seal.get("version") == 1, "offloaded page has unsupported metadata seal version")
        tag = seal.get("tag")
        require(isinstance(tag, str) and len(tag) == 64, "offloaded page has malformed metadata seal tag")
        require(all(ch in "0123456789abcdef" for ch in tag), "metadata seal tag must be lowercase hex")


def assert_common_live_vram_evidence(evidence: dict, aggregate_codec_gate: bool) -> None:
    total_pages = int(evidence.get("total_pages", 0))
    require(total_pages > 0, "runtime reclaim evidence has no pages")
    offloaded_pages = int(evidence.get("offloaded_pages", 0))
    compressed_pages = int(evidence.get("compressed_pages", 0))
    pass_through_pages = int(evidence.get("pass_through_pages", 0))
    pages = evidence.get("pages")
    require(isinstance(pages, list), "runtime reclaim evidence is missing page rows")
    offloaded_page_rows = [page for page in pages if page.get("schedule") == "offload"]
    require(len(offloaded_page_rows) == offloaded_pages, "offloaded page count does not match page rows")
    offloaded_beating_zstd = sum(1 for page in offloaded_page_rows if int(page.get("qatq_candidate_bytes", 0)) < int(page.get("zstd_bytes", 0)))
    offloaded_beating_lz4 = sum(1 for page in offloaded_page_rows if int(page.get("qatq_candidate_bytes", 0)) < int(page.get("lz4_bytes", 0)))
    offloaded_beating_best = sum(
        1
        for page in offloaded_page_rows
        if int(page.get("qatq_candidate_bytes", 0))
        < min(int(page.get("zstd_bytes", 0)), int(page.get("lz4_bytes", 0)))
    )
    require(offloaded_pages > 0, "runtime reclaim evidence did not offload any pages")
    require(evidence.get("verified_restores") == total_pages, "not every page restored exactly")
    require(compressed_pages == offloaded_pages, "not every offloaded page used compressed QATQ storage")
    require(pass_through_pages == 0, "pass-through pages are present")
    if aggregate_codec_gate:
        require(int(evidence.get("qatq_candidate_bytes", 0)) < int(evidence.get("zstd_bytes", 0)), "QATQ did not beat zstd bytes in aggregate")
        require(int(evidence.get("qatq_candidate_bytes", 0)) < int(evidence.get("lz4_bytes", 0)), "QATQ did not beat lz4 bytes in aggregate")
    else:
        require(offloaded_beating_zstd == offloaded_pages, "QATQ did not beat zstd on every offloaded page")
        require(offloaded_beating_lz4 == offloaded_pages, "QATQ did not beat lz4 on every offloaded page")
        require(offloaded_beating_best == offloaded_pages, "QATQ did not beat the best general codec on every offloaded page")


def assert_reclaim_fields(evidence: dict, min_gpu_saved_ratio: float) -> None:
    residency = evidence.get("residency_estimate") or {}
    before = int(residency.get("gpu_context_bytes_before", 0))
    after = int(residency.get("gpu_context_bytes_after", 0))
    reclaimed = int(residency.get("reclaimable_gpu_bytes", 0))
    require(before > 0 and after >= 0 and reclaimed > 0, "GPU reclaim fields are missing or zero")
    require(after <= before, "GPU context bytes after reclaim exceed the before value")
    require(after < before, "GPU context bytes did not decrease")
    ratio = reclaimed / before
    require(ratio >= min_gpu_saved_ratio, f"GPU saved ratio {ratio:.6f} below required {min_gpu_saved_ratio:.6f}")
    restore = evidence.get("restore_deadline_report") or {}
    require(restore.get("prefetch_misses") == 0, "restore deadline report contains prefetch misses")
    require(int(evidence.get("qatq_candidate_bytes", 0)) < int(evidence.get("zstd_bytes", 0)), "QATQ did not beat zstd bytes")
    require(int(evidence.get("qatq_candidate_bytes", 0)) < int(evidence.get("lz4_bytes", 0)), "QATQ did not beat lz4 bytes")


def assert_event_trace_evidence(evidence: dict) -> None:
    trace = evidence.get("event_trace_report")
    require(isinstance(trace, dict), "event trace report is missing from evidence")
    require(trace.get("passed") is True, "event trace report did not pass")
    require(int(trace.get("events", 0)) > 0, "event trace report has no events")
    require(int(trace.get("snapshots", 0)) > 0, "event trace report has no snapshots")
    require(int(trace.get("offloads", 0)) > 0, "event trace report has no offloads")
    require(int(trace.get("restores", 0)) > 0, "event trace report has no restores")
    require(int(trace.get("attention_uses", 0)) > 0, "event trace report has no attention-use events")
    require(trace.get("failures") == [], "event trace report contains failures")


def build_native_page_streaming_status(
    *,
    page_composed_source_summary: dict | None,
    deep_page_composed_source_summary: dict | None,
    persistent_page_source_summary: dict | None,
    deep_persistent_page_source_summary: dict | None,
    deep_page_segments_summary: dict | None,
    attention_equivalence: dict | None,
    mlx_streaming_attention: dict | None,
) -> dict:
    """Classify whether the current evidence proves native non-concat attention.

    The current llama.cpp adapter can stage pages and can prove external MLX
    page-streamed attention. That is not the same thing as the runtime's own
    attention graph consuming K/V through a native streaming/paged kernel.
    """

    concat_sources = []
    non_native_sources = []
    for label, summary in (
        ("page-composed-source", page_composed_source_summary),
        ("deep-page-composed-source", deep_page_composed_source_summary),
        ("persistent-page-source", persistent_page_source_summary),
        ("deep-persistent-page-source", deep_persistent_page_source_summary),
    ):
        if summary is None:
            continue
        compositions = set(summary.get("composition_values", []))
        native_values = set(summary.get("native_page_streaming_values", []))
        if "ggml_concat" in compositions:
            concat_sources.append(label)
        if native_values != {True}:
            non_native_sources.append(label)
    equivalence_passed = bool((attention_equivalence or {}).get("passed") is True)
    equivalence_max_abs_error = (attention_equivalence or {}).get("max_abs_error")
    equivalence_peak_page_kv_ratio = (attention_equivalence or {}).get("peak_page_kv_ratio")
    mlx_passed = bool((mlx_streaming_attention or {}).get("passed") is True)
    failures = []
    if concat_sources:
        failures.append(
            "llama.cpp attention still uses ggml_concat-composed page sources: "
            + ",".join(concat_sources)
        )
    if non_native_sources:
        failures.append(
            "llama.cpp page-source traces report native_page_streaming=false for: "
            + ",".join(non_native_sources)
        )
    segment_api_present = deep_page_segments_summary is not None
    segment_attention_consumed = bool(
        (deep_page_segments_summary or {}).get(
            "cold_attention_consumed",
            (deep_page_segments_summary or {}).get("attention_consumed"),
        )
        is True
    )
    segment_native_values = set((deep_page_segments_summary or {}).get("native_page_streaming_values", []))
    segment_cold_native = bool(
        (deep_page_segments_summary or {}).get(
            "cold_native_page_streaming",
            segment_native_values == {True},
        )
        is True
    )
    segment_live_offloaded_events = int((deep_page_segments_summary or {}).get("live_offloaded_events", 0) or 0)
    segment_consumers = sorted(str(value) for value in (deep_page_segments_summary or {}).get("consumers", []))
    segment_consumer_set = set(segment_consumers)
    segmented_graph_bridge = bool((deep_page_segments_summary or {}).get("segmented_graph_bridge") is True)
    backend_scheduled_segmented_attention = bool(
        (deep_page_segments_summary or {}).get("backend_scheduled_segmented_attention") is True
    )
    backend_scheduled_flattened_flash_attention = bool(
        (deep_page_segments_summary or {}).get("backend_scheduled_flattened_flash_attention") is True
    )
    backend_scheduled_native_attention = (
        backend_scheduled_segmented_attention
        or backend_scheduled_flattened_flash_attention
    )
    accelerated_consumer = bool(
        segment_consumer_set
        & {
            "ggml_segmented_kqv",
            "backend_scheduled_segmented_attention",
            "backend_scheduled_flattened_flash_attention",
            "kernel_qatq_segmented_kqv",
            "ggml_segmented_kqv_backend",
            "ggml_segmented_kqv_online_page_summary",
        }
    )
    preflight_only_consumer = (
        "ggml_segmented_kqv_preflight" in segment_consumer_set
        and not accelerated_consumer
    )
    if not segment_api_present:
        failures.append("llama.cpp native page-streaming attention trace is not present")
    elif not segment_attention_consumed:
        failures.append(
            "llama.cpp page segment API is present but segments are not yet consumed by a native attention path"
        )
    if segment_api_present and not segment_cold_native:
        failures.append(
            "llama.cpp cold/offloaded page segment trace does not report native_page_streaming=true"
        )
    if segment_api_present and segment_live_offloaded_events <= 0:
        failures.append("llama.cpp page segment trace did not include any live-offloaded cold segment events")
    if segment_api_present and segment_attention_consumed and not segmented_graph_bridge and not backend_scheduled_native_attention:
        failures.append(
            "llama.cpp page segment consumer is neither the segmented graph bridge nor a backend-scheduled native attention path"
        )
    if segment_api_present and segmented_graph_bridge and not backend_scheduled_native_attention:
        failures.append(
            "llama.cpp segmented attention is still a graph bridge; "
            "backend-schedulable native page attention is not present"
        )
    if segment_api_present and preflight_only_consumer:
        failures.append(
            "llama.cpp page segment trace only reports ggml_segmented_kqv_preflight, which is not a native attention consumer"
        )
    if mlx_passed:
        pass
    else:
        failures.append("external MLX page-streaming attention reference did not run or did not pass")
    if equivalence_passed:
        pass
    else:
        failures.append("page-bounded attention equivalence gate did not run or did not pass")
    passed = (
        segment_api_present
        and segment_attention_consumed
        and segment_cold_native
        and segment_live_offloaded_events > 0
        and accelerated_consumer
        and backend_scheduled_native_attention
        and not concat_sources
        and not non_native_sources
        and equivalence_passed
        and mlx_passed
    )
    return {
        "format": "qatq-native-page-streaming-status-v1",
        "passed": passed,
        "native_runtime_attention_graph": segmented_graph_bridge or backend_scheduled_native_attention,
        "accelerated_runtime_attention_graph": backend_scheduled_native_attention and passed,
        "concat_composed_page_sources": concat_sources,
        "non_native_page_sources": non_native_sources,
        "page_segment_api_present": segment_api_present,
        "page_segments_attention_consumed": segment_attention_consumed,
        "page_segments_accelerated_consumer": accelerated_consumer,
        "segmented_graph_bridge": segmented_graph_bridge,
        "backend_scheduled_segmented_attention": backend_scheduled_segmented_attention,
        "backend_scheduled_flattened_flash_attention": backend_scheduled_flattened_flash_attention,
        "backend_scheduled_native_attention": backend_scheduled_native_attention,
        "page_segments_preflight_only_consumer": preflight_only_consumer,
        "page_segment_native_values": sorted(segment_native_values),
        "page_segment_cold_native": segment_cold_native,
        "page_segment_live_offloaded_events": segment_live_offloaded_events,
        "page_segment_resident_fast_path_events": int((deep_page_segments_summary or {}).get("resident_fast_path_events", 0) or 0),
        "runtime_attention_consumers": segment_consumers,
        "page_bounded_attention_equivalence_passed": equivalence_passed,
        "page_bounded_attention_equivalence_max_abs_error": equivalence_max_abs_error,
        "page_bounded_attention_equivalence_peak_page_kv_ratio": equivalence_peak_page_kv_ratio,
        "external_mlx_streaming_attention_passed": mlx_passed,
        "failures": failures,
    }


def assert_native_page_streaming_evidence(status: dict) -> None:
    require(status.get("passed") is True, "native page-streaming attention gate did not pass: " + "; ".join(status.get("failures", [])))


def assert_attention_trace(path: Path, *, model_id: str, output: Path) -> dict:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid attention trace JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-attention-trace-v1",
                f"unsupported attention trace format at line {line_number}: {row.get('format')}",
            )
            require(row.get("event") == "attention-use", f"unexpected attention trace event at line {line_number}")
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            require(row.get("model_id") == model_id, f"unexpected model_id at line {line_number}: {row.get('model_id')}")
            require(row.get("kind") in ("key", "value"), f"unexpected attention trace kind at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("token_start"), int) and row["token_start"] >= 0, f"invalid token_start at line {line_number}")
            require(isinstance(row.get("token_end"), int) and row["token_end"] > row["token_start"], f"invalid token range at line {line_number}")
            streams = row.get("streams")
            require(isinstance(streams, list) and streams, f"attention trace line {line_number} has no streams")
            rows.append(row)
    require(rows, "attention trace is empty")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"attention trace must include key and value reads, got {kinds}")
    layers = sorted({int(row["layer_id"]) for row in rows})
    require(layers, "attention trace has no layers")
    summary = {
        "format": "qatq-live-vram-attention-trace-summary-v1",
        "path": str(path),
        "events": len(rows),
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "token_ends": sorted({int(row["token_end"]) for row in rows}),
        "streams": sorted({int(stream) for row in rows for stream in row["streams"]}),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_attention_materialized_source_trace(path: Path, *, model_id: str, output: Path) -> dict:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid materialized source JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-attention-materialized-source-v1",
                f"unsupported materialized source format at line {line_number}: {row.get('format')}",
            )
            require(row.get("event") == "attention-materialized-source", f"unexpected materialized source event at line {line_number}")
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            require(row.get("model_id") == model_id, f"unexpected model_id at line {line_number}: {row.get('model_id')}")
            require(row.get("kind") in ("key", "value"), f"unexpected materialized source kind at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("n_kv"), int) and row["n_kv"] > 0, f"invalid n_kv at line {line_number}")
            source_bytes = row.get("source_bytes")
            materialized_bytes = row.get("materialized_bytes")
            require(isinstance(source_bytes, int) and source_bytes > 0, f"invalid source_bytes at line {line_number}")
            require(
                isinstance(materialized_bytes, int) and materialized_bytes == source_bytes,
                f"materialized/source byte mismatch at line {line_number}",
            )
            require(isinstance(row.get("source_shape"), list) and len(row["source_shape"]) == 4, f"invalid source_shape at line {line_number}")
            require(
                isinstance(row.get("materialized_shape"), list) and row["materialized_shape"] == row["source_shape"],
                f"materialized/source shape mismatch at line {line_number}",
            )
            rows.append(row)
    require(rows, "materialized source trace is empty")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"materialized source trace must include key and value, got {kinds}")
    layers = sorted({int(row["layer_id"]) for row in rows})
    summary = {
        "format": "qatq-live-vram-attention-materialized-source-summary-v1",
        "path": str(path),
        "events": len(rows),
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "n_kv_values": sorted({int(row["n_kv"]) for row in rows}),
        "source_bytes": sum(int(row["source_bytes"]) for row in rows),
        "materialized_bytes": sum(int(row["materialized_bytes"]) for row in rows),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_attention_page_composed_source_trace(
    path: Path,
    *,
    model_id: str,
    output: Path,
    require_multi_page: bool,
) -> dict:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid page-composed source JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-attention-page-composed-source-v1",
                f"unsupported page-composed source format at line {line_number}: {row.get('format')}",
            )
            require(row.get("event") == "attention-page-composed-source", f"unexpected page-composed source event at line {line_number}")
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            require(row.get("model_id") == model_id, f"unexpected model_id at line {line_number}: {row.get('model_id')}")
            require(row.get("kind") in ("key", "value"), f"unexpected page-composed source kind at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("n_kv"), int) and row["n_kv"] > 0, f"invalid n_kv at line {line_number}")
            require(isinstance(row.get("page_tokens"), int) and row["page_tokens"] > 0, f"invalid page_tokens at line {line_number}")
            require(isinstance(row.get("page_count"), int) and row["page_count"] > 0, f"invalid page_count at line {line_number}")
            require(row.get("composition") == "ggml_concat", f"unexpected page-composed composition at line {line_number}: {row.get('composition')}")
            require(row.get("native_page_streaming") is False, f"page-composed trace must report native_page_streaming=false at line {line_number}")
            source_bytes = row.get("source_bytes")
            composed_bytes = row.get("composed_bytes")
            require(isinstance(source_bytes, int) and source_bytes > 0, f"invalid source_bytes at line {line_number}")
            require(
                isinstance(composed_bytes, int) and composed_bytes == source_bytes,
                f"composed/source byte mismatch at line {line_number}",
            )
            require(isinstance(row.get("source_shape"), list) and len(row["source_shape"]) == 4, f"invalid source_shape at line {line_number}")
            require(
                isinstance(row.get("composed_shape"), list) and row["composed_shape"] == row["source_shape"],
                f"composed/source shape mismatch at line {line_number}",
            )
            rows.append(row)
    require(rows, "page-composed source trace is empty")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"page-composed source trace must include key and value, got {kinds}")
    max_page_count = max(int(row["page_count"]) for row in rows)
    if require_multi_page:
        require(max_page_count > 1, "page-composed source trace did not compose multiple pages")
    layers = sorted({int(row["layer_id"]) for row in rows})
    summary = {
        "format": "qatq-live-vram-attention-page-composed-source-summary-v1",
        "path": str(path),
        "events": len(rows),
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "n_kv_values": sorted({int(row["n_kv"]) for row in rows}),
        "page_tokens_values": sorted({int(row["page_tokens"]) for row in rows}),
        "min_page_count": min(int(row["page_count"]) for row in rows),
        "max_page_count": max_page_count,
        "composition_values": sorted({str(row["composition"]) for row in rows}),
        "native_page_streaming_values": sorted({bool(row["native_page_streaming"]) for row in rows}),
        "source_bytes": sum(int(row["source_bytes"]) for row in rows),
        "composed_bytes": sum(int(row["composed_bytes"]) for row in rows),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_attention_persistent_page_source_trace(
    path: Path,
    *,
    model_id: str,
    output: Path,
    require_multi_page: bool,
) -> dict:
    rows = []
    last_retained_pages = -1
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid persistent page source JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-attention-persistent-page-source-v1",
                f"unsupported persistent page source format at line {line_number}: {row.get('format')}",
            )
            require(row.get("event") == "attention-persistent-page-source", f"unexpected persistent page source event at line {line_number}")
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            require(row.get("model_id") == model_id, f"unexpected model_id at line {line_number}: {row.get('model_id')}")
            require(row.get("kind") in ("key", "value"), f"unexpected persistent page source kind at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("n_kv"), int) and row["n_kv"] > 0, f"invalid n_kv at line {line_number}")
            require(isinstance(row.get("page_tokens"), int) and row["page_tokens"] > 0, f"invalid page_tokens at line {line_number}")
            require(isinstance(row.get("page_count"), int) and row["page_count"] > 0, f"invalid page_count at line {line_number}")
            require(isinstance(row.get("event_index"), int) and row["event_index"] >= 0, f"invalid event_index at line {line_number}")
            require(row.get("composition") == "ggml_concat", f"unexpected persistent page-source composition at line {line_number}: {row.get('composition')}")
            require(row.get("native_page_streaming") is False, f"persistent page-source trace must report native_page_streaming=false at line {line_number}")
            backend = row.get("backend")
            require(isinstance(backend, str) and backend and backend not in ("CPU", "host"), f"invalid backend at line {line_number}: {backend}")
            source_bytes = row.get("source_bytes")
            composed_bytes = row.get("composed_bytes")
            requested_bytes = row.get("requested_bytes")
            allocated_bytes = row.get("allocated_bytes")
            retained_pages = row.get("retained_pages")
            retained_bytes = row.get("retained_bytes")
            staged_page_count = row.get("staged_page_count")
            max_source_bytes = row.get("max_source_bytes")
            max_retained_bytes = row.get("max_retained_bytes")
            page_count = int(row["page_count"])
            require(isinstance(source_bytes, int) and source_bytes > 0, f"invalid source_bytes at line {line_number}")
            require(
                isinstance(composed_bytes, int) and composed_bytes == source_bytes,
                f"composed/source byte mismatch at line {line_number}",
            )
            require(isinstance(staged_page_count, int) and staged_page_count >= 0, f"invalid staged_page_count at line {line_number}")
            require(staged_page_count <= page_count, f"staged_page_count exceeds page_count at line {line_number}")
            require(isinstance(requested_bytes, int) and requested_bytes >= 0, f"invalid requested_bytes at line {line_number}")
            require(
                isinstance(allocated_bytes, int) and allocated_bytes >= requested_bytes,
                f"allocated/requested byte mismatch at line {line_number}",
            )
            require(
                isinstance(retained_pages, int) and retained_pages >= staged_page_count,
                f"invalid retained_pages at line {line_number}",
            )
            require(
                isinstance(retained_bytes, int) and retained_bytes >= allocated_bytes,
                f"invalid retained_bytes at line {line_number}",
            )
            require(
                isinstance(max_source_bytes, int) and max_source_bytes > 0,
                f"invalid max_source_bytes at line {line_number}",
            )
            require(
                isinstance(max_retained_bytes, int) and max_retained_bytes > 0,
                f"invalid max_retained_bytes at line {line_number}",
            )
            require(
                requested_bytes <= max_source_bytes,
                f"requested bytes exceed source budget at line {line_number}",
            )
            require(
                retained_bytes <= max_retained_bytes,
                f"retained bytes exceed retained budget at line {line_number}",
            )
            require(retained_pages >= last_retained_pages, f"retained_pages regressed at line {line_number}")
            last_retained_pages = retained_pages
            require(isinstance(row.get("source_shape"), list) and len(row["source_shape"]) == 4, f"invalid source_shape at line {line_number}")
            require(
                isinstance(row.get("composed_shape"), list) and row["composed_shape"] == row["source_shape"],
                f"composed/source shape mismatch at line {line_number}",
            )
            rows.append(row)
    require(rows, "persistent page source trace is empty")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"persistent page source trace must include key and value, got {kinds}")
    max_page_count = max(int(row["page_count"]) for row in rows)
    if require_multi_page:
        require(max_page_count > 1, "persistent page source trace did not compose multiple pages")
    layers = sorted({int(row["layer_id"]) for row in rows})
    summary = {
        "format": "qatq-live-vram-attention-persistent-page-source-summary-v1",
        "path": str(path),
        "events": len(rows),
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "backends": sorted({str(row["backend"]) for row in rows}),
        "n_kv_values": sorted({int(row["n_kv"]) for row in rows}),
        "page_tokens_values": sorted({int(row["page_tokens"]) for row in rows}),
        "min_page_count": min(int(row["page_count"]) for row in rows),
        "max_page_count": max_page_count,
        "composition_values": sorted({str(row["composition"]) for row in rows}),
        "native_page_streaming_values": sorted({bool(row["native_page_streaming"]) for row in rows}),
        "staged_pages": sum(int(row.get("staged_page_count", 0)) for row in rows),
        "max_staged_page_count": max(int(row.get("staged_page_count", 0)) for row in rows),
        "source_bytes": sum(int(row["source_bytes"]) for row in rows),
        "composed_bytes": sum(int(row["composed_bytes"]) for row in rows),
        "requested_bytes": sum(int(row["requested_bytes"]) for row in rows),
        "allocated_bytes": sum(int(row["allocated_bytes"]) for row in rows),
        "max_requested_bytes": max(int(row["requested_bytes"]) for row in rows),
        "max_allocated_bytes": max(int(row["allocated_bytes"]) for row in rows),
        "max_retained_bytes_observed": max(int(row["retained_bytes"]) for row in rows),
        "max_source_bytes": max(int(row["max_source_bytes"]) for row in rows),
        "max_retained_bytes": max(int(row["max_retained_bytes"]) for row in rows),
        "max_retained_pages": max(int(row["retained_pages"]) for row in rows),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_attention_page_segments_trace(
    path: Path,
    *,
    model_id: str,
    output: Path,
    require_multi_page: bool,
    expect_native_page_streaming: bool,
    expect_attention_consumed: bool,
) -> dict:
    rows = []
    normalized_rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid page segment JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-attention-page-segments-v1",
                f"unsupported page segment format at line {line_number}: {row.get('format')}",
            )
            require(row.get("event") == "attention-page-segments", f"unexpected page segment event at line {line_number}")
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            require(row.get("model_id") == model_id, f"unexpected model_id at line {line_number}: {row.get('model_id')}")
            require(row.get("kind") in ("key", "value"), f"unexpected page segment kind at line {line_number}")
            require(isinstance(row.get("seq_id"), str) and row["seq_id"], f"invalid seq_id at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("n_kv"), int) and row["n_kv"] > 0, f"invalid n_kv at line {line_number}")
            require(isinstance(row.get("segment_count"), int) and row["segment_count"] > 0, f"invalid segment_count at line {line_number}")
            require(isinstance(row.get("segment_bytes"), int) and row["segment_bytes"] > 0, f"invalid segment_bytes at line {line_number}")
            require(isinstance(row.get("event_index"), int) and row["event_index"] >= 0, f"invalid event_index at line {line_number}")
            require(row.get("composition") == "none", f"page segment trace must not compose K/V at line {line_number}")
            live_offloaded_only = bool(row.get("live_offloaded_only", False))
            segments = row.get("segments")
            require(isinstance(segments, list) and len(segments) == row["segment_count"], f"invalid segments array at line {line_number}")
            expected_start = 0
            previous_end = 0
            byte_total = 0
            ranges = []
            max_segment_tokens = 0
            live_offloaded = False
            for index, segment in enumerate(segments):
                require(isinstance(segment, dict), f"segment {index} is not an object at line {line_number}")
                start = segment.get("token_start")
                end = segment.get("token_end")
                size = segment.get("bytes")
                shape = segment.get("shape")
                segment_live_offloaded = bool(segment.get("live_offloaded", False))
                require(isinstance(start, int) and start >= 0, f"invalid segment start at line {line_number}")
                if live_offloaded_only:
                    require(segment_live_offloaded, f"live-offloaded-only segment row contains resident segment at line {line_number}")
                    require(start >= previous_end, f"overlapping live-offloaded segment start at line {line_number}")
                else:
                    require(start == expected_start, f"non-contiguous segment start at line {line_number}")
                require(isinstance(end, int) and end > start, f"invalid segment token range at line {line_number}")
                require(end <= row["n_kv"], f"segment token range exceeds n_kv at line {line_number}")
                require(isinstance(size, int) and size > 0, f"invalid segment bytes at line {line_number}")
                require(isinstance(shape, list) and len(shape) == 4, f"invalid segment shape at line {line_number}")
                require(
                    all(isinstance(value, int) and value > 0 for value in shape),
                    f"invalid segment shape values at line {line_number}",
                )
                ranges.append((start, end))
                max_segment_tokens = max(max_segment_tokens, end - start)
                expected_start = end
                previous_end = end
                byte_total += size
                live_offloaded = live_offloaded or segment_live_offloaded
            if live_offloaded_only:
                require(live_offloaded, f"live-offloaded-only row has no live-offloaded segments at line {line_number}")
            else:
                require(expected_start == row["n_kv"], f"page segments do not cover n_kv at line {line_number}")
            expected_native = expect_native_page_streaming and live_offloaded
            expected_consumed = expect_attention_consumed and live_offloaded
            require(
                row.get("native_page_streaming") is expected_native,
                f"page segment trace native_page_streaming mismatch at line {line_number}",
            )
            require(
                row.get("attention_consumed") is expected_consumed,
                f"page segment trace attention_consumed mismatch at line {line_number}",
            )
            if expected_consumed:
                require(
                    isinstance(row.get("consumer"), str) and row["consumer"],
                    f"consumed page segment trace must include consumer at line {line_number}",
                )
            require(byte_total == row["segment_bytes"], f"segment byte total mismatch at line {line_number}")
            rows.append(row)
            normalized_rows.append(
                {
                    "seq_id": row["seq_id"],
                    "layer_id": row["layer_id"],
                    "kind": row["kind"],
                    "n_kv": row["n_kv"],
                    "ranges": tuple(ranges),
                    "native_page_streaming": row["native_page_streaming"],
                    "attention_consumed": row["attention_consumed"],
                    "consumer": row.get("consumer", ""),
                    "max_segment_tokens": max_segment_tokens,
                    "live_offloaded": live_offloaded,
                    "live_offloaded_only": live_offloaded_only,
                }
            )
    require(rows, "page segment trace is empty")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"page segment trace must include key and value, got {kinds}")
    group_counts: dict[tuple, dict[str, int]] = {}
    max_segment_tokens = 0
    for row in normalized_rows:
        max_segment_tokens = max(max_segment_tokens, int(row["max_segment_tokens"]))
        key = (
            row["seq_id"],
            row["layer_id"],
            row["n_kv"],
            row["ranges"],
            row["native_page_streaming"],
            row["attention_consumed"],
            row["consumer"],
            row["live_offloaded"],
            row["live_offloaded_only"],
        )
        counts = group_counts.setdefault(key, {"key": 0, "value": 0})
        counts[str(row["kind"])] += 1
    unpaired = [
        f"seq={key[0]} layer={key[1]} n_kv={key[2]} key={counts['key']} value={counts['value']}"
        for key, counts in group_counts.items()
        if counts["key"] != counts["value"]
    ]
    require(
        not unpaired,
        "page segment trace has unpaired K/V segment groups: " + "; ".join(unpaired[:8]),
    )
    max_segment_count = max(int(row["segment_count"]) for row in rows)
    if require_multi_page:
        require(max_segment_count > 1, "page segment trace did not expose multiple segments")
    layers = sorted({int(row["layer_id"]) for row in rows})
    kind_event_counts = {kind: sum(1 for row in rows if row["kind"] == kind) for kind in kinds}
    consumers = sorted({str(row.get("consumer", "")) for row in rows if row.get("consumer")})
    consumer_set = set(consumers)
    native_values = sorted({bool(row["native_page_streaming"]) for row in rows})
    attention_consumed = all(bool(row.get("attention_consumed")) for row in rows)
    cold_rows = [row for row in normalized_rows if bool(row["live_offloaded"])]
    resident_rows = [row for row in normalized_rows if not bool(row["live_offloaded"])]
    cold_attention_consumed = bool(cold_rows) and all(bool(row["attention_consumed"]) for row in cold_rows)
    cold_native_page_streaming = bool(cold_rows) and all(bool(row["native_page_streaming"]) for row in cold_rows)
    resident_fast_path_events = sum(1 for row in resident_rows if not bool(row["native_page_streaming"]) and not bool(row["attention_consumed"]))
    segmented_graph_bridge = (
        cold_attention_consumed
        and cold_native_page_streaming
        and "ggml_segmented_kqv" in consumer_set
    )
    backend_surface_consumers = sorted(
        consumer_set
        & {
            "backend_scheduled_segmented_attention",
            "backend_scheduled_flattened_flash_attention",
            "kernel_qatq_segmented_kqv",
            "ggml_segmented_kqv_backend",
            "ggml_segmented_kqv_online_page_summary",
        }
    )
    summary = {
        "format": "qatq-live-vram-attention-page-segments-summary-v1",
        "path": str(path),
        "events": len(rows),
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "kind_event_counts": kind_event_counts,
        "n_kv_values": sorted({int(row["n_kv"]) for row in rows}),
        "min_segment_count": min(int(row["segment_count"]) for row in rows),
        "max_segment_count": max_segment_count,
        "max_segment_tokens": max_segment_tokens,
        "paired_segment_groups": len(group_counts),
        "segment_bytes": sum(int(row["segment_bytes"]) for row in rows),
        "composition_values": sorted({str(row["composition"]) for row in rows}),
        "native_page_streaming_values": native_values,
        "live_offloaded_only_values": sorted({bool(row.get("live_offloaded_only", False)) for row in rows}),
        "attention_consumed": attention_consumed,
        "live_offloaded_events": len(cold_rows),
        "resident_fast_path_events": resident_fast_path_events,
        "cold_native_page_streaming": cold_native_page_streaming,
        "cold_attention_consumed": cold_attention_consumed,
        "consumers": consumers,
        "segmented_graph_bridge": segmented_graph_bridge,
        "backend_scheduled_segmented_attention": "backend_scheduled_segmented_attention" in backend_surface_consumers,
        "backend_scheduled_flattened_flash_attention": "backend_scheduled_flattened_flash_attention" in backend_surface_consumers,
        "backend_scheduled_native_attention": bool(backend_surface_consumers),
        "backend_surface_consumers": backend_surface_consumers,
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_attention_page_tensor_self_test(path: Path, *, output: Path) -> dict:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid attention page tensor self-test JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("event") == "attention-page-tensor-roundtrip",
                f"unexpected attention page tensor event at line {line_number}: {row.get('event')}",
            )
            backend = row.get("backend")
            require(isinstance(backend, str) and backend, f"missing backend at line {line_number}")
            require("CPU" not in backend.upper(), f"attention page tensor self-test used CPU backend at line {line_number}: {backend}")
            require(row.get("kind") in ("key", "value"), f"unexpected page tensor kind at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("stream"), int) and row["stream"] >= 0, f"invalid stream at line {line_number}")
            require(isinstance(row.get("token_start"), int) and row["token_start"] >= 0, f"invalid token_start at line {line_number}")
            require(
                isinstance(row.get("token_end"), int) and row["token_end"] > row["token_start"],
                f"invalid token range at line {line_number}",
            )
            requested = row.get("requested_bytes")
            allocated = row.get("allocated_bytes")
            require(isinstance(requested, int) and requested > 0, f"invalid requested_bytes at line {line_number}")
            require(isinstance(allocated, int) and allocated >= requested, f"invalid allocated_bytes at line {line_number}")
            rows.append(row)
    require(rows, "attention page tensor self-test is empty")
    kinds = sorted({row["kind"] for row in rows})
    layers = sorted({int(row["layer_id"]) for row in rows})
    backends = sorted({str(row["backend"]) for row in rows})
    requested_bytes = sum(int(row["requested_bytes"]) for row in rows)
    allocated_bytes = sum(int(row["allocated_bytes"]) for row in rows)
    summary = {
        "format": "qatq-live-vram-attention-page-tensor-self-test-summary-v1",
        "path": str(path),
        "events": len(rows),
        "backends": backends,
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "streams": sorted({int(row["stream"]) for row in rows}),
        "token_ranges": [[int(row["token_start"]), int(row["token_end"])] for row in rows],
        "requested_bytes": requested_bytes,
        "allocated_bytes": allocated_bytes,
        "min_requested_bytes": min(int(row["requested_bytes"]) for row in rows),
        "max_requested_bytes": max(int(row["requested_bytes"]) for row in rows),
        "min_allocated_bytes": min(int(row["allocated_bytes"]) for row in rows),
        "max_allocated_bytes": max(int(row["allocated_bytes"]) for row in rows),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_persistent_page_pool_trace(path: Path, *, expected_pages: int, output: Path) -> dict:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid persistent page pool JSONL at line {line_number}: {exc}") from exc
            require(
                row.get("format") == "qatq-live-vram-persistent-page-pool-v1",
                f"unexpected persistent page pool format at line {line_number}: {row.get('format')}",
            )
            require(
                row.get("event") == "persistent-page-retained",
                f"unexpected persistent page pool event at line {line_number}: {row.get('event')}",
            )
            require(row.get("runtime_id") == "llama.cpp", f"unexpected runtime_id at line {line_number}")
            backend = row.get("backend")
            require(isinstance(backend, str) and backend, f"missing backend at line {line_number}")
            require("CPU" not in backend.upper(), f"persistent page pool used CPU backend at line {line_number}: {backend}")
            require(row.get("kind") in ("key", "value"), f"unexpected persistent page kind at line {line_number}")
            require(isinstance(row.get("pool_index"), int) and row["pool_index"] >= 0, f"invalid pool_index at line {line_number}")
            require(isinstance(row.get("layer_id"), int) and row["layer_id"] >= 0, f"invalid layer_id at line {line_number}")
            require(isinstance(row.get("stream"), int) and row["stream"] >= 0, f"invalid stream at line {line_number}")
            require(isinstance(row.get("token_start"), int) and row["token_start"] >= 0, f"invalid token_start at line {line_number}")
            require(
                isinstance(row.get("token_end"), int) and row["token_end"] > row["token_start"],
                f"invalid token range at line {line_number}",
            )
            requested = row.get("requested_bytes")
            allocated = row.get("allocated_bytes")
            retained = row.get("retained_pages")
            require(isinstance(requested, int) and requested > 0, f"invalid requested_bytes at line {line_number}")
            require(isinstance(allocated, int) and allocated >= requested, f"invalid allocated_bytes at line {line_number}")
            require(isinstance(retained, int) and retained == line_number, f"invalid retained_pages at line {line_number}")
            rows.append(row)

    require(rows, "persistent page pool trace is empty")
    require(len(rows) == expected_pages, f"persistent page pool retained {len(rows)} pages, expected {expected_pages}")
    kinds = sorted({row["kind"] for row in rows})
    require(kinds == ["key", "value"], f"persistent page pool must retain both key and value pages, got {kinds}")
    layers = sorted({int(row["layer_id"]) for row in rows})
    backends = sorted({str(row["backend"]) for row in rows})
    requested_bytes = sum(int(row["requested_bytes"]) for row in rows)
    allocated_bytes = sum(int(row["allocated_bytes"]) for row in rows)
    summary = {
        "format": "qatq-live-vram-persistent-page-pool-summary-v1",
        "path": str(path),
        "events": len(rows),
        "backends": backends,
        "layers": len(layers),
        "min_layer": layers[0],
        "max_layer": layers[-1],
        "kinds": kinds,
        "streams": sorted({int(row["stream"]) for row in rows}),
        "token_ranges": [[int(row["token_start"]), int(row["token_end"])] for row in rows],
        "requested_bytes": requested_bytes,
        "allocated_bytes": allocated_bytes,
        "min_requested_bytes": min(int(row["requested_bytes"]) for row in rows),
        "max_requested_bytes": max(int(row["requested_bytes"]) for row in rows),
        "min_allocated_bytes": min(int(row["allocated_bytes"]) for row in rows),
        "max_allocated_bytes": max(int(row["allocated_bytes"]) for row in rows),
    }
    output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def assert_performance_gate(output_json: dict, cpu_output_json: dict | None, max_mixed_decode_regression: float) -> None:
    require(max_mixed_decode_regression >= 0.0, "--max-mixed-decode-regression must be non-negative")
    baseline_us = int(output_json["baseline"]["total_us"])
    mixed_us = int(output_json["candidate"]["total_us"])
    require(baseline_us > 0 and mixed_us > 0, "output manifests did not record positive decode times")
    regression = (mixed_us / baseline_us) - 1.0
    require(
        regression <= max_mixed_decode_regression,
        f"mixed-KV decode regression {regression:.6f} exceeds allowed {max_mixed_decode_regression:.6f}",
    )
    if cpu_output_json is not None:
        cpu_us = int(cpu_output_json["candidate"]["total_us"])
        require(cpu_us > 0, "CPU-KV output manifest did not record positive decode time")
        require(
            mixed_us < cpu_us,
            f"mixed-KV decode time {mixed_us} us is not faster than all-CPU-KV baseline {cpu_us} us",
        )


def parse_token_latency_stats(path: Path, *, baseline_run: str, candidate_run: str) -> TokenLatencyStats:
    full_decode_us: list[float] = []
    mixed_decode_us: list[float] = []
    require(path.exists(), f"missing token timing CSV: {path}")
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        require(reader.fieldnames is not None, f"{path} is missing a CSV header")
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


def assert_deep_latency_gate(
    stats: TokenLatencyStats,
    *,
    min_samples: int,
    max_p95_regression: float,
    max_p99_regression: float,
    output: Path,
) -> None:
    report = {
        "format": "qatq-live-vram-deep-latency-gate-v1",
        "full_samples": stats.full_samples,
        "mixed_samples": stats.mixed_samples,
        "full_p95_decode_us": stats.full_p95,
        "mixed_p95_decode_us": stats.mixed_p95,
        "full_p99_decode_us": stats.full_p99,
        "mixed_p99_decode_us": stats.mixed_p99,
        "mixed_p95_regression": stats.p95_regression,
        "mixed_p99_regression": stats.p99_regression,
        "min_samples": min_samples,
        "max_p95_regression": max_p95_regression,
        "max_p99_regression": max_p99_regression,
        "passed": True,
        "failures": [],
    }
    failures: list[str] = []
    required_samples = max(min_samples, 1) if (min_samples > 0 or max_p95_regression > 0.0 or max_p99_regression > 0.0) else 0
    observed_samples = min(stats.full_samples, stats.mixed_samples)
    if required_samples > 0 and observed_samples < required_samples:
        failures.append(f"deep token latency samples {observed_samples} below required {required_samples}")
    if max_p95_regression > 0.0 and stats.p95_regression > max_p95_regression:
        failures.append(f"deep mixed p95 token decode regression {stats.p95_regression:.6f} exceeded {max_p95_regression:.6f}")
    if max_p99_regression > 0.0 and stats.p99_regression > max_p99_regression:
        failures.append(f"deep mixed p99 token decode regression {stats.p99_regression:.6f} exceeded {max_p99_regression:.6f}")
    report["passed"] = not failures
    report["failures"] = failures
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    require(not failures, "deep latency gate failed: " + "; ".join(failures))


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


def run_mlx_streaming_attention_gate(
    *,
    args,
    root: Path,
    export_dir: Path,
    fixture_dir: Path,
    output: Path,
    store_dir: Path,
) -> dict:
    script = require_file(Path(args.mlx_streaming_attention_script), "--mlx-streaming-attention-script")
    qatq_bin = require_file(Path(args.mlx_qatq_bin), "--mlx-qatq-bin")
    command = [
        str(Path(args.mlx_python)),
        str(script),
        "--export-dir",
        str(export_dir),
        "--attention-fixture-dir",
        str(fixture_dir),
        "--layer",
        "-1",
        "--head",
        "-1",
        "--stream-from-qatq-store",
        "--qatq-bin",
        str(qatq_bin),
        "--qatq-store-dir",
        str(store_dir),
        "--tolerance",
        str(args.attention_equivalence_tolerance),
        "--max-peak-page-kv-ratio",
        str(args.attention_max_peak_page_kv_ratio),
        "--output",
        str(output),
    ]
    run(command, cwd=root, timeout=args.timeout)
    report = load_json(output)
    require(report.get("passed") is True, "MLX streaming attention gate did not pass")
    require(str(report.get("device", "")).startswith("Device(gpu"), "MLX streaming attention did not run on a GPU device")
    layers_checked = int(report.get("layers_checked", 1))
    heads_checked = int(report.get("heads_checked", 1))
    require(
        layers_checked >= int(args.mlx_min_layers_checked),
        f"MLX streaming attention checked {layers_checked} layers, below required {args.mlx_min_layers_checked}",
    )
    require(
        heads_checked >= int(args.mlx_min_heads_checked),
        f"MLX streaming attention checked {heads_checked} heads, below required {args.mlx_min_heads_checked}",
    )
    store = report.get("qatq_store") or {}
    require(store.get("enabled") is True, "MLX streaming attention did not stream from a QATQ page store")
    require(int(store.get("pages", 0)) > 0, "MLX QATQ page store encoded zero pages")
    store_ratio = float(store.get("compression_ratio", 1.0))
    require(store_ratio < 1.0, f"MLX QATQ page store did not compress selected pages: ratio {store_ratio:.9f}")
    if float(args.mlx_max_streaming_slowdown) > 0.0:
        materialized_seconds = float((report.get("materialized") or {}).get("seconds", 0.0))
        streaming_seconds = float((report.get("streaming") or {}).get("seconds", 0.0))
        require(materialized_seconds > 0.0, "MLX materialised attention timing was not positive")
        slowdown = streaming_seconds / materialized_seconds
        require(
            slowdown <= float(args.mlx_max_streaming_slowdown),
            f"MLX streaming/materialised slowdown {slowdown:.6f} exceeds {args.mlx_max_streaming_slowdown:.6f}",
        )
    return report


def build_summary(
    args,
    model: Path,
    work_dir: Path,
    output_json: dict,
    cpu_output_json: dict | None,
    evidence: dict,
    elapsed: float,
    frontier: list[FrontierResult],
    selected: FrontierResult,
    attention_trace_summary: dict | None,
    attention_event_trace_report: dict | None,
    materialized_source_summary: dict | None,
    materialized_source_compare_json: dict | None,
    deep_materialized_source_summary: dict | None,
    page_composed_source_summary: dict | None,
    page_composed_source_compare_json: dict | None,
    deep_page_composed_source_summary: dict | None,
    persistent_page_source_summary: dict | None,
    persistent_page_source_compare_json: dict | None,
    deep_persistent_page_source_summary: dict | None,
    deep_page_segments_summary: dict | None,
    attention_equivalence: dict | None,
    mlx_streaming_attention: dict | None,
    native_page_streaming: dict,
    attention_page_tensor_self_test: dict | None,
    persistent_page_pool: dict | None,
    deep_latency: TokenLatencyStats | None,
) -> str:
    baseline = output_json["baseline"]
    candidate = output_json["candidate"]
    cpu_candidate = (cpu_output_json or {}).get("candidate")
    residency = evidence.get("residency_estimate")
    if residency is None:
        manifest_path = work_dir / f"deep-mixed-kv-l{selected.layers}" / "manifest.json"
        manifest = load_json(manifest_path)
        before_bytes = int(manifest.get("total_context_bytes", 0))
        after_bytes = int(manifest.get("gpu_context_bytes", 0))
        residency = {
            "gpu_context_bytes_before": before_bytes,
            "gpu_context_bytes_after": after_bytes,
            "reclaimable_gpu_bytes": max(0, before_bytes - after_bytes),
            "allocation_granularity": manifest.get("gpu_allocation_granularity", "runtime-unknown"),
        }
    restore = evidence["restore_deadline_report"]
    event_trace = evidence.get("event_trace_report")
    raw = int(evidence["raw_bytes"])
    qatq = int(evidence["qatq_candidate_bytes"])
    zstd = int(evidence["zstd_bytes"])
    lz4 = int(evidence["lz4_bytes"])
    before = int(residency["gpu_context_bytes_before"])
    after = int(residency["gpu_context_bytes_after"])
    reclaimed = int(residency["reclaimable_gpu_bytes"])
    out = "# llama.cpp Live VRAM Evidence\n\n"
    out += "Generated by `scripts/llama_cpp_live_vram_evidence.py`.\n\n"
    out += "## Inputs\n\n"
    out += f"- model: `{model}`\n"
    out += f"- work dir: `{work_dir}`\n"
    out += f"- selected mixed KV GPU layers: `{selected.layers}`\n"
    out += f"- cache types: K `{args.cache_type_k}`, V `{args.cache_type_v}`\n"
    out += f"- short prompt bytes: `{len(args.short_prompt.encode('utf-8'))}`\n"
    out += f"- deep prompt seed bytes: `{len(args.deep_prompt_seed.encode('utf-8'))}`\n"
    out += f"- deep repeat: `{args.deep_repeat}`\n"
    out += f"- export page tokens: `{args.page_tokens if args.page_tokens > 0 else 'whole tensor'}`\n"
    out += f"- GPU page staging: `{'enabled' if args.gpu_page_staging else 'disabled'}`\n"
    out += f"- native transient page-pool max bytes: `{args.native_page_streaming_transient_pool_max_bytes}`\n"
    out += f"- persistent page-source max source pages: `{args.attention_persistent_page_source_max_source_pages}`\n"
    out += f"- persistent page-source max source bytes: `{args.attention_persistent_page_source_max_source_bytes}`\n"
    out += f"- persistent page-source max retained bytes: `{args.attention_persistent_page_source_max_retained_bytes}`\n"
    out += f"- current token: `{args.current_token}`\n"
    out += f"- hot window tokens: `{args.hot_window_tokens}`\n"
    out += f"- prefetch window tokens: `{args.prefetch_window_tokens}`\n"
    out += f"- next-required mode: `{args.next_required}`\n"
    out += f"- max queued/offloaded pages: `{args.max_queued_pages if args.max_queued_pages > 0 else 'default'}`\n"
    out += f"- codec proof policy: `{'aggregate' if args.aggregate_codec_gate else 'per-offloaded-page'}`\n"
    out += f"- bulk artifact pruning: `{'enabled' if args.prune_bulk_artifacts else 'disabled'}`\n"
    out += f"- elapsed seconds: `{elapsed:.2f}`\n\n"
    evidence_gate_label = (
        "live-paging"
        if args.require_live_paging
        else "native-correctness"
        if args.skip_runtime_reclaim_gate
        else "runtime-reclaim"
    )
    out += f"- evidence gate: `{evidence_gate_label}`\n"
    out += f"- event trace: `{'enabled' if event_trace is not None else 'disabled'}`\n\n"
    out += f"- metadata page seals: `{'required' if not args.skip_runtime_reclaim_gate else 'disabled'}`\n\n"
    out += f"- attention trace: `{'enabled' if attention_trace_summary is not None else 'disabled'}`\n\n"
    out += f"- attention event trace: `{'enabled' if attention_event_trace_report is not None else 'disabled'}`\n\n"
    out += f"- attention materialized source: `{'enabled' if materialized_source_summary is not None else 'disabled'}`\n\n"
    out += f"- attention page-composed source: `{'enabled' if page_composed_source_summary is not None else 'disabled'}`\n\n"
    out += f"- attention persistent page source: `{'enabled' if persistent_page_source_summary is not None else 'disabled'}`\n\n"
    out += f"- attention page segment trace: `{'enabled' if deep_page_segments_summary is not None else 'disabled'}`\n\n"
    out += f"- attention equivalence: `{'enabled' if attention_equivalence is not None else 'disabled'}`\n\n"
    out += f"- MLX streaming attention gate: `{'enabled' if mlx_streaming_attention is not None else 'disabled'}`\n\n"
    out += f"- MLX minimum layers checked: `{args.mlx_min_layers_checked}`\n\n"
    out += f"- MLX minimum heads checked: `{args.mlx_min_heads_checked}`\n\n"
    out += f"- MLX max streaming slowdown: `{args.mlx_max_streaming_slowdown if args.mlx_max_streaming_slowdown > 0 else 'disabled'}`\n\n"
    out += f"- attention page tensor self-test: `{'enabled' if attention_page_tensor_self_test is not None else 'disabled'}`\n\n"
    out += f"- live persistent page pool self-test: `{'enabled' if persistent_page_pool is not None else 'disabled'}`\n\n"
    out += f"- live page self-test tokens: `{args.live_page_self_test_tokens}`\n\n"
    out += f"- live physical page alloc self-test tokens: `{args.live_physical_page_alloc_self_test_tokens}`\n\n"
    out += f"- live restore slot pressure self-test tokens: `{args.live_restore_slot_pressure_self_test_tokens}`\n\n"
    out += f"- live restore slot pressure max bytes: `{args.live_restore_slot_pressure_max_bytes}`\n\n"
    out += f"- live persistent page pool pages: `{args.live_persistent_page_pool_self_test_pages}`\n\n"
    out += f"- deep token latency sample gate: `{args.min_deep_token_latency_samples}`\n\n"
    out += f"- deep mixed p95 token regression gate: `{args.max_deep_mixed_token_p95_regression_ratio:.6f}`\n\n"
    out += f"- deep mixed p99 token regression gate: `{args.max_deep_mixed_token_p99_regression_ratio:.6f}`\n\n"
    out += "- page CSV: `pages.csv`\n"
    out += "- timing CSV: `tokens.csv`\n\n"
    out += "## Frontier\n\n"
    out += "| KV GPU layers | status | decode us | regression | GPU saved | GPU KV MiB | tensors on GPU | failure |\n"
    out += "| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |\n"
    for result in frontier:
        status = "pass" if result.passed else "fail"
        failure = result.failure.replace("|", "/") if result.failure else ""
        out += (
            f"| {result.layers} | {status} | {result.total_us} | {result.decode_regression:.3f} | "
            f"{result.saved_ratio:.3f} | {mib(result.gpu_context_bytes):.2f} | "
            f"{result.gpu_resident_tensors}/{result.total_tensors} | {failure} |\n"
        )
    out += f"\nSelected fastest passing frontier point: `{selected.layers}` KV GPU layers.\n\n"
    out += "## Gates\n\n"
    out += f"- output comparison: `pass`, generated tokens `{candidate['generated_token_count']}`\n"
    out += f"- generated text hash: `{candidate['generated_text_hash']}`\n"
    out += f"- full-GPU decode time: `{baseline['total_us']}` us\n"
    out += f"- mixed-KV decode time: `{candidate['total_us']}` us\n"
    if cpu_candidate is not None:
        out += f"- all-CPU-KV decode time: `{cpu_candidate['total_us']}` us\n"
        out += f"- mixed-KV faster than all-CPU-KV: `{candidate['total_us'] < cpu_candidate['total_us']}`\n"
    out += f"- max mixed decode regression: `{args.max_mixed_decode_regression:.2f}`\n"
    out += f"- exact restores: `{evidence['verified_restores']}/{evidence['total_pages']}`\n"
    out += f"- offloaded compressed pages: `{evidence['compressed_pages']}/{evidence['offloaded_pages']}`\n"
    out += f"- resident pages: `{evidence['resident_pages']}`\n"
    out += f"- pass-through pages: `{evidence['pass_through_pages']}`\n"
    out += f"- restore prefetch misses: `{restore['prefetch_misses']}`\n"
    out += f"- QATQ pages beating best general codec: `{evidence['qatq_beats_best_general_codec_pages']}/{evidence['total_pages']}`\n\n"
    if deep_latency is not None:
        out += "## Deep Token Latency\n\n"
        out += "| metric | full GPU | mixed KV | regression |\n"
        out += "| --- | ---: | ---: | ---: |\n"
        out += f"| samples | {deep_latency.full_samples} | {deep_latency.mixed_samples} | - |\n"
        out += f"| p95 decode us | {deep_latency.full_p95:.0f} | {deep_latency.mixed_p95:.0f} | {deep_latency.p95_regression:.6f} |\n"
        out += f"| p99 decode us | {deep_latency.full_p99:.0f} | {deep_latency.mixed_p99:.0f} | {deep_latency.p99_regression:.6f} |\n\n"
    if event_trace is not None:
        out += "## Event Trace\n\n"
        out += f"- trace passed: `{event_trace['passed']}`\n"
        out += f"- events: `{event_trace['events']}`\n"
        out += f"- snapshots: `{event_trace['snapshots']}`\n"
        out += f"- offload commits: `{event_trace['offloads']}`\n"
        out += f"- restore commits: `{event_trace['restores']}`\n"
        out += f"- attention uses: `{event_trace['attention_uses']}`\n"
        out += f"- unfinished offloads: `{event_trace['offloaded_pages_at_end']}`\n\n"
    if attention_trace_summary is not None:
        out += "## Attention Trace\n\n"
        out += "- source: actual llama.cpp attention-path KV reads, not export-time synthetic lifecycle events\n"
        out += f"- events: `{attention_trace_summary['events']}`\n"
        out += f"- layers: `{attention_trace_summary['layers']}` (`{attention_trace_summary['min_layer']}`..`{attention_trace_summary['max_layer']}`)\n"
        out += f"- kinds: `{','.join(attention_trace_summary['kinds'])}`\n"
        out += f"- token ends: `{','.join(str(value) for value in attention_trace_summary['token_ends'])}`\n"
        out += f"- streams: `{','.join(str(value) for value in attention_trace_summary['streams'])}`\n\n"
    if attention_event_trace_report is not None:
        out += "## Attention Event Trace\n\n"
        out += "- source: actual llama.cpp `get_k/get_v` attention path lifecycle events\n"
        out += "- check: QATQ event verifier requires snapshot before offload, restore before attention use, matching restore checksums, monotonic tokens, and no unfinished offloads\n"
        out += f"- trace passed: `{attention_event_trace_report['passed']}`\n"
        out += f"- events: `{attention_event_trace_report['events']}`\n"
        out += f"- snapshots: `{attention_event_trace_report['snapshots']}`\n"
        out += f"- offload commits: `{attention_event_trace_report['offloads']}`\n"
        out += f"- restore commits: `{attention_event_trace_report['restores']}`\n"
        out += f"- attention uses: `{attention_event_trace_report['attention_uses']}`\n"
        out += f"- peak offloaded pages: `{attention_event_trace_report['peak_offloaded_pages']}`\n"
        out += f"- unfinished offloads: `{attention_event_trace_report['offloaded_pages_at_end']}`\n\n"
    if materialized_source_summary is not None and materialized_source_compare_json is not None:
        materialized_candidate = materialized_source_compare_json["candidate"]
        materialized_baseline = materialized_source_compare_json["baseline"]
        deep_events = (deep_materialized_source_summary or {}).get("events", "disabled")
        out += "## Attention Materialized Source\n\n"
        out += "- source: actual llama.cpp attention source path wrapped in `ggml_cont` materialization before attention consumption\n"
        out += "- check: generated-token output is compared against the native full-GPU attention source baseline\n"
        out += f"- short-run trace events: `{materialized_source_summary['events']}`\n"
        out += f"- deep-run trace events: `{deep_events}`\n"
        out += f"- layers: `{materialized_source_summary['layers']}` (`{materialized_source_summary['min_layer']}`..`{materialized_source_summary['max_layer']}`)\n"
        out += f"- kinds: `{','.join(materialized_source_summary['kinds'])}`\n"
        out += f"- native generated text hash: `{materialized_baseline['generated_text_hash']}`\n"
        out += f"- materialized generated text hash: `{materialized_candidate['generated_text_hash']}`\n"
        out += f"- native decode time: `{materialized_baseline['total_us']}` us\n"
        out += f"- materialized decode time: `{materialized_candidate['total_us']}` us\n"
        out += "- boundary: this proves attention can consume materialized K/V source tensors without output drift; it does not free the persistent KV allocation\n\n"
    if page_composed_source_summary is not None and page_composed_source_compare_json is not None:
        page_candidate = page_composed_source_compare_json["candidate"]
        page_baseline = page_composed_source_compare_json["baseline"]
        deep_events = (deep_page_composed_source_summary or {}).get("events", "")
        max_page_count = (deep_page_composed_source_summary or page_composed_source_summary).get("max_page_count", "")
        page_tokens = ",".join(str(value) for value in (deep_page_composed_source_summary or page_composed_source_summary).get("page_tokens_values", []))
        out += "## Attention Page-Composed Source\n\n"
        out += "- source: actual llama.cpp attention source path split into bounded token pages, materialized page-by-page, then composed with `ggml_concat` before attention consumption\n"
        if deep_page_composed_source_summary is not None:
            out += "- check: generated-token output is compared against the native full-GPU attention source baseline, and the deep run must compose more than one page\n"
        else:
            out += "- check: generated-token output is compared against the native full-GPU attention source baseline; the deep run is reserved for the stronger persistent page-source path when that hook is enabled\n"
        out += f"- short-run trace events: `{page_composed_source_summary['events']}`\n"
        out += f"- deep-run trace events: `{deep_events}`\n"
        out += f"- layers: `{page_composed_source_summary['layers']}` (`{page_composed_source_summary['min_layer']}`..`{page_composed_source_summary['max_layer']}`)\n"
        out += f"- kinds: `{','.join(page_composed_source_summary['kinds'])}`\n"
        out += f"- page tokens: `{page_tokens}`\n"
        out += f"- max page count: `{max_page_count}`\n"
        out += f"- native generated text hash: `{page_baseline['generated_text_hash']}`\n"
        out += f"- page-composed generated text hash: `{page_candidate['generated_text_hash']}`\n"
        out += f"- native decode time: `{page_baseline['total_us']}` us\n"
        out += f"- page-composed decode time: `{page_candidate['total_us']}` us\n"
        out += "- boundary: this proves attention can consume a page-composed K/V source without output drift; it still composes from the persistent KV allocation\n\n"
    if persistent_page_source_summary is not None and persistent_page_source_compare_json is not None:
        persistent_candidate = persistent_page_source_compare_json["candidate"]
        persistent_baseline = persistent_page_source_compare_json["baseline"]
        deep_events = (deep_persistent_page_source_summary or {}).get("events", "")
        max_page_count = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_page_count", "")
        max_retained_pages = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_retained_pages", "")
        max_requested_bytes = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_requested_bytes", "")
        max_allocated_bytes = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_allocated_bytes", "")
        max_retained_bytes_observed = (deep_persistent_page_source_summary or persistent_page_source_summary).get(
            "max_retained_bytes_observed",
            "",
        )
        max_source_bytes = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_source_bytes", "")
        max_retained_bytes = (deep_persistent_page_source_summary or persistent_page_source_summary).get("max_retained_bytes", "")
        backends = ",".join((deep_persistent_page_source_summary or persistent_page_source_summary).get("backends", []))
        page_tokens = ",".join(
            str(value) for value in (deep_persistent_page_source_summary or persistent_page_source_summary).get("page_tokens_values", [])
        )
        out += "## Attention Persistent Page Source\n\n"
        out += "- source: actual llama.cpp attention source path split into bounded token pages, graph-copied into independently allocated retained backend page tensors, then composed with `ggml_concat` before attention consumption\n"
        out += "- check: generated-token output is compared against the native full-GPU attention source baseline, and the deep run must compose more than one page\n"
        out += f"- short-run trace events: `{persistent_page_source_summary['events']}`\n"
        out += f"- deep-run trace events: `{deep_events}`\n"
        out += f"- backends: `{backends}`\n"
        out += f"- layers: `{persistent_page_source_summary['layers']}` (`{persistent_page_source_summary['min_layer']}`..`{persistent_page_source_summary['max_layer']}`)\n"
        out += f"- kinds: `{','.join(persistent_page_source_summary['kinds'])}`\n"
        out += f"- page tokens: `{page_tokens}`\n"
        out += f"- max page count: `{max_page_count}`\n"
        out += f"- max retained pages: `{max_retained_pages}`\n"
        out += f"- max requested bytes per source event: `{max_requested_bytes}` / `{max_source_bytes}` budget\n"
        out += f"- max allocated bytes per source event: `{max_allocated_bytes}`\n"
        out += f"- max retained bytes observed: `{max_retained_bytes_observed}` / `{max_retained_bytes}` budget\n"
        out += f"- native generated text hash: `{persistent_baseline['generated_text_hash']}`\n"
        out += f"- persistent-source generated text hash: `{persistent_candidate['generated_text_hash']}`\n"
        out += f"- native decode time: `{persistent_baseline['total_us']}` us\n"
        out += f"- persistent-source decode time: `{persistent_candidate['total_us']}` us\n"
        if args.gpu_page_staging:
            out += "- boundary: this proves attention can consume scheduler-resident accelerator page tensors while colder pages remain CPU-backed; it still uses concat-composed page sources rather than a final streaming attention kernel\n\n"
        else:
            out += "- boundary: this proves attention can consume retained backend page tensors filled through graph-native copies; it still sources those copies from the default persistent KV allocation, so it is not yet page-granular allocator reclaim\n\n"
    if deep_page_segments_summary is not None:
        out += "## Attention Page Segments\n\n"
        out += "- source: actual llama.cpp `get_k/get_v` attention graph-build path enumerated as bounded page tensors before page-source composition\n"
        if deep_page_segments_summary.get("attention_consumed") is True:
            consumers = ",".join(deep_page_segments_summary.get("consumers", []))
            consumer_set = set(deep_page_segments_summary.get("consumers", []))
            if "ggml_segmented_kqv" in consumer_set or "backend_scheduled_segmented_attention" in consumer_set:
                out += f"- boundary: this proves the runtime attention graph consumes bounded page segments through the accelerator-schedulable `{consumers}` path without concat-composed K/V page sources\n"
            elif "backend_scheduled_flattened_flash_attention" in consumer_set:
                out += f"- boundary: this proves the runtime attention graph consumes bounded page segments by flattening the eligible page table into llama.cpp's backend-scheduled Flash Attention path (`{consumers}`) without concat-composed K/V page sources\n"
            else:
                out += f"- boundary: this proves the runtime attention graph consumes bounded page segments through `{consumers}`; CPU custom-op consumers prove correctness rather than final accelerator performance\n"
        else:
            if deep_page_segments_summary.get("cold_attention_consumed") is True:
                consumers = ",".join(deep_page_segments_summary.get("consumers", []))
                out += f"- boundary: cold/offloaded page segments are consumed through `{consumers}` while resident rows stay on the stock fast path\n"
            else:
                out += "- boundary: this proves a segment API exists; it does not prove native attention consumes those segments yet\n"
        out += f"- events: `{deep_page_segments_summary['events']}`\n"
        out += f"- layers: `{deep_page_segments_summary['layers']}` (`{deep_page_segments_summary['min_layer']}`..`{deep_page_segments_summary['max_layer']}`)\n"
        out += f"- kinds: `{','.join(deep_page_segments_summary['kinds'])}`\n"
        out += f"- kind event counts: `{json.dumps(deep_page_segments_summary.get('kind_event_counts', {}), sort_keys=True)}`\n"
        out += f"- max segment count: `{deep_page_segments_summary['max_segment_count']}`\n"
        out += f"- max segment tokens: `{deep_page_segments_summary.get('max_segment_tokens', '')}`\n"
        out += f"- paired segment groups: `{deep_page_segments_summary.get('paired_segment_groups', '')}`\n"
        out += f"- segment bytes: `{deep_page_segments_summary['segment_bytes']}`\n"
        out += f"- composition values: `{','.join(deep_page_segments_summary.get('composition_values', []))}`\n"
        out += f"- native page streaming values: `{','.join(str(v).lower() for v in deep_page_segments_summary.get('native_page_streaming_values', []))}`\n"
        out += f"- attention consumed: `{deep_page_segments_summary.get('attention_consumed')}`\n"
        out += f"- consumers: `{','.join(deep_page_segments_summary.get('consumers', []))}`\n\n"
    if attention_equivalence is not None:
        out += "## Attention Equivalence\n\n"
        out += "- source: real llama.cpp query fixture plus exported K/V pages sliced by layer and head\n"
        out += "- check: QATQ page-bounded streaming attention versus materialised attention reference\n"
        out += f"- passed: `{attention_equivalence['passed']}`\n"
        out += f"- query dtype: `{attention_equivalence['query_dtype']}`\n"
        out += f"- page dtype: `{attention_equivalence['page_dtype']}`\n"
        out += f"- pages: `{attention_equivalence['pages']}`\n"
        out += f"- tokens: `{attention_equivalence['tokens']}`\n"
        out += f"- head dim: `{attention_equivalence['head_dim']}`\n"
        out += f"- max abs error: `{attention_equivalence['max_abs_error']:.9f}`\n"
        out += f"- max relative error: `{attention_equivalence['max_relative_error']:.9f}`\n"
        out += f"- peak page/materialised KV ratio: `{attention_equivalence['peak_page_kv_ratio']:.9f}`\n\n"
    if mlx_streaming_attention is not None:
        store = mlx_streaming_attention.get("qatq_store") or {}
        streaming = mlx_streaming_attention.get("streaming") or {}
        materialized = mlx_streaming_attention.get("materialized") or {}
        peak_ratio = streaming.get("max_peak_page_kv_ratio", streaming.get("peak_page_kv_ratio", 0.0))
        out += "## MLX Streaming Attention\n\n"
        out += "- source: MLX GPU page-bounded streaming attention over real llama.cpp K/V pages restored from a QATQ page store\n"
        out += "- check: all captured layers and heads stream page-by-page without using the llama.cpp concat-composed attention source\n"
        out += f"- passed: `{mlx_streaming_attention['passed']}`\n"
        out += f"- device: `{mlx_streaming_attention.get('device', '')}`\n"
        out += f"- layers checked: `{mlx_streaming_attention.get('layers_checked', 1)}`\n"
        out += f"- heads checked: `{mlx_streaming_attention.get('heads_checked', 1)}`\n"
        out += f"- max abs error: `{float(mlx_streaming_attention.get('max_abs_error', 0.0)):.9f}`\n"
        out += f"- max relative error: `{float(mlx_streaming_attention.get('max_relative_error', 0.0)):.9f}`\n"
        out += f"- peak page/materialised KV ratio: `{float(peak_ratio):.9f}`\n"
        out += f"- materialised seconds: `{float(materialized.get('seconds', 0.0)):.6f}`\n"
        out += f"- streaming seconds: `{float(streaming.get('seconds', 0.0)):.6f}`\n"
        out += f"- QATQ store pages: `{store.get('pages', '')}`\n"
        out += f"- QATQ store ratio: `{float(store.get('compression_ratio', 0.0)):.9f}`\n"
        out += f"- QATQ encode seconds: `{float(store.get('encode_seconds', 0.0)):.6f}`\n"
        out += f"- QATQ decode seconds: `{float(store.get('decode_seconds', 0.0)):.6f}`\n"
        if float(materialized.get("seconds", 0.0)) > 0.0:
            slowdown = float(streaming.get("seconds", 0.0)) / float(materialized.get("seconds", 0.0))
            out += f"- streaming/materialised time ratio: `{slowdown:.6f}`\n"
        out += "- boundary: this is an external MLX verifier for the non-concat page-bounded attention primitive; it does not replace llama.cpp's native attention graph by itself\n\n"
    out += "## Native Page-Streaming Status\n\n"
    out += f"- passed: `{native_page_streaming['passed']}`\n"
    out += f"- native runtime attention graph: `{native_page_streaming['native_runtime_attention_graph']}`\n"
    out += f"- accelerated runtime attention graph: `{native_page_streaming['accelerated_runtime_attention_graph']}`\n"
    out += f"- segmented graph bridge: `{native_page_streaming['segmented_graph_bridge']}`\n"
    out += (
        "- backend-scheduled segmented attention: `"
        + str(native_page_streaming["backend_scheduled_segmented_attention"])
        + "`\n"
    )
    out += (
        "- concat-composed page sources: `"
        + ",".join(native_page_streaming.get("concat_composed_page_sources", []))
        + "`\n"
    )
    out += (
        "- non-native page sources: `"
        + ",".join(native_page_streaming.get("non_native_page_sources", []))
        + "`\n"
    )
    out += (
        "- page-bounded attention equivalence passed: `"
        + str(native_page_streaming["page_bounded_attention_equivalence_passed"])
        + "`\n"
    )
    if native_page_streaming.get("page_bounded_attention_equivalence_max_abs_error") is not None:
        out += (
            "- page-bounded attention equivalence max abs error: `"
            + str(native_page_streaming["page_bounded_attention_equivalence_max_abs_error"])
            + "`\n"
        )
    if native_page_streaming.get("page_bounded_attention_equivalence_peak_page_kv_ratio") is not None:
        out += (
            "- page-bounded attention equivalence peak page/materialised KV ratio: `"
            + str(native_page_streaming["page_bounded_attention_equivalence_peak_page_kv_ratio"])
            + "`\n"
        )
    out += f"- external MLX streaming reference passed: `{native_page_streaming['external_mlx_streaming_attention_passed']}`\n"
    for failure in native_page_streaming.get("failures", []):
        out += f"- gap: {failure}\n"
    out += "\n"
    if attention_page_tensor_self_test is not None:
        out += "## Attention Page Tensor Self-Test\n\n"
        out += "- source: actual llama.cpp `get_k/get_v` attention path over real runtime K/V bytes\n"
        out += "- operation: materialise bounded K/V page bytes into separate non-host backend tensors, round-trip exact bytes, then free the temporary tensors\n"
        out += f"- events: `{attention_page_tensor_self_test['events']}`\n"
        out += f"- backends: `{','.join(attention_page_tensor_self_test['backends'])}`\n"
        out += f"- layers: `{attention_page_tensor_self_test['layers']}` (`{attention_page_tensor_self_test['min_layer']}`..`{attention_page_tensor_self_test['max_layer']}`)\n"
        out += f"- kinds: `{','.join(attention_page_tensor_self_test['kinds'])}`\n"
        out += f"- requested bytes: `{attention_page_tensor_self_test['requested_bytes']}`\n"
        out += f"- allocated bytes: `{attention_page_tensor_self_test['allocated_bytes']}`\n"
        out += "- boundary: this proves attention-path page tensor materialisation mechanics, not that the attention graph consumes those page tensors yet\n\n"
    if args.live_page_self_test_tokens > 0:
        out += "## Live Page Self-Test\n\n"
        out += "- source: real llama.cpp backend KV tensor storage after generation\n"
        out += "- operation: snapshot active key page bytes, overwrite with zeroes, verify the mutation, restore original bytes, and verify checksum\n"
        out += f"- requested page tokens: `{args.live_page_self_test_tokens}`\n"
        out += "- result: `pass`\n"
        out += "- boundary: this proves backend page mutation and restore mechanics, not per-page GPU allocation reclaim\n\n"
    if args.live_physical_page_alloc_self_test_tokens > 0:
        out += "## Live Physical Page Allocation Self-Test\n\n"
        out += "- source: real llama.cpp backend tensor allocator after generation\n"
        out += "- operation: allocate one page-sized non-host backend tensor on the same backend as an active key page, round-trip real KV bytes through it, and free it\n"
        out += f"- requested page tokens: `{args.live_physical_page_alloc_self_test_tokens}`\n"
        out += "- result: `pass`\n"
        out += "- boundary: this proves page-sized backend tensor storage mechanics, not attention-loop integration or sustained page-granular VRAM reclaim\n\n"
    if args.live_restore_slot_pressure_self_test_tokens > 0:
        out += "## Live Restore Slot Pressure Self-Test\n\n"
        out += "- source: real llama.cpp non-host backend tensor allocator after generation\n"
        out += "- operation: measure one active key page and require the configured restore-slot byte budget to reject it before allocation\n"
        out += f"- requested page tokens: `{args.live_restore_slot_pressure_self_test_tokens}`\n"
        out += f"- restore-slot byte ceiling: `{args.live_restore_slot_pressure_max_bytes}`\n"
        out += "- result: `pass`\n"
        out += "- boundary: this proves bounded resource-limit rejection in the runtime adapter path; it intentionally avoids inducing an unbounded device OOM\n\n"
    if persistent_page_pool is not None:
        out += "## Live Persistent Page Pool Self-Test\n\n"
        out += "- source: real llama.cpp backend tensor allocator after generation\n"
        out += "- operation: allocate, exact-byte verify, and retain a bounded pool of independent non-host K/V page tensors until llama_context teardown\n"
        out += f"- events: `{persistent_page_pool['events']}`\n"
        out += f"- backends: `{','.join(persistent_page_pool['backends'])}`\n"
        out += f"- layers: `{persistent_page_pool['layers']}` (`{persistent_page_pool['min_layer']}`..`{persistent_page_pool['max_layer']}`)\n"
        out += f"- kinds: `{','.join(persistent_page_pool['kinds'])}`\n"
        out += f"- requested bytes: `{persistent_page_pool['requested_bytes']}`\n"
        out += f"- allocated bytes: `{persistent_page_pool['allocated_bytes']}`\n"
        out += "- boundary: this proves persistent page-resident backend storage mechanics; it still coexists with, rather than replaces, the default whole-layer KV allocation\n\n"
    out += "## Size And Residency\n\n"
    out += "| metric | bytes | MiB |\n"
    out += "| --- | ---: | ---: |\n"
    out += f"| raw KV pages | {raw} | {mib(raw):.2f} |\n"
    out += f"| QATQ stored | {qatq} | {mib(qatq):.2f} |\n"
    out += f"| zstd | {zstd} | {mib(zstd):.2f} |\n"
    out += f"| lz4 | {lz4} | {mib(lz4):.2f} |\n"
    out += f"| GPU KV before | {before} | {mib(before):.2f} |\n"
    out += f"| GPU KV after | {after} | {mib(after):.2f} |\n"
    out += f"| reclaimable GPU KV | {reclaimed} | {mib(reclaimed):.2f} |\n\n"
    out += "## Claim Boundary\n\n"
    if args.skip_runtime_reclaim_gate:
        out += "- Observed: the manifest reported reduced persistent GPU K/V residency for this run, but the runtime reclaim gate was intentionally skipped by operator request.\n"
    elif args.gpu_page_staging and args.require_live_paging:
        out += "- Supported: page-staged runtime KV placement reduced persistent GPU K/V residency while preserving the deterministic continuation in this run.\n"
    elif args.gpu_page_staging:
        out += "- Supported: runtime-attested KV placement reduced persistent GPU K/V residency while preserving the deterministic continuation in this run.\n"
        out += "- Boundary: this is a latency-budget runtime-reclaim fallback, not a strict native page-streaming attention proof.\n"
    else:
        out += "- Supported: whole-tensor/layer-granularity runtime KV placement reduced GPU KV allocation while preserving the deterministic continuation in this run.\n"
    if evidence["qatq_beats_best_general_codec_pages"] == evidence["total_pages"]:
        out += "- Supported: QATQ replay restored every exported page exactly and beat raw, zstd, and lz4 on every exported page boundary.\n"
    else:
        out += (
            "- Supported: QATQ replay restored every exported page exactly and beat raw, zstd, and lz4 in aggregate; "
            f"`{evidence['qatq_beats_best_general_codec_pages']}/{evidence['total_pages']}` pages beat the best general codec under the current page policy.\n"
        )
    if args.require_live_paging:
        out += "- Supported: strict page-staging proof gate passed for this runtime adapter.\n"
    else:
        out += "- Not supported by this run: transparent live token-page eviction and restore inside the attention loop.\n"
        out += "- Use `--require-live-paging` as the stricter fail-closed gate for page-granular GPU reclaim.\n"
    if native_page_streaming.get("passed") is True:
        consumers = ",".join(native_page_streaming.get("runtime_attention_consumers", []))
        out += f"- Supported: the runtime attention graph consumed pages through a non-concat page-streaming path (`{consumers}`).\n"
        consumer_set = set(native_page_streaming.get("runtime_attention_consumers", []))
        if "ggml_segmented_kqv" in consumer_set or "backend_scheduled_segmented_attention" in consumer_set:
            out += "- Supported: the native consumer is the accelerator-schedulable ggml segmented KQ/V path.\n"
            if native_page_streaming.get("backend_scheduled_segmented_attention") is True:
                out += "- Supported: the backend-scheduled route uses retained tiled page-table tensors with explicit token-to-page/token-to-local tables; production readiness still depends on breadth, latency-tail, memory-pressure, and soak gates.\n"
        elif "backend_scheduled_flattened_flash_attention" in consumer_set:
            out += "- Supported: the native consumer flattens eligible bounded page tables into llama.cpp's backend-scheduled Flash Attention path.\n"
            out += "- Boundary: this avoids the custom segmented Metal kernel on eligible one-stream or stream-split multi-stream layouts; production readiness still depends on breadth, latency-tail, memory-pressure, and soak gates.\n"
        else:
            out += "- Boundary: CPU custom-op consumers prove exactness but remain a production performance blocker until replaced with an accelerator backend kernel.\n"
    else:
        out += "- Not supported: production native page-streaming attention inside llama.cpp.\n"
        out += "- Use `--require-native-page-streaming --native-page-streaming-attention` to fail closed until a runtime adapter consumes page segments without concat composition.\n"
    return out


def write_pages_csv(evidence: dict, output: Path) -> None:
    pages = evidence.get("pages")
    require(isinstance(pages, list), "evidence JSON is missing page rows")
    require(len(pages) == int(evidence.get("total_pages", -1)), "evidence page row count does not match total_pages")
    fields = [
        "page_index",
        "runtime_id",
        "runtime_commit",
        "adapter_version",
        "model_id",
        "seq_id",
        "layer_id",
        "kind",
        "dtype",
        "layout",
        "token_start",
        "token_end",
        "schedule",
        "keep_reason",
        "storage",
        "strategy",
        "raw_bytes",
        "qatq_candidate_bytes",
        "scheduled_stored_bytes",
        "zstd_bytes",
        "lz4_bytes",
        "verified_restore",
    ]
    with output.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        for page in pages:
            require(isinstance(page, dict), "evidence page row is not an object")
            require(page.get("verified_restore") is True, f"page {page.get('page_index')} was not verified")
            writer.writerow({field: page.get(field, "") for field in fields})


def write_tokens_csv(
    *,
    output_json: dict,
    cpu_output_json: dict | None,
    evidence: dict,
    selected: FrontierResult,
    timing_paths: dict[str, Path | None],
    attention_trace_summary: dict | None,
    attention_event_trace_report: dict | None,
    materialized_source_summary: dict | None,
    materialized_source_compare_json: dict | None,
    deep_materialized_source_summary: dict | None,
    page_composed_source_summary: dict | None,
    page_composed_source_compare_json: dict | None,
    deep_page_composed_source_summary: dict | None,
    persistent_page_source_summary: dict | None,
    persistent_page_source_compare_json: dict | None,
    deep_persistent_page_source_summary: dict | None,
    deep_page_segments_summary: dict | None,
    attention_equivalence: dict | None,
    mlx_streaming_attention: dict | None,
    native_page_streaming: dict,
    deep_output_compare_json: dict | None,
    attention_page_tensor_self_test: dict | None,
    persistent_page_pool: dict | None,
    output: Path,
) -> None:
    fields = [
        "row_type",
        "run_name",
        "generated_tokens",
        "total_us",
        "us_per_token",
        "selected_kv_gpu_layers",
        "gpu_saved_ratio",
        "reclaimable_gpu_bytes",
        "raw_bytes",
        "qatq_candidate_bytes",
        "zstd_bytes",
        "lz4_bytes",
        "exact_restores",
        "total_pages",
        "attention_read_events",
        "attention_event_trace_events",
        "attention_event_trace_failures",
        "attention_materialized_source_events",
        "attention_materialized_source_deep_events",
        "attention_materialized_source_output_passed",
        "attention_materialized_source_decode_us",
        "attention_page_composed_source_events",
        "attention_page_composed_source_deep_events",
        "attention_page_composed_source_output_passed",
        "attention_page_composed_source_decode_us",
        "attention_page_composed_source_max_page_count",
        "attention_page_composed_source_composition",
        "attention_page_composed_source_native_streaming",
        "attention_persistent_page_source_events",
        "attention_persistent_page_source_deep_events",
        "attention_persistent_page_source_output_passed",
        "attention_persistent_page_source_decode_us",
        "attention_persistent_page_source_backends",
        "attention_persistent_page_source_max_page_count",
        "attention_persistent_page_source_max_retained_pages",
        "attention_persistent_page_source_composition",
        "attention_persistent_page_source_native_streaming",
        "attention_page_segments_events",
        "attention_page_segments_layers",
        "attention_page_segments_max_segment_count",
        "attention_page_segments_max_segment_tokens",
        "attention_page_segments_paired_groups",
        "attention_page_segments_composition",
        "attention_page_segments_native_streaming",
        "attention_page_segments_attention_consumed",
        "attention_page_segments_consumers",
        "attention_equivalence_pages",
        "attention_equivalence_tokens",
        "attention_equivalence_max_abs_error",
        "attention_equivalence_peak_page_kv_ratio",
        "mlx_streaming_attention_passed",
        "mlx_streaming_attention_device",
        "mlx_streaming_attention_layers_checked",
        "mlx_streaming_attention_heads_checked",
        "mlx_streaming_attention_max_abs_error",
        "mlx_streaming_attention_peak_page_kv_ratio",
        "mlx_streaming_attention_qatq_pages",
        "mlx_streaming_attention_qatq_ratio",
        "mlx_streaming_attention_encode_seconds",
        "mlx_streaming_attention_decode_seconds",
        "mlx_streaming_attention_time_ratio",
        "native_page_streaming_passed",
        "native_page_streaming_accelerated",
        "native_page_streaming_graph_bridge",
        "native_page_streaming_backend_scheduled",
        "native_page_streaming_flattened_flash",
        "native_page_streaming_runtime_consumers",
        "native_page_streaming_concat_sources",
        "native_page_streaming_non_native_sources",
        "native_page_streaming_attention_equivalence_passed",
        "native_page_streaming_attention_equivalence_max_abs_error",
        "native_page_streaming_attention_equivalence_peak_page_kv_ratio",
        "native_page_streaming_failures",
        "deep_output_comparison_passed",
        "deep_output_comparison_decode_us",
        "attention_page_tensor_events",
        "attention_page_tensor_backends",
        "attention_page_tensor_requested_bytes",
        "attention_page_tensor_allocated_bytes",
        "persistent_page_pool_events",
        "persistent_page_pool_backends",
        "persistent_page_pool_requested_bytes",
        "persistent_page_pool_allocated_bytes",
        "decode_index",
        "n_pos_before",
        "n_pos_after",
        "batch_tokens",
        "decode_us",
        "generated_token",
    ]

    rows = []

    def add_manifest_row(row_type: str, run_name: str, manifest: dict) -> None:
        tokens = int(manifest.get("generated_token_count", 0))
        total_us = int(manifest.get("total_us", 0))
        rows.append(
            {
                "row_type": row_type,
                "run_name": run_name,
                "generated_tokens": tokens,
                "total_us": total_us,
                "us_per_token": f"{(total_us / tokens):.6f}" if tokens > 0 else "",
                "selected_kv_gpu_layers": selected.layers,
            }
        )

    add_manifest_row("decode-summary", "short-full-gpu", output_json["baseline"])
    add_manifest_row("decode-summary", "short-mixed-kv", output_json["candidate"])
    if cpu_output_json is not None:
        add_manifest_row("decode-summary", "short-cpu-kv", cpu_output_json["candidate"])

    residency = evidence.get("residency_estimate") or {}
    before = int(residency.get("gpu_context_bytes_before", 0))
    reclaimed = int(residency.get("reclaimable_gpu_bytes", 0))
    trace_failures = attention_event_trace_report.get("failures", []) if attention_event_trace_report else []
    mlx_store = (mlx_streaming_attention or {}).get("qatq_store") or {}
    mlx_streaming = (mlx_streaming_attention or {}).get("streaming") or {}
    mlx_materialized = (mlx_streaming_attention or {}).get("materialized") or {}
    mlx_peak_ratio = mlx_streaming.get("max_peak_page_kv_ratio", mlx_streaming.get("peak_page_kv_ratio", ""))
    mlx_materialized_seconds = float(mlx_materialized.get("seconds", 0.0) or 0.0)
    mlx_time_ratio = (
        float(mlx_streaming.get("seconds", 0.0) or 0.0) / mlx_materialized_seconds
        if mlx_materialized_seconds > 0.0
        else ""
    )
    rows.append(
        {
            "row_type": "evidence-summary",
            "run_name": "deep-mixed-kv",
            "selected_kv_gpu_layers": selected.layers,
            "gpu_saved_ratio": f"{(reclaimed / before):.9f}" if before > 0 else "",
            "reclaimable_gpu_bytes": reclaimed,
            "raw_bytes": evidence.get("raw_bytes", ""),
            "qatq_candidate_bytes": evidence.get("qatq_candidate_bytes", ""),
            "zstd_bytes": evidence.get("zstd_bytes", ""),
            "lz4_bytes": evidence.get("lz4_bytes", ""),
            "exact_restores": evidence.get("verified_restores", ""),
            "total_pages": evidence.get("total_pages", ""),
            "attention_read_events": (attention_trace_summary or {}).get("events", ""),
            "attention_event_trace_events": (attention_event_trace_report or {}).get("events", ""),
            "attention_event_trace_failures": len(trace_failures),
            "attention_materialized_source_events": (materialized_source_summary or {}).get("events", ""),
            "attention_materialized_source_deep_events": (deep_materialized_source_summary or {}).get("events", ""),
            "attention_materialized_source_output_passed": (materialized_source_compare_json or {}).get("passed", ""),
            "attention_materialized_source_decode_us": ((materialized_source_compare_json or {}).get("candidate") or {}).get("total_us", ""),
            "attention_page_composed_source_events": (page_composed_source_summary or {}).get("events", ""),
            "attention_page_composed_source_deep_events": (deep_page_composed_source_summary or {}).get("events", ""),
            "attention_page_composed_source_output_passed": (page_composed_source_compare_json or {}).get("passed", ""),
            "attention_page_composed_source_decode_us": ((page_composed_source_compare_json or {}).get("candidate") or {}).get("total_us", ""),
            "attention_page_composed_source_max_page_count": (deep_page_composed_source_summary or {}).get("max_page_count", ""),
            "attention_page_composed_source_composition": ",".join((deep_page_composed_source_summary or page_composed_source_summary or {}).get("composition_values", [])),
            "attention_page_composed_source_native_streaming": ",".join(str(v).lower() for v in (deep_page_composed_source_summary or page_composed_source_summary or {}).get("native_page_streaming_values", [])),
            "attention_persistent_page_source_events": (persistent_page_source_summary or {}).get("events", ""),
            "attention_persistent_page_source_deep_events": (deep_persistent_page_source_summary or {}).get("events", ""),
            "attention_persistent_page_source_output_passed": (persistent_page_source_compare_json or {}).get("passed", ""),
            "attention_persistent_page_source_decode_us": ((persistent_page_source_compare_json or {}).get("candidate") or {}).get("total_us", ""),
            "attention_persistent_page_source_backends": ",".join((deep_persistent_page_source_summary or persistent_page_source_summary or {}).get("backends", [])),
            "attention_persistent_page_source_max_page_count": (deep_persistent_page_source_summary or {}).get("max_page_count", ""),
            "attention_persistent_page_source_max_retained_pages": (deep_persistent_page_source_summary or {}).get("max_retained_pages", ""),
            "attention_persistent_page_source_composition": ",".join((deep_persistent_page_source_summary or persistent_page_source_summary or {}).get("composition_values", [])),
            "attention_persistent_page_source_native_streaming": ",".join(str(v).lower() for v in (deep_persistent_page_source_summary or persistent_page_source_summary or {}).get("native_page_streaming_values", [])),
            "attention_page_segments_events": (deep_page_segments_summary or {}).get("events", ""),
            "attention_page_segments_layers": (deep_page_segments_summary or {}).get("layers", ""),
            "attention_page_segments_max_segment_count": (deep_page_segments_summary or {}).get("max_segment_count", ""),
            "attention_page_segments_max_segment_tokens": (deep_page_segments_summary or {}).get("max_segment_tokens", ""),
            "attention_page_segments_paired_groups": (deep_page_segments_summary or {}).get("paired_segment_groups", ""),
            "attention_page_segments_composition": ",".join((deep_page_segments_summary or {}).get("composition_values", [])),
            "attention_page_segments_native_streaming": ",".join(str(v).lower() for v in (deep_page_segments_summary or {}).get("native_page_streaming_values", [])),
            "attention_page_segments_attention_consumed": (deep_page_segments_summary or {}).get("attention_consumed", ""),
            "attention_page_segments_consumers": ",".join((deep_page_segments_summary or {}).get("consumers", [])),
            "attention_equivalence_pages": (attention_equivalence or {}).get("pages", ""),
            "attention_equivalence_tokens": (attention_equivalence or {}).get("tokens", ""),
            "attention_equivalence_max_abs_error": (attention_equivalence or {}).get("max_abs_error", ""),
            "attention_equivalence_peak_page_kv_ratio": (attention_equivalence or {}).get("peak_page_kv_ratio", ""),
            "mlx_streaming_attention_passed": (mlx_streaming_attention or {}).get("passed", ""),
            "mlx_streaming_attention_device": (mlx_streaming_attention or {}).get("device", ""),
            "mlx_streaming_attention_layers_checked": (mlx_streaming_attention or {}).get("layers_checked", ""),
            "mlx_streaming_attention_heads_checked": (mlx_streaming_attention or {}).get("heads_checked", ""),
            "mlx_streaming_attention_max_abs_error": (mlx_streaming_attention or {}).get("max_abs_error", ""),
            "mlx_streaming_attention_peak_page_kv_ratio": mlx_peak_ratio,
            "mlx_streaming_attention_qatq_pages": mlx_store.get("pages", ""),
            "mlx_streaming_attention_qatq_ratio": mlx_store.get("compression_ratio", ""),
            "mlx_streaming_attention_encode_seconds": mlx_store.get("encode_seconds", ""),
            "mlx_streaming_attention_decode_seconds": mlx_store.get("decode_seconds", ""),
            "mlx_streaming_attention_time_ratio": mlx_time_ratio,
            "native_page_streaming_passed": native_page_streaming.get("passed", ""),
            "native_page_streaming_accelerated": native_page_streaming.get("accelerated_runtime_attention_graph", ""),
            "native_page_streaming_graph_bridge": native_page_streaming.get("segmented_graph_bridge", ""),
            "native_page_streaming_backend_scheduled": native_page_streaming.get("backend_scheduled_segmented_attention", ""),
            "native_page_streaming_flattened_flash": native_page_streaming.get("backend_scheduled_flattened_flash_attention", ""),
            "native_page_streaming_runtime_consumers": ",".join(native_page_streaming.get("runtime_attention_consumers", [])),
            "native_page_streaming_concat_sources": ",".join(native_page_streaming.get("concat_composed_page_sources", [])),
            "native_page_streaming_non_native_sources": ",".join(native_page_streaming.get("non_native_page_sources", [])),
            "native_page_streaming_attention_equivalence_passed": native_page_streaming.get("page_bounded_attention_equivalence_passed", ""),
            "native_page_streaming_attention_equivalence_max_abs_error": native_page_streaming.get("page_bounded_attention_equivalence_max_abs_error", ""),
            "native_page_streaming_attention_equivalence_peak_page_kv_ratio": native_page_streaming.get("page_bounded_attention_equivalence_peak_page_kv_ratio", ""),
            "native_page_streaming_failures": "; ".join(native_page_streaming.get("failures", [])),
            "deep_output_comparison_passed": (deep_output_compare_json or {}).get("passed", ""),
            "deep_output_comparison_decode_us": ((deep_output_compare_json or {}).get("candidate") or {}).get("total_us", ""),
            "attention_page_tensor_events": (attention_page_tensor_self_test or {}).get("events", ""),
            "attention_page_tensor_backends": ",".join((attention_page_tensor_self_test or {}).get("backends", [])),
            "attention_page_tensor_requested_bytes": (attention_page_tensor_self_test or {}).get("requested_bytes", ""),
            "attention_page_tensor_allocated_bytes": (attention_page_tensor_self_test or {}).get("allocated_bytes", ""),
            "persistent_page_pool_events": (persistent_page_pool or {}).get("events", ""),
            "persistent_page_pool_backends": ",".join((persistent_page_pool or {}).get("backends", [])),
            "persistent_page_pool_requested_bytes": (persistent_page_pool or {}).get("requested_bytes", ""),
            "persistent_page_pool_allocated_bytes": (persistent_page_pool or {}).get("allocated_bytes", ""),
        }
    )

    for run_name, path in timing_paths.items():
        if path is None:
            continue
        require(path.exists(), f"missing token timing CSV for {run_name}: {path}")
        with path.open("r", encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle)
            required = {"decode_index", "n_pos_before", "n_pos_after", "batch_tokens", "decode_us", "generated_token"}
            require(reader.fieldnames is not None and required.issubset(reader.fieldnames), f"{path} is missing token timing columns")
            for row in reader:
                rows.append(
                    {
                        "row_type": "decode-token",
                        "run_name": run_name,
                        "selected_kv_gpu_layers": selected.layers,
                        "decode_index": row.get("decode_index", ""),
                        "n_pos_before": row.get("n_pos_before", ""),
                        "n_pos_after": row.get("n_pos_after", ""),
                        "batch_tokens": row.get("batch_tokens", ""),
                        "decode_us": row.get("decode_us", ""),
                        "generated_token": row.get("generated_token", ""),
                    }
                )

    with output.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def ensure_kv_bench(root: Path, path: Path) -> Path:
    full = path if path.is_absolute() else root / path
    if full.exists() and kv_bench_supports_live_vram_trace(full, root):
        return full
    run(["cargo", "build", "--release", "--bin", "qatq-kv-bench"], cwd=root, timeout=240)
    require(full.exists(), f"qatq-kv-bench was not built at {full}")
    require(
        kv_bench_supports_live_vram_trace(full, root),
        f"qatq-kv-bench at {full} does not expose live VRAM event-trace flags after rebuild",
    )
    return full


def kv_bench_supports_live_vram_trace(path: Path, root: Path) -> bool:
    completed = run([str(path), "--help"], cwd=root, timeout=30, capture=True, allow_failure=True)
    help_text = completed.stdout + completed.stderr
    return (
        completed.returncode == 0
        and "--live-vram-event-trace" in help_text
        and "--live-vram-live-paging-gate" in help_text
        and "--live-vram-page-seal-key-hex" in help_text
        and "--live-vram-require-page-seals" in help_text
        and "--live-vram-event-trace-only" in help_text
        and "--attention-query" in help_text
    )


def require_file(value: str, flag: str) -> Path:
    require(bool(value), f"{flag} is required")
    path = Path(value)
    require(path.exists(), f"{flag} path does not exist: {path}")
    return path


def run(
    command: list[str],
    cwd: Path,
    timeout: int,
    capture: bool = False,
    allow_failure: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        capture_output=capture,
        timeout=timeout,
    )
    if completed.returncode != 0 and not allow_failure:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise SystemExit(f"command failed with exit code {completed.returncode}: {shell_join(command)}\n{detail}")
    return completed


def parse_layer_sweep(value: str, fallback: int) -> list[int]:
    if not value.strip():
        return [fallback]
    layers = []
    seen = set()
    for part in value.split(","):
        part = part.strip()
        require(part, "--sweep-kv-gpu-layers contains an empty entry")
        parsed = int(part)
        require(parsed >= 0, "--sweep-kv-gpu-layers values must be non-negative")
        if parsed not in seen:
            layers.append(parsed)
            seen.add(parsed)
    require(layers, "--sweep-kv-gpu-layers did not contain any values")
    return layers


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


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


def mib(value: int) -> float:
    return value / (1024 * 1024)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        raise SystemExit(130)
