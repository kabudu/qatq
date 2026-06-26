#!/usr/bin/env python3
"""Probe in-process llama-server request cancellation with QATQ live-VRAM tracing.

The earlier abort probe proves fail-closed behaviour when the whole llama-simple
process is interrupted after QATQ export. This probe exercises the more
production-shaped server path: start patched llama-server with QATQ native page
staging enabled through environment variables, open a streaming completion,
close the client connection mid-stream, then verify the same server process is
healthy and can serve a follow-up request.
"""

from __future__ import annotations

import argparse
import http.client
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from llama_cpp_live_vram_hardware_counters import parse_nvidia_smi_process_memory


READY_PATH = "/health"
COMPLETION_PATH = "/completion"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-server", default="/private/tmp/qatq-llama-live-work/build/bin/llama-server")
    parser.add_argument("--model", required=True)
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-server-cancel-probe")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--ctx-size", type=int, default=4096)
    parser.add_argument("--parallel-slots", type=int, default=1)
    parser.add_argument("--kv-unified", action="store_true")
    parser.add_argument("--gpu-layers", type=int, default=99)
    parser.add_argument("--kv-gpu-layers", type=int, default=4)
    parser.add_argument("--page-tokens", type=int, default=1024)
    parser.add_argument("--current-token", type=int, default=2048)
    parser.add_argument("--hot-window-tokens", type=int, default=32)
    parser.add_argument("--prefetch-window-tokens", type=int, default=32)
    parser.add_argument("--max-queued-pages", type=int, default=32)
    parser.add_argument(
        "--max-page-segments",
        type=int,
        default=0,
        help=(
            "Maximum page segments permitted per K/V tensor. Default 0 derives "
            "ceil(ctx_size / page_tokens) so smaller page sizes do not trip the "
            "adapter's fail-closed graph-object budget."
        ),
    )
    parser.add_argument(
        "--graph-extra-nodes",
        type=int,
        default=0,
        help=(
            "Extra llama.cpp graph nodes to reserve. Default 0 derives a safe "
            "floor from max-page-segments, matching the patched llama-simple "
            "native page-streaming reservation policy."
        ),
    )
    parser.add_argument("--cache-type-k", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--cache-type-v", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--model-id", default="qatq-live-vram-server-cancel-probe")
    parser.add_argument("--n-predict", type=int, default=512)
    parser.add_argument("--cancel-after-bytes", type=int, default=256)
    parser.add_argument(
        "--iterations",
        type=int,
        default=1,
        help=(
            "Number of cancellation/follow-up cycles to run against one "
            "llama-server process. Values above 1 provide a soak-shaped "
            "durability check instead of a single-cycle smoke test."
        ),
    )
    parser.add_argument(
        "--warmup-iterations",
        type=int,
        default=0,
        help=(
            "Number of cancellation/follow-up cycles to run before the measured "
            "iterations. Warmup cycles must pass, and their RSS growth is "
            "reported separately; steady-state RSS gates use the post-warmup "
            "baseline."
        ),
    )
    parser.add_argument(
        "--host-memory-pressure-mib",
        type=int,
        default=0,
        help=(
            "Allocate and touch this many MiB in the probe process while "
            "llama-server runs, matching the matrix runner's bounded host "
            "pressure convention."
        ),
    )
    parser.add_argument(
        "--max-server-rss-growth-mib",
        type=int,
        default=0,
        help=(
            "Fail if llama-server RSS grows by more than this many MiB between "
            "post-readiness sampling and final sampling. Default 0 disables "
            "the RSS growth gate."
        ),
    )
    parser.add_argument(
        "--max-rss-tail-growth-kib",
        type=int,
        default=0,
        help=(
            "Fail if the measured iteration RSS-after tail range exceeds this "
            "many KiB. This catches slow steady-state memory drift after "
            "warmup. Default 0 disables the tail-growth gate."
        ),
    )
    parser.add_argument(
        "--rss-tail-window",
        type=int,
        default=4,
        help=(
            "Number of measured iteration RSS-after samples to use for "
            "--max-rss-tail-growth-kib. Default 4."
        ),
    )
    parser.add_argument(
        "--max-retained-page-pool-mib",
        type=int,
        default=0,
        help=(
            "Maximum retained QATQ page-table pool bytes for the patched "
            "runtime, in MiB. Default 0 derives max(1024, "
            "--max-server-rss-growth-mib), keeping the llama.cpp retained "
            "pool budget aligned with the server RSS gate instead of leaving "
            "larger multi-stream models on the 1024 MiB runtime default."
        ),
    )
    parser.add_argument(
        "--max-iteration-seconds",
        type=float,
        default=0.0,
        help=(
            "Fail if any cancellation/follow-up iteration takes longer than "
            "this many seconds. Default 0 disables the iteration latency gate."
        ),
    )
    parser.add_argument(
        "--max-followup-seconds",
        type=float,
        default=0.0,
        help=(
            "Fail if any follow-up completion takes longer than this many "
            "seconds. Default 0 disables the follow-up latency gate."
        ),
    )
    parser.add_argument(
        "--sample-direct-peak-vram",
        action="store_true",
        help=(
            "Sample direct per-process GPU memory while llama-server runs. "
            "Currently supports NVIDIA nvidia-smi pid,used_memory sampling."
        ),
    )
    parser.add_argument(
        "--require-direct-peak-vram-counter",
        action="store_true",
        help=(
            "Fail unless --sample-direct-peak-vram captures at least one "
            "direct per-process GPU memory sample."
        ),
    )
    parser.add_argument(
        "--direct-peak-vram-sample-interval-ms",
        type=int,
        default=100,
        help="Sampling interval for --sample-direct-peak-vram. Default 100.",
    )
    parser.add_argument(
        "--concurrent-followup-during-cancel",
        action="store_true",
        help=(
            "Start a follow-up completion while the streaming request is still "
            "open, cancel the stream, and require the follow-up to complete "
            "after the cancellation. Use with --parallel-slots >= 2."
        ),
    )
    parser.add_argument(
        "--enable-event-trace",
        action="store_true",
        help=(
            "Also enable the checksum lifecycle event trace. Leave disabled "
            "for llama-server startup probes because llama.cpp may reserve "
            "graphs before backing tensor data is allocated."
        ),
    )
    parser.add_argument(
        "--disable-qatq-traces",
        action="store_true",
        help=(
            "Keep QATQ live-VRAM runtime environment variables enabled but "
            "disable QATQ JSONL trace-file outputs. This is for performance "
            "diagnosis only and cannot be combined with strict trace gates."
        ),
    )
    parser.add_argument(
        "--native-baseline",
        action="store_true",
        help=(
            "Run the same cancellation/follow-up probe without QATQ live-VRAM "
            "environment variables. This records a native llama-server latency "
            "and RSS baseline; it cannot be combined with QATQ trace gates."
        ),
    )
    parser.add_argument(
        "--require-flattened-flash-consumer",
        action="store_true",
        help=(
            "Fail unless attention-consumed page-segment rows are consumed by "
            "backend_scheduled_flattened_flash_attention. This hardens native "
            "multi-stream server evidence against silently falling back to a "
            "different attention path."
        ),
    )
    parser.add_argument(
        "--require-live-offloaded-stream-count",
        type=int,
        default=0,
        help=(
            "Fail unless at least this many distinct stream_index values appear "
            "on live-offloaded page segments. Use 2 for non-unified multi-stream "
            "server evidence."
        ),
    )
    parser.add_argument(
        "--require-backend-memory-diagnostics",
        action="store_true",
        help=(
            "Fail unless llama-server stderr includes projected device memory "
            "and at least one accelerator memory-breakdown row. This turns "
            "llama.cpp backend allocator diagnostics into an explicit "
            "evidence gate for strict Metal runs."
        ),
    )
    parser.add_argument("--startup-timeout", type=float, default=180.0)
    parser.add_argument("--request-timeout", type=float, default=120.0)
    parser.add_argument("--shutdown-timeout", type=float, default=20.0)
    parser.add_argument(
        "--prompt",
        default=(
            "You are validating a live LLM runtime migration engine. Explain "
            "the safety invariants, cancellation behaviour, exact KV restore "
            "requirements, compression accounting, and latency risks in detail. "
        ),
    )
    parser.add_argument("--prompt-repeat", type=int, default=80)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    validate_args(args)

    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    port = args.port if args.port or args.dry_run else choose_free_port(args.host)
    artifacts = build_artifacts(work_dir)
    command = build_command(args, port)
    derived = derive_resource_bounds(args)
    env = build_env(args, artifacts, derived)
    mode = probe_mode(args)
    plan = {
        "format": "qatq-live-vram-server-cancel-probe-plan-v1",
        "mode": mode,
        "command": command,
        "env": env,
        "derived": derived,
        "artifacts": {key: str(value) for key, value in artifacts.items()},
        "host": args.host,
        "port": port,
        "iterations": args.iterations,
        "warmup_iterations": args.warmup_iterations,
        "host_memory_pressure_mib": args.host_memory_pressure_mib,
        "max_server_rss_growth_mib": args.max_server_rss_growth_mib,
        "max_rss_tail_growth_kib": args.max_rss_tail_growth_kib,
        "rss_tail_window": args.rss_tail_window,
        "max_retained_page_pool_mib": derived["max_retained_page_pool_mib"],
        "max_iteration_seconds": args.max_iteration_seconds,
        "max_followup_seconds": args.max_followup_seconds,
        "qatq_traces_enabled": qatq_traces_enabled(args),
        "require_flattened_flash_consumer": args.require_flattened_flash_consumer,
        "require_live_offloaded_stream_count": args.require_live_offloaded_stream_count,
        "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
        "sample_direct_peak_vram": args.sample_direct_peak_vram,
        "require_direct_peak_vram_counter": args.require_direct_peak_vram_counter,
        "direct_peak_vram_sample_interval_ms": args.direct_peak_vram_sample_interval_ms,
        "dry_run": args.dry_run,
    }
    write_json(work_dir / "server-cancel-probe-plan.json", plan)

    if args.dry_run:
        summary = {
            "format": "qatq-live-vram-server-cancel-probe-summary-v1",
            "status": "dry-run",
            "mode": mode,
            "plan": str(work_dir / "server-cancel-probe-plan.json"),
            "command": command,
            "env": env,
            "derived": derived,
            "iterations": args.iterations,
            "warmup_iterations": args.warmup_iterations,
            "host_memory_pressure_mib": args.host_memory_pressure_mib,
            "max_server_rss_growth_mib": args.max_server_rss_growth_mib,
            "max_rss_tail_growth_kib": args.max_rss_tail_growth_kib,
            "rss_tail_window": args.rss_tail_window,
            "max_retained_page_pool_mib": derived["max_retained_page_pool_mib"],
            "max_iteration_seconds": args.max_iteration_seconds,
            "max_followup_seconds": args.max_followup_seconds,
            "qatq_traces_enabled": qatq_traces_enabled(args),
            "require_flattened_flash_consumer": args.require_flattened_flash_consumer,
            "require_live_offloaded_stream_count": args.require_live_offloaded_stream_count,
            "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
            "sample_direct_peak_vram": args.sample_direct_peak_vram,
            "require_direct_peak_vram_counter": args.require_direct_peak_vram_counter,
            "direct_peak_vram_sample_interval_ms": args.direct_peak_vram_sample_interval_ms,
            "artifacts": {key: str(value) for key, value in artifacts.items()},
            "boundary": "Dry-run plan only; no llama-server process was started.",
        }
        write_json(work_dir / "summary.json", summary)
        write_markdown(work_dir / "summary.md", summary)
        print((work_dir / "summary.md").read_text(encoding="utf-8"))
        return 0

    require(Path(args.llama_server).is_file(), f"missing llama-server: {args.llama_server}")
    require(Path(args.model).is_file(), f"missing model: {args.model}")

    stdout_path = work_dir / "server-stdout.log"
    stderr_path = work_dir / "server-stderr.log"
    proc: subprocess.Popen[bytes] | None = None
    stream_cancelled = False
    followup_ok = False
    health_after_cancel_ok = False
    followup_bytes = 0
    stream_bytes = 0
    concurrent_details: dict[str, object] = {}
    warmup_iterations: list[dict[str, object]] = []
    iterations: list[dict[str, object]] = []
    memory_samples: list[dict[str, object]] = []
    failures: list[str] = []
    host_pressure = allocate_host_memory_pressure(args.host_memory_pressure_mib)
    direct_peak_sampler: DirectPeakVramSampler | None = None
    direct_peak_vram_counter: dict[str, object] = {
        "enabled": args.sample_direct_peak_vram or args.require_direct_peak_vram_counter,
        "available": False,
        "backend": None,
        "reason": "direct peak-VRAM sampling was not requested",
    }

    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            proc = subprocess.Popen(
                command,
                env={**os.environ, **env},
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
            wait_for_server(args.host, port, proc, args.startup_timeout)
            memory_samples.append(sample_memory("post-readiness", proc.pid))
            if args.sample_direct_peak_vram or args.require_direct_peak_vram_counter:
                direct_peak_sampler = DirectPeakVramSampler(
                    proc.pid,
                    interval_ms=args.direct_peak_vram_sample_interval_ms,
                )
                direct_peak_sampler.start()
            for iteration in range(1, args.warmup_iterations + 1):
                result = run_cancel_iteration(args, port, iteration, proc.pid)
                result["phase"] = "warmup"
                warmup_iterations.append(result)
                failures.extend(
                    f"warmup iteration {iteration}: {failure}" for failure in result["failures"]
                )
                if is_fatal_request_shape_failure(result["failures"]):
                    break
                if not bool(result["health_after_cancel_ok"]):
                    break
            if args.warmup_iterations:
                memory_samples.append(sample_memory("post-warmup", proc.pid))
            if len(warmup_iterations) != args.warmup_iterations:
                failures.append(
                    f"completed {len(warmup_iterations)} warmup iterations; "
                    f"expected {args.warmup_iterations}"
                )
            if len(warmup_iterations) == args.warmup_iterations:
                for iteration in range(1, args.iterations + 1):
                    result = run_cancel_iteration(args, port, iteration, proc.pid)
                    result["phase"] = "measured"
                    iterations.append(result)
                    failures.extend(
                        f"iteration {iteration}: {failure}" for failure in result["failures"]
                    )
                    if is_fatal_request_shape_failure(result["failures"]):
                        break
                    if not bool(result["health_after_cancel_ok"]):
                        break
            memory_samples.append(sample_memory("post-iterations", proc.pid))
    except Exception as exc:
        failures.append(str(exc))
    finally:
        if host_pressure is not None:
            host_pressure[0] ^= 0xA5
        if direct_peak_sampler is not None:
            direct_peak_vram_counter = direct_peak_sampler.stop()
        returncode = stop_server(proc, args.shutdown_timeout)

    if args.require_direct_peak_vram_counter and not bool(direct_peak_vram_counter["available"]):
        failures.append(
            "direct peak-VRAM counter was required but unavailable: "
            f"{direct_peak_vram_counter.get('reason', 'unknown')}"
        )
    if len(iterations) != args.iterations:
        failures.append(
            f"completed {len(iterations)} cancellation iterations; expected {args.iterations}"
        )

    stream_cancelled = bool(iterations) and all(
        bool(iteration["stream_cancelled"]) for iteration in iterations
    )
    health_after_cancel_ok = bool(iterations) and all(
        bool(iteration["health_after_cancel_ok"]) for iteration in iterations
    )
    followup_ok = bool(iterations) and all(bool(iteration["followup_ok"]) for iteration in iterations)
    stream_bytes = sum(int(iteration["stream_bytes_before_cancel"]) for iteration in iterations)
    followup_bytes = sum(int(iteration["followup_bytes"]) for iteration in iterations)
    concurrent_details = next(
        (
            iteration["concurrent_details"]
            for iteration in reversed(iterations)
            if isinstance(iteration.get("concurrent_details"), dict)
            and iteration["concurrent_details"]
        ),
        {},
    )
    steady_state_memory_samples = measured_memory_samples(memory_samples, args.warmup_iterations > 0)
    memory_checks = evaluate_memory_samples(
        steady_state_memory_samples,
        args.max_server_rss_growth_mib,
        iterations=iterations,
        max_rss_tail_growth_kib=args.max_rss_tail_growth_kib,
        rss_tail_window=args.rss_tail_window,
    )
    warmup_memory_checks = (
        evaluate_memory_samples(
            warmup_memory_samples(memory_samples),
            0,
            iterations=warmup_iterations,
        )
        if args.warmup_iterations
        else {}
    )
    failures.extend(memory_checks["failures"])
    latency_checks = evaluate_latency_samples(
        iterations,
        max_iteration_seconds=args.max_iteration_seconds,
        max_followup_seconds=args.max_followup_seconds,
    )
    failures.extend(latency_checks["failures"])
    followup_completion_metrics = aggregate_followup_completion_metrics(iterations)
    backend_memory = parse_llama_cpp_backend_memory(stderr_path)
    if args.require_backend_memory_diagnostics:
        failures.extend(evaluate_backend_memory_diagnostics(backend_memory))

    checks = evaluate_result(
        artifacts=artifacts,
        stream_cancelled=stream_cancelled,
        stream_bytes=stream_bytes,
        health_after_cancel_ok=health_after_cancel_ok,
        followup_ok=followup_ok,
        followup_bytes=followup_bytes,
        process_returncode=returncode,
        startup_failures=failures,
        require_flattened_flash_consumer=args.require_flattened_flash_consumer,
        require_live_offloaded_stream_count=args.require_live_offloaded_stream_count,
        qatq_live_vram=not args.native_baseline,
        qatq_trace_evidence=qatq_traces_enabled(args),
    )
    summary = {
        "format": "qatq-live-vram-server-cancel-probe-summary-v1",
        "status": "pass" if not checks["failures"] else "fail",
        "mode": mode,
        "stream_cancelled": stream_cancelled,
        "stream_bytes_before_cancel": stream_bytes,
        "health_after_cancel_ok": health_after_cancel_ok,
        "followup_ok": followup_ok,
        "followup_bytes": followup_bytes,
        "iterations_requested": args.iterations,
        "iterations_completed": len(iterations),
        "warmup_iterations_requested": args.warmup_iterations,
        "warmup_iterations_completed": len(warmup_iterations),
        "warmup_iterations": warmup_iterations,
        "iterations": iterations,
        "host_memory_pressure_mib": args.host_memory_pressure_mib,
        "max_server_rss_growth_mib": args.max_server_rss_growth_mib,
        "max_rss_tail_growth_kib": args.max_rss_tail_growth_kib,
        "rss_tail_window": args.rss_tail_window,
        "max_retained_page_pool_mib": derived["max_retained_page_pool_mib"],
        "max_iteration_seconds": args.max_iteration_seconds,
        "max_followup_seconds": args.max_followup_seconds,
        "qatq_traces_enabled": qatq_traces_enabled(args),
        "require_flattened_flash_consumer": args.require_flattened_flash_consumer,
        "require_live_offloaded_stream_count": args.require_live_offloaded_stream_count,
        "require_backend_memory_diagnostics": args.require_backend_memory_diagnostics,
        "sample_direct_peak_vram": args.sample_direct_peak_vram,
        "require_direct_peak_vram_counter": args.require_direct_peak_vram_counter,
        "direct_peak_vram_sample_interval_ms": args.direct_peak_vram_sample_interval_ms,
        "direct_peak_vram_counter": direct_peak_vram_counter,
        "memory_samples": memory_samples,
        "memory_checks": memory_checks,
        "warmup_memory_checks": warmup_memory_checks,
        "latency_checks": latency_checks,
        "followup_completion_metrics": followup_completion_metrics,
        "backend_memory": backend_memory,
        "process_returncode": returncode,
        "stdout": str(stdout_path),
        "stderr": str(stderr_path),
        "artifacts": {key: str(value) for key, value in artifacts.items()},
        "checks": checks,
        "concurrent_details": concurrent_details,
        "command": command,
        "boundary": (
            (
                "Real in-process llama-server streaming client-disconnect "
                "native baseline evidence without QATQ live-VRAM environment "
                "variables. This is a performance comparison baseline, not a "
                "live-VRAM reduction proof."
            )
            if args.native_baseline
            else (
                "Real in-process llama-server streaming client-disconnect "
                + (
                    "evidence with QATQ live-VRAM tracing enabled. "
                    if qatq_traces_enabled(args)
                    else "performance-only evidence with QATQ live-VRAM enabled and trace files disabled. "
                )
                + "This does not replace broader multi-runtime adapter cancellation tests."
            )
        ),
    }
    write_json(work_dir / "summary.json", summary)
    write_markdown(work_dir / "summary.md", summary)
    print((work_dir / "summary.md").read_text(encoding="utf-8"))
    return 0 if summary["status"] == "pass" else 1


def validate_args(args: argparse.Namespace) -> None:
    require(args.ctx_size > 0, "--ctx-size must be positive")
    require(args.parallel_slots > 0, "--parallel-slots must be positive")
    require(
        not args.concurrent_followup_during_cancel or args.parallel_slots >= 2,
        "--concurrent-followup-during-cancel requires --parallel-slots >= 2",
    )
    require(args.gpu_layers >= 0, "--gpu-layers must be non-negative")
    require(args.kv_gpu_layers >= 0, "--kv-gpu-layers must be non-negative")
    require(args.page_tokens > 0, "--page-tokens must be positive")
    require(args.current_token >= 0, "--current-token must be non-negative")
    require(args.hot_window_tokens >= 0, "--hot-window-tokens must be non-negative")
    require(args.prefetch_window_tokens >= 0, "--prefetch-window-tokens must be non-negative")
    require(args.max_queued_pages >= 0, "--max-queued-pages must be non-negative")
    require(args.max_page_segments >= 0, "--max-page-segments must be non-negative")
    require(args.graph_extra_nodes >= 0, "--graph-extra-nodes must be non-negative")
    require(
        args.direct_peak_vram_sample_interval_ms > 0,
        "--direct-peak-vram-sample-interval-ms must be positive",
    )
    require(
        not args.require_direct_peak_vram_counter or args.sample_direct_peak_vram,
        "--require-direct-peak-vram-counter requires --sample-direct-peak-vram",
    )
    require(args.n_predict > 0, "--n-predict must be positive")
    require(args.cancel_after_bytes > 0, "--cancel-after-bytes must be positive")
    require(args.iterations > 0, "--iterations must be positive")
    require(args.warmup_iterations >= 0, "--warmup-iterations must be non-negative")
    require(args.host_memory_pressure_mib >= 0, "--host-memory-pressure-mib must be non-negative")
    require(args.host_memory_pressure_mib <= 8192, "--host-memory-pressure-mib is capped at 8192 MiB")
    require(args.max_server_rss_growth_mib >= 0, "--max-server-rss-growth-mib must be non-negative")
    require(args.max_rss_tail_growth_kib >= 0, "--max-rss-tail-growth-kib must be non-negative")
    require(args.rss_tail_window > 0, "--rss-tail-window must be positive")
    require(args.max_retained_page_pool_mib >= 0, "--max-retained-page-pool-mib must be non-negative")
    require(args.max_retained_page_pool_mib <= 8192, "--max-retained-page-pool-mib is capped at 8192 MiB")
    require(args.max_iteration_seconds >= 0.0, "--max-iteration-seconds must be non-negative")
    require(args.max_followup_seconds >= 0.0, "--max-followup-seconds must be non-negative")
    require(
        args.require_live_offloaded_stream_count >= 0,
        "--require-live-offloaded-stream-count must be non-negative",
    )
    require(
        not args.native_baseline or not args.enable_event_trace,
        "--native-baseline cannot enable QATQ event tracing",
    )
    require(
        not args.disable_qatq_traces or not args.native_baseline,
        "--disable-qatq-traces applies only to QATQ live-VRAM runs",
    )
    require(
        not args.disable_qatq_traces or not args.enable_event_trace,
        "--disable-qatq-traces cannot be combined with --enable-event-trace",
    )
    require(
        not args.disable_qatq_traces or not args.require_flattened_flash_consumer,
        "--disable-qatq-traces cannot require a flattened Flash consumer trace",
    )
    require(
        not args.disable_qatq_traces or args.require_live_offloaded_stream_count == 0,
        "--disable-qatq-traces cannot require live-offloaded QATQ stream indices",
    )
    require(
        not args.native_baseline or not args.require_flattened_flash_consumer,
        "--native-baseline cannot require a QATQ flattened Flash consumer trace",
    )
    require(
        not args.native_baseline or args.require_live_offloaded_stream_count == 0,
        "--native-baseline cannot require live-offloaded QATQ stream indices",
    )
    require(args.startup_timeout > 0, "--startup-timeout must be positive")
    require(args.request_timeout > 0, "--request-timeout must be positive")
    require(args.shutdown_timeout > 0, "--shutdown-timeout must be positive")
    require(args.prompt_repeat > 0, "--prompt-repeat must be positive")


def is_fatal_request_shape_failure(failures: list[object]) -> bool:
    """Stop soak loops when the request itself is deterministically invalid."""
    fatal_markers = (
        "streaming completion returned HTTP 400",
        "exceeds the available context size",
        "exceed_context_size_error",
    )
    return any(
        any(marker in str(failure) for marker in fatal_markers)
        for failure in failures
    )


def build_artifacts(work_dir: Path) -> dict[str, Path]:
    return {
        "event_trace": work_dir / "event-trace.jsonl",
        "page_segments": work_dir / "page-segments.jsonl",
        "persistent_page_source": work_dir / "persistent-page-source.jsonl",
        "persistent_pool": work_dir / "persistent-pool.jsonl",
    }


def build_command(args: argparse.Namespace, port: int) -> list[str]:
    command = [
        args.llama_server,
        "-m",
        args.model,
        "--host",
        args.host,
        "--port",
        str(port),
        "-ngl",
        str(args.gpu_layers),
        "-c",
        str(args.ctx_size),
        "-np",
        str(args.parallel_slots),
        "--slots",
        "--cache-type-k",
        args.cache_type_k,
        "--cache-type-v",
        args.cache_type_v,
        "--flash-attn",
        "on",
    ]
    if args.kv_unified:
        command.append("--kv-unified")
    return command


def derive_resource_bounds(args: argparse.Namespace) -> dict[str, int]:
    derived_page_segments = max(1, ceil_div(args.ctx_size, args.page_tokens))
    max_page_segments = args.max_page_segments if args.max_page_segments else derived_page_segments
    require(
        max_page_segments >= derived_page_segments,
        "--max-page-segments must be at least ceil(ctx-size / page-tokens)",
    )
    graph_floor = max(32768, max_page_segments * 2048)
    graph_extra_nodes = max(args.graph_extra_nodes, graph_floor)
    max_retained_page_pool_mib = args.max_retained_page_pool_mib or max(
        1024,
        args.max_server_rss_growth_mib,
    )
    return {
        "derived_page_segments": derived_page_segments,
        "max_page_segments": max_page_segments,
        "graph_extra_nodes": graph_extra_nodes,
        "max_retained_page_pool_mib": max_retained_page_pool_mib,
        "max_retained_page_pool_bytes": max_retained_page_pool_mib * 1024 * 1024,
    }


def ceil_div(numerator: int, denominator: int) -> int:
    return (numerator + denominator - 1) // denominator


def build_env(
    args: argparse.Namespace,
    artifacts: dict[str, Path],
    derived: dict[str, int],
) -> dict[str, str]:
    if args.native_baseline:
        return {}
    env = {
        **(
            {"LLAMA_QATQ_ATTENTION_EVENT_TRACE": str(artifacts["event_trace"])}
            if args.enable_event_trace
            else {}
        ),
        "LLAMA_QATQ_GPU_PAGE_STAGING": "1",
        "LLAMA_QATQ_KV_GPU_LAYERS": str(args.kv_gpu_layers),
        "LLAMA_QATQ_PAGE_TOKENS": str(args.page_tokens),
        "LLAMA_QATQ_TRACE_CURRENT_TOKEN": str(args.current_token),
        "LLAMA_QATQ_TRACE_HOT_WINDOW_TOKENS": str(args.hot_window_tokens),
        "LLAMA_QATQ_TRACE_PREFETCH_WINDOW_TOKENS": str(args.prefetch_window_tokens),
        "LLAMA_QATQ_TRACE_NEXT_REQUIRED": "cold-after-hot",
        "LLAMA_QATQ_TRACE_MAX_QUEUED_PAGES": str(args.max_queued_pages),
        "LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_MAX_PAGES": str(derived["max_page_segments"]),
        "LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_SOURCE_PAGES": str(derived["max_page_segments"]),
        "LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES": str(
            derived["max_retained_page_pool_bytes"]
        ),
        "LLAMA_QATQ_GRAPH_EXTRA_NODES": str(derived["graph_extra_nodes"]),
        "LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION": "1",
        "LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION_BACKEND": "backend-op",
        "LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH": "1",
        "LLAMA_QATQ_MODEL_ID": args.model_id,
    }
    if qatq_traces_enabled(args):
        env.update(
            {
                "LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_TRACE": str(artifacts["page_segments"]),
                "LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_TRACE": str(
                    artifacts["persistent_page_source"]
                ),
                "LLAMA_QATQ_LIVE_PERSISTENT_PAGE_POOL_TRACE": str(artifacts["persistent_pool"]),
            }
        )
    return env


def probe_mode(args: argparse.Namespace) -> str:
    return "native-baseline" if args.native_baseline else "qatq-live-vram"


def qatq_traces_enabled(args: argparse.Namespace) -> bool:
    return not args.native_baseline and not args.disable_qatq_traces


def choose_free_port(host: str) -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind((host, 0))
        return int(sock.getsockname()[1])


def wait_for_server(host: str, port: int, proc: subprocess.Popen[bytes], timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"llama-server exited before readiness: {proc.returncode}")
        if http_get_ok(host, port, READY_PATH, timeout=2.0):
            return
        time.sleep(0.5)
    raise RuntimeError("llama-server did not become healthy before timeout")


def http_get_ok(host: str, port: int, path: str, timeout: float) -> bool:
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    try:
        conn.request("GET", path)
        res = conn.getresponse()
        res.read()
        return 200 <= res.status < 300
    except OSError:
        return False
    finally:
        conn.close()


def run_cancel_iteration(
    args: argparse.Namespace,
    port: int,
    iteration: int,
    server_pid: int,
) -> dict[str, object]:
    stream_cancelled = False
    health_after_cancel_ok = False
    followup_ok = False
    stream_bytes = 0
    followup_bytes = 0
    concurrent_details: dict[str, object] = {}
    failures: list[str] = []
    iteration_started_at = time.monotonic()
    rss_before = sample_memory(f"iteration-{iteration}-before", server_pid)
    followup_duration_seconds: float | None = None
    followup_metrics: dict[str, float | int] = {}

    try:
        if args.concurrent_followup_during_cancel:
            concurrent_details = run_concurrent_cancel_and_followup(args, port)
            stream_bytes = int(concurrent_details["stream_bytes_before_cancel"])
            stream_cancelled = bool(concurrent_details["stream_cancelled"])
            followup_bytes = int(concurrent_details["followup_bytes"])
            followup_ok = bool(concurrent_details["followup_ok"])
            followup_duration_seconds = float(concurrent_details["followup_duration_seconds"])
            metrics = concurrent_details.get("followup_metrics")
            if isinstance(metrics, dict):
                followup_metrics = {
                    str(key): value
                    for key, value in metrics.items()
                    if isinstance(value, (float, int))
                }
            failures.extend(str(failure) for failure in concurrent_details["failures"])
        else:
            stream_bytes = open_stream_then_cancel(args, port)
            stream_cancelled = True
    except Exception as exc:
        failures.append(str(exc))

    time.sleep(1.0)
    health_after_cancel_ok = http_get_ok(args.host, port, READY_PATH, timeout=10.0)
    if not args.concurrent_followup_during_cancel:
        try:
            followup_started_at = time.monotonic()
            followup_completion = run_followup_completion(args, port)
            followup_duration_seconds = time.monotonic() - followup_started_at
            followup_bytes = int(followup_completion["bytes"])
            followup_metrics = dict(followup_completion["metrics"])
            followup_ok = followup_bytes > 0
        except Exception as exc:
            failures.append(str(exc))

    if not stream_cancelled:
        failures.append("streaming request was not cancelled")
    if stream_bytes <= 0:
        failures.append("no streaming bytes were observed before cancellation")
    if not health_after_cancel_ok:
        failures.append("server health check failed after streaming cancellation")
    if not followup_ok:
        failures.append("server did not complete a follow-up request after cancellation")
    rss_after = sample_memory(f"iteration-{iteration}-after", server_pid)
    iteration_completed_at = time.monotonic()

    return {
        "iteration": iteration,
        "started_at": iteration_started_at,
        "completed_at": iteration_completed_at,
        "duration_seconds": iteration_completed_at - iteration_started_at,
        "stream_cancelled": stream_cancelled,
        "stream_bytes_before_cancel": stream_bytes,
        "health_after_cancel_ok": health_after_cancel_ok,
        "followup_ok": followup_ok,
        "followup_bytes": followup_bytes,
        "followup_duration_seconds": followup_duration_seconds,
        "followup_metrics": followup_metrics,
        "rss_before_kib": rss_before["rss_kib"],
        "rss_after_kib": rss_after["rss_kib"],
        "concurrent_details": concurrent_details,
        "failures": failures,
    }


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


class DirectPeakVramSampler:
    def __init__(self, pid: int, *, interval_ms: int) -> None:
        self.pid = pid
        self.interval_seconds = max(0.01, interval_ms / 1000.0)
        self.path = shutil.which("nvidia-smi")
        self.samples: list[int] = []
        self.errors: list[str] = []
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._start_monotonic: float | None = None
        self._end_monotonic: float | None = None
        self._capability_reason = "direct peak-VRAM sampling has not started"

    def start(self) -> None:
        self._start_monotonic = time.monotonic()
        if not self.path:
            self._capability_reason = "nvidia-smi is not available on PATH"
            return
        if not self._nvidia_smi_supports_process_memory():
            return
        self._thread = threading.Thread(target=self._run, name="qatq-direct-peak-vram", daemon=True)
        self._thread.start()

    def stop(self) -> dict[str, object]:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=max(1.0, self.interval_seconds * 2.0))
        self._end_monotonic = time.monotonic()
        peak = max(self.samples) if self.samples else None
        available = peak is not None
        return {
            "enabled": True,
            "available": available,
            "backend": "nvidia-smi",
            "sample_pid": self.pid,
            "sample_interval_ms": int(self.interval_seconds * 1000),
            "started_at": self._start_monotonic,
            "completed_at": self._end_monotonic,
            "duration_seconds": (
                self._end_monotonic - self._start_monotonic
                if self._start_monotonic is not None and self._end_monotonic is not None
                else None
            ),
            "samples": self.samples,
            "sample_count": len(self.samples),
            "peak_memory_mib": peak,
            "errors": self.errors[:5],
            "reason": (
                "sampled per-process GPU memory with nvidia-smi"
                if available
                else self._capability_reason
            ),
        }

    def _nvidia_smi_supports_process_memory(self) -> bool:
        assert self.path is not None
        try:
            result = subprocess.run(
                [self.path, "--help-query-compute-apps"],
                check=False,
                text=True,
                capture_output=True,
                timeout=5.0,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            self._capability_reason = f"nvidia-smi capability probe failed: {exc}"
            return False
        help_text = result.stdout + result.stderr
        supported = "pid" in help_text and "used_memory" in help_text
        if not supported:
            self._capability_reason = (
                "nvidia-smi is present but does not advertise pid,used_memory "
                "compute-app queries"
            )
        return supported

    def _run(self) -> None:
        assert self.path is not None
        while not self._stop.is_set():
            try:
                result = subprocess.run(
                    [
                        self.path,
                        "--query-compute-apps=pid,used_memory",
                        "--format=csv,noheader,nounits",
                    ],
                    check=False,
                    text=True,
                    capture_output=True,
                    timeout=5.0,
                )
            except (OSError, subprocess.TimeoutExpired) as exc:
                self.errors.append(str(exc))
                self._stop.wait(self.interval_seconds)
                continue
            if result.returncode == 0:
                values = parse_nvidia_smi_process_memory(result.stdout, self.pid)
                self.samples.extend(values)
                if values:
                    self._capability_reason = "sampled per-process GPU memory with nvidia-smi"
            else:
                text = (result.stderr or result.stdout).strip()
                self.errors.append(text.splitlines()[0] if text else f"return code {result.returncode}")
            self._stop.wait(self.interval_seconds)


def sample_memory(label: str, pid: int) -> dict[str, object]:
    rss_kib = read_process_rss_kib(pid)
    return {
        "label": label,
        "timestamp_monotonic": time.monotonic(),
        "pid": pid,
        "rss_kib": rss_kib,
    }


def measured_memory_samples(
    samples: list[dict[str, object]],
    has_warmup: bool,
) -> list[dict[str, object]]:
    if not has_warmup:
        return list(samples)
    selected: list[dict[str, object]] = []
    for sample in samples:
        label = sample.get("label")
        if label == "post-warmup" or label == "post-iterations":
            selected.append(sample)
    return selected


def warmup_memory_samples(samples: list[dict[str, object]]) -> list[dict[str, object]]:
    selected: list[dict[str, object]] = []
    for sample in samples:
        label = sample.get("label")
        if label == "post-readiness" or label == "post-warmup":
            selected.append(sample)
    return selected


def read_process_rss_kib(pid: int) -> int | None:
    try:
        completed = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)],
            check=False,
            text=True,
            capture_output=True,
            timeout=5.0,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None
    text = completed.stdout.strip()
    if not text:
        return None
    try:
        return int(text.splitlines()[-1].strip())
    except ValueError:
        return None


