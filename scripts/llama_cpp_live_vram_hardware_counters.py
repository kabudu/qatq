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
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix-summary", help="Optional server matrix summary.json to inspect")
    parser.add_argument("--output", default="/tmp/qatq-live-vram-hardware-counters.json")
    parser.add_argument("--require-direct-peak-vram", action="store_true")
    args = parser.parse_args()

    report = build_report(Path(args.matrix_summary) if args.matrix_summary else None)
    write_json(Path(args.output), report)
    if args.require_direct_peak_vram and not report["direct_peak_vram_counter"]["available"]:
        print(report["direct_peak_vram_counter"]["reason"])
        return 1
    print(json.dumps(report, indent=2))
    return 0


def build_report(matrix_summary: Path | None) -> dict[str, Any]:
    powermetrics = inspect_powermetrics()
    vmmap = inspect_vmmap()
    backend = inspect_matrix_summary(matrix_summary) if matrix_summary else {}
    direct_available = False
    reasons = [
        "macOS powermetrics requires superuser for sampling on this host",
        "powermetrics documents per-process GPU time, not per-process peak GPU memory",
        "vmmap reports virtual memory regions and is not a direct peak VRAM counter",
    ]
    return {
        "format": "qatq-live-vram-hardware-counter-capability-v1",
        "matrix_summary": str(matrix_summary) if matrix_summary else None,
        "backend_memory_diagnostics": backend,
        "powermetrics": powermetrics,
        "vmmap": vmmap,
        "direct_peak_vram_counter": {
            "available": direct_available,
            "reason": "; ".join(reasons),
        },
        "boundary": (
            "This report separates llama.cpp backend allocation diagnostics from "
            "direct hardware peak-VRAM counters. Backend projected memory and "
            "RSS gates are not treated as direct peak-VRAM proof."
        ),
    }


def inspect_powermetrics() -> dict[str, Any]:
    path = shutil.which("powermetrics")
    info: dict[str, Any] = {
        "path": path,
        "available": path is not None,
        "supports_process_gpu_time": False,
        "supports_process_gpu_memory": False,
        "requires_superuser": None,
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
