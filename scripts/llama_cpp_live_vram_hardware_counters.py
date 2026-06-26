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
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix-summary", help="Optional server matrix summary.json to inspect")
    parser.add_argument("--output", default="/tmp/qatq-live-vram-hardware-counters.json")
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
    args = parser.parse_args()

    report = build_report(
        Path(args.matrix_summary) if args.matrix_summary else None,
        sample_pid=args.sample_pid,
        sample_seconds=args.sample_seconds,
        sample_interval_ms=args.sample_interval_ms,
    )
    write_json(Path(args.output), report)
    if args.require_direct_peak_vram and not report["direct_peak_vram_counter"]["available"]:
        print(report["direct_peak_vram_counter"]["reason"])
        return 1
    print(json.dumps(report, indent=2))
    return 0


def build_report(
    matrix_summary: Path | None,
    *,
    sample_pid: int | None = None,
    sample_seconds: float = 1.0,
    sample_interval_ms: int = 100,
) -> dict[str, Any]:
    powermetrics = inspect_powermetrics()
    vmmap = inspect_vmmap()
    nvidia_smi = inspect_nvidia_smi(
        sample_pid=sample_pid,
        sample_seconds=sample_seconds,
        sample_interval_ms=sample_interval_ms,
    )
    backend = inspect_matrix_summary(matrix_summary) if matrix_summary else {}
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

    help_result = subprocess.run(
        [path, "--help-query-compute-apps"],
        check=False,
        text=True,
        capture_output=True,
        timeout=5.0,
    )
    help_text = help_result.stdout + help_result.stderr
    info["supports_process_gpu_memory"] = (
        "used_memory" in help_text and "pid" in help_text
    )
    info["help_returncode"] = help_result.returncode
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
    )
    info["peak_sample"] = sample
    if sample["peak_memory_mib"] is not None:
        info["direct_peak_vram_counter"] = {
            "available": True,
            "backend": "nvidia-smi",
            "sample_pid": sample_pid,
            "peak_memory_mib": sample["peak_memory_mib"],
            "samples": sample["samples"],
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
) -> dict[str, Any]:
    sample_seconds = max(0.0, sample_seconds)
    interval_seconds = max(0.01, sample_interval_ms / 1000.0)
    deadline = time.monotonic() + sample_seconds
    samples: list[int] = []
    errors: list[str] = []

    while True:
        result = subprocess.run(
            [
                path,
                "--query-compute-apps=pid,used_memory",
                "--format=csv,noheader,nounits",
            ],
            check=False,
            text=True,
            capture_output=True,
            timeout=5.0,
        )
        if result.returncode != 0:
            text = (result.stderr or result.stdout).strip()
            errors.append(text.splitlines()[0] if text else f"return code {result.returncode}")
        else:
            samples.extend(parse_nvidia_smi_process_memory(result.stdout, sample_pid))

        if time.monotonic() >= deadline:
            break
        time.sleep(interval_seconds)

    peak = max(samples) if samples else None
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
    help_result = subprocess.run(
        [path, "--help"],
        check=False,
        text=True,
        capture_output=True,
        timeout=5.0,
    )
    help_text = help_result.stdout + help_result.stderr
    info["supports_process_gpu_time"] = "--show-process-gpu" in help_text
    info["supports_process_gpu_memory"] = "gpu memory" in help_text.lower() or "vram" in help_text.lower()
    sample_result = subprocess.run(
        [path, "--show-process-gpu", "--samplers", "gpu_power", "-n", "1", "-i", "100", "-f", "plist"],
        check=False,
        text=True,
        capture_output=True,
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
    if path is None or not path.is_file():
        return {
            "present": False,
            "cases_with_projected_device_memory": 0,
            "cases_with_accelerator_breakdown": 0,
        }
    summary = load_json(path)
    cases = summary.get("cases")
    cases = cases if isinstance(cases, list) else []
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
    return {
        "present": True,
        "summary_status": summary.get("status"),
        "total_cases": len(cases),
        "cases_with_projected_device_memory": projected,
        "cases_with_accelerator_breakdown": breakdown,
        "direct_peak_vram_counter": False,
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