def evaluate_memory_samples(
    samples: list[dict[str, object]],
    max_rss_growth_mib: int,
    *,
    iterations: list[dict[str, object]] | None = None,
    max_rss_tail_growth_kib: int = 0,
    rss_tail_window: int = 4,
) -> dict[str, object]:
    rss_values = [sample.get("rss_kib") for sample in samples]
    numeric = [int(value) for value in rss_values if isinstance(value, int)]
    rss_after_values: list[int] = []
    if iterations is not None:
        for iteration in iterations:
            for key in ("rss_before_kib", "rss_after_kib"):
                value = iteration.get(key)
                if isinstance(value, int):
                    numeric.append(value)
            after = iteration.get("rss_after_kib")
            if isinstance(after, int):
                rss_after_values.append(after)
    failures: list[str] = []
    if max_rss_growth_mib > 0 and len(numeric) < 2:
        failures.append("server RSS growth gate requested but fewer than two RSS samples were available")
    baseline = numeric[0] if numeric else None
    peak = max(numeric) if numeric else None
    growth_kib = (peak - baseline) if baseline is not None and peak is not None else None
    rss_after_last_minus_first_kib = (
        rss_after_values[-1] - rss_after_values[0] if len(rss_after_values) >= 2 else None
    )
    tail_window_used = min(rss_tail_window, len(rss_after_values)) if rss_tail_window > 0 else 0
    tail_values = rss_after_values[-tail_window_used:] if tail_window_used else []
    rss_tail_range_kib = (max(tail_values) - min(tail_values)) if tail_values else None
    rss_tail_last_minus_first_kib = (
        tail_values[-1] - tail_values[0] if len(tail_values) >= 2 else None
    )
    rss_tail_growth_kib = (
        max(0, rss_tail_last_minus_first_kib)
        if rss_tail_last_minus_first_kib is not None
        else None
    )
    rss_tail_gate_growth_kib = None
    if len(tail_values) >= 2 and rss_after_values:
        tail_gate_baseline = max(tail_values[0], rss_after_values[0])
        rss_tail_gate_growth_kib = max(0, tail_values[-1] - tail_gate_baseline)
    if (
        max_rss_growth_mib > 0
        and growth_kib is not None
        and growth_kib > max_rss_growth_mib * 1024
    ):
        failures.append(
            "server RSS grew by "
            f"{growth_kib / 1024:.2f} MiB, exceeding {max_rss_growth_mib} MiB"
        )
    if max_rss_tail_growth_kib > 0:
        if rss_tail_window <= 0:
            failures.append("server RSS tail gate requested with a non-positive tail window")
        elif len(rss_after_values) < rss_tail_window:
            failures.append(
                "server RSS tail gate requested but only "
                f"{len(rss_after_values)} RSS-after iteration samples were available; "
                f"{rss_tail_window} are required"
            )
        elif (
            rss_tail_gate_growth_kib is not None
            and rss_tail_gate_growth_kib > max_rss_tail_growth_kib
        ):
            failures.append(
                "server steady RSS tail growth was "
                f"{rss_tail_gate_growth_kib} KiB, exceeding {max_rss_tail_growth_kib} KiB "
                f"over the last {rss_tail_window} measured iterations"
            )
    return {
        "failures": failures,
        "sample_count": len(samples),
        "rss_sample_count": len(numeric),
        "baseline_rss_kib": baseline,
        "peak_rss_kib": peak,
        "growth_kib": growth_kib,
        "max_server_rss_growth_mib": max_rss_growth_mib,
        "rss_after_sample_count": len(rss_after_values),
        "rss_after_last_minus_first_kib": rss_after_last_minus_first_kib,
        "rss_tail_window": rss_tail_window,
        "rss_tail_window_used": tail_window_used,
        "rss_tail_range_kib": rss_tail_range_kib,
        "rss_tail_last_minus_first_kib": rss_tail_last_minus_first_kib,
        "rss_tail_growth_kib": rss_tail_growth_kib,
        "rss_tail_gate_growth_kib": rss_tail_gate_growth_kib,
        "max_rss_tail_growth_kib": max_rss_tail_growth_kib,
    }


