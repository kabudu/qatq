#!/usr/bin/env python3
"""Combine QATQ checker and SageMath reproduction evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
from pathlib import Path
import subprocess


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def version(command: list[str]) -> str:
    process = subprocess.run(command, check=True, text=True, stdout=subprocess.PIPE)
    return process.stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--qatq-oracle", type=Path, required=True)
    parser.add_argument("--sage-results", type=Path, required=True)
    parser.add_argument("--sage-image", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    manifest_path = args.root / "export-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    sage = json.loads(args.sage_results.read_text())
    sage_by_id = {row["id"]: row for row in sage["rows"]}
    rows = []
    for entry in manifest["rows"]:
        certificate = args.root / entry["certificate"]
        process = subprocess.run(
            [str(args.qatq_oracle), "check", str(certificate)],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            check_payload = json.loads(process.stdout)
        except json.JSONDecodeError:
            check_payload = {"status": "MALFORMED_CHECKER_OUTPUT", "stdout": process.stdout}
        sage_row = sage_by_id.get(entry["id"])
        status = (
            "AGREE"
            if process.returncode == 0
            and check_payload.get("status") == "VALID"
            and sage_row is not None
            and sage_row.get("status") == "AGREE"
            else "DISAGREE"
        )
        rows.append(
            {
                "id": entry["id"],
                "certificate_sha256": entry["certificate_sha256"],
                "qatq_checker_exit_status": process.returncode,
                "qatq_checker_status": check_payload.get("status"),
                "qatq_claimed_upper_bound": None if sage_row is None else sage_row["claimed_upper_bound"],
                "sagemath_reproduced_upper_bound": None if sage_row is None else sage_row["reproduced_upper_bound"],
                "sagemath_witness_matches": False if sage_row is None else sage_row["witness_matches"],
                "decisive_inequality_holds": False if sage_row is None else sage_row["decisive_inequality_holds"],
                "status": status,
            }
        )

    ids_match = set(sage_by_id) == {entry["id"] for entry in manifest["rows"]}
    all_agree = ids_match and len(rows) == manifest["row_count"] and all(
        row["status"] == "AGREE" for row in rows
    )
    output = {
        "schema_version": 1,
        "claim": "independently_reproduced_by_separate_software",
        "corpus_sha256": manifest["corpus_sha256"],
        "export_manifest_sha256": sha256(manifest_path),
        "sage_results_sha256": sha256(args.sage_results),
        "environment": {
            "host_platform": platform.platform(),
            "rustc": version(["rustc", "--version"]),
            "cargo": version(["cargo", "--version"]),
            "qatq_oracle_binary_sha256": sha256(args.qatq_oracle),
            "sagemath": sage["environment"],
            "sagemath_container_image": args.sage_image,
        },
        "row_count": len(rows),
        "all_row_ids_match": ids_match,
        "all_rows_agree": all_agree,
        "rows": rows,
    }
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return 0 if all_agree else 1


if __name__ == "__main__":
    raise SystemExit(main())
