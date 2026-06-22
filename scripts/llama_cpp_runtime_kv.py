#!/usr/bin/env python3
"""Run llama.cpp runtime prompts and optional direct KV tensor compression.

The script intentionally separates two claims:

1. Real local llama.cpp model/task execution over representative prompts.
2. Direct raw KV tensor compression, only when a patched/exporting llama.cpp
   adapter has written `.f16le`, `.bf16le`, or `.f32le` tensor files.

It does not treat llama.cpp session/state blobs as raw KV tensors.
"""

from __future__ import annotations

import argparse
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path


DEFAULT_MODELS = [
    (
        "daily-driver",
        "/Users/kabudu/projex/deliberium-group/deliberium/models/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
    ),
    (
        "software-engineering",
        "/Users/kabudu/projex/deliberium-group/deliberium/models/Qwen2.5-Coder-3B-Instruct-Q4_K_M.gguf",
    ),
]

PROMPTS = [
    (
        "daily-summary",
        "Summarize this plan in three practical bullets: migrate a small Rust codec from research prototype to release candidate with stronger tests and runtime benchmarks.",
    ),
    (
        "engineering-debug",
        "You are reviewing a Rust binary codec. Name three bugs you would look for in chunked decode before trusting it in production.",
    ),
    (
        "engineering-implementation",
        "Write a concise Rust test name and assertion idea for proving an f16 little-endian tensor round-trips without widening to f32.",
    ),
]


@dataclass
class ModelSpec:
    label: str
    path: Path


@dataclass
class PromptResult:
    model: str
    prompt: str
    ok: bool
    elapsed: float
    stdout: str
    stderr: str
    error: str


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-cli", default="/opt/homebrew/bin/llama-cli")
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--model", action="append", default=[], help="label:path")
    parser.add_argument("--kv-dir", default="")
    parser.add_argument("--kv-report", default="docs/LLAMA_CPP_KV_COMPRESSION_REPORT.md")
    parser.add_argument("--report", default="docs/LLAMA_CPP_RUNTIME_KV_EXPERIMENTS.md")
    parser.add_argument("--work-dir", default="captures/llama-cpp-runtime")
    parser.add_argument("--ctx-size", type=int, default=1024)
    parser.add_argument("--predict", type=int, default=64)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--cache-dtype", default="f16", choices=["f16", "bf16", "f32"])
    args = parser.parse_args()

    root = Path.cwd()
    work_dir = root / args.work_dir
    work_dir.mkdir(parents=True, exist_ok=True)
    models = parse_models(args.model)
    started = time.time()
    results = []
    for model in models:
        for prompt_label, prompt in PROMPTS:
            results.append(run_prompt(args, model, prompt_label, prompt, work_dir))

    kv_report_text = ""
    kv_dir = Path(args.kv_dir) if args.kv_dir else None
    if kv_dir and kv_dir.exists():
        kv_report = root / args.kv_report
        ensure_kv_bench(root / args.qatq_kv_bench)
        command = [
            str(root / args.qatq_kv_bench),
            "--dir",
            str(kv_dir),
            "--iters",
            "5",
            "--output",
            str(kv_report),
        ]
        kv_run = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=args.timeout)
        if kv_run.returncode == 0:
            kv_report_text = kv_report.read_text(encoding="utf-8")
        else:
            kv_report_text = (
                "## KV Compression Attempt\n\n"
                f"- command: `{shell_join(command)}`\n"
                f"- exit code: `{kv_run.returncode}`\n"
                f"- stderr: `{trim(kv_run.stderr, 2000)}`\n"
            )
    elif kv_dir:
        kv_report_text = (
            "## KV Compression Attempt\n\n"
            f"- requested KV directory: `{kv_dir}`\n"
            "- result: directory does not exist, so direct KV tensor compression was not run.\n"
        )

    report = render_report(args, models, results, kv_report_text, time.time() - started)
    report_path = root / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(report, encoding="utf-8")
    print(report)
    return 0 if all(result.ok for result in results) else 1


def parse_models(values: list[str]) -> list[ModelSpec]:
    if not values:
        values = [f"{label}:{path}" for label, path in DEFAULT_MODELS if Path(path).exists()]
    models = []
    for value in values:
        label, sep, path = value.partition(":")
        if not sep or not label or not path:
            raise SystemExit(f"invalid --model {value!r}; expected label:path")
        model_path = Path(path)
        if not model_path.exists():
            raise SystemExit(f"model path does not exist: {model_path}")
        models.append(ModelSpec(label, model_path))
    if not models:
        raise SystemExit("no model paths found; pass --model label:/path/to/model.gguf")
    return models


