#!/usr/bin/env python3
"""Run a broader direct llama.cpp KV export/compression matrix.

This script expects a patched llama.cpp `llama-simple` binary with QATQ's
`--qatq-kv-export-dir`, `--cache-type-k`, and `--cache-type-v` flags.
It keeps raw tensor captures outside the repository by default and writes only
the Markdown evidence report.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path


DEFAULT_MODELS = [
    (
        "qwen2.5-1.5b",
        os.environ.get("QATQ_LLAMA_MODEL_QWEN25_15B", ""),
    ),
    (
        "qwen2.5-coder-3b",
        os.environ.get("QATQ_LLAMA_MODEL_QWEN25_CODER_3B", ""),
    ),
]

DEFAULT_PROMPTS = [
    (
        "daily-driver",
        "Summarize a project status update into three concrete next actions for a technical founder.",
    ),
    (
        "software-engineering",
        "Review this Rust codec release plan and name the top three production risks to test before publishing.",
    ),
    (
        "long-context",
        "You are maintaining a compression codec. Explain how to validate exact tensor round trips, resource limits, fuzzing, and benchmark reproducibility. "
        "Include details about native f16 and bf16 KV cache tensors, packed transport bundles, and corruption handling.",
    ),
]


@dataclass(frozen=True)
class ModelSpec:
    label: str
    path: Path


@dataclass(frozen=True)
class PromptSpec:
    label: str
    text: str


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-simple", default="/tmp/qatq-llama.cpp/build-qatq/bin/llama-simple")
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--model", action="append", default=[], help="label:path")
    parser.add_argument("--prompt", action="append", default=[], help="label:text")
    parser.add_argument("--dtype", action="append", choices=["f16", "bf16", "f32"], default=[])
    parser.add_argument("--predict", type=int, action="append", default=[], help="repeatable token budget")
    parser.add_argument("--work-dir", default="/tmp/qatq-llama-kv-matrix")
    parser.add_argument("--report", default="docs/LLAMA_CPP_KV_MATRIX.md")
    parser.add_argument("--iters", type=int, default=3)
    parser.add_argument("--max-cases", type=int, default=0)
    parser.add_argument("--timeout", type=int, default=240)
    args = parser.parse_args()

    root = Path.cwd()
    models = parse_models(args.model)
    prompts = parse_prompts(args.prompt)
    dtypes = args.dtype or ["f16", "bf16", "f32"]
    predicts = args.predict or [16, 64]
    work_dir = Path(args.work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)
    ensure_kv_bench(root / args.qatq_kv_bench)

    cases = []
    for model in models:
        for prompt in prompts:
            for dtype in dtypes:
                for predict in predicts:
                    cases.append((model, prompt, dtype, predict))
    if args.max_cases > 0:
        cases = cases[: args.max_cases]

    started = time.time()
    rows = []
    details = []
    for index, (model, prompt, dtype, predict) in enumerate(cases, start=1):
        case_label = safe_name(f"{index:02d}-{model.label}-{prompt.label}-{dtype}-n{predict}")
        export_dir = work_dir / case_label / "raw"
        packed_dir = work_dir / case_label / "packed"
        report_path = work_dir / case_label / "packed-report.md"
        shutil.rmtree(export_dir.parent, ignore_errors=True)
        export_dir.mkdir(parents=True)
        command = [
            args.llama_simple,
            "-m",
            str(model.path),
            "-ngl",
            "0",
            "-n",
            str(predict),
            "--cache-type-k",
            dtype,
            "--cache-type-v",
            dtype,
            "--qatq-kv-export-dir",
            str(export_dir),
            prompt.text,
        ]
        run = subprocess.run(command, text=True, capture_output=True, timeout=args.timeout)
        if run.returncode != 0:
            rows.append(f"| {case_label} | failed | - | - | - | - | - | - | `{md(trim(run.stderr, 160))}` |")
            continue
        pack_exports(export_dir, packed_dir, dtype)
        bench_command = [
            str(root / args.qatq_kv_bench),
            "--dir",
            str(packed_dir),
            "--iters",
            str(args.iters),
            "--output",
            str(report_path),
        ]
        bench = subprocess.run(bench_command, cwd=root, text=True, capture_output=True, timeout=args.timeout)
        if bench.returncode != 0:
            rows.append(f"| {case_label} | bench failed | - | - | - | - | - | - | `{md(trim(bench.stderr, 160))}` |")
            continue
        parsed = parse_qatq_kv_bench(report_path)
        all_row = parsed["cache_all_packed"]
        winner = "QATQ" if all_row["qatq_bytes"] < all_row["zstd_bytes"] and all_row["qatq_bytes"] < all_row["lz4_bytes"] else "baseline"
        rows.append(
            "| "
            + " | ".join(
                [
                    case_label,
                    "ok",
                    str(all_row["raw_bytes"]),
                    str(all_row["qatq_bytes"]),
                    f"{all_row['qatq_ratio']:.4f}",
                    str(all_row["zstd_bytes"]),
                    f"{all_row['zstd_ratio']:.4f}",
                    winner,
                    "",
                ]
            )
            + " |"
        )
        details.append((case_label, report_path.read_text(encoding="utf-8")))

    report = render_report(args, models, prompts, dtypes, predicts, rows, details, time.time() - started)
    output = root / args.report
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(report, encoding="utf-8")
    print(report)
    return 0


def parse_models(values: list[str]) -> list[ModelSpec]:
    if not values:
        values = [
            f"{label}:{path}"
            for label, path in DEFAULT_MODELS
            if path and Path(path).is_file()
        ]
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


def parse_prompts(values: list[str]) -> list[PromptSpec]:
    if not values:
        values = [f"{label}:{text}" for label, text in DEFAULT_PROMPTS]
    prompts = []
    for value in values:
        label, sep, text = value.partition(":")
        if not sep or not label or not text:
            raise SystemExit(f"invalid --prompt {value!r}; expected label:text")
        prompts.append(PromptSpec(label, text))
    return prompts


def ensure_kv_bench(path: Path) -> None:
    if path.exists():
        return
    subprocess.run(["cargo", "build", "--release", "--bin", "qatq-kv-bench"], check=True)


def pack_exports(export_dir: Path, packed_dir: Path, dtype: str) -> None:
    packed_dir.mkdir(parents=True, exist_ok=True)
    suffix = f".{dtype}le"
    groups = {
        "cache_k_packed": sorted(export_dir.glob(f"cache_k_*{suffix}")),
        "cache_v_packed": sorted(export_dir.glob(f"cache_v_*{suffix}")),
        "cache_all_packed": sorted(export_dir.glob(f"cache_*{suffix}")),
    }
    for name, paths in groups.items():
        if not paths:
            raise RuntimeError(f"no exported tensors matched {name} in {export_dir}")
        with (packed_dir / f"{name}{suffix}").open("wb") as out:
            for path in paths:
                out.write(path.read_bytes())


def parse_qatq_kv_bench(path: Path) -> dict[str, dict[str, float]]:
    rows = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("| cache_"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        rows[cells[0]] = {
            "raw_bytes": int(cells[3]),
            "qatq_bytes": int(cells[4]),
            "qatq_ratio": float(cells[5]),
            "zstd_bytes": int(cells[7]),
            "zstd_ratio": float(cells[8]),
            "lz4_bytes": int(cells[9]),
            "lz4_ratio": float(cells[10]),
        }
    return rows


def render_report(args, models, prompts, dtypes, predicts, rows, details, elapsed: float) -> str:
    out = "# llama.cpp KV Matrix\n\n"
    out += "Generated by `scripts/llama_cpp_kv_matrix.py`.\n\n"
    out += f"- llama-simple: `{args.llama_simple}`\n"
    out += f"- dtypes: `{', '.join(dtypes)}`\n"
    out += f"- token budgets: `{', '.join(str(p) for p in predicts)}`\n"
    out += f"- elapsed seconds: `{elapsed:.2f}`\n\n"
    out += "## Models\n\n"
    for model in models:
        out += f"- `{model.label}`: `{model.path}`\n"
    out += "\n## Prompts\n\n"
    for prompt in prompts:
        out += f"- `{prompt.label}`: {prompt.text}\n"
    out += "\n## Packed All-KV Results\n\n"
    out += "| case | status | raw bytes | QATQ bytes | QATQ ratio | zstd bytes | zstd ratio | winner | note |\n"
    out += "| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |\n"
    out += "\n".join(rows)
    out += "\n\n## Per-Case Reports\n\n"
    for label, text in details:
        out += f"### {label}\n\n"
        out += trim_table(text)
        out += "\n\n"
    out += "## Claim Boundary\n\n"
    out += "- The matrix benchmarks packed exported KV bundles, the production-relevant transport shape.\n"
    out += "- Raw per-layer captures remain useful for debugging but are not the compression claim surface.\n"
    out += "- Every QATQ/zstd/lz4 row is exact decode-checked by `qatq-kv-bench`.\n"
    return out


def trim_table(text: str) -> str:
    lines = []
    keep = False
    for line in text.splitlines():
        if keep and line.startswith("## Claim Boundary"):
            break
        if line.startswith("| tensor |"):
            keep = True
        if keep:
            lines.append(line)
    return "\n".join(lines).rstrip()


def safe_name(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "-._" else "-" for ch in value).strip("-")


def trim(text: str, length: int) -> str:
    text = " ".join(text.split())
    if len(text) <= length:
        return text
    return text[: length - 1] + "..."


def md(text: str) -> str:
    return text.replace("|", "\\|").replace("\n", " ")


if __name__ == "__main__":
    raise SystemExit(main())