def evaluate_latency_samples(
    iterations: list[dict[str, object]],
    *,
    max_iteration_seconds: float,
    max_followup_seconds: float,
) -> dict[str, object]:
    iteration_durations = [
        float(iteration["duration_seconds"])
        for iteration in iterations
        if isinstance(iteration.get("duration_seconds"), (float, int))
    ]
    followup_durations = [
        float(iteration["followup_duration_seconds"])
        for iteration in iterations
        if isinstance(iteration.get("followup_duration_seconds"), (float, int))
    ]
    failures: list[str] = []
    if max_iteration_seconds > 0.0:
        for iteration in iterations:
            duration = iteration.get("duration_seconds")
            if isinstance(duration, (float, int)) and float(duration) > max_iteration_seconds:
                failures.append(
                    "iteration "
                    f"{iteration.get('iteration', '?')} took {float(duration):.3f}s, "
                    f"exceeding {max_iteration_seconds:.3f}s"
                )
        if len(iteration_durations) != len(iterations):
            failures.append("iteration latency gate requested but some iterations lack duration samples")
    if max_followup_seconds > 0.0:
        for iteration in iterations:
            duration = iteration.get("followup_duration_seconds")
            if isinstance(duration, (float, int)) and float(duration) > max_followup_seconds:
                failures.append(
                    "follow-up for iteration "
                    f"{iteration.get('iteration', '?')} took {float(duration):.3f}s, "
                    f"exceeding {max_followup_seconds:.3f}s"
                )
        if len(followup_durations) != len(iterations):
            failures.append("follow-up latency gate requested but some iterations lack duration samples")
    return {
        "failures": failures,
        "iteration_count": len(iterations),
        "iteration_duration_seconds": latency_stats(iteration_durations),
        "followup_duration_seconds": latency_stats(followup_durations),
        "max_iteration_seconds": max_iteration_seconds,
        "max_followup_seconds": max_followup_seconds,
    }


