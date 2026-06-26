#!/usr/bin/env python3
"""Report direct hardware-counter availability for live-VRAM evidence.

llama.cpp backend memory diagnostics are useful, but they are not the same as
a process-level peak VRAM hardware counter. This helper makes that distinction
machine-checkable: it records which diagnostics are present and fails closed
when a caller requires direct peak-VRAM counter evidence that is unavailable.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--matrix-summary",
        help=(
            "Optional server matrix summary.json or server burn-in summary.json "
            "to inspect."
        ),
    )
    parser.add_argument("--output", default="/tmp/qatq-live-vram-hardware-counters.json")
    parser.add_argument(
        "--require-backend-memory-diagnostics",
        action="store_true",
        help=(
            "Fail unless the matrix summary passed and every case contains "
            "projected device memory plus non-host accelerator breakdown."
        ),
    )
    parser.add_argument("--require-direct-peak-vram", action="store_true")
    parser.add_argument(
        "--sample-pid",
        type=int,
        help="Optional process id to sample with direct GPU-memory tooling.",
    )
    parser.add_argument(
        "--sample-seconds",
        type=float,
        default=1.0,
        help="Seconds to sample --sample-pid when a direct counter backend is available.",
    )
    parser.add_argument(
        "--sample-interval-ms",
        type=int,
        default=100,
        help="Sampling interval for --sample-pid.",
    )
    parser.add_argument(
        "--max-retained-samples",
        type=int,
        default=1024,
        help="Maximum direct-counter sample values to retain in JSON evidence.",
    )
    args = parser.parse_args()
    validate_args(parser, args)

    report = build_report(
        Path(args.matrix_summary) if args.matrix_summary else None,
        sample_pid=args.sample_pid,
        sample_seconds=args.sample_seconds,
        sample_interval_ms=args.sample_interval_ms,
        max_retained_samples=args.max_retained_samples,
    )
    write_json(Path(args.output), report)
    if (
        args.require_backend_memory_diagnostics
        and not report["backend_memory_diagnostics"]["available"]
    ):
        print(report["backend_memory_diagnostics"]["reason"])
        return 1
    if args.require_direct_peak_vram and not report["direct_peak_vram_counter"]["available"]:
        print(report["direct_peak_vram_counter"]["reason"])
        return 1
    print(json.dumps(report, indent=2))
    return 0


def validate_args(parser: argparse.ArgumentParser, args: argparse.Namespace) -> None:
    if args.sample_pid is not None and args.sample_pid <= 0:
        parser.error("--sample-pid must be positive")
    if args.sample_seconds < 0.0:
        parser.error("--sample-seconds must be non-negative")
    if args.sample_interval_ms <= 0:
        parser.error("--sample-interval-ms must be positive")
    if args.max_retained_samples < 0:
        parser.error("--max-retained-samples must be non-negative")


def build_report(
    matrix_summary: Path | None,
    *,
    sample_pid: int | None = None,
    sample_seconds: float = 1.0,
    sample_interval_ms: int = 100,
    max_retained_samples: int = 1024,
) -> dict[str, Any]:
    max_retained_samples = max(0, max_retained_samples)
    powermetrics = inspect_powermetrics()
    vmmap = inspect_vmmap()
    nvidia_smi = inspect_nvidia_smi(
        sample_pid=sample_pid,
        sample_seconds=sample_seconds,
        sample_interval_ms=sample_interval_ms,
        max_retained_samples=max_retained_samples,
    )
    backend = inspect_matrix_summary(matrix_summary)
    direct_sources: list[dict[str, Any]] = []
    if nvidia_smi["direct_peak_vram_counter"]["available"]:
        direct_sources.append(nvidia_smi["direct_peak_vram_counter"])
    direct_available = bool(direct_sources)
    reasons = [
        reason
        for reason in [
            nvidia_smi["direct_peak_vram_counter"].get("reason"),
            powermetrics["direct_peak_vram_counter"].get("reason"),
            vmmap["reason"],
        ]
        if reason
    ]
    return {
        "format": "qatq-live-vram-hardware-counter-capability-v1",
        "matrix_summary": str(matrix_summary) if matrix_summary else None,
        "backend_memory_diagnostics": backend,
        "sample_pid": sample_pid,
        "sample_seconds": sample_seconds,
        "sample_interval_ms": sample_interval_ms,
        "max_retained_samples": max_retained_samples,
        "nvidia_smi": nvidia_smi,
        "powermetrics": powermetrics,
        "vmmap": vmmap,
        "direct_peak_vram_counter": {
            "available": direct_available,
            "sources": direct_sources,
            "reason": "direct peak-VRAM sample captured" if direct_available else "; ".join(reasons),
        },
        "boundary": (
            "This report separates llama.cpp backend allocation diagnostics from "
            "direct hardware peak-VRAM counters. Backend projected memory and "
            "RSS gates are not treated as direct peak-VRAM proof."
        ),
    }


def inspect_nvidia_smi(
    *,
    sample_pid: int | None,
    sample_seconds: float,
    sample_interval_ms: int,
    max_retained_samples: int,
) -> dict[str, Any]:
    path = shutil.which("nvidia-smi")
    info: dict[str, Any] = {
        "path": path,
        "available": path is not None,
        "supports_process_gpu_memory": False,
        "peak_sample": None,
        "direct_peak_vram_counter": {
            "available": False,
            "backend": "nvidia-smi",
            "reason": "nvidia-smi is not available on PATH",
        },
    }
    if not path:
        return info

    help_result = run_probe_command(
        [path, "--help-query-compute-apps"],
        timeout=5.0,
    )
    help_text = help_result.stdout + help_result.stderr
    info["help_returncode"] = help_result.returncode
    if help_result.returncode == 124:
        info["direct_peak_vram_counter"]["reason"] = (
            help_text.strip() or "nvidia-smi capability probe timed out"
        )
        return info
    if help_result.returncode != 0:
        text = help_text.strip()
        info["direct_peak_vram_counter"]["reason"] = (
            "nvidia-smi capability probe failed"
            + (f": {text.splitlines()[0]}" if text else f": return code {help_result.returncode}")
        )
        return info
    info["supports_process_gpu_memory"] = (
        "used_memory" in help_text and "pid" in help_text
    )
    if not info["supports_process_gpu_memory"]:
        info["direct_peak_vram_counter"]["reason"] = (
            "nvidia-smi is present but does not advertise pid,used_memory "
            "compute-app queries"
        )
        return info

    if sample_pid is None:
        info["direct_peak_vram_counter"]["reason"] = (
            "nvidia-smi can expose per-process GPU memory, but no --sample-pid "
            "was provided"
        )
        return info

    sample = sample_nvidia_smi_process_memory(
        path,
        sample_pid=sample_pid,
        sample_seconds=sample_seconds,
        sample_interval_ms=sample_interval_ms,
        max_retained_samples=max_retained_samples,
    )
    info["peak_sample"] = sample
    if sample["peak_memory_mib"] is not None:
        info["direct_peak_vram_counter"] = {
            "available": True,
            "backend": "nvidia-smi",
            "sample_pid": sample_pid,
            "peak_memory_mib": sample["peak_memory_mib"],
            "samples": sample["samples"],
            "sample_count": sample["sample_count"],
            "samples_retained": sample["samples_retained"],
            "samples_truncated": sample["samples_truncated"],
            "reason": "sampled per-process GPU memory with nvidia-smi",
        }
    else:
        info["direct_peak_vram_counter"]["reason"] = sample["reason"]
    return info


def sample_nvidia_smi_process_memory(
    path: str,
    *,
    sample_pid: int,
    sample_seconds: float,
    sample_interval_ms: int,
    max_retained_samples: int,
) -> dict[str, Any]:
    sample_seconds = max(0.0, sample_seconds)
    interval_seconds = max(0.01, sample_interval_ms / 1000.0)
    max_retained_samples = max(0, max_retained_samples)
    deadline = time.monotonic() + sample_seconds
    samples: list[int] = []
    sample_count = 0
    peak: int | None = None
    errors: list[str] = []
    one_shot = sample_seconds == 0.0

    while True:
        remaining = deadline - time.monotonic()
        probe_timeout = 5.0 if one_shot else max(0.001, min(5.0, remaining))
        result = run_probe_command(
            [
                path,
                "--query-compute-apps=pid,used_memory",
                "--format=csv,noheader,nounits",
            ],
            timeout=probe_timeout,
        )
        if result.returncode != 0:
            text = (result.stderr or result.stdout).strip()
            errors.append(text.splitlines()[0] if text else f"return code {result.returncode}")
        else:
            values = parse_nvidia_smi_process_memory(result.stdout, sample_pid)
            for value in values:
                sample_count += 1
                peak = value if peak is None else max(peak, value)
                if len(samples) < max_retained_samples:
                    samples.append(value)

        remaining = deadline - time.monotonic()
        if one_shot or remaining <= 0.0:
            break
        time.sleep(min(interval_seconds, remaining))

    reason = (
        "sampled per-process GPU memory with nvidia-smi"
        if peak is not None
        else "nvidia-smi did not report GPU memory for the sampled process"
    )
    if errors and peak is None:
        reason = f"{reason}; first error: {errors[0]}"
    return {
        "sample_pid": sample_pid,
        "sample_seconds": sample_seconds,
        "sample_interval_ms": sample_interval_ms,
        "samples": samples,
        "sample_count": sample_count,
        "samples_retained": len(samples),
        "samples_truncated": sample_count > len(samples),
        "peak_memory_mib": peak,
        "errors": errors[:5],
        "reason": reason,
    }


def parse_nvidia_smi_process_memory(output: str, sample_pid: int) -> list[int]:
    values: list[int] = []
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        parts = [part.strip() for part in line.split(",")]
        if len(parts) < 2:
            continue
        try:
            pid = int(parts[0])
            memory = int(parts[1].split()[0])
        except (TypeError, ValueError):
            continue
        if pid == sample_pid:
            values.append(memory)
    return values


def run_probe_command(command: list[str], *, timeout: float) -> subprocess.CompletedProcess[str]:
    proc = subprocess.Popen(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = proc.communicate(timeout=timeout)
        return subprocess.CompletedProcess(command, proc.returncode, stdout, stderr)
    except subprocess.TimeoutExpired as error:
        cleanup = terminate_process_group(proc)
        stdout = error.stdout if isinstance(error.stdout, str) else ""
        stderr = error.stderr if isinstance(error.stderr, str) else ""
        detail = (
            f"command timed out after {timeout:g}s; "
            f"cleanup={cleanup.get('signal', 'unknown')}"
        )
        stderr = f"{stderr.rstrip()}\n{detail}".strip()
        return subprocess.CompletedProcess(command, 124, stdout, stderr)


def terminate_process_group(proc: subprocess.Popen[str]) -> dict[str, object]:
    cleanup: dict[str, object] = {
        "attempted": True,
        "signal": None,
        "escalated": False,
        "returncode": None,
    }
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        cleanup["signal"] = "SIGTERM"
    except ProcessLookupError:
        pass
    except PermissionError as error:
        cleanup["error"] = str(error)
    try:
        cleanup["returncode"] = proc.wait(timeout=5.0)
        return cleanup
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
            cleanup["signal"] = "SIGKILL"
            cleanup["escalated"] = True
        except ProcessLookupError:
            pass
        except PermissionError as error:
            cleanup["error"] = str(error)
        try:
            cleanup["returncode"] = proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            cleanup["error"] = "process group did not exit after SIGKILL"
        return cleanup


def inspect_powermetrics() -> dict[str, Any]:
    path = shutil.which("powermetrics")
    info: dict[str, Any] = {
        "path": path,
        "available": path is not None,
        "supports_process_gpu_time": False,
        "supports_process_gpu_memory": False,
        "requires_superuser": None,
        "direct_peak_vram_counter": {
            "available": False,
            "backend": "powermetrics",
            "reason": "powermetrics is not available on PATH",
        },
    }
    if not path:
        return info
    help_result = run_probe_command(
        [path, "--help"],
        timeout=5.0,
    )
    help_text = help_result.stdout + help_result.stderr
    info["supports_process_gpu_time"] = "--show-process-gpu" in help_text
    info["supports_process_gpu_memory"] = "gpu memory" in help_text.lower() or "vram" in help_text.lower()
    sample_result = run_probe_command(
        [path, "--show-process-gpu", "--samplers", "gpu_power", "-n", "1", "-i", "100", "-f", "plist"],
        timeout=5.0,
    )
    sample_text = sample_result.stdout + sample_result.stderr
    info["sample_returncode"] = sample_result.returncode
    info["requires_superuser"] = "superuser" in sample_text.lower()
    info["sample_stderr_excerpt"] = sample_text.strip().splitlines()[:3]
    reason_parts = []
    if info["requires_superuser"]:
        reason_parts.append("powermetrics requires superuser for sampling on this host")
    if not info["supports_process_gpu_memory"]:
        reason_parts.append(
            "powermetrics documents per-process GPU time, not per-process peak GPU memory"
        )
    info["direct_peak_vram_counter"] = {
        "available": False,
        "backend": "powermetrics",
        "reason": "; ".join(reason_parts)
        if reason_parts
        else "powermetrics is not treated as a direct peak GPU memory counter",
    }
    return info


def inspect_vmmap() -> dict[str, Any]:
    path = shutil.which("vmmap")
    return {
        "path": path,
        "available": path is not None,
        "direct_peak_vram_counter": False,
        "reason": "vmmap reports process virtual memory maps, not direct peak GPU memory.",
    }


def inspect_matrix_summary(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {
            "present": False,
            "available": False,
            "summary_kind": None,
            "summary_status": None,
            "completed_runs": 0,
            "cases_with_projected_device_memory": 0,
            "cases_with_accelerator_breakdown": 0,
            "reason": "no --matrix-summary was provided",
        }
    if not path.is_file():
        return {
            "present": False,
            "available": False,
            "summary_kind": None,
            "summary_status": None,
            "completed_runs": 0,
            "cases_with_projected_device_memory": 0,
            "cases_with_accelerator_breakdown": 0,
            "reason": "matrix summary does not exist",
        }
    summary = load_json(path)
    if summary.get("format") == "qatq-live-vram-server-burnin-summary-v1":
        return inspect_burnin_summary(path, summary)
    return inspect_case_summary(summary, summary_kind="matrix")


def inspect_burnin_summary(path: Path, summary: dict[str, Any]) -> dict[str, Any]:
    runs = summary.get("runs")
    runs = runs if isinstance(runs, list) else []
    cases: list[dict[str, Any]] = []
    missing: list[str] = []
    completed_runs = 0
    for run in runs:
        if not isinstance(run, dict):
            continue
        run_summary_path = run.get("summary")
        if not isinstance(run_summary_path, str) or not run_summary_path:
            missing.append(f"run {run.get('index', '<unknown>')}: missing summary path")
            continue
        resolved = Path(run_summary_path)
        if not resolved.is_absolute():
            resolved = path.parent / resolved
        run_summary = load_json(resolved)
        if run_summary.get("status") != "pass":
            missing.append(f"run {run.get('index', '<unknown>')}: summary status is not pass")
            continue
        run_cases = run_summary.get("cases")
        if not isinstance(run_cases, list) or not run_cases:
            missing.append(f"run {run.get('index', '<unknown>')}: no cases")
            continue
        completed_runs += 1
        cases.extend(case for case in run_cases if isinstance(case, dict))

    result = inspect_cases(
        cases,
        status=summary.get("status"),
        summary_kind="burn-in",
        completed_runs=completed_runs,
    )
    if missing and result["available"]:
        result["available"] = False
        result["reason"] = "; ".join(missing[:8])
    result["missing_run_summaries"] = missing[:16]
    return result


def inspect_case_summary(summary: dict[str, Any], *, summary_kind: str) -> dict[str, Any]:
    cases = summary.get("cases")
    cases = cases if isinstance(cases, list) else []
    return inspect_cases(
        cases,
        status=summary.get("status"),
        summary_kind=summary_kind,
        completed_runs=1 if summary.get("status") == "pass" else 0,
    )


def inspect_cases(
    cases: list[dict[str, Any]],
    *,
    status: Any,
    summary_kind: str,
    completed_runs: int,
) -> dict[str, Any]:
    projected = 0
    breakdown = 0
    for case in cases:
        if not isinstance(case, dict):
            continue
        if isinstance(case.get("projected_device_memory_mib"), (int, float)):
            projected += 1
        backend_memory = case.get("backend_memory")
        memory_breakdown = (
            backend_memory.get("memory_breakdown_mib")
            if isinstance(backend_memory, dict)
            else None
        )
        if isinstance(memory_breakdown, dict) and any(key != "Host" for key in memory_breakdown):
            breakdown += 1
    available = (
        status == "pass"
        and len(cases) > 0
        and projected == len(cases)
        and breakdown == len(cases)
    )
    reason = "all matrix cases include backend memory diagnostics"
    if status != "pass":
        reason = "matrix summary status is not pass"
    elif not cases:
        reason = "matrix summary contains no cases"
    elif projected != len(cases):
        reason = (
            "not every matrix case has projected_device_memory_mib "
            f"({projected}/{len(cases)})"
        )
    elif breakdown != len(cases):
        reason = (
            "not every matrix case has non-host accelerator memory breakdown "
            f"({breakdown}/{len(cases)})"
        )
    return {
        "present": True,
        "available": available,
        "summary_kind": summary_kind,
        "summary_status": status,
        "completed_runs": completed_runs,
        "total_cases": len(cases),
        "cases_with_projected_device_memory": projected,
        "cases_with_accelerator_breakdown": breakdown,
        "direct_peak_vram_counter": False,
        "reason": reason,
    }


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
