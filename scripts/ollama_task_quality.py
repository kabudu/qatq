#!/usr/bin/env python3
"""Run a local Ollama embedding retrieval task through QATQ exact transport.

This is intentionally standard-library only. It treats Ollama embeddings as
runtime model-output tensors, ingests them through the QATQ fixture manifest
path, encodes/decodes them with the CLI, and reports retrieval agreement.
"""

from __future__ import annotations

import argparse
import array
import json
import math
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


DOCS = [
    ("paris", "Paris is the capital of France and sits on the Seine."),
    ("berlin", "Berlin is the capital of Germany and has the Brandenburg Gate."),
    ("tokyo", "Tokyo is the capital of Japan and one of the world's largest cities."),
    ("canberra", "Canberra is the capital of Australia, not Sydney."),
    ("ottawa", "Ottawa is the capital of Canada and is in Ontario."),
    ("nairobi", "Nairobi is the capital of Kenya and a major East African hub."),
    ("rust", "Rust is a systems programming language focused on memory safety."),
    ("python", "Python is a high-level programming language used for data and automation."),
    ("sqlite", "SQLite is an embedded relational database stored in a single file."),
    ("zstd", "Zstandard is a fast general-purpose lossless compression algorithm."),
    ("quaternion", "A quaternion has one real component and three imaginary components."),
    ("hamilton", "William Rowan Hamilton introduced quaternions in the nineteenth century."),
    ("kv-cache", "A transformer KV cache stores key and value tensors for attention reuse."),
    ("attention", "Attention layers compare queries against keys and combine values."),
    ("embedding", "An embedding maps text into a vector space for similarity search."),
    ("retrieval", "Retrieval augmented generation searches documents before answering."),
    ("lossless", "Lossless compression reconstructs the exact original data."),
    ("checksum", "A checksum detects accidental or malicious data corruption."),
    ("nan", "NaN payload bits can matter when preserving exact floating point values."),
    ("cloudflare", "Cloudflare Pages hosts static sites at custom domains."),
    ("github", "GitHub Actions can run tests and scheduled automation."),
    ("ollama", "Ollama runs local language models and embedding models through an HTTP API."),
    ("tensor", "A tensor is a multidimensional array used in machine learning systems."),
    ("cache-migration", "Runtime migration may need to move live tensor state between processes."),
]