def aggregate_followup_completion_metrics(
    iterations: list[dict[str, object]],
) -> dict[str, dict[str, float | int | None]]:
    samples_by_key: dict[str, list[float]] = {}
    for iteration in iterations:
        metrics = iteration.get("followup_metrics")
        if not isinstance(metrics, dict):
            continue
        for key, value in metrics.items():
            if isinstance(key, str) and isinstance(value, (float, int)):
                samples_by_key.setdefault(key, []).append(float(value))
    return {
        key: latency_stats(samples)
        for key, samples in sorted(samples_by_key.items())
    }


def latency_stats(samples: list[float]) -> dict[str, float | int | None]:
    if not samples:
        return {
            "count": 0,
            "min": None,
            "max": None,
            "p05": None,
            "p50": None,
            "p95": None,
            "p99": None,
        }
    ordered = sorted(samples)
    return {
        "count": len(ordered),
        "min": ordered[0],
        "max": ordered[-1],
        "p05": percentile_nearest_rank(ordered, 0.05),
        "p50": percentile_nearest_rank(ordered, 0.50),
        "p95": percentile_nearest_rank(ordered, 0.95),
        "p99": percentile_nearest_rank(ordered, 0.99),
    }


def percentile_nearest_rank(ordered: list[float], quantile: float) -> float:
    index = max(0, min(len(ordered) - 1, int(round((len(ordered) - 1) * quantile))))
    return ordered[index]


