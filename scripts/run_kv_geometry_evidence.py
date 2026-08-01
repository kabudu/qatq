#!/usr/bin/env python3
"""Build the preregistered, bounded KV geometry evidence corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from dataclasses import dataclass
from pathlib import Path


PROMPTS = {
    "factual-short": "State three facts about the Moon.",
    "conversational-medium": "A teammate says a production incident made them lose confidence. Reply with empathy, ask two useful questions, and suggest a calm next step without assigning blame.",
    "code-long": "Review a Rust service design that accepts untrusted binary tensor captures. Explain validation of lengths, offsets, integer overflow, resource ceilings, deterministic sampling, non-finite values, atomic output publication, reproducible manifests, tests, and operational observability. Include concrete failure modes and mitigations for each area, then give a compact release checklist.",
}


@dataclass(frozen=True)
class Model:
    label: str
    family: str
    path: Path
    heads: int
    layers: str


def args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-simple", type=Path, required=True)
    parser.add_argument("--runtime-version", required=True)
    parser.add_argument("--model", action="append", required=True, help="label:family:path:heads:layers")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profiler", type=Path, default=Path("target/release/qatq-kv-geometry"))
    parser.add_argument("--resume", action="store_true")
    return parser.parse_args()


def parse_model(value: str) -> Model:
    parts = value.split(":", 4)
    if len(parts) != 5:
        raise SystemExit("--model must be label:family:path:heads:layers")
    label, family, path, heads, layers = parts
    model_path = Path(path)
    if not model_path.is_file():
        raise SystemExit(f"model does not exist: {model_path}")
    return Model(label, family, model_path, int(heads), layers)


def run(command: list[str]) -> None:
    subprocess.run(command, check=True)


def main() -> None:
    options = args()
    models = [parse_model(value) for value in options.model]
    if len({model.family for model in models}) < 2:
        raise SystemExit("at least two model families are required")
    if options.output.exists() and not options.resume:
        raise SystemExit(f"refusing to overwrite {options.output}")
    options.work_dir.mkdir(parents=True, exist_ok=True)
    options.output.mkdir(parents=True, exist_ok=options.resume)
    if not options.profiler.is_file():
        run(["cargo", "build", "--release", "--features", "geometry", "--bin", "qatq-kv-geometry"])
    cases = []
    predictions = {"factual-short": 8, "conversational-medium": 32, "code-long": 64}
    dtypes = ("f16", "bf16")
    for model in models:
        for prompt_name, prompt in PROMPTS.items():
            for dtype in dtypes:
                case_id = f"{model.label}-{prompt_name}-{dtype}"
                case_work = options.work_dir / case_id
                raw = case_work / "raw"
                capture = case_work / "capture"
                prompt_file = case_work / "prompt.txt"
                case_work.mkdir(parents=True, exist_ok=True)
                prompt_file.write_text(prompt + "\n")
                if not (raw / "manifest.json").is_file():
                    raw.mkdir()
                    run(
                        [
                            str(options.llama_simple), "-m", str(model.path), "-ngl", "0",
                            "-n", str(predictions[prompt_name]), "--cache-type-k", dtype,
                            "--cache-type-v", dtype, "--qatq-kv-export-dir", str(raw), prompt,
                        ]
                    )
                if not (capture / "capture.json").is_file():
                    run(
                        [
                            "python3", "scripts/convert_llama_kv_capture.py", "--raw", str(raw),
                            "--output", str(capture), "--capture-id", case_id, "--model", model.label,
                            "--model-family", model.family, "--runtime-version", options.runtime_version,
                            "--prompt-class", prompt_name.split("-")[0], "--prompt-file", str(prompt_file),
                            "--heads", str(model.heads), "--layers", model.layers, "--rope-stage", "post_rope",
                        ]
                    )
                for partition in ("layer-head-token", "layer-token", "layer-head-chunk"):
                    bundle = options.output / case_id / partition
                    if not (bundle / "geometry.json").is_file():
                        run(
                            [
                                str(options.profiler), "profile", "--capture", str(capture / "capture.kv"),
                                "--metadata", str(capture / "capture.json"), "--partition", partition,
                                "--normalization", "unit-l2", "--seed", "42", "--max-pairs", "1000000",
                                "--chunk-tokens", "32", "--output", str(bundle),
                            ]
                        )
                    geometry = json.loads((bundle / "geometry.json").read_text())
                    cases.append(
                        {
                            "case_id": case_id,
                            "partition": partition,
                            "model_family": model.family,
                            "model_sha256": file_hash(model.path),
                            "prompt_class": prompt_name.split("-")[0],
                            "context_regime": prompt_name.split("-")[1],
                            "dtype": dtype,
                            "capture_sha256": geometry["capture_sha256"],
                            "status": geometry["status"],
                            "group_count": len(geometry["groups"]),
                        }
                    )
    corpus = {
        "schema_version": 1,
        "tool": "qatq-kv-geometry",
        "seed": 42,
        "max_pairs": 1_000_000,
        "runtime": "llama.cpp",
        "runtime_version": options.runtime_version,
        "cases": cases,
        "claim_boundary": "observations only; no capacity requirement or Oracle verdict is derived",
    }
    (options.output / "corpus.json").write_text(json.dumps(corpus, indent=2) + "\n")
    manifest = []
    for path in sorted(options.output.rglob("*")):
        if path.is_file() and path.name != "corpus-manifest.json":
            manifest.append({"path": str(path.relative_to(options.output)), "bytes": path.stat().st_size, "sha256": file_hash(path)})
    (options.output / "corpus-manifest.json").write_text(json.dumps({"schema_version": 1, "files": manifest}, indent=2) + "\n")


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


if __name__ == "__main__":
    main()
