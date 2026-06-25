#!/usr/bin/env python3
"""Probe fail-closed abort behaviour for the patched llama.cpp QATQ adapter.

This is not a substitute for an in-process llama-server request-cancellation
test. It exercises the currently wired `llama-simple` QATQ path, waits until a
real KV export has happened, interrupts generation, and verifies that normal
completion artifacts were not written after the abort.
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
from pathlib import Path


EXPORT_MARKER = "exported QATQ KV tensors"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-simple", default="/private/tmp/qatq-llama-live-work/build/bin/llama-simple")
    parser.add_argument("--model", required=True)
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-abort-probe")
    parser.add_argument("--prompt", default="Abort-safety probe for QATQ live VRAM page export.")
    parser.add_argument("--model-id", default="qatq-live-vram-abort-probe")
    parser.add_argument("--n-predict", type=int, default=1024)
    parser.add_argument("--gpu-layers", type=int, default=99)
    parser.add_argument("--kv-gpu-layers", type=int, default=4)
    parser.add_argument("--page-tokens", type=int, default=1024)
    parser.add_argument("--current-token", type=int, default=2048)
    parser.add_argument("--hot-window-tokens", type=int, default=1024)
    parser.add_argument("--prefetch-window-tokens", type=int, default=32)
    parser.add_argument("--max-queued-pages", type=int, default=32)
    parser.add_argument("--cache-type-k", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--cache-type-v", default="f16", choices=["f16", "bf16", "f32"])
    parser.add_argument("--export-timeout", type=float, default=120.0)
    parser.add_argument("--abort-after-export-seconds", type=float, default=0.5)
    parser.add_argument("--shutdown-timeout", type=float, default=10.0)
    parser.add_argument("--abort-signal", default="INT", choices=["INT", "TERM"])
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    validate_args(args)

    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    artifacts = build_artifacts(work_dir)
    command = build_command(args, artifacts)
    write_json(
        work_dir / "abort-probe-plan.json",
        {
            "format": "qatq-live-vram-abort-probe-plan-v1",
            "command": command,
            "artifacts": {key: str(value) for key, value in artifacts.items()},
            "dry_run": args.dry_run,
        },
    )

    if args.dry_run:
        summary = {
            "format": "qatq-live-vram-abort-probe-summary-v1",
            "status": "dry-run",
            "command": command,
            "artifacts": {key: str(value) for key, value in artifacts.items()},
        }
        write_json(work_dir / "summary.json", summary)
        write_markdown(work_dir / "summary.md", summary)
        print((work_dir / "summary.md").read_text(encoding="utf-8"))
        return 0

    require(Path(args.llama_simple).is_file(), f"missing llama-simple: {args.llama_simple}")
    require(Path(args.model).is_file(), f"missing model: {args.model}")

    stdout_path = work_dir / "stdout.log"
    stderr_path = work_dir / "stderr.log"
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        proc = subprocess.Popen(
            command,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        observed_export = wait_for_export(stderr_path, proc, args.export_timeout)
        aborted = False
        if observed_export and proc.poll() is None:
            time.sleep(args.abort_after_export_seconds)
            send_signal(proc, args.abort_signal)
            aborted = True
        returncode = wait_after_abort(proc, args.shutdown_timeout)

    checks = evaluate_abort_result(artifacts, stderr_path, observed_export, aborted, returncode)
    summary = {
        "format": "qatq-live-vram-abort-probe-summary-v1",
        "status": "pass" if not checks["failures"] else "fail",
        "observed_export": observed_export,
        "aborted": aborted,
        "returncode": returncode,
        "stdout": str(stdout_path),
        "stderr": str(stderr_path),
        "artifacts": {key: str(value) for key, value in artifacts.items()},
        "checks": checks,
        "command": command,
        "boundary": (
            "Process-abort fail-closed evidence for the patched llama-simple QATQ path; "
            "not an in-process llama-server request-cancellation proof."
        ),
    }
    write_json(work_dir / "summary.json", summary)
    write_markdown(work_dir / "summary.md", summary)
    print((work_dir / "summary.md").read_text(encoding="utf-8"))
    return 0 if summary["status"] == "pass" else 1


def validate_args(args: argparse.Namespace) -> None:
    require(args.n_predict > 0, "--n-predict must be positive")
    require(args.gpu_layers >= 0, "--gpu-layers must be non-negative")
    require(args.kv_gpu_layers >= 0, "--kv-gpu-layers must be non-negative")
    require(args.page_tokens > 0, "--page-tokens must be positive")
    require(args.current_token >= 0, "--current-token must be non-negative")
    require(args.hot_window_tokens >= 0, "--hot-window-tokens must be non-negative")
    require(args.prefetch_window_tokens >= 0, "--prefetch-window-tokens must be non-negative")
    require(args.max_queued_pages >= 0, "--max-queued-pages must be non-negative")
    require(args.export_timeout > 0, "--export-timeout must be positive")
    require(args.abort_after_export_seconds >= 0, "--abort-after-export-seconds must be non-negative")
    require(args.shutdown_timeout > 0, "--shutdown-timeout must be positive")


def build_artifacts(work_dir: Path) -> dict[str, Path]:
    return {
        "export_dir": work_dir / "exported-kv",
        "output_manifest": work_dir / "output-manifest.json",
        "token_timings": work_dir / "token-timings.csv",
        "event_trace": work_dir / "event-trace.json",
        "page_segments": work_dir / "page-segments.jsonl",
    }


def build_command(args: argparse.Namespace, artifacts: dict[str, Path]) -> list[str]:
    return [
        args.llama_simple,
        "-m",
        args.model,
        "-ngl",
        str(args.gpu_layers),
        "-n",
        str(args.n_predict),
        "--cache-type-k",
        args.cache_type_k,
        "--cache-type-v",
        args.cache_type_v,
        "--qatq-kv-gpu-layers",
        str(args.kv_gpu_layers),
        "--qatq-gpu-page-staging",
        "--qatq-kv-export-dir",
        str(artifacts["export_dir"]),
        "--qatq-output-manifest",
        str(artifacts["output_manifest"]),
        "--qatq-token-timings",
        str(artifacts["token_timings"]),
        "--qatq-event-trace",
        str(artifacts["event_trace"]),
        "--qatq-attention-page-segments-trace",
        str(artifacts["page_segments"]),
        "--qatq-page-tokens",
        str(args.page_tokens),
        "--qatq-trace-current-token",
        str(args.current_token),
        "--qatq-trace-hot-window-tokens",
        str(args.hot_window_tokens),
        "--qatq-trace-prefetch-window-tokens",
        str(args.prefetch_window_tokens),
        "--qatq-trace-next-required",
        "cold-after-hot",
        "--qatq-trace-max-queued-pages",
        str(args.max_queued_pages),
        "--qatq-native-page-streaming-attention-backend-op",
        "--qatq-native-page-streaming-flatten-flash",
        "--qatq-model-id",
        args.model_id,
        args.prompt,
    ]


def wait_for_export(stderr_path: Path, proc: subprocess.Popen[bytes], timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if stderr_path.exists() and EXPORT_MARKER in stderr_path.read_text(errors="replace"):
            return True
        if proc.poll() is not None:
            return False
        time.sleep(0.25)
    return False


def send_signal(proc: subprocess.Popen[bytes], signal_name: str) -> None:
    signum = signal.SIGINT if signal_name == "INT" else signal.SIGTERM
    try:
        os.killpg(proc.pid, signum)
    except ProcessLookupError:
        return


def wait_after_abort(proc: subprocess.Popen[bytes], timeout: float) -> int | None:
    try:
        return proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            return proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            return None


def evaluate_abort_result(
    artifacts: dict[str, Path],
    stderr_path: Path,
    observed_export: bool,
    aborted: bool,
    returncode: int | None,
) -> dict[str, object]:
    export_dir = artifacts["export_dir"]
    exported_files = list(export_dir.rglob("*")) if export_dir.exists() else []
    event_trace = artifacts["event_trace"]
    output_manifest = artifacts["output_manifest"]
    token_timings = artifacts["token_timings"]
    page_segments = artifacts["page_segments"]
    failures: list[str] = []

    if not observed_export:
        failures.append("QATQ export marker was not observed before abort")
    if not exported_files:
        failures.append("QATQ export directory is empty or missing")
    if not event_trace.is_file() or event_trace.stat().st_size == 0:
        failures.append("QATQ event trace is missing or empty")
    if not page_segments.is_file():
        failures.append("QATQ page-segment trace was not created")
    if not aborted:
        failures.append("process was not interrupted after export")
    if returncode == 0:
        failures.append("process exited successfully instead of being interrupted")
    if returncode is None:
        failures.append("process did not terminate after forced shutdown")
    if output_manifest.exists():
        failures.append("completion output manifest exists after abort")
    if token_timings.exists():
        failures.append("completion token timings exist after abort")
    if stderr_path.exists() and "wrote QATQ output manifest" in stderr_path.read_text(errors="replace"):
        failures.append("stderr reports normal output-manifest completion after abort")

    return {
        "failures": failures,
        "exported_file_count": len([path for path in exported_files if path.is_file()]),
        "event_trace_bytes": event_trace.stat().st_size if event_trace.exists() else 0,
        "page_segments_bytes": page_segments.stat().st_size if page_segments.exists() else 0,
        "output_manifest_exists": output_manifest.exists(),
        "token_timings_exists": token_timings.exists(),
    }


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def write_markdown(path: Path, summary: dict[str, object]) -> None:
    checks = summary.get("checks", {})
    failures = checks.get("failures", []) if isinstance(checks, dict) else []
    out = "# llama.cpp Live VRAM Abort Probe\n\n"
    out += f"- status: `{summary['status']}`\n"
    out += f"- observed export: `{summary.get('observed_export', False)}`\n"
    out += f"- aborted: `{summary.get('aborted', False)}`\n"
    out += f"- return code: `{summary.get('returncode')}`\n"
    if isinstance(checks, dict):
        out += f"- exported files: `{checks.get('exported_file_count', 0)}`\n"
        out += f"- event trace bytes: `{checks.get('event_trace_bytes', 0)}`\n"
        out += f"- output manifest after abort: `{checks.get('output_manifest_exists', False)}`\n"
        out += f"- token timings after abort: `{checks.get('token_timings_exists', False)}`\n"
    if failures:
        out += "\n## Failures\n\n"
        for failure in failures:
            out += f"- {failure}\n"
    out += "\n## Boundary\n\n"
    out += str(
        summary.get(
            "boundary",
            "Dry-run plan only; no runtime cancellation evidence collected.",
        )
    )
    out += "\n"
    path.write_text(out, encoding="utf-8")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