def open_stream_then_cancel(args: argparse.Namespace, port: int) -> int:
    body = json.dumps(
        {
            "prompt": args.prompt * args.prompt_repeat,
            "n_predict": args.n_predict,
            "temperature": 0,
            "stream": True,
            "cache_prompt": False,
        }
    )
    conn = http.client.HTTPConnection(args.host, port, timeout=args.request_timeout)
    try:
        conn.request(
            "POST",
            COMPLETION_PATH,
            body=body,
            headers={"Content-Type": "application/json"},
        )
        res = conn.getresponse()
        if res.status < 200 or res.status >= 300:
            raise RuntimeError(f"streaming completion returned HTTP {res.status}: {res.read(4096)!r}")
        received = 0
        deadline = time.monotonic() + args.request_timeout
        while received < args.cancel_after_bytes and time.monotonic() < deadline:
            chunk = res.read(min(128, args.cancel_after_bytes - received))
            if not chunk:
                break
            received += len(chunk)
        if received == 0:
            raise RuntimeError("streaming completion produced no bytes before cancellation")
        return received
    finally:
        conn.close()


def run_concurrent_cancel_and_followup(args: argparse.Namespace, port: int) -> dict[str, object]:
    body = json.dumps(
        {
            "prompt": args.prompt * args.prompt_repeat,
            "n_predict": args.n_predict,
            "temperature": 0,
            "stream": True,
            "cache_prompt": False,
        }
    )
    conn = http.client.HTTPConnection(args.host, port, timeout=args.request_timeout)
    failures: list[str] = []
    followup: dict[str, object] = {
        "followup_bytes": 0,
        "followup_metrics": {},
        "followup_ok": False,
        "followup_error": "",
        "followup_started_at": 0.0,
        "followup_completed_at": 0.0,
    }
    stream_cancelled = False
    stream_bytes = 0
    cancel_at = 0.0

    def run_followup() -> None:
        followup["followup_started_at"] = time.monotonic()
        try:
            completion = run_followup_completion(
                args,
                port,
                prompt=(
                    "While another stream is being cancelled, write a concise "
                    "operator health note about live KV page restore safety."
                ),
                n_predict=128,
            )
            followup["followup_bytes"] = int(completion["bytes"])
            followup["followup_metrics"] = dict(completion["metrics"])
            followup["followup_ok"] = int(followup["followup_bytes"]) > 0
        except Exception as exc:  # pragma: no cover - exercised by integration runs
            followup["followup_error"] = str(exc)
        finally:
            followup["followup_completed_at"] = time.monotonic()

    try:
        conn.request(
            "POST",
            COMPLETION_PATH,
            body=body,
            headers={"Content-Type": "application/json"},
        )
        res = conn.getresponse()
        if res.status < 200 or res.status >= 300:
            raise RuntimeError(f"streaming completion returned HTTP {res.status}: {res.read(4096)!r}")

        deadline = time.monotonic() + args.request_timeout
        while stream_bytes < args.cancel_after_bytes and time.monotonic() < deadline:
            chunk = res.read(min(128, args.cancel_after_bytes - stream_bytes))
            if not chunk:
                break
            stream_bytes += len(chunk)
        if stream_bytes == 0:
            raise RuntimeError("streaming completion produced no bytes before cancellation")

        thread = threading.Thread(target=run_followup, name="qatq-followup-during-cancel")
        thread.start()
        time.sleep(0.05)
        cancel_at = time.monotonic()
        conn.close()
        stream_cancelled = True
        thread.join(timeout=args.request_timeout)
        if thread.is_alive():
            failures.append("concurrent follow-up did not finish before request timeout")
        if followup["followup_error"]:
            failures.append(f"concurrent follow-up failed: {followup['followup_error']}")
        if not followup["followup_ok"]:
            failures.append("concurrent follow-up returned no bytes")
        if float(followup["followup_completed_at"]) <= cancel_at:
            failures.append("concurrent follow-up completed before stream cancellation; overlap was not proven")
    finally:
        conn.close()

    return {
        "stream_cancelled": stream_cancelled,
        "stream_bytes_before_cancel": stream_bytes,
        "cancel_at": cancel_at,
        "followup_bytes": int(followup["followup_bytes"]),
        "followup_metrics": followup["followup_metrics"],
        "followup_ok": bool(followup["followup_ok"]),
        "followup_started_at": followup["followup_started_at"],
        "followup_completed_at": followup["followup_completed_at"],
        "followup_duration_seconds": float(followup["followup_completed_at"])
        - float(followup["followup_started_at"]),
        "cancel_to_followup_completed_seconds": float(followup["followup_completed_at"])
        - cancel_at,
        "failures": failures,
    }


