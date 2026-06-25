#!/usr/bin/env python3
"""Sweep llama.cpp QATQ token-page export sizes with fail-closed evidence.

This is a thin orchestration layer over `llama_cpp_live_vram_evidence.py`.
Each page size still runs the full evidence gate: Metal execution, deterministic
output preservation, exact QATQ restore, event-trace validation, attention-read
trace validation, runtime reclaim evidence, and raw/zstd/lz4 comparisons.
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


@dataclass(frozen=True)
class SweepResult:
    page_tokens: int
    status: str
    work_dir: Path
    total_pages: int
    verified_restores: int
    qatq_best_pages: int
    qatq_bytes: int
    zstd_bytes: int
    lz4_bytes: int
    raw_bytes: int
    attention_events: int
    elapsed_seconds: float
    failure: str


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-runner", default="scripts/llama_cpp_live_vram_evidence.py")
    parser.add_argument("--llama-simple", default="/private/tmp/qatq-llama.cpp/build/bin/llama-simple")
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--model", required=True)
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--work-dir", default="/private/tmp/qatq-live-vram-page-size-sweep")
    parser.add_argument("--page-tokens", required=True, help="Comma-separated page sizes, such as 512,1024,2048")
    parser.add_argument("--mixed-kv-gpu-layers", type=int, default=24)
    parser.add_argument("--sweep-kv-gpu-layers", default="")
    parser.add_argument("--short-prompt", default="")
    parser.add_argument("--deep-prompt-seed", default="")
    parser.add_argument("--deep-repeat", type=int, default=80)
    parser.add_argument("--current-token", type=int, default=0)
    parser.add_argument("--hot-window-tokens", type=int, default=0)
    parser.add_argument("--next-required", default="uniform-after-hot", choices=["uniform-after-hot", "page-end", "cold-after-hot"])
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--fail-on-any", action="store_true")
    args = parser.parse_args()

    root = Path.cwd()
    runner = require_file(root / args.evidence_runner, "--evidence-runner")
    llama_simple = require_file(Path(args.llama_simple), "--llama-simple")
    model = require_file(Path(args.model), "--model")
    page_sizes = parse_page_sizes(args.page_tokens)
    work_dir = Path(args.work_dir)
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    results: list[SweepResult] = []
    for page_tokens in page_sizes:
        results.append(
            run_page_size(
                root=root,
                runner=runner,
                llama_simple=llama_simple,
                kv_bench=args.qatq_kv_bench,
                model=model,
                model_id=args.model_id,
                page_tokens=page_tokens,
                work_dir=work_dir / f"page-{page_tokens}",
                mixed_kv_gpu_layers=args.mixed_kv_gpu_layers,
                sweep_kv_gpu_layers=args.sweep_kv_gpu_layers,
                short_prompt=args.short_prompt,
                deep_prompt_seed=args.deep_prompt_seed,
                deep_repeat=args.deep_repeat,
                current_token=args.current_token,
                hot_window_tokens=args.hot_window_tokens,
                next_required=args.next_required,
                timeout=args.timeout,
            )
        )

    summary = build_summary(results, work_dir)
    (work_dir / "summary.md").write_text(summary, encoding="utf-8")
    print(summary)

    failures = [result for result in results if result.status != "pass"]
    if args.fail_on_any and failures:
        return 1
    if not any(result.status == "pass" for result in results):
        return 1
    return 0


def run_page_size(
    *,
    root: Path,
    runner: Path,
    llama_simple: Path,
    kv_bench: str,
    model: Path,
    model_id: str,
    page_tokens: int,
    work_dir: Path,
    mixed_kv_gpu_layers: int,
    sweep_kv_gpu_layers: str,
    short_prompt: str,
    deep_prompt_seed: str,
    deep_repeat: int,
    current_token: int,
    hot_window_tokens: int,
    next_required: str,
    timeout: int,
) -> SweepResult:
    command = [
        sys.executable,
        str(runner),
        "--llama-simple",
        str(llama_simple),
        "--qatq-kv-bench",
        kv_bench,
        "--model",
        str(model),
        "--model-id",
        model_id,
        "--work-dir",
        str(work_dir),
        "--mixed-kv-gpu-layers",
        str(mixed_kv_gpu_layers),
        "--page-tokens",
        str(page_tokens),
        "--deep-repeat",
        str(deep_repeat),
        "--current-token",
        str(current_token),
        "--hot-window-tokens",
        str(hot_window_tokens),
        "--next-required",
        next_required,
        "--timeout",
        str(timeout),
    ]
    if sweep_kv_gpu_layers:
        command.extend(["--sweep-kv-gpu-layers", sweep_kv_gpu_layers])
    if short_prompt:
        command.extend(["--short-prompt", short_prompt])
    if deep_prompt_seed:
        command.extend(["--deep-prompt-seed", deep_prompt_seed])

    started = time.time()
    completed = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=timeout + 60)
    elapsed = time.time() - started
    work_dir.mkdir(parents=True, exist_ok=True)
    (work_dir / "sweep-command.txt").write_text(shell_join(command) + "\n", encoding="utf-8")
    (work_dir / "sweep-run.log").write_text(completed.stdout + completed.stderr, encoding="utf-8")

    if completed.returncode != 0:
        return SweepResult(
            page_tokens=page_tokens,
            status="fail",
            work_dir=work_dir,
            total_pages=0,
            verified_restores=0,
            qatq_best_pages=0,
            qatq_bytes=0,
            zstd_bytes=0,
            lz4_bytes=0,
            raw_bytes=0,
            attention_events=0,
            elapsed_seconds=elapsed,
            failure=(completed.stderr or completed.stdout or "").strip(),
        )

    evidence = load_json(work_dir / "runtime-reclaim-evidence.json")
    attention = load_optional_json(work_dir / "attention-trace-summary.json")
    return SweepResult(
        page_tokens=page_tokens,
        status="pass",
        work_dir=work_dir,
        total_pages=int(evidence.get("total_pages", 0)),
        verified_restores=int(evidence.get("verified_restores", 0)),
        qatq_best_pages=int(evidence.get("qatq_beats_best_general_codec_pages", 0)),
        qatq_bytes=int(evidence.get("qatq_candidate_bytes", 0)),
        zstd_bytes=int(evidence.get("zstd_bytes", 0)),
        lz4_bytes=int(evidence.get("lz4_bytes", 0)),
        raw_bytes=int(evidence.get("raw_bytes", 0)),
        attention_events=int(attention.get("events", 0)),
        elapsed_seconds=elapsed,
        failure="",
    )


def build_summary(results: list[SweepResult], work_dir: Path) -> str:
    out = "# llama.cpp QATQ Page-Size Sweep\n\n"
    out += "Generated by `scripts/llama_cpp_live_vram_page_size_sweep.py`.\n\n"
    out += f"- work dir: `{work_dir}`\n"
    out += f"- page sizes: `{','.join(str(result.page_tokens) for result in results)}`\n"
    out += f"- passed: `{sum(1 for result in results if result.status == 'pass')}`\n"
    out += f"- failed: `{sum(1 for result in results if result.status != 'pass')}`\n\n"
    out += "| page tokens | status | exact restores | pages beating best codec | attention reads | raw MiB | QATQ MiB | zstd MiB | lz4 MiB | elapsed s |\n"
    out += "| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"
    for result in results:
        out += (
            f"| {result.page_tokens} | {result.status} | {result.verified_restores}/{result.total_pages} | "
            f"{result.qatq_best_pages}/{result.total_pages} | {result.attention_events} | "
            f"{mib(result.raw_bytes):.2f} | {mib(result.qatq_bytes):.2f} | {mib(result.zstd_bytes):.2f} | "
            f"{mib(result.lz4_bytes):.2f} | {result.elapsed_seconds:.2f} |\n"
        )
    passing = [result for result in results if result.status == "pass"]
    if passing:
        selected = sorted(passing, key=lambda result: (result.page_tokens, result.qatq_bytes))[0]
        out += f"\nRecommended first experimental page size: `{selected.page_tokens}` tokens.\n"
        out += "This chooses the smallest passing page size, because smaller pages give the future runtime allocator finer reclaim granularity.\n"
    failures = [result for result in results if result.status != "pass"]
    if failures:
        out += "\n## Failures\n\n"
        for result in failures:
            out += f"### {result.page_tokens} tokens\n\n"
            out += fenced(result.failure[-4000:] or "unknown failure")
            out += "\n"
    out += "\n## Claim Boundary\n\n"
    out += "- Supported: token-page export sizes above were tested through the same runtime-reclaim evidence gate.\n"
    out += "- Not supported: this sweep does not prove transparent live token-page eviction from GPU memory.\n"
    return out


def parse_page_sizes(value: str) -> list[int]:
    sizes: list[int] = []
    seen = set()
    for raw in value.split(","):
        part = raw.strip()
        require(part, "--page-tokens contains an empty entry")
        parsed = int(part)
        require(parsed > 0, "--page-tokens values must be positive")
        if parsed not in seen:
            sizes.append(parsed)
            seen.add(parsed)
    require(sizes, "--page-tokens did not contain any sizes")
    return sizes


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