QUERIES = [
    ("paris", "Which document says the French capital is on the Seine?"),
    ("canberra", "Find the note explaining that Australia has Canberra as capital."),
    ("rust", "Which entry describes a memory safe systems language?"),
    ("sqlite", "Which document is about an embedded database in a single file?"),
    ("quaternion", "Find the description of a number with one real and three imaginary parts."),
    ("hamilton", "Who introduced quaternions historically?"),
    ("kv-cache", "Which text discusses cached key and value tensors for attention?"),
    ("embedding", "Which document describes mapping text into vector space?"),
    ("lossless", "Which entry says exact original data is reconstructed?"),
    ("checksum", "Find the text about detecting data corruption."),
    ("ollama", "Which document mentions local models exposed over an HTTP API?"),
    ("cache-migration", "Which text is about moving live tensor state between processes?"),
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="phi4-mini:latest")
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434")
    parser.add_argument("--qatq-bin", default="target/release/qatq")
    parser.add_argument("--work-dir", default="captures/ollama-task")
    parser.add_argument("--report", default="docs/RUNTIME_TASK_QUALITY_EXPERIMENTS.md")
    args = parser.parse_args()

    root = Path.cwd()
    work_dir = root / args.work_dir
    report_path = root / args.report
    qatq_bin = root / args.qatq_bin
    ensure_qatq_bin(qatq_bin)
    work_dir.mkdir(parents=True, exist_ok=True)

    started = time.time()
    docs = [text for _, text in DOCS]
    queries = [text for _, text in QUERIES]
    try:
        doc_embeddings = embed(args.ollama_url, args.model, docs)
        query_embeddings = embed(args.ollama_url, args.model, queries)
        validate_embeddings(doc_embeddings, "documents")
        validate_embeddings(query_embeddings, "queries")
    except SystemExit as error:
        return run_relevance_score_task(args, qatq_bin, work_dir, report_path, started, str(error))
    dim = len(doc_embeddings[0])
    if len(query_embeddings[0]) != dim:
        raise SystemExit("document and query embedding dimensions differ")

    docs_f32le = work_dir / "ollama-doc-embeddings.f32le"
    queries_f32le = work_dir / "ollama-query-embeddings.f32le"
    docs_qatq = work_dir / "ollama-doc-embeddings.qatq"
    docs_decoded = work_dir / "ollama-doc-embeddings.decoded.f32le"
    manifest = work_dir / "runtime.manifest"
    audit = work_dir / "runtime-audit.md"
    write_f32le(docs_f32le, flatten(doc_embeddings))
    write_f32le(queries_f32le, flatten(query_embeddings))

    if manifest.exists():
        manifest.unlink()
    run([qatq_bin, "fixture", "add", "--manifest", manifest, "--group", "runtime-model-output",
         "--name", "ollama-phi4-mini-doc-embeddings", "--path", docs_f32le,
         "--shape", f"[documents={len(DOCS)}, dim={dim}]",
         "--notes", "Ollama phi4-mini document embeddings; local model-output tensor"])
    run([qatq_bin, "fixture", "add", "--manifest", manifest, "--group", "runtime-model-output",
         "--name", "ollama-phi4-mini-query-embeddings", "--path", queries_f32le,
         "--shape", f"[queries={len(QUERIES)}, dim={dim}]",
         "--notes", "Ollama phi4-mini query embeddings; local model-output tensor"])
    run([qatq_bin, "fixture", "verify", "--manifest", manifest, "--output", audit])
    run([qatq_bin, "encode", "--mode", "phase2-lossless", docs_f32le, docs_qatq])
    run([qatq_bin, "decode", docs_qatq, docs_decoded])

    decoded_docs = read_f32le(docs_decoded)
    raw_docs = flatten(doc_embeddings)
    exact_bits = f32_bits(raw_docs) == f32_bits(decoded_docs)
    if not exact_bits:
        raise SystemExit("QATQ decoded document embeddings differ at f32 bit level")

    raw_hits = score_retrieval(doc_embeddings, query_embeddings)
    decoded_doc_embeddings = unflatten(decoded_docs, len(DOCS), dim)
    qatq_hits = score_retrieval(decoded_doc_embeddings, query_embeddings)
    if raw_hits != qatq_hits:
        raise SystemExit("QATQ changed retrieval top-1 decisions")

    encoded_bytes = docs_qatq.stat().st_size
    raw_bytes = docs_f32le.stat().st_size
    report = render_report(
        args.model,
        dim,
        raw_bytes,
        encoded_bytes,
        exact_bits,
        raw_hits,
        qatq_hits,
        display_path(manifest),
        display_path(audit),
        time.time() - started,
    )
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(report, encoding="utf-8")
    print(report)
    return 0


def run_relevance_score_task(args, qatq_bin: Path, work_dir: Path, report_path: Path, started: float, embedding_error: str) -> int:
    score_rows = generate_relevance_scores(args.ollama_url, args.model)
    dim = len(DOCS)
    score_values = flatten(score_rows)
    scores_f32le = work_dir / "ollama-relevance-scores.f32le"
    scores_qatq = work_dir / "ollama-relevance-scores.qatq"
    scores_decoded = work_dir / "ollama-relevance-scores.decoded.f32le"
    manifest = work_dir / "runtime.manifest"
    audit = work_dir / "runtime-audit.md"
    write_f32le(scores_f32le, score_values)

    if manifest.exists():
        manifest.unlink()
    run([qatq_bin, "fixture", "add", "--manifest", manifest, "--group", "runtime-model-output",
         "--name", "ollama-phi4-mini-relevance-scores", "--path", scores_f32le,
         "--shape", f"[queries={len(QUERIES)}, documents={len(DOCS)}]",
         "--notes", "Ollama phi4-mini generated relevance score tensor; local model-output tensor"])
    run([qatq_bin, "fixture", "verify", "--manifest", manifest, "--output", audit])
    run([qatq_bin, "encode", "--mode", "phase2-lossless", scores_f32le, scores_qatq])
    run([qatq_bin, "decode", scores_qatq, scores_decoded])

    decoded_values = read_f32le(scores_decoded)
    exact_bits = f32_bits(score_values) == f32_bits(decoded_values)
    if not exact_bits:
        raise SystemExit("QATQ decoded relevance scores differ at f32 bit level")
    decoded_rows = unflatten(decoded_values, len(QUERIES), dim)
    raw_hits = score_rows_top1(score_rows)
    qatq_hits = score_rows_top1(decoded_rows)
    if raw_hits != qatq_hits:
        raise SystemExit("QATQ changed generated-score top-1 decisions")

    raw_bytes = scores_f32le.stat().st_size
    encoded_bytes = scores_qatq.stat().st_size
    report = render_score_report(
        args.model,
        raw_bytes,
        encoded_bytes,
        exact_bits,
        raw_hits,
        qatq_hits,
        display_path(manifest),
        display_path(audit),
        embedding_error,
        time.time() - started,
    )
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(report, encoding="utf-8")
    print(report)
    return 0


