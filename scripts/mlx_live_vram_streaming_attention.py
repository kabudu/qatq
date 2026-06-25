#!/usr/bin/env python3
"""Validate page-bounded attention over llama.cpp KV pages on MLX.

This is an integration proof for the live-VRAM roadmap. It consumes a real
llama.cpp QATQ attention fixture plus exported KV page files, then compares:

- materialised attention, where all selected K/V pages are resident together;
- streaming attention, where one K/V page is loaded to MLX at a time and the
  online softmax state is carried across pages.

The script intentionally reports aggregate metrics only. It does not print or
persist prompt text, query contents, KV payloads, or attention outputs.
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

import mlx.core as mx
import numpy as np


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--export-dir", required=True, help="llama.cpp QATQ KV export directory")
    parser.add_argument("--attention-fixture-dir", required=True, help="directory containing attention-fixture.json")
    parser.add_argument("--layer", type=int, default=0)
    parser.add_argument("--head", type=int, default=0)
    parser.add_argument("--tolerance", type=float, default=1.0e-4)
    parser.add_argument("--max-peak-page-kv-ratio", type=float, default=0.75)
    parser.add_argument(
        "--stream-from-qatq-store",
        action="store_true",
        help="Encode selected K/V pages to QATQ and stream from verified decoded page restores",
    )
    parser.add_argument("--qatq-bin", default="target/release/qatq", help="qatq CLI used for page encode/decode")
    parser.add_argument("--qatq-store-dir", help="directory for encoded QATQ page store")
    parser.add_argument("--keep-qatq-store", action="store_true")
    parser.add_argument(
        "--allow-qatq-expansion",
        action="store_true",
        help="do not fail if the selected QATQ page store is larger than raw selected pages",
    )
    parser.add_argument("--output", help="optional JSON report path")
    args = parser.parse_args()

    try:
        report = run(args)
    except Exception as error:  # noqa: BLE001 - CLI should fail with one clear line.
        print(f"mlx-live-vram-streaming-attention: {error}", file=sys.stderr)
        return 1

    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    if not report["passed"]:
        print(
            "mlx-live-vram-streaming-attention: gate failed: "
            f"max_abs_error={report['max_abs_error']:.9g}, "
            f"peak_page_kv_ratio={report['streaming']['peak_page_kv_ratio']:.9g}",
            file=sys.stderr,
        )
        return 1
    return 0


def run(args: argparse.Namespace) -> dict[str, Any]:
    if args.layer < -1 or args.head < -1:
        raise ValueError("--layer and --head must be non-negative, or -1 for all")
    if not math.isfinite(args.tolerance) or args.tolerance < 0.0:
        raise ValueError("--tolerance must be finite and non-negative")
    if not math.isfinite(args.max_peak_page_kv_ratio) or args.max_peak_page_kv_ratio <= 0.0:
        raise ValueError("--max-peak-page-kv-ratio must be finite and positive")

    export_dir = Path(args.export_dir)
    fixture_dir = Path(args.attention_fixture_dir)
    manifest = load_json(export_dir / "manifest.json")
    fixture = load_json(fixture_dir / "attention-fixture.json")
    if args.layer == -1:
        return run_layer_sweep(args=args, fixture=fixture)
    key_pages_meta = select_pages(manifest, kind="k", layer=args.layer)
    value_pages_meta = select_pages(manifest, kind="v", layer=args.layer)
    if len(key_pages_meta) != len(value_pages_meta):
        raise ValueError("key/value page count mismatch")

    page_store: QatqPageStore | None = None
    if args.stream_from_qatq_store:
        page_store = build_qatq_page_store(
            export_dir=export_dir,
            key_pages_meta=key_pages_meta,
            value_pages_meta=value_pages_meta,
            qatq_bin=Path(args.qatq_bin),
            store_dir=Path(args.qatq_store_dir) if args.qatq_store_dir else None,
            keep_store=args.keep_qatq_store,
        )
        if not args.allow_qatq_expansion and page_store.stored_bytes >= page_store.raw_bytes:
            page_store.cleanup()
            raise ValueError(
                "QATQ page store did not shrink selected pages: "
                f"{page_store.stored_bytes} >= {page_store.raw_bytes}"
            )

    try:
        if args.head == -1:
            records = find_fixture_records_for_layer(fixture, args.layer)
            reports = [
                run_head_check(
                    args=args,
                    export_dir=export_dir,
                    fixture_dir=fixture_dir,
                    fixture=fixture,
                    record=record,
                    key_pages_meta=key_pages_meta,
                    value_pages_meta=value_pages_meta,
                    page_store=page_store,
                )
                for record in records
            ]
            max_abs_error = max(report["max_abs_error"] for report in reports)
            max_relative_error = max(report["max_relative_error"] for report in reports)
            max_peak_ratio = max(report["streaming"]["peak_page_kv_ratio"] for report in reports)
            passed = all(report["passed"] for report in reports)
            aggregate = {
                "format": "qatq-mlx-live-vram-streaming-attention-v1",
                "passed": passed,
                "device": str(mx.default_device()),
                "export_dir": str(export_dir),
                "attention_fixture_dir": str(fixture_dir),
                "layer": args.layer,
                "head": "all",
                "heads_checked": len(reports),
                "head_dim": reports[0]["head_dim"],
                "pages": reports[0]["pages"],
                "tokens": reports[0]["tokens"],
                "dtype": reports[0]["dtype"],
                "max_abs_error": max_abs_error,
                "max_relative_error": max_relative_error,
                "tolerance": args.tolerance,
                "materialized": {
                    "seconds": sum(report["materialized"]["seconds"] for report in reports),
                    "max_peak_memory_bytes": max(report["materialized"]["peak_memory_bytes"] for report in reports),
                },
                "streaming": {
                    "seconds": sum(report["streaming"]["seconds"] for report in reports),
                    "max_peak_memory_bytes": max(report["streaming"]["peak_memory_bytes"] for report in reports),
                    "max_peak_page_kv_ratio": max_peak_ratio,
                    "peak_page_kv_ratio": max_peak_ratio,
                },
                "gate": {
                    "max_abs_error": args.tolerance,
                    "max_peak_page_kv_ratio": args.max_peak_page_kv_ratio,
                },
                "heads": reports,
            }
            attach_qatq_store_report(aggregate, page_store, args.keep_qatq_store)
            return aggregate

        record = find_fixture_record(fixture, args.layer, args.head)
        report = run_head_check(
            args=args,
            export_dir=export_dir,
            fixture_dir=fixture_dir,
            fixture=fixture,
            record=record,
            key_pages_meta=key_pages_meta,
            value_pages_meta=value_pages_meta,
            page_store=page_store,
        )
        attach_qatq_store_report(report, page_store, args.keep_qatq_store)
        return report
    finally:
        if page_store is not None and not args.keep_qatq_store:
            page_store.cleanup()


def run_layer_sweep(*, args: argparse.Namespace, fixture: dict[str, Any]) -> dict[str, Any]:
    layers = find_fixture_layers(fixture)
    reports = []
    for layer in layers:
        layer_args = argparse.Namespace(**vars(args))
        layer_args.layer = layer
        if args.qatq_store_dir:
            layer_args.qatq_store_dir = str(Path(args.qatq_store_dir) / f"layer-{layer}")
        reports.append(run(layer_args))

    max_abs_error = max(report["max_abs_error"] for report in reports)
    max_relative_error = max(report["max_relative_error"] for report in reports)
    max_peak_ratio = max(report["streaming"]["peak_page_kv_ratio"] for report in reports)
    heads_checked = sum(int(report.get("heads_checked", 1)) for report in reports)
    passed = all(report["passed"] for report in reports)
    store_reports = [report["qatq_store"] for report in reports if report["qatq_store"]["enabled"]]
    aggregate = {
        "format": "qatq-mlx-live-vram-streaming-attention-v1",
        "passed": passed,
        "device": str(mx.default_device()),
        "export_dir": str(Path(args.export_dir)),
        "attention_fixture_dir": str(Path(args.attention_fixture_dir)),
        "layer": "all",
        "layers_checked": len(reports),
        "layers": layers,
        "head": "all" if args.head == -1 else args.head,
        "heads_checked": heads_checked,
        "max_abs_error": max_abs_error,
        "max_relative_error": max_relative_error,
        "tolerance": args.tolerance,
        "materialized": {
            "seconds": sum(report["materialized"]["seconds"] for report in reports),
            "max_peak_memory_bytes": max(report["materialized"]["max_peak_memory_bytes"] for report in reports),
        },
        "streaming": {
            "seconds": sum(report["streaming"]["seconds"] for report in reports),
            "max_peak_memory_bytes": max(report["streaming"]["max_peak_memory_bytes"] for report in reports),
            "max_peak_page_kv_ratio": max_peak_ratio,
            "peak_page_kv_ratio": max_peak_ratio,
        },
        "gate": {
            "max_abs_error": args.tolerance,
            "max_peak_page_kv_ratio": args.max_peak_page_kv_ratio,
        },
        "layer_reports": reports,
    }
    if store_reports:
        raw_bytes = sum(int(report["raw_bytes"]) for report in store_reports)
        stored_bytes = sum(int(report["stored_bytes"]) for report in store_reports)
        aggregate["qatq_store"] = {
            "enabled": True,
            "dir": str(Path(args.qatq_store_dir)) if args.qatq_store_dir else None,
            "pages": sum(int(report["pages"]) for report in store_reports),
            "raw_bytes": raw_bytes,
            "stored_bytes": stored_bytes,
            "decoded_bytes": sum(int(report["decoded_bytes"]) for report in store_reports),
            "compression_ratio": float(stored_bytes) / float(raw_bytes) if raw_bytes else 1.0,
            "decode_seconds": sum(float(report["decode_seconds"]) for report in store_reports),
            "encode_seconds": sum(float(report["encode_seconds"]) for report in store_reports),
            "kept": args.keep_qatq_store,
        }
    else:
        aggregate["qatq_store"] = {"enabled": False}
    return aggregate


def run_head_check(
    *,
    args: argparse.Namespace,
    export_dir: Path,
    fixture_dir: Path,
    fixture: dict[str, Any],
    record: dict[str, Any],
    key_pages_meta: list[dict[str, Any]],
    value_pages_meta: list[dict[str, Any]],
    page_store: "QatqPageStore | None",
) -> dict[str, Any]:
    head = int(record["head"])
    head_dim = int(record["head_dim"])
    heads = int(record["heads"])
    if head < 0 or head >= heads:
        raise ValueError(f"fixture head {head} is outside [0, {heads})")
    kv_embedding = int(key_pages_meta[0]["embedding"])
    if kv_embedding % head_dim != 0:
        raise ValueError(f"KV embedding {kv_embedding} is not divisible by head dim {head_dim}")
    kv_heads = kv_embedding // head_dim
    if kv_heads <= 0:
        raise ValueError("KV page has no addressable heads")
    kv_head = min(kv_heads - 1, (head * kv_heads) // heads)

    query = read_typed_vector(
        fixture_dir / str(record["query_file"]),
        str(fixture.get("query_dtype", "f32")),
        expected_values=head_dim,
    )
    q_gpu = mx.array(query, dtype=mx.float32)
    mx.eval(q_gpu)

    mx.clear_cache()
    mx.reset_peak_memory()
    materialized_started = time.perf_counter()
    mat_output, materialized_values, total_tokens = materialized_attention(
        export_dir,
        key_pages_meta,
        value_pages_meta,
        q_gpu,
        kv_head,
        head_dim,
    )
    mx.eval(mat_output)
    materialized_seconds = time.perf_counter() - materialized_started
    materialized_peak_memory = int(mx.get_peak_memory())
    materialized_active_memory = int(mx.get_active_memory())

    del mat_output
    mx.clear_cache()
    mx.reset_peak_memory()
    streaming_started = time.perf_counter()
    streaming_output, peak_page_values, peak_page_bytes = streaming_attention(
        export_dir,
        key_pages_meta,
        value_pages_meta,
        q_gpu,
        kv_head,
        head_dim,
        page_store,
    )
    mx.eval(streaming_output)
    streaming_seconds = time.perf_counter() - streaming_started
    streaming_peak_memory = int(mx.get_peak_memory())
    streaming_active_memory = int(mx.get_active_memory())

    # Recompute the expected materialised output after the streaming run so the
    # comparison does not keep materialised K/V resident during streaming.
    expected, _, _ = materialized_attention(
        export_dir,
        key_pages_meta,
        value_pages_meta,
        q_gpu,
        kv_head,
        head_dim,
    )
    diff = mx.abs(streaming_output - expected)
    max_abs_error = float(mx.max(diff))
    expected_abs = mx.maximum(mx.abs(expected), mx.array(np.finfo(np.float32).eps, dtype=mx.float32))
    max_relative_error = float(mx.max(diff / expected_abs))
    mx.eval(diff)

    materialized_kv_values = total_tokens * (head_dim + head_dim)
    peak_page_kv_ratio = (
        float(peak_page_values) / float(materialized_kv_values)
        if materialized_kv_values
        else 1.0
    )
    passed = (
        max_abs_error <= args.tolerance
        and peak_page_kv_ratio <= args.max_peak_page_kv_ratio
        and total_tokens > 0
    )

    report = {
        "format": "qatq-mlx-live-vram-streaming-attention-v1",
        "passed": passed,
        "device": str(mx.default_device()),
        "export_dir": str(export_dir),
        "attention_fixture_dir": str(fixture_dir),
        "layer": args.layer,
        "head": head,
        "kv_head": kv_head,
        "kv_heads": kv_heads,
        "head_dim": head_dim,
        "pages": len(key_pages_meta),
        "tokens": total_tokens,
        "dtype": {
            "query": str(fixture.get("query_dtype", "f32")),
            "key": page_dtype(key_pages_meta[0]),
            "value": page_dtype(value_pages_meta[0]),
        },
        "max_abs_error": max_abs_error,
        "max_relative_error": max_relative_error,
        "tolerance": args.tolerance,
        "materialized": {
            "seconds": materialized_seconds,
            "kv_values": materialized_values,
            "peak_memory_bytes": materialized_peak_memory,
            "active_memory_bytes": materialized_active_memory,
        },
        "streaming": {
            "seconds": streaming_seconds,
            "peak_page_kv_values": peak_page_values,
            "materialized_kv_values": materialized_kv_values,
            "peak_page_kv_ratio": peak_page_kv_ratio,
            "peak_page_bytes": peak_page_bytes,
            "peak_memory_bytes": streaming_peak_memory,
            "active_memory_bytes": streaming_active_memory,
        },
        "gate": {
            "max_abs_error": args.tolerance,
            "max_peak_page_kv_ratio": args.max_peak_page_kv_ratio,
        },
    }
    return report


def attach_qatq_store_report(report: dict[str, Any], page_store: "QatqPageStore | None", kept: bool) -> None:
    if page_store is None:
        report["qatq_store"] = {"enabled": False}
        return
    report["qatq_store"] = {
        "enabled": True,
        "dir": str(page_store.store_dir),
        "pages": page_store.pages,
        "raw_bytes": page_store.raw_bytes,
        "stored_bytes": page_store.stored_bytes,
        "decoded_bytes": page_store.decoded_bytes,
        "compression_ratio": (
            float(page_store.stored_bytes) / float(page_store.raw_bytes)
            if page_store.raw_bytes
            else 1.0
        ),
        "decode_seconds": page_store.decode_seconds,
        "encode_seconds": page_store.encode_seconds,
        "kept": kept,
    }


def materialized_attention(
    export_dir: Path,
    key_pages_meta: list[dict[str, Any]],
    value_pages_meta: list[dict[str, Any]],
    q_gpu: mx.array,
    head: int,
    head_dim: int,
) -> tuple[mx.array, int, int]:
    keys = []
    values = []
    total_tokens = 0
    for key_meta, value_meta in zip(key_pages_meta, value_pages_meta):
        key_page = load_page_head(export_dir, key_meta, head, head_dim)
        value_page = load_page_head(export_dir, value_meta, head, head_dim)
        total_tokens += key_page.shape[0]
        keys.append(mx.array(key_page, dtype=mx.float32))
        values.append(mx.array(value_page, dtype=mx.float32))
    k_gpu = mx.concatenate(keys, axis=0)
    v_gpu = mx.concatenate(values, axis=0)
    scores = (k_gpu @ q_gpu) * (1.0 / math.sqrt(float(head_dim)))
    weights = mx.softmax(scores, axis=0)
    output = weights @ v_gpu
    mx.eval(output)
    return output, int(k_gpu.size + v_gpu.size), total_tokens


def streaming_attention(
    export_dir: Path,
    key_pages_meta: list[dict[str, Any]],
    value_pages_meta: list[dict[str, Any]],
    q_gpu: mx.array,
    head: int,
    head_dim: int,
    page_store: "QatqPageStore | None" = None,
) -> tuple[mx.array, int, int]:
    max_score: mx.array | None = None
    denominator = mx.array(0.0, dtype=mx.float32)
    output = mx.zeros((head_dim,), dtype=mx.float32)
    peak_page_values = 0
    peak_page_bytes = 0
    scale = 1.0 / math.sqrt(float(head_dim))

    for key_meta, value_meta in zip(key_pages_meta, value_pages_meta):
        key_page = load_page_head(export_dir, key_meta, head, head_dim, page_store)
        value_page = load_page_head(export_dir, value_meta, head, head_dim, page_store)
        peak_page_values = max(peak_page_values, int(key_page.size + value_page.size))
        peak_page_bytes = max(peak_page_bytes, int(key_page.nbytes + value_page.nbytes))

        k_gpu = mx.array(key_page, dtype=mx.float32)
        v_gpu = mx.array(value_page, dtype=mx.float32)
        scores = (k_gpu @ q_gpu) * scale
        page_max = mx.max(scores)
        if max_score is None:
            weights = mx.exp(scores - page_max)
            denominator = mx.sum(weights)
            output = weights @ v_gpu
            max_score = page_max
        else:
            next_max = mx.maximum(max_score, page_max)
            old_weight = mx.exp(max_score - next_max)
            weights = mx.exp(scores - next_max)
            denominator = denominator * old_weight + mx.sum(weights)
            output = output * old_weight + weights @ v_gpu
            max_score = next_max
        mx.eval(output, denominator, max_score)
        del k_gpu, v_gpu, scores, weights
        mx.clear_cache()

    if max_score is None:
        raise ValueError("no K/V pages selected")
    normalized = output / denominator
    mx.eval(normalized)
    return normalized, peak_page_values, peak_page_bytes


class QatqPageStore:
    def __init__(
        self,
        *,
        store_dir: Path,
        temp_root: tempfile.TemporaryDirectory[str] | None,
        remove_store_dir_on_cleanup: bool,
        decoded_by_file: dict[str, Path],
        pages: int,
        raw_bytes: int,
        stored_bytes: int,
        decoded_bytes: int,
        encode_seconds: float,
        decode_seconds: float,
    ) -> None:
        self.store_dir = store_dir
        self.temp_root = temp_root
        self.remove_store_dir_on_cleanup = remove_store_dir_on_cleanup
        self.decoded_by_file = decoded_by_file
        self.pages = pages
        self.raw_bytes = raw_bytes
        self.stored_bytes = stored_bytes
        self.decoded_bytes = decoded_bytes
        self.encode_seconds = encode_seconds
        self.decode_seconds = decode_seconds

    def decoded_path(self, file_name: str) -> Path:
        try:
            return self.decoded_by_file[file_name]
        except KeyError as error:
            raise ValueError(f"QATQ store is missing decoded page for {file_name}") from error

    def cleanup(self) -> None:
        if self.temp_root is not None:
            self.temp_root.cleanup()
        elif self.remove_store_dir_on_cleanup and self.store_dir.exists():
            shutil.rmtree(self.store_dir)


def build_qatq_page_store(
    *,
    export_dir: Path,
    key_pages_meta: list[dict[str, Any]],
    value_pages_meta: list[dict[str, Any]],
    qatq_bin: Path,
    store_dir: Path | None,
    keep_store: bool,
) -> QatqPageStore:
    if not qatq_bin.exists():
        raise ValueError(f"QATQ binary does not exist: {qatq_bin}")
    temp_root: tempfile.TemporaryDirectory[str] | None = None
    if store_dir is None:
        temp_root = tempfile.TemporaryDirectory(prefix="qatq-mlx-page-store-")
        root = Path(temp_root.name)
    else:
        root = store_dir
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    encoded_dir = root / "encoded"
    decoded_dir = root / "decoded"
    encoded_dir.mkdir(parents=True, exist_ok=True)
    decoded_dir.mkdir(parents=True, exist_ok=True)

    metas = key_pages_meta + value_pages_meta
    decoded_by_file: dict[str, Path] = {}
    raw_bytes = 0
    stored_bytes = 0
    decoded_bytes = 0
    encode_seconds = 0.0
    decode_seconds = 0.0
    for meta in metas:
        source = export_dir / str(meta["file"])
        if not source.exists():
            raise ValueError(f"missing source page {source}")
        dtype = page_dtype(meta)
        if dtype not in ("f16", "bf16", "f32"):
            raise ValueError(f"unsupported QATQ page dtype {dtype}")
        encoded = encoded_dir / f"{source.name}.qatq"
        decoded = decoded_dir / source.name

        started = time.perf_counter()
        run_qatq([qatq_bin, "encode", "--mode", "qatq-exact", "--dtype", dtype, source, encoded])
        encode_seconds += time.perf_counter() - started
        started = time.perf_counter()
        run_qatq([qatq_bin, "decode", encoded, decoded])
        decode_seconds += time.perf_counter() - started

        raw = source.read_bytes()
        restored = decoded.read_bytes()
        if restored != raw:
            raise ValueError(f"QATQ restore mismatch for {source.name}")
        raw_bytes += len(raw)
        stored_bytes += encoded.stat().st_size
        decoded_bytes += len(restored)
        decoded_by_file[str(meta["file"])] = decoded

    return QatqPageStore(
        store_dir=root,
        temp_root=temp_root,
        remove_store_dir_on_cleanup=not keep_store,
        decoded_by_file=decoded_by_file,
        pages=len(metas),
        raw_bytes=raw_bytes,
        stored_bytes=stored_bytes,
        decoded_bytes=decoded_bytes,
        encode_seconds=encode_seconds,
        decode_seconds=decode_seconds,
    )


def run_qatq(command: list[Path | str]) -> None:
    completed = subprocess.run(
        [str(part) for part in command],
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.strip()
        stdout = completed.stdout.strip()
        detail = stderr or stdout or f"exit code {completed.returncode}"
        raise ValueError(f"QATQ command failed: {detail}")


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ValueError(f"missing required file {path}") from error


def find_fixture_record(fixture: dict[str, Any], layer: int, head: int) -> dict[str, Any]:
    for record in fixture.get("records", []):
        if int(record.get("layer", -1)) == layer and int(record.get("head", -1)) == head:
            return record
    raise ValueError(f"attention fixture does not contain layer {layer}, head {head}")


def find_fixture_records_for_layer(fixture: dict[str, Any], layer: int) -> list[dict[str, Any]]:
    records = [
        record
        for record in fixture.get("records", [])
        if int(record.get("layer", -1)) == layer
    ]
    records.sort(key=lambda record: int(record.get("head", -1)))
    if not records:
        raise ValueError(f"attention fixture does not contain layer {layer}")
    expected_heads = int(records[0].get("heads", len(records)))
    if len(records) != expected_heads:
        raise ValueError(
            f"attention fixture has {len(records)} records for layer {layer}, expected {expected_heads} heads"
        )
    for expected, record in enumerate(records):
        if int(record.get("head", -1)) != expected:
            raise ValueError(f"attention fixture layer {layer} is missing head {expected}")
    return records


def find_fixture_layers(fixture: dict[str, Any]) -> list[int]:
    layers = sorted({int(record.get("layer", -1)) for record in fixture.get("records", [])})
    layers = [layer for layer in layers if layer >= 0]
    if not layers:
        raise ValueError("attention fixture does not contain any layers")
    return layers


def select_pages(manifest: dict[str, Any], *, kind: str, layer: int) -> list[dict[str, Any]]:
    pages = [
        tensor
        for tensor in manifest.get("tensors", [])
        if tensor.get("kind") == kind and tensor.get("name") == f"cache_{kind}_l{layer}"
    ]
    pages.sort(key=lambda item: (int(item["stream"]), int(item["token_start"]), int(item["token_end"])))
    if not pages:
        raise ValueError(f"manifest has no {kind} pages for layer {layer}")
    return pages


def page_dtype(meta: dict[str, Any]) -> str:
    return str(meta.get("dtype", "")).removesuffix("le")


def read_typed_vector(path: Path, dtype: str, *, expected_values: int | None = None) -> np.ndarray:
    if dtype in ("f32", "f32le"):
        values = np.fromfile(path, dtype="<f4").astype(np.float32, copy=False)
    elif dtype in ("f16", "f16le"):
        values = np.fromfile(path, dtype="<f2").astype(np.float32)
    elif dtype in ("bf16", "bf16le"):
        raw = np.fromfile(path, dtype="<u2").astype(np.uint32)
        values = (raw << 16).view(np.float32)
    else:
        raise ValueError(f"unsupported dtype {dtype!r}")
    if expected_values is not None and values.size != expected_values:
        raise ValueError(f"{path} has {values.size} values, expected {expected_values}")
    if not np.isfinite(values).all():
        raise ValueError(f"{path} contains non-finite values")
    return values


def load_page_head(
    export_dir: Path,
    meta: dict[str, Any],
    head: int,
    head_dim: int,
    page_store: QatqPageStore | None = None,
) -> np.ndarray:
    embedding = int(meta["embedding"])
    active_cells = int(meta["active_cells"])
    if embedding <= 0 or active_cells <= 0:
        raise ValueError("invalid page metadata")
    start = head * head_dim
    end = start + head_dim
    if end > embedding:
        raise ValueError(f"head {head} with dim {head_dim} exceeds page embedding {embedding}")

    page_path = page_store.decoded_path(str(meta["file"])) if page_store else export_dir / str(meta["file"])
    values = read_typed_vector(page_path, str(meta["dtype"]))
    expected = embedding * active_cells
    if values.size != expected:
        raise ValueError(f"{meta['file']} has {values.size} values, expected {expected}")
    if bool(meta.get("transposed", False)):
        page = values.reshape(embedding, active_cells).T
    else:
        page = values.reshape(active_cells, embedding)
    return np.ascontiguousarray(page[:, start:end], dtype=np.float32)


if __name__ == "__main__":
    raise SystemExit(main())
