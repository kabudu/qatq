#!/usr/bin/env python3
"""Run real llama.cpp live-VRAM matrix jobs concurrently.

This is a production-shaped stress wrapper around
`llama_cpp_live_vram_matrix.py`. It intentionally reuses the existing strict
matrix/evidence gates instead of reimplementing live-VRAM validation. Each job
gets a one-case config and its own work directory, then the wrapper runs several
jobs in parallel to expose process-level runtime contention, Metal pressure,
artifact isolation, and fail-closed aggregation.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class StressJob:
    index: int
    case_id: str
    iteration: int
    config_path: Path
    work_dir: Path
    command: list[str]


@dataclass(frozen=True)
class StressResult:
    index: int
    case_id: str
    iteration: int
    status: str
    work_dir: Path
    elapsed_seconds: float
    returncode: int
    stdout_path: Path
    stderr_path: Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True, help="Matrix JSON config with a top-level `cases` array")
    parser.add_argument("--matrix-runner", default="scripts/llama_cpp_live_vram_matrix.py")
    parser.add_argument("--llama-simple", default="/private/tmp/qatq-llama-live-work/build/bin/llama-simple")
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-parallel-stress")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--max-cases", type=int, default=0)
    parser.add_argument("--timeout", type=int, default=1200)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--require-live-paging", action="store_true")
    parser.add_argument("--require-native-page-streaming", action="store_true")
    parser.add_argument("--gpu-page-staging", action="store_true")
    parser.add_argument("--native-page-streaming-attention-backend-op", action="store_true")
    parser.add_argument("--native-page-streaming-flatten-flash", action="store_true")
    parser.add_argument("--attention-page-segments-live-offloaded-only", action="store_true")
    parser.add_argument("--aggregate-codec-gate", action="store_true")
    parser.add_argument("--deep-latency-baseline", action="store_true")
    parser.add_argument("--prune-bulk-artifacts", action="store_true")
    parser.add_argument("--require-stable-reclaim", action="store_true")
    parser.add_argument("--require-stable-qc-bytes", action="store_true")
    parser.add_argument("--override-short-predict", type=int, default=0)
    parser.add_argument("--override-deep-predict", type=int, default=0)
    parser.add_argument("--override-max-queued-pages", type=int, default=-1)
    parser.add_argument("--host-memory-pressure-mib", type=int, default=0)
    parser.add_argument("--min-token-latency-samples", type=int, default=0)
    parser.add_argument("--max-mixed-token-p95-regression-ratio", type=float, default=0.0)
    parser.add_argument("--max-mixed-token-p99-regression-ratio", type=float, default=0.0)
    parser.add_argument("--min-deep-token-latency-samples", type=int, default=0)
    parser.add_argument("--max-deep-mixed-token-p95-regression-ratio", type=float, default=0.0)
    parser.add_argument("--max-deep-mixed-token-p99-regression-ratio", type=float, default=0.0)
    args = parser.parse_args()

    require(args.jobs > 0, "--jobs must be positive")
    require(args.jobs <= 16, "--jobs is capped at 16")
    require(args.iterations > 0, "--iterations must be positive")
    require(args.max_cases >= 0, "--max-cases must be non-negative")
    require(args.timeout > 0, "--timeout must be positive")
    require(args.host_memory_pressure_mib >= 0, "--host-memory-pressure-mib must be non-negative")

    root = Path.cwd()
    config_path = root / args.config
    matrix_runner = root / args.matrix_runner
    require(config_path.is_file(), f"missing config: {config_path}")
    require(matrix_runner.is_file(), f"missing matrix runner: {matrix_runner}")
    if not args.dry_run:
        require(Path(args.llama_simple).is_file(), f"missing llama-simple: {args.llama_simple}")

    config = load_json(config_path)
    raw_cases = config.get("cases")
    require(isinstance(raw_cases, list) and raw_cases, "config must contain a non-empty `cases` array")
    cases = raw_cases[: args.max_cases] if args.max_cases else raw_cases

    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    jobs = build_jobs(args, config, cases, matrix_runner, work_dir)
    plan_path = work_dir / "parallel-stress-plan.json"
    plan_path.write_text(
        json.dumps(
            {
                "format": "qatq-live-vram-parallel-stress-plan-v1",
                "jobs": [
                    {
                        "index": job.index,
                        "case_id": job.case_id,
                        "iteration": job.iteration,
                        "config": str(job.config_path),
                        "work_dir": str(job.work_dir),
                        "command": job.command,
                    }
                    for job in jobs
                ],
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    if args.dry_run:
        results = [
            StressResult(
                index=job.index,
                case_id=job.case_id,
                iteration=job.iteration,
                status="dry-run",
                work_dir=job.work_dir,
                elapsed_seconds=0.0,
                returncode=0,
                stdout_path=job.work_dir / "stdout.log",
                stderr_path=job.work_dir / "stderr.log",
            )
            for job in jobs
        ]
    else:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
            results = list(executor.map(run_job, jobs))

    summary = build_summary(results, jobs=args.jobs, dry_run=args.dry_run)
    (work_dir / "summary.md").write_text(summary, encoding="utf-8")
    (work_dir / "summary.json").write_text(
        json.dumps(
            {
                "format": "qatq-live-vram-parallel-stress-summary-v1",
                "dry_run": args.dry_run,
                "jobs_requested": args.jobs,
                "total_jobs": len(results),
                "passed": sum(1 for result in results if result.status in {"pass", "dry-run"}),
                "failed": sum(1 for result in results if result.status == "fail"),
                "results": [
                    {
                        "index": result.index,
                        "case_id": result.case_id,
                        "iteration": result.iteration,
                        "status": result.status,
                        "work_dir": str(result.work_dir),
                        "elapsed_seconds": result.elapsed_seconds,
                        "returncode": result.returncode,
                        "stdout": str(result.stdout_path),
                        "stderr": str(result.stderr_path),
                    }
                    for result in results
                ],
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(summary)

    return 1 if any(result.status == "fail" for result in results) else 0


def build_jobs(args: argparse.Namespace, config: dict, cases: list[dict], matrix_runner: Path, work_dir: Path) -> list[StressJob]:
    jobs: list[StressJob] = []
    matrix_gates = config.get("matrix_gates")
    for iteration in range(1, args.iterations + 1):
        for case in cases:
            require(isinstance(case, dict), "each case must be an object")
            case_id = str(case.get("id", ""))
            require(case_id, "each case must have an id")
            job_index = len(jobs) + 1
            job_dir = work_dir / f"job-{job_index:03d}-{case_id}-i{iteration}"
            job_dir.mkdir(parents=True, exist_ok=True)
            job_config = {"cases": [case]}
            if isinstance(matrix_gates, dict):
                job_config["matrix_gates"] = matrix_gates
            config_path = job_dir / "config.json"
            config_path.write_text(json.dumps(job_config, indent=2) + "\n", encoding="utf-8")
            command = build_matrix_command(args, matrix_runner, config_path, job_dir / "matrix")
            jobs.append(
                StressJob(
                    index=job_index,
                    case_id=case_id,
                    iteration=iteration,
                    config_path=config_path,
                    work_dir=job_dir,
                    command=command,
                )
            )
    return jobs


def build_matrix_command(args: argparse.Namespace, matrix_runner: Path, config_path: Path, work_dir: Path) -> list[str]:
    command = [
        sys.executable,
        str(matrix_runner),
        "--config",
        str(config_path),
        "--llama-simple",
        args.llama_simple,
        "--qatq-kv-bench",
        args.qatq_kv_bench,
        "--work-dir",
        str(work_dir),
        "--timeout",
        str(args.timeout),
        "--iterations",
        "1",
    ]
    flag_names = [
        "require_live_paging",
        "require_native_page_streaming",
        "gpu_page_staging",
        "native_page_streaming_attention_backend_op",
        "native_page_streaming_flatten_flash",
        "attention_page_segments_live_offloaded_only",
        "aggregate_codec_gate",
        "deep_latency_baseline",
        "prune_bulk_artifacts",
        "require_stable_reclaim",
        "require_stable_qc_bytes",
    ]
    for name in flag_names:
        if getattr(args, name):
            command.append("--" + name.replace("_", "-"))
    value_args = [
        ("override_short_predict", "--override-short-predict", 0),
        ("override_deep_predict", "--override-deep-predict", 0),
        ("override_max_queued_pages", "--override-max-queued-pages", -1),
        ("host_memory_pressure_mib", "--host-memory-pressure-mib", 0),
        ("min_token_latency_samples", "--min-token-latency-samples", 0),
        ("max_mixed_token_p95_regression_ratio", "--max-mixed-token-p95-regression-ratio", 0.0),
        ("max_mixed_token_p99_regression_ratio", "--max-mixed-token-p99-regression-ratio", 0.0),
        ("min_deep_token_latency_samples", "--min-deep-token-latency-samples", 0),
        ("max_deep_mixed_token_p95_regression_ratio", "--max-deep-mixed-token-p95-regression-ratio", 0.0),
        ("max_deep_mixed_token_p99_regression_ratio", "--max-deep-mixed-token-p99-regression-ratio", 0.0),
    ]
    for attr, flag, default in value_args:
        value = getattr(args, attr)
        if value != default:
            command.extend([flag, str(value)])
    return command


def run_job(job: StressJob) -> StressResult:
    stdout_path = job.work_dir / "stdout.log"
    stderr_path = job.work_dir / "stderr.log"
    start = time.monotonic()
    completed = subprocess.run(job.command, text=True, capture_output=True)
    elapsed = time.monotonic() - start
    stdout_path.write_text(completed.stdout, encoding="utf-8")
    stderr_path.write_text(completed.stderr, encoding="utf-8")
    return StressResult(
        index=job.index,
        case_id=job.case_id,
        iteration=job.iteration,
        status="pass" if completed.returncode == 0 else "fail",
        work_dir=job.work_dir,
        elapsed_seconds=elapsed,
        returncode=completed.returncode,
        stdout_path=stdout_path,
        stderr_path=stderr_path,
    )


def build_summary(results: list[StressResult], jobs: int, dry_run: bool) -> str:
    passed = sum(1 for result in results if result.status in {"pass", "dry-run"})
    failed = sum(1 for result in results if result.status == "fail")
    lines = [
        "# QATQ live VRAM parallel stress",
        "",
        f"- mode: {'dry-run' if dry_run else 'execute'}",
        f"- worker limit: {jobs}",
        f"- total jobs: {len(results)}",
        f"- passed: {passed}",
        f"- failed: {failed}",
        "",
        "| job | case | iteration | status | elapsed s | return code | work dir |",
        "| ---: | --- | ---: | --- | ---: | ---: | --- |",
    ]
    for result in results:
        lines.append(
            f"| {result.index} | `{result.case_id}` | {result.iteration} | {result.status} | "
            f"{result.elapsed_seconds:.2f} | {result.returncode} | `{result.work_dir}` |"
        )
    lines.append("")
    return "\n".join(lines)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    raise SystemExit(main())