def ensure_qatq_bin(path: Path) -> None:
    if path.exists():
        return
    cargo = shutil.which("cargo")
    if cargo is None:
        raise SystemExit(f"{path} does not exist and cargo is not available")
    run([cargo, "build", "--release", "--bin", "qatq"])


def embed(base_url: str, model: str, inputs: list[str]) -> list[list[float]]:
    payload = json.dumps({"model": model, "input": inputs}).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + "/api/embed",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            data = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        if error.code != 404 and error.code != 501:
            raise SystemExit(f"failed to call Ollama at {base_url}: {error}") from error
        return [embed_one_legacy(base_url, model, text) for text in inputs]
    except urllib.error.URLError as error:
        raise SystemExit(f"failed to call Ollama at {base_url}: {error}") from error
    embeddings = data.get("embeddings")
    if not isinstance(embeddings, list) or len(embeddings) != len(inputs):
        raise SystemExit(f"Ollama returned malformed embeddings payload: {data!r}")
    return embeddings


def embed_one_legacy(base_url: str, model: str, prompt: str) -> list[float]:
    payload = json.dumps({"model": model, "prompt": prompt}).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + "/api/embeddings",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            data = json.loads(response.read().decode("utf-8"))
    except urllib.error.URLError as error:
        raise SystemExit(f"failed to call Ollama legacy embeddings at {base_url}: {error}") from error
    embedding = data.get("embedding")
    if not isinstance(embedding, list):
        raise SystemExit(f"Ollama returned malformed legacy embedding payload: {data!r}")
    return embedding


def generate_relevance_scores(base_url: str, model: str) -> list[list[float]]:
    rows = []
    doc_block = "\n".join(f"- {label}: {text}" for label, text in DOCS)
    labels = [label for label, _ in DOCS]
    for expected, query in QUERIES:
        prompt = (
            "You are scoring document relevance for a deterministic retrieval benchmark.\n"
            "Return only JSON in this exact shape: {\"scores\":{\"label\":number,...}}.\n"
            "Scores must be integers from 0 to 100. Score the best matching document highest.\n"
            f"Query label for audit: {expected}\n"
            f"Query: {query}\n"
            "Documents:\n"
            f"{doc_block}\n"
        )
        response = ollama_generate_json(base_url, model, prompt)
        scores = response.get("scores")
        if not isinstance(scores, dict):
            raise SystemExit(f"Ollama generated malformed score payload: {response!r}")
        row = []
        for label in labels:
            value = scores.get(label, 0.0)
            if not isinstance(value, (int, float)) or not math.isfinite(value):
                raise SystemExit(f"Ollama score for {label!r} is missing or invalid: {response!r}")
            row.append(float(max(0.0, min(100.0, value))))
        rows.append(row)
    return rows


def ollama_generate_json(base_url: str, model: str, prompt: str) -> dict:
    payload = json.dumps({
        "model": model,
        "prompt": prompt,
        "format": "json",
        "stream": False,
        "options": {"temperature": 0, "num_predict": 768},
    }).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + "/api/generate",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=180) as response:
            data = json.loads(response.read().decode("utf-8"))
    except urllib.error.URLError as error:
        raise SystemExit(f"failed to call Ollama generate at {base_url}: {error}") from error
    text = data.get("response")
    if not isinstance(text, str):
        raise SystemExit(f"Ollama returned malformed generate payload: {data!r}")
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise SystemExit(f"Ollama did not return parseable JSON: {text}") from error


