#!/usr/bin/env python3
"""Run QATQ page-bounded attention evidence over real llama.cpp exports."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


DTYPE_WIDTH = {
    "f32le": 4,
    "f16le": 2,
    "bf16le": 2,
}

CLI_DTYPE = {
    "f32le": "f32",
    "f16le": "f16",
    "bf16le": "bf16",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--export-dir", required=True, help="llama.cpp --qatq-kv-export-dir output")
    parser.add_argument("--attention-fixture-dir", required=True, help="llama.cpp --qatq-attention-fixture-dir output")
    parser.add_argument("--qatq-kv-bench", default="target/release/qatq-kv-bench")
    parser.add_argument("--layer", type=int, default=-1, help="Layer to validate; default uses the first fixture record")
    parser.add_argument("--head", type=int, default=0, help="Attention head to slice from exported K/V pages")
    parser.add_argument("--tolerance", type=float, default=1.0e-4)
    parser.add_argument(
        "--max-peak-page-kv-ratio",
        type=float,
        default=0.0,
        help="Require streaming attention peak page KV values to be at or below this materialized-KV ratio.",
    )
    parser.add_argument("--work-dir", default="", help="Directory for sliced temporary pages")
    parser.add_argument("--output", required=True, help="QATQ attention-equivalence JSON output")
    parser.add_argument("--keep-work-dir", action="store_true")
    args = parser.parse_args()

    export_dir = Path(args.export_dir)
    fixture_dir = Path(args.attention_fixture_dir)
    output = Path(args.output)
    manifest = load_json(export_dir / "manifest.json")
    fixture = load_json(fixture_dir / "attention-fixture.json")

    require(fixture.get("format") == "qatq-llama-cpp-attention-fixture-v1", "unsupported attention fixture format")
    records = fixture.get("records")
    require(isinstance(records, list) and records, "attention fixture has no records")
    record = choose_record(records, args.layer)
    layer = int(record["layer"])
    head = args.head
    head_dim = int(record["head_dim"])
    require(head >= 0, "--head must be non-negative")
    require(head_dim > 0, "attention fixture head_dim must be positive")
    require(args.max_peak_page_kv_ratio >= 0.0, "--max-peak-page-kv-ratio must be non-negative")

    query_file = fixture_dir / str(record["query_file"])
    require_file(query_file)
    work_dir = Path(args.work_dir) if args.work_dir else output.with_suffix(".pages")
    if work_dir.exists() and not args.keep_work_dir:
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    tensors = manifest.get("tensors")
    require(isinstance(tensors, list) and tensors, "KV export manifest has no tensors")
    key_entries = sorted(
        (entry for entry in tensors if entry.get("kind") == "k" and entry_layer(entry) == layer),
        key=lambda entry: (int(entry.get("stream", 0)), int(entry.get("token_start", 0))),
    )
    value_entries = sorted(
        (entry for entry in tensors if entry.get("kind") == "v" and entry_layer(entry) == layer),
        key=lambda entry: (int(entry.get("stream", 0)), int(entry.get("token_start", 0))),
    )
    require(key_entries, f"no key pages found for layer {layer}")
    require(len(key_entries) == len(value_entries), "key/value page counts differ")

    key_args: list[str] = []
    value_args: list[str] = []
    page_dtype = None
    for index, (key_entry, value_entry) in enumerate(zip(key_entries, value_entries)):
        require(int(key_entry.get("stream", 0)) == int(value_entry.get("stream", 0)), "key/value streams differ")
        require(int(key_entry.get("token_start", 0)) == int(value_entry.get("token_start", 0)), "key/value token starts differ")
        require(int(key_entry.get("token_end", 0)) == int(value_entry.get("token_end", 0)), "key/value token ends differ")
        require(int(key_entry.get("active_cells", 0)) == int(value_entry.get("active_cells", 0)), "key/value active cell counts differ")
        key_dtype = str(key_entry["dtype"])
        value_dtype = str(value_entry["dtype"])
        require(key_dtype == value_dtype, "key/value dtypes differ")
        require(key_dtype in DTYPE_WIDTH, f"unsupported dtype {key_dtype}")
        if page_dtype is None:
            page_dtype = key_dtype
        require(page_dtype == key_dtype, "mixed page dtypes are not supported")

        key_out = work_dir / f"key-l{layer}-h{head}-p{index:04d}.{key_dtype}"
        value_out = work_dir / f"value-l{layer}-h{head}-p{index:04d}.{value_dtype}"
        slice_page(export_dir / str(key_entry["file"]), key_out, key_entry, head, head_dim, transposed=False)
        slice_page(
            export_dir / str(value_entry["file"]),
            value_out,
            value_entry,
            head,
            head_dim,
            transposed=bool(value_entry.get("transposed", False)),
        )
        key_args.extend(["--attention-key-page", f"{CLI_DTYPE[key_dtype]}:{key_out}"])
        value_args.extend(["--attention-value-page", f"{CLI_DTYPE[value_dtype]}:{value_out}"])

    command = [
        str(Path(args.qatq_kv_bench)),
        "--attention-query",
        f"f32:{query_file}",
        *key_args,
        *value_args,
        "--attention-head-dim",
        str(head_dim),
        "--attention-value-dim",
        str(head_dim),
        "--attention-tolerance",
        str(args.tolerance),
        "--attention-equivalence-gate",
        "--output",
        str(output),
    ]
    if args.max_peak_page_kv_ratio > 0.0:
        command.extend(["--attention-max-peak-page-kv-ratio", str(args.max_peak_page_kv_ratio)])
    completed = subprocess.run(command, text=True, capture_output=True)
    (work_dir / "qatq-kv-bench-command.txt").write_text(shell_join(command) + "\n", encoding="utf-8")
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr or completed.stdout)
        return completed.returncode
    if not args.keep_work_dir:
        shutil.rmtree(work_dir)
    return 0


def slice_page(source: Path, destination: Path, entry: dict, head: int, head_dim: int, *, transposed: bool) -> None:
    require_file(source)
    dtype = str(entry["dtype"])
    width = DTYPE_WIDTH[dtype]
    embedding = int(entry["embedding"])
    tokens = int(entry["active_cells"])
    require(tokens > 0, f"{source} has no active cells")
    require(embedding >= (head + 1) * head_dim, f"{source} embedding is too small for head {head}")
    raw = source.read_bytes()
    expected = embedding * tokens * width
    require(len(raw) == expected, f"{source} has {len(raw)} bytes, expected {expected}")
    out = bytearray(tokens * head_dim * width)
    for token in range(tokens):
        for dim in range(head_dim):
            source_dim = head * head_dim + dim
            if transposed:
                src = (source_dim * tokens + token) * width
            else:
                src = (token * embedding + source_dim) * width
            dst = (token * head_dim + dim) * width
            out[dst : dst + width] = raw[src : src + width]
    destination.write_bytes(out)


def choose_record(records: list[dict], layer: int) -> dict:
    if layer < 0:
        return records[0]
    for record in records:
        if int(record.get("layer", -1)) == layer:
            return record
    raise SystemExit(f"attention fixture does not contain layer {layer}")


def entry_layer(entry: dict) -> int:
    if "layer" in entry:
        return int(entry["layer"])
    return layer_from_name(entry)


def layer_from_name(entry: dict) -> int:
    name = str(entry.get("name", ""))
    marker = "_l"
    if marker not in name:
        raise SystemExit(f"tensor entry is missing layer metadata: {name}")
    tail = name.split(marker, 1)[1]
    digits = []
    for ch in tail:
        if ch.isdigit():
            digits.append(ch)
        else:
            break
    require(digits, f"tensor entry has invalid layer name: {name}")
    return int("".join(digits))


def load_json(path: Path) -> dict:
    require_file(path)
    return json.loads(path.read_text(encoding="utf-8"))


def require_file(path: Path) -> None:
    require(path.is_file(), f"missing file: {path}")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def shell_join(command: list[str]) -> str:
    return " ".join(quote(arg) for arg in command)


def quote(value: str) -> str:
    if value and all(ch.isalnum() or ch in "._/-:=+" for ch in value):
        return value
    return "'" + value.replace("'", "'\"'\"'") + "'"


if __name__ == "__main__":
    raise SystemExit(main())
