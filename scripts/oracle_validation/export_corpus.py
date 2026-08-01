#!/usr/bin/env python3
"""Export deterministic QATQ requests and certificates for independent checking."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

REQUIRED_STATES = "1" + ("0" * 200)


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def request_for(row: dict[str, object]) -> dict[str, object]:
    if row["kind"] == "binary":
        representation = {
            "kind": "binary",
            "dimension": row["dimension"],
            "required_states": REQUIRED_STATES,
            "separation": {
                "minimum_hamming_distance": row["minimum_hamming_distance"]
            },
        }
        engine = "finite-binary-hamming"
    elif row["kind"] == "spherical":
        representation = {
            "kind": "spherical",
            "ambient_dimension": row["ambient_dimension"],
            "required_states": REQUIRED_STATES,
            "normalization": "unit_l2",
            "separation": {"maximum_inner_product": row["maximum_inner_product"]},
        }
        engine = "finite-spherical-rankin"
    else:
        raise ValueError(f"unknown corpus kind: {row['kind']}")
    return {
        "schema_version": 1,
        "request_id": f"validation-{row['id']}",
        "representation": representation,
        "bounds": {"engines": [engine], "require_rigorous_certificate": True},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--qatq-oracle", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    corpus = json.loads(args.corpus.read_text())
    if corpus.get("schema_version") != 1 or not isinstance(corpus.get("rows"), list):
        raise SystemExit("unsupported corpus schema")
    if args.output.exists() and any(args.output.iterdir()):
        raise SystemExit(f"refusing to overwrite non-empty output directory: {args.output}")
    requests_dir = args.output / "requests"
    certificates_dir = args.output / "certificates"
    requests_dir.mkdir(parents=True, exist_ok=True)
    certificates_dir.mkdir(parents=True, exist_ok=True)

    records = []
    seen = set()
    for row in corpus["rows"]:
        row_id = row.get("id")
        if not isinstance(row_id, str) or not row_id or row_id in seen:
            raise SystemExit(f"invalid or duplicate row id: {row_id!r}")
        seen.add(row_id)
        request_path = requests_dir / f"{row_id}.json"
        bundle_path = certificates_dir / row_id
        request_path.write_bytes(canonical_bytes(request_for(row)))
        process = subprocess.run(
            [str(args.qatq_oracle), "bound", str(request_path), "--output", str(bundle_path)],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, "LC_ALL": "C", "LANG": "C"},
        )
        if process.returncode != 1:
            print(process.stdout, file=sys.stderr)
            print(process.stderr, file=sys.stderr)
            raise SystemExit(
                f"{row_id}: expected certified-infeasible exit 1, got {process.returncode}"
            )
        certificate_path = bundle_path / "certificate.json"
        if not certificate_path.is_file():
            raise SystemExit(f"{row_id}: missing certificate output")
        records.append(
            {
                "id": row_id,
                "request": str(request_path.relative_to(args.output)),
                "request_sha256": sha256(request_path),
                "certificate": str(certificate_path.relative_to(args.output)),
                "certificate_sha256": sha256(certificate_path),
                "qatq_exit_status": process.returncode,
            }
        )

    manifest = {
        "schema_version": 1,
        "corpus_sha256": sha256(args.corpus),
        "row_count": len(records),
        "rows": records,
    }
    (args.output / "export-manifest.json").write_bytes(canonical_bytes(manifest))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