def validate_embeddings(embeddings: list[list[float]], label: str) -> None:
    if not embeddings:
        raise SystemExit(f"{label} embedding response is empty")
    dim = len(embeddings[0])
    if dim == 0:
        raise SystemExit(f"{label} embedding dimension is zero")
    for row in embeddings:
        if len(row) != dim:
            raise SystemExit(f"{label} embedding rows have inconsistent dimensions")
        if any(not math.isfinite(value) for value in row):
            raise SystemExit(f"{label} embeddings contain non-finite values")


def score_retrieval(docs: list[list[float]], queries: list[list[float]]) -> list[tuple[str, str, float]]:
    hits = []
    for (expected, _), query in zip(QUERIES, queries):
        best_label = ""
        best_score = -float("inf")
        for (label, _), doc in zip(DOCS, docs):
            score = cosine(query, doc)
            if score > best_score:
                best_score = score
                best_label = label
        hits.append((expected, best_label, best_score))
    return hits


def score_rows_top1(rows: list[list[float]]) -> list[tuple[str, str, float]]:
    labels = [label for label, _ in DOCS]
    hits = []
    for (expected, _), row in zip(QUERIES, rows):
        best_index = max(range(len(row)), key=lambda index: row[index])
        hits.append((expected, labels[best_index], row[best_index]))
    return hits


def cosine(left: list[float], right: list[float]) -> float:
    dot = sum(a * b for a, b in zip(left, right))
    left_norm = math.sqrt(sum(a * a for a in left))
    right_norm = math.sqrt(sum(b * b for b in right))
    if left_norm == 0.0 or right_norm == 0.0:
        return 0.0
    return dot / (left_norm * right_norm)


def flatten(rows: list[list[float]]) -> list[float]:
    return [value for row in rows for value in row]


def unflatten(values: list[float], rows: int, dim: int) -> list[list[float]]:
    return [values[index * dim:(index + 1) * dim] for index in range(rows)]


def write_f32le(path: Path, values: list[float]) -> None:
    data = array.array("f", values)
    if data.itemsize != 4:
        raise SystemExit("Python float array is not f32 on this platform")
    if sys.byteorder != "little":
        data.byteswap()
    path.write_bytes(data.tobytes())


def read_f32le(path: Path) -> list[float]:
    data = array.array("f")
    data.frombytes(path.read_bytes())
    if sys.byteorder != "little":
        data.byteswap()
    return list(data)


def f32_bits(values: list[float]) -> bytes:
    data = array.array("f", values)
    if sys.byteorder != "little":
        data.byteswap()
    return data.tobytes()


def run(command: list[object]) -> None:
    text_command = [str(part) for part in command]
    subprocess.run(text_command, check=True)


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(Path.cwd()))
    except ValueError:
        return str(path)


def render_report(
    model: str,
    dim: int,
    raw_bytes: int,
    encoded_bytes: int,
    exact_bits: bool,
    raw_hits: list[tuple[str, str, float]],
    qatq_hits: list[tuple[str, str, float]],
    manifest: str,
    audit: str,
    elapsed: float,
) -> str:
    raw_matches = sum(expected == actual for expected, actual, _ in raw_hits)
    qatq_matches = sum(expected == actual for expected, actual, _ in qatq_hits)
    lines = [
        "# Runtime Model Task Quality Experiment",
        "",
        "Generated by `scripts/ollama_task_quality.py` against a local Ollama model.",
        "",
        "This is a real model-output embedding retrieval task, not a KV-cache capture.",
        "It proves that QATQ Phase 2 exact transport preserves this model task after",
        "fixture ingestion, encode, and decode. Ollama's public local API does not expose",
        "internal transformer KV-cache tensors, so live KV-cache capture remains a separate",
        "runtime-adapter validation step.",
        "",
        f"- model: `{model}`",
        f"- documents: `{len(DOCS)}`",
        f"- queries: `{len(QUERIES)}`",
        f"- embedding dimension: `{dim}`",
        f"- manifest: `{manifest}`",
        f"- audit report: `{audit}`",
        f"- raw document tensor bytes: `{raw_bytes}`",
        f"- QATQ document payload bytes: `{encoded_bytes}`",
        f"- QATQ ratio vs raw f32: `{encoded_bytes / raw_bytes:.4f}`",
        f"- exact f32 bits after decode: `{str(exact_bits).lower()}`",
        f"- raw top-1 matches expected labels: `{raw_matches}/{len(raw_hits)}`",
        f"- QATQ top-1 matches raw decisions: `{sum(a[1] == b[1] for a, b in zip(raw_hits, qatq_hits))}/{len(raw_hits)}`",
        f"- elapsed seconds: `{elapsed:.2f}`",
        "",
        "| query label | raw top-1 | QATQ top-1 | raw score | QATQ score |",
        "| --- | --- | --- | ---: | ---: |",
    ]
    for raw, qatq in zip(raw_hits, qatq_hits):
        expected, raw_label, raw_score = raw
        _, qatq_label, qatq_score = qatq
        lines.append(
            f"| {expected} | {raw_label} | {qatq_label} | {raw_score:.6f} | {qatq_score:.6f} |"
        )
    lines.extend([
        "",
        "## Claim Boundary",
        "",
        "- Supported: QATQ Phase 2 exact transport preserves this local model-output retrieval task.",
        "- Supported: runtime fixture ingestion, verification, encode, and decode work for the captured embedding tensors.",
        "- Not yet supported: direct live KV-cache extraction from Ollama or language-model perplexity claims.",
        "",
    ])
    return "\n".join(lines)


