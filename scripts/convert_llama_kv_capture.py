#!/usr/bin/env python3
"""Convert a pinned llama.cpp QATQ KV export into the geometry capture contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
from pathlib import Path


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--raw", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--capture-id", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--model-family", required=True)
    parser.add_argument("--runtime-version", required=True)
    parser.add_argument("--prompt-class", required=True)
    parser.add_argument("--prompt-file", type=Path, required=True)
    parser.add_argument("--heads", type=int, required=True)
    parser.add_argument("--layers", help="comma-separated layer indexes; default keeps every layer")
    parser.add_argument("--rope-stage", choices=("pre_rope", "post_rope", "unknown"), default="unknown")
    return parser.parse_args()


def main() -> None:
    args = arguments()
    if args.heads <= 0:
        raise SystemExit("--heads must be greater than zero")
    raw_manifest = json.loads((args.raw / "manifest.json").read_text())
    if raw_manifest.get("format") != "qatq-llama-cpp-kv-v1":
        raise SystemExit("unsupported llama.cpp KV manifest format")
    prompt = args.prompt_file.read_bytes()
    capture = bytearray()
    tensors = []
    dtype = None
    selected_layers = None if args.layers is None else {int(value) for value in args.layers.split(",")}
    for source in raw_manifest["tensors"]:
        layer_match = re.search(r"_l(\d+)$", source["name"])
        if layer_match is None:
            raise SystemExit(f"cannot derive layer from {source['name']}")
        layer = int(layer_match.group(1))
        if selected_layers is not None and layer not in selected_layers:
            continue
        source_dtype = {"f16le": "f16", "bf16le": "bf16"}.get(source["dtype"])
        if source_dtype is None:
            raise SystemExit(f"unsupported dtype {source['dtype']}")
        if dtype is not None and dtype != source_dtype:
            raise SystemExit("mixed dtypes are not supported by capture schema v1")
        dtype = source_dtype
        embedding = int(source["embedding"])
        if embedding % args.heads:
            raise SystemExit(f"embedding {embedding} is not divisible by {args.heads} heads")
        source_path = args.raw / source["file"]
        data = source_path.read_bytes()
        expected = int(source["active_cells"]) * embedding * 2
        if len(data) != expected:
            raise SystemExit(f"{source_path} has {len(data)} bytes, expected {expected}")
        offset = len(capture)
        capture.extend(data)
        tensors.append(
            {
                "id": source["name"],
                "offset_bytes": offset,
                "byte_length": len(data),
                "layer": layer,
                "kind": {"k": "key", "v": "value"}[source["kind"]],
                "rope_stage": args.rope_stage if source["kind"] == "k" else "not_applicable",
                "token_start": 0,
                "token_count": int(source["active_cells"]),
                "heads": args.heads,
                "dimension": embedding // args.heads,
                "layout": "token_head_dimension",
            }
        )
    if not tensors:
        raise SystemExit("no tensors matched the selected layers")
    metadata = {
        "schema_version": 1,
        "capture_id": args.capture_id,
        "model": args.model,
        "model_family": args.model_family,
        "runtime": "llama.cpp",
        "runtime_version": args.runtime_version,
        "prompt_class": args.prompt_class,
        "prompt_sha256": sha256(prompt),
        "context_length": max(tensor["token_count"] for tensor in tensors),
        "dtype": dtype,
        "tensors": tensors,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.output.exists():
        raise SystemExit(f"refusing to overwrite {args.output}")
    temporary = Path(tempfile.mkdtemp(prefix=f".{args.output.name}.", dir=args.output.parent))
    try:
        (temporary / "capture.kv").write_bytes(capture)
        (temporary / "capture.json").write_text(json.dumps(metadata, indent=2) + "\n")
        provenance = {
            "schema_version": 1,
            "source_manifest": str((args.raw / "manifest.json").resolve()),
            "source_manifest_sha256": sha256((args.raw / "manifest.json").read_bytes()),
            "capture_sha256": sha256(capture),
            "metadata_sha256": sha256((temporary / "capture.json").read_bytes()),
            "prompt_sha256": sha256(prompt),
        }
        (temporary / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        os.rename(temporary, args.output)
    finally:
        if temporary.exists():
            temporary.rmdir()


if __name__ == "__main__":
    main()