def run_followup_completion(
    args: argparse.Namespace,
    port: int,
    *,
    prompt: str = "Return exactly the word healthy.",
    n_predict: int = 8,
) -> dict[str, object]:
    body = json.dumps(
        {
            "prompt": prompt,
            "n_predict": n_predict,
            "temperature": 0,
            "stream": False,
            "cache_prompt": False,
        }
    )
    conn = http.client.HTTPConnection(args.host, port, timeout=args.request_timeout)
    try:
        conn.request(
            "POST",
            COMPLETION_PATH,
            body=body,
            headers={"Content-Type": "application/json"},
        )
        res = conn.getresponse()
        payload = res.read()
        if res.status < 200 or res.status >= 300:
            raise RuntimeError(f"follow-up completion returned HTTP {res.status}: {payload[:4096]!r}")
        return {
            "bytes": len(payload),
            "metrics": extract_completion_metrics(payload),
        }
    finally:
        conn.close()


def extract_completion_metrics(payload: bytes) -> dict[str, float | int]:
    try:
        decoded = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return {}
    if not isinstance(decoded, dict):
        return {}

    timings = decoded.get("timings")
    if not isinstance(timings, dict):
        timings = {}

    metrics: dict[str, float | int] = {}
    aliases = {
        "prompt_tokens": [
            ("timings", "prompt_n"),
            ("prompt_n",),
            ("tokens_evaluated",),
            ("prompt_tokens",),
        ],
        "predicted_tokens": [
            ("timings", "predicted_n"),
            ("predicted_n",),
            ("tokens_predicted",),
            ("completion_tokens",),
        ],
        "prompt_ms": [("timings", "prompt_ms"), ("prompt_ms",)],
        "predicted_ms": [("timings", "predicted_ms"), ("predicted_ms",)],
        "prompt_per_second": [
            ("timings", "prompt_per_second"),
            ("prompt_per_second",),
        ],
        "predicted_per_second": [
            ("timings", "predicted_per_second"),
            ("predicted_per_second",),
            ("tokens_per_second",),
        ],
    }
    for metric_key, paths in aliases.items():
        value = first_numeric_path(decoded, paths)
        if value is not None:
            metrics[metric_key] = value
    return metrics


def first_numeric_path(
    payload: dict[str, object],
    paths: list[tuple[str, ...]],
) -> float | int | None:
    for path in paths:
        cursor: object = payload
        for key in path:
            if not isinstance(cursor, dict):
                cursor = None
                break
            cursor = cursor.get(key)
        if isinstance(cursor, bool):
            continue
        if isinstance(cursor, int):
            return cursor
        if isinstance(cursor, float):
            return cursor
    return None


def stop_server(proc: subprocess.Popen[bytes] | None, timeout: float) -> int | None:
    if proc is None:
        return None
    if proc.poll() is not None:
        return proc.returncode
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        return proc.poll()
    except PermissionError:
        return proc.poll()
    try:
        return proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        try:
            return proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            return None


def evaluate_result(
    artifacts: dict[str, Path],
    stream_cancelled: bool,
    stream_bytes: int,
    health_after_cancel_ok: bool,
    followup_ok: bool,
    followup_bytes: int,
    process_returncode: int | None,
    startup_failures: list[str],
    *,
    require_flattened_flash_consumer: bool = False,
    require_live_offloaded_stream_count: int = 0,
    qatq_live_vram: bool = True,
    qatq_trace_evidence: bool = True,
) -> dict[str, object]:
    failures = list(startup_failures)
    event_trace = artifacts["event_trace"]
    page_segments = artifacts["page_segments"]
    persistent_page_source = artifacts["persistent_page_source"]
    persistent_pool = artifacts["persistent_pool"]

    if not stream_cancelled:
        failures.append("streaming request was not cancelled")
    if stream_bytes <= 0:
        failures.append("no streaming bytes were observed before cancellation")
    if not health_after_cancel_ok:
        failures.append("server health check failed after streaming cancellation")
    if not followup_ok:
        failures.append("server did not complete a follow-up request after cancellation")
    if process_returncode is None:
        failures.append("server did not terminate after shutdown signal")
    if (
        qatq_live_vram
        and qatq_trace_evidence
        and (not page_segments.is_file() or page_segments.stat().st_size == 0)
    ):
        failures.append("QATQ page-segment trace is missing or empty")

    event_counts = count_events(event_trace)
    segment_counts = count_page_segment_events(page_segments) if qatq_trace_evidence else {}
    persistent_page_source_stats = (
        count_persistent_page_source_events(persistent_page_source) if qatq_trace_evidence else {}
    )
    if (
        qatq_live_vram
        and qatq_trace_evidence
        and segment_counts.get("attention-page-segments", 0) == 0
    ):
        failures.append("QATQ page-segment trace has no attention-page-segments events")
    if qatq_live_vram and qatq_trace_evidence and segment_counts.get("live_offloaded_segments", 0) == 0:
        failures.append("QATQ page-segment trace has no live-offloaded segments")
    if (
        qatq_live_vram
        and qatq_trace_evidence
        and segment_counts.get("attention_consumed_events", 0) == 0
    ):
        failures.append("QATQ page-segment trace has no attention-consumed events")
    if require_flattened_flash_consumer:
        flattened_count = segment_counts.get(
            "consumer.backend_scheduled_flattened_flash_attention",
            0,
        )
        if flattened_count == 0:
            failures.append("QATQ page-segment trace has no flattened Flash attention consumers")
        unexpected_consumers = [
            key.removeprefix("consumer.")
            for key, value in segment_counts.items()
            if key.startswith("consumer.")
            and key != "consumer.backend_scheduled_flattened_flash_attention"
            and value > 0
        ]
        if unexpected_consumers:
            failures.append(
                "QATQ page-segment trace has unexpected attention consumers: "
                + ", ".join(sorted(unexpected_consumers))
            )
    if require_live_offloaded_stream_count > 0:
        stream_ids = [
            key.removeprefix("live_offloaded_stream.")
            for key, value in segment_counts.items()
            if key.startswith("live_offloaded_stream.") and value > 0
        ]
        if len(stream_ids) < require_live_offloaded_stream_count:
            failures.append(
                "QATQ page-segment trace has "
                f"{len(stream_ids)} live-offloaded stream indices; expected at least "
                f"{require_live_offloaded_stream_count}"
            )

    return {
        "failures": failures,
        "stream_bytes_before_cancel": stream_bytes,
        "followup_bytes": followup_bytes,
        "event_trace_bytes": event_trace.stat().st_size if event_trace.exists() else 0,
        "page_segments_bytes": page_segments.stat().st_size if page_segments.exists() else 0,
        "persistent_page_source_bytes": persistent_page_source.stat().st_size
        if persistent_page_source.exists()
        else 0,
        "persistent_pool_bytes": persistent_pool.stat().st_size if persistent_pool.exists() else 0,
        "event_counts": event_counts,
        "page_segment_counts": segment_counts,
        "persistent_page_source_stats": persistent_page_source_stats,
    }