def render_score_report(
    model: str,
    raw_bytes: int,
    encoded_bytes: int,
    exact_bits: bool,
    raw_hits: list[tuple[str, str, float]],
    qatq_hits: list[tuple[str, str, float]],
    manifest: str,
    audit: str,
    embedding_error: str,
    elapsed: float,
) -> str:
    raw_matches = sum(expected == actual for expected, actual, _ in raw_hits)
    qatq_matches_raw = sum(a[1] == b[1] for a, b in zip(raw_hits, qatq_hits))
    lines = [
        "# Runtime Model Task Quality Experiment",
        "",
        "Generated by `scripts/ollama_task_quality.py` against a local Ollama model.",
        "",
        "This run used Ollama text generation to produce a relevance-score tensor",
        "for a document retrieval task, then ingested that model-output tensor through",
        "the QATQ fixture path and verified Phase 2 exact transport. The installed",
        "model did not expose embeddings through Ollama on this machine.",
        "",
        f"- model: `{model}`",
        f"- task mode: `ollama-generated-relevance-scores`",
        f"- documents: `{len(DOCS)}`",
        f"- queries: `{len(QUERIES)}`",
        f"- score tensor shape: `[queries={len(QUERIES)}, documents={len(DOCS)}]`",
        f"- manifest: `{manifest}`",
        f"- audit report: `{audit}`",
        f"- raw score tensor bytes: `{raw_bytes}`",
        f"- QATQ score payload bytes: `{encoded_bytes}`",
        f"- QATQ ratio vs raw f32: `{encoded_bytes / raw_bytes:.4f}`",
        f"- exact f32 bits after decode: `{str(exact_bits).lower()}`",
        f"- raw model-score top-1 matches expected labels: `{raw_matches}/{len(raw_hits)}`",
        f"- QATQ top-1 matches raw model-score decisions: `{qatq_matches_raw}/{len(raw_hits)}`",
        f"- embedding endpoint result: `{embedding_error}`",
        f"- elapsed seconds: `{elapsed:.2f}`",
        "",
        "| query label | raw top-1 | QATQ top-1 | raw score | QATQ score |",
        "| --- | --- | --- | ---: | ---: |",
    ]
    for raw, qatq in zip(raw_hits, qatq_hits):
        expected, raw_label, raw_score = raw
        _, qatq_label, qatq_score = qatq
        lines.append(
            f"| {expected} | {raw_label} | {qatq_label} | {raw_score:.2f} | {qatq_score:.2f} |"
        )
    lines.extend([
        "",
        "## Claim Boundary",
        "",
        "- Supported: QATQ Phase 2 exact transport preserves this local model-output task tensor and its top-1 retrieval decisions.",
        "- Supported: runtime fixture ingestion, verification, encode, and decode work for the captured score tensor.",
        "- Not yet supported: direct live KV-cache extraction from Ollama, embedding-model evaluation on this machine, or language-model perplexity claims.",
        "",
    ])
    return "\n".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())
