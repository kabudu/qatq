#!/usr/bin/env python3
"""Verify the published KV geometry corpus and its observation-only boundary."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path("validation/geometry-v0.4.2")
FORBIDDEN = (b"INFEASIBLE_UNDER_MODEL", b"CONSTRUCTED", b'"UNKNOWN"')


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_manifest(root: Path, manifest_name: str) -> None:
    manifest = json.loads((root / manifest_name).read_text())
    for entry in manifest["files"]:
        relative = Path(entry["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit(f"unsafe manifest path: {relative}")
        data = (root / relative).read_bytes()
        if len(data) != entry["bytes"] or digest(data) != entry["sha256"]:
            raise SystemExit(f"manifest mismatch: {root / relative}")


def main() -> None:
    verify_manifest(ROOT, "corpus-manifest.json")
    corpus = json.loads((ROOT / "corpus.json").read_text())
    if len(corpus["cases"]) != 36:
        raise SystemExit("corpus must contain 36 partition profiles")
    if {case["model_family"] for case in corpus["cases"]} != {"qwen2.5", "phi3"}:
        raise SystemExit("corpus model-family coverage changed")
    if {case["prompt_class"] for case in corpus["cases"]} != {"factual", "conversational", "code"}:
        raise SystemExit("corpus prompt coverage changed")
    if {case["dtype"] for case in corpus["cases"]} != {"f16", "bf16"}:
        raise SystemExit("corpus dtype coverage changed")
    if {case["partition"] for case in corpus["cases"]} != {"layer-head-token", "layer-token", "layer-head-chunk"}:
        raise SystemExit("corpus partition coverage changed")
    for bundle_manifest in ROOT.glob("*/*/manifest.json"):
        verify_manifest(bundle_manifest.parent, "manifest.json")
    for path in ROOT.rglob("*"):
        if path.is_file():
            data = path.read_bytes()
            for forbidden in FORBIDDEN:
                if forbidden in data:
                    raise SystemExit(f"Oracle verdict found in {path}")
    decision = Path("docs/oracle/KV_GEOMETRY_RELEVANCE.md").read_text()
    final = "FREEZE: no credible QATQ product application was demonstrated"
    if decision.count(final) != 1 or decision.rstrip().splitlines()[-1] != final:
        raise SystemExit("geometry report must end with exactly one authorized decision token")
    print("KV geometry corpus verified: 36 observation-only profiles")


if __name__ == "__main__":
    main()