def count_events(path: Path) -> dict[str, int]:
    counts: dict[str, int] = {}
    if not path.exists():
        return counts
    for line in path.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line).get("event")
        except json.JSONDecodeError:
            continue
        if isinstance(event, str):
            counts[event] = counts.get(event, 0) + 1
    return counts


def count_page_segment_events(path: Path) -> dict[str, int]:
    counts: dict[str, int] = {}
    if not path.exists():
        return counts
    for line in path.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        event = payload.get("event")
        if isinstance(event, str):
            counts[event] = counts.get(event, 0) + 1
        if payload.get("attention_consumed") is True:
            counts["attention_consumed_events"] = counts.get("attention_consumed_events", 0) + 1
            consumer = payload.get("consumer")
            if isinstance(consumer, str) and consumer:
                consumer_key = f"consumer.{consumer}"
                counts[consumer_key] = counts.get(consumer_key, 0) + 1
        for segment in payload.get("segments", []):
            if not isinstance(segment, dict):
                continue
            stream_index = segment.get("stream_index")
            if isinstance(stream_index, int):
                stream_key = f"stream.{stream_index}"
                counts[stream_key] = counts.get(stream_key, 0) + 1
            shape = segment.get("shape")
            if isinstance(shape, list) and all(isinstance(dim, int) for dim in shape):
                shape_key = "shape." + "x".join(str(dim) for dim in shape)
                counts[shape_key] = counts.get(shape_key, 0) + 1
            if segment.get("live_offloaded") is True:
                counts["live_offloaded_segments"] = counts.get("live_offloaded_segments", 0) + 1
                if isinstance(stream_index, int):
                    live_stream_key = f"live_offloaded_stream.{stream_index}"
                    counts[live_stream_key] = counts.get(live_stream_key, 0) + 1
                if isinstance(shape, list) and all(isinstance(dim, int) for dim in shape):
                    live_shape_key = "live_offloaded_shape." + "x".join(str(dim) for dim in shape)
                    counts[live_shape_key] = counts.get(live_shape_key, 0) + 1
    return counts


def count_persistent_page_source_events(path: Path) -> dict[str, object]:
    stats: dict[str, object] = {
        "events": 0,
        "max_retained_bytes": 0,
        "max_source_bytes": 0,
        "max_composed_bytes": 0,
        "max_requested_bytes": 0,
        "max_allocated_bytes": 0,
        "max_retained_pages": 0,
        "composition_counts": {},
        "native_page_streaming_true": 0,
        "native_page_streaming_false": 0,
    }
    if not path.exists():
        return stats
    composition_counts: dict[str, int] = {}
    for line in path.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        stats["events"] = int(stats["events"]) + 1
        for key in (
            "retained_bytes",
            "source_bytes",
            "composed_bytes",
            "requested_bytes",
            "allocated_bytes",
            "retained_pages",
        ):
            value = payload.get(key)
            if isinstance(value, bool) or not isinstance(value, int):
                continue
            stat_key = f"max_{key}"
            stats[stat_key] = max(int(stats[stat_key]), value)
        composition = payload.get("composition")
        if isinstance(composition, str) and composition:
            composition_counts[composition] = composition_counts.get(composition, 0) + 1
        native_page_streaming = payload.get("native_page_streaming")
        if native_page_streaming is True:
            stats["native_page_streaming_true"] = int(stats["native_page_streaming_true"]) + 1
        elif native_page_streaming is False:
            stats["native_page_streaming_false"] = int(stats["native_page_streaming_false"]) + 1
    stats["composition_counts"] = composition_counts
    return stats


def parse_llama_cpp_backend_memory(path: Path) -> dict[str, object]:
    stats: dict[str, object] = {
        "source_log_bytes": path.stat().st_size if path.exists() else 0,
        "projected_device_memory_mib": None,
        "projected_free_device_memory_mib": None,
        "device_free_mib": {},
        "model_buffers_mib": {},
        "kv_buffers_mib": {},
        "compute_buffers_mib": {},
        "memory_breakdown_mib": {},
    }
    if not path.exists():
        return stats

    device_free_mib: dict[str, int] = {}
    model_buffers_mib: dict[str, float] = {}
    kv_buffers_mib: dict[str, float] = {}
    compute_buffers_mib: dict[str, float] = {}
    memory_breakdown_mib: dict[str, dict[str, int]] = {}

    projected_re = re.compile(
        r"projected to use\s+(?P<projected>\d+)\s+MiB of device memory vs\.\s+"
        r"(?P<free>\d+)\s+MiB of free device memory"
    )
    device_free_re = re.compile(
        r"using device\s+(?P<device>.+?)\s+\(unknown id\)\s+-\s+"
        r"(?P<free>\d+)\s+MiB free"
    )
    model_buffer_re = re.compile(
        r"load_tensors:\s+(?P<backend>\S+)\s+model buffer size\s+=\s+"
        r"(?P<mib>\d+(?:\.\d+)?)\s+MiB"
    )
    kv_buffer_re = re.compile(
        r"llama_kv_cache:\s+(?P<backend>\S+)\s+KV buffer size\s+=\s+"
        r"(?P<mib>\d+(?:\.\d+)?)\s+MiB"
    )
    compute_buffer_re = re.compile(
        r"sched_reserve:\s+(?P<backend>\S+)\s+compute buffer size\s+=\s+"
        r"(?P<mib>\d+(?:\.\d+)?)\s+MiB"
    )
    accelerator_breakdown_re = re.compile(
        r"llama_memory_breakdown_print:\s+\|\s+-\s+(?P<backend>[^|]+?)\s+\|\s+"
        r"(?P<total>\d+)\s+=\s+(?P<free>\d+)\s+\+\s+\("
        r"(?P<self>\d+)\s+=\s+(?P<model>\d+)\s+\+\s+"
        r"(?P<context>\d+)\s+\+\s+(?P<compute>\d+)\)\s+\+\s+"
        r"(?P<unaccounted>\d+)\s+\|"
    )
    host_breakdown_re = re.compile(
        r"llama_memory_breakdown_print:\s+\|\s+-\s+(?P<backend>Host)\s+\|\s+"
        r"(?P<self>\d+)\s+=\s+(?P<model>\d+)\s+\+\s+"
        r"(?P<context>\d+)\s+\+\s+(?P<compute>\d+)\s+\|"
    )

    for line in path.read_text(errors="replace").splitlines():
        if match := projected_re.search(line):
            stats["projected_device_memory_mib"] = int(match.group("projected"))
            stats["projected_free_device_memory_mib"] = int(match.group("free"))
            continue
        if match := device_free_re.search(line):
            device_free_mib[match.group("device").strip()] = int(match.group("free"))
            continue
        if match := model_buffer_re.search(line):
            model_buffers_mib[match.group("backend")] = float(match.group("mib"))
            continue
        if match := kv_buffer_re.search(line):
            kv_buffers_mib[match.group("backend")] = float(match.group("mib"))
            continue
        if match := compute_buffer_re.search(line):
            compute_buffers_mib[match.group("backend")] = float(match.group("mib"))
            continue
        if match := accelerator_breakdown_re.search(line):
            memory_breakdown_mib[match.group("backend").strip()] = {
                "total": int(match.group("total")),
                "free": int(match.group("free")),
                "self": int(match.group("self")),
                "model": int(match.group("model")),
                "context": int(match.group("context")),
                "compute": int(match.group("compute")),
                "unaccounted": int(match.group("unaccounted")),
            }
            continue
        if match := host_breakdown_re.search(line):
            memory_breakdown_mib[match.group("backend").strip()] = {
                "self": int(match.group("self")),
                "model": int(match.group("model")),
                "context": int(match.group("context")),
                "compute": int(match.group("compute")),
            }

    stats["device_free_mib"] = device_free_mib
    stats["model_buffers_mib"] = model_buffers_mib
    stats["kv_buffers_mib"] = kv_buffers_mib
    stats["compute_buffers_mib"] = compute_buffers_mib
    stats["memory_breakdown_mib"] = memory_breakdown_mib
    return stats