def run_prompt(args, model: ModelSpec, prompt_label: str, prompt: str, work_dir: Path) -> PromptResult:
    stdout_path = work_dir / f"{model.label}-{prompt_label}.stdout.txt"
    stderr_path = work_dir / f"{model.label}-{prompt_label}.stderr.txt"
    command = [
        args.llama_cli,
        "-m",
        str(model.path),
        "--device",
        "none",
        "--gpu-layers",
        "0",
        "--no-kv-offload",
        "--cache-type-k",
        args.cache_dtype,
        "--cache-type-v",
        args.cache_dtype,
        "--ctx-size",
        str(args.ctx_size),
        "--predict",
        str(args.predict),
        "--temp",
        "0",
        "--top-k",
        "1",
        "--seed",
        "51415451",
        "--no-display-prompt",
        "--no-warmup",
        "--single-turn",
        "--simple-io",
        "--log-disable",
        "--prompt",
        prompt,
    ]
    started = time.time()
    try:
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            timeout=args.timeout,
            cwd=Path.cwd(),
        )
        elapsed = time.time() - started
        stdout_path.write_text(completed.stdout, encoding="utf-8")
        stderr_path.write_text(completed.stderr, encoding="utf-8")
        return PromptResult(
            model=model.label,
            prompt=prompt_label,
            ok=completed.returncode == 0,
            elapsed=elapsed,
            stdout=completed.stdout,
            stderr=completed.stderr,
            error="" if completed.returncode == 0 else f"exit code {completed.returncode}",
        )
    except subprocess.TimeoutExpired as error:
        elapsed = time.time() - started
        stdout = text_or_empty(error.stdout)
        stderr = text_or_empty(error.stderr)
        stdout_path.write_text(stdout, encoding="utf-8")
        stderr_path.write_text(stderr, encoding="utf-8")
        return PromptResult(
            model=model.label,
            prompt=prompt_label,
            ok=False,
            elapsed=elapsed,
            stdout=stdout,
            stderr=stderr,
            error=f"timeout after {args.timeout}s",
        )


def ensure_kv_bench(path: Path) -> None:
    if path.exists():
        return
    subprocess.run(["cargo", "build", "--release", "--bin", "qatq-kv-bench"], check=True)


def render_report(args, models, results, kv_report_text: str, elapsed: float) -> str:
    ok_count = sum(1 for result in results if result.ok)
    out = "# llama.cpp Runtime KV Experiment\n\n"
    out += "Generated by `scripts/llama_cpp_runtime_kv.py`.\n\n"
    out += "## Runtime Prompt Matrix\n\n"
    out += f"- llama executable: `{args.llama_cli}`\n"
    out += f"- cache dtype: `{args.cache_dtype}`\n"
    out += f"- ctx size: `{args.ctx_size}`\n"
    out += f"- generated tokens per prompt: `{args.predict}`\n"
    out += f"- prompt runs: `{ok_count}/{len(results)}` succeeded\n"
    out += f"- elapsed seconds: `{elapsed:.2f}`\n\n"
    out += "| model | prompt | status | elapsed seconds | output excerpt | error |\n"
    out += "| --- | --- | --- | ---: | --- | --- |\n"
    for result in results:
        out += (
            f"| {result.model} | {result.prompt} | {'ok' if result.ok else 'failed'} | "
            f"{result.elapsed:.2f} | {md(trim(clean_output(result.stdout), 240))} | {md(result.error)} |\n"
        )
    out += "\n## Models\n\n"
    for model in models:
        out += f"- `{model.label}`: `{model.path}`\n"
    out += "\n## Direct KV Tensor Compression\n\n"
    if kv_report_text:
        out += kv_report_text + "\n"
    else:
        out += (
            "- direct raw KV tensor compression was not run because no `--kv-dir` was provided.\n"
            "- the installed llama.cpp public CLI/API exposes KV dtype controls and state/session save APIs, but not raw per-layer `cache_k_l*` / `cache_v_l*` tensor export.\n"
            "- rerun with `--kv-dir <dir>` after a patched llama.cpp exporter writes `.f16le`, `.bf16le`, or `.f32le` tensors.\n"
        )
    out += "\n## Claim Boundary\n\n"
    out += "- Supported: the prompt matrix uses real local GGUF models through llama.cpp with deterministic CPU-only settings.\n"
    out += "- Supported when `--kv-dir` is present: QATQ exact, zstd, and lz4 are compared on direct raw typed KV tensor files, with exact decode checks.\n"
    out += "- Not supported by an unpatched Homebrew llama.cpp build: direct raw internal KV tensor export from the public CLI/API.\n"
    return out


def clean_output(text: str) -> str:
    lines = []
    for line in strip_control_chars(text).replace("\r", "\n").splitlines():
        line = line.strip()
        if not line or line.startswith("load_backend:") or line.startswith("build"):
            continue
        if "Loading model..." in line:
            continue
        if is_llama_banner_line(line):
            continue
        if line.startswith(("model", "modalities", "available commands:", "/exit", "/regen", "/clear", "/read", "/glob")):
            continue
        if line.startswith((">", "[ Prompt:", "Exiting...")):
            continue
        if line.startswith("[ Prompt:"):
            continue
        lines.append(line)
    return " ".join(lines)


def trim(text: str, length: int) -> str:
    text = " ".join(str(text).split())
    if len(text) <= length:
        return text
    return text[: length - 1] + "…"


def md(text: str) -> str:
    return str(text).replace("|", "\\|").replace("\n", " ")


def shell_join(command: list[str]) -> str:
    return " ".join(command)


def text_or_empty(value) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return str(value)


def strip_control_chars(text: str) -> str:
    return "".join(ch for ch in text if ch == "\n" or ch == "\r" or ord(ch) >= 32)


def is_llama_banner_line(line: str) -> bool:
    return bool(line) and all(ch in "▄▀█ ETHMLNRCB.| /\\-" for ch in line)


if __name__ == "__main__":
    raise SystemExit(main())