def evaluate_backend_memory_diagnostics(backend_memory: dict[str, object]) -> list[str]:
    failures: list[str] = []
    if not isinstance(backend_memory.get("projected_device_memory_mib"), int):
        failures.append("llama.cpp backend memory diagnostics missing projected device memory")
    breakdown = backend_memory.get("memory_breakdown_mib")
    if not isinstance(breakdown, dict):
        failures.append("llama.cpp backend memory diagnostics missing memory breakdown")
        return failures
    accelerator_rows = [
        values
        for backend, values in breakdown.items()
        if backend != "Host" and isinstance(values, dict)
    ]
    if not accelerator_rows:
        failures.append("llama.cpp backend memory diagnostics missing accelerator memory breakdown")
        return failures
    required_keys = {"self", "model", "context", "compute"}
    if not any(required_keys <= set(row) for row in accelerator_rows):
        failures.append("llama.cpp backend memory diagnostics missing accelerator self/model/context/compute fields")
    return failures


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def write_markdown(path: Path, summary: dict[str, object]) -> None:
    checks = summary.get("checks", {})
    failures = checks.get("failures", []) if isinstance(checks, dict) else []
    out = "# llama.cpp Live VRAM Server Cancellation Probe\n\n"
    out += f"- status: `{summary['status']}`\n"
    out += f"- mode: `{summary.get('mode', 'qatq-live-vram')}`\n"
    out += f"- stream cancelled: `{summary.get('stream_cancelled', False)}`\n"
    out += f"- stream bytes before cancel: `{summary.get('stream_bytes_before_cancel', 0)}`\n"
    out += f"- health after cancel: `{summary.get('health_after_cancel_ok', False)}`\n"
    out += f"- follow-up request: `{summary.get('followup_ok', False)}`\n"
    out += (
        f"- warmup iterations completed: "
        f"`{summary.get('warmup_iterations_completed', 0)}` / "
        f"`{summary.get('warmup_iterations_requested', summary.get('warmup_iterations', 0))}`\n"
    )
    out += f"- iterations completed: `{summary.get('iterations_completed', 0)}` / `{summary.get('iterations_requested', 1)}`\n"
    out += f"- host memory pressure: `{summary.get('host_memory_pressure_mib', 0)} MiB`\n"
    out += f"- QATQ traces enabled: `{summary.get('qatq_traces_enabled', False)}`\n"
    out += f"- require backend memory diagnostics: `{summary.get('require_backend_memory_diagnostics', False)}`\n"
    direct_peak = summary.get("direct_peak_vram_counter", {})
    if isinstance(direct_peak, dict) and direct_peak.get("enabled"):
        out += f"- direct peak-VRAM counter: `{direct_peak.get('available', False)}`\n"
        if direct_peak.get("peak_memory_mib") is not None:
            out += f"- direct peak-VRAM MiB: `{direct_peak.get('peak_memory_mib')}`\n"
        else:
            out += f"- direct peak-VRAM reason: `{direct_peak.get('reason', 'unknown')}`\n"
    memory_checks = summary.get("memory_checks", {})
    if isinstance(memory_checks, dict) and memory_checks:
        growth_kib = memory_checks.get("growth_kib")
        growth_mib = float(growth_kib) / 1024.0 if isinstance(growth_kib, int) else None
        tail_range_kib = memory_checks.get("rss_tail_range_kib")
        tail_range_mib = (
            float(tail_range_kib) / 1024.0 if isinstance(tail_range_kib, int) else None
        )
        tail_growth_kib = memory_checks.get("rss_tail_growth_kib")
        tail_growth_mib = (
            float(tail_growth_kib) / 1024.0 if isinstance(tail_growth_kib, int) else None
        )
        tail_gate_growth_kib = memory_checks.get("rss_tail_gate_growth_kib")
        tail_gate_growth_mib = (
            float(tail_gate_growth_kib) / 1024.0
            if isinstance(tail_gate_growth_kib, int)
            else None
        )
        out += f"- server steady RSS baseline KiB: `{memory_checks.get('baseline_rss_kib')}`\n"
        out += f"- server steady RSS peak KiB: `{memory_checks.get('peak_rss_kib')}`\n"
        out += f"- server steady RSS growth MiB: `{growth_mib}`\n"
        out += (
            f"- server steady RSS tail range MiB: `{tail_range_mib}` "
            f"(window `{memory_checks.get('rss_tail_window_used')}` / "
            f"`{memory_checks.get('rss_tail_window')}`)\n"
        )
        out += f"- server steady RSS tail growth MiB: `{tail_growth_mib}`\n"
        out += f"- server steady RSS tail gate growth MiB: `{tail_gate_growth_mib}`\n"
        out += (
            f"- server steady RSS after last-minus-first KiB: "
            f"`{memory_checks.get('rss_after_last_minus_first_kib')}`\n"
        )
    warmup_memory_checks = summary.get("warmup_memory_checks", {})
    if isinstance(warmup_memory_checks, dict) and warmup_memory_checks:
        warmup_growth_kib = warmup_memory_checks.get("growth_kib")
        warmup_growth_mib = (
            float(warmup_growth_kib) / 1024.0 if isinstance(warmup_growth_kib, int) else None
        )
        out += f"- server warmup RSS baseline KiB: `{warmup_memory_checks.get('baseline_rss_kib')}`\n"
        out += f"- server warmup RSS peak KiB: `{warmup_memory_checks.get('peak_rss_kib')}`\n"
        out += f"- server warmup RSS growth MiB: `{warmup_growth_mib}`\n"
    latency_checks = summary.get("latency_checks", {})
    if isinstance(latency_checks, dict) and latency_checks:
        out += f"- iteration latency stats: `{latency_checks.get('iteration_duration_seconds', {})}`\n"
        out += f"- follow-up latency stats: `{latency_checks.get('followup_duration_seconds', {})}`\n"
    followup_completion_metrics = summary.get("followup_completion_metrics", {})
    if isinstance(followup_completion_metrics, dict) and followup_completion_metrics:
        out += f"- follow-up completion metrics: `{followup_completion_metrics}`\n"
    backend_memory = summary.get("backend_memory", {})
    if isinstance(backend_memory, dict) and backend_memory:
        out += f"- backend memory: `{backend_memory}`\n"
    concurrent = summary.get("concurrent_details", {})
    if isinstance(concurrent, dict) and concurrent:
        out += f"- concurrent follow-up bytes: `{concurrent.get('followup_bytes', 0)}`\n"
        out += f"- concurrent cancel timestamp: `{concurrent.get('cancel_at', 0)}`\n"
        out += f"- concurrent follow-up completed: `{concurrent.get('followup_completed_at', 0)}`\n"
    if isinstance(checks, dict):
        out += f"- event trace bytes: `{checks.get('event_trace_bytes', 0)}`\n"
        out += f"- page-segment trace bytes: `{checks.get('page_segments_bytes', 0)}`\n"
        out += f"- persistent page-source trace bytes: `{checks.get('persistent_page_source_bytes', 0)}`\n"
        out += f"- event counts: `{checks.get('event_counts', {})}`\n"
        out += f"- page-segment counts: `{checks.get('page_segment_counts', {})}`\n"
        out += f"- persistent page-source stats: `{checks.get('persistent_page_source_stats', {})}`\n"
    warmups = summary.get("warmup_iterations", [])
    if isinstance(warmups, list) and warmups:
        out += "\n## Warmup Iterations\n\n"
        for iteration in warmups:
            if not isinstance(iteration, dict):
                continue
            out += (
                f"- {iteration.get('iteration', '?')}: "
                f"stream_bytes=`{iteration.get('stream_bytes_before_cancel', 0)}`, "
                f"health=`{iteration.get('health_after_cancel_ok', False)}`, "
                f"followup=`{iteration.get('followup_ok', False)}`, "
                f"duration_s=`{iteration.get('duration_seconds')}`, "
                f"followup_s=`{iteration.get('followup_duration_seconds')}`, "
                f"rss_before_kib=`{iteration.get('rss_before_kib')}`, "
                f"rss_after_kib=`{iteration.get('rss_after_kib')}`\n"
            )
    iterations = summary.get("iterations", [])
    if isinstance(iterations, list) and iterations:
        out += "\n## Iterations\n\n"
        for iteration in iterations:
            if not isinstance(iteration, dict):
                continue
            out += (
                f"- {iteration.get('iteration', '?')}: "
                f"stream_bytes=`{iteration.get('stream_bytes_before_cancel', 0)}`, "
                f"health=`{iteration.get('health_after_cancel_ok', False)}`, "
                f"followup=`{iteration.get('followup_ok', False)}`, "
                f"duration_s=`{iteration.get('duration_seconds')}`, "
                f"followup_s=`{iteration.get('followup_duration_seconds')}`, "
                f"followup_metrics=`{iteration.get('followup_metrics', {})}`, "
                f"rss_before_kib=`{iteration.get('rss_before_kib')}`, "
                f"rss_after_kib=`{iteration.get('rss_after_kib')}`\n"
            )
    if failures:
        out += "\n## Failures\n\n"
        for failure in failures:
            out += f"- {failure}\n"
    out += "\n## Boundary\n\n"
    out += str(summary.get("boundary", "Dry-run plan only."))
    out += "\n"
    path.write_text(out, encoding="utf-8")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
