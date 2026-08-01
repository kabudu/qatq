#!/usr/bin/env python3
"""Compare the semantic content of two validation result bundles."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def semantic_view(value: dict[str, object]) -> dict[str, object]:
    return {
        "schema_version": value.get("schema_version"),
        "claim": value.get("claim"),
        "corpus_sha256": value.get("corpus_sha256"),
        "row_count": value.get("row_count"),
        "all_row_ids_match": value.get("all_row_ids_match"),
        "all_rows_agree": value.get("all_rows_agree"),
        "rows": value.get("rows"),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--actual", type=Path, required=True)
    args = parser.parse_args()
    expected = semantic_view(json.loads(args.expected.read_text()))
    actual = semantic_view(json.loads(args.actual.read_text()))
    if expected != actual:
        print("independent validation semantic results differ")
        return 1
    if actual["all_rows_agree"] is not True:
        print("independent validation did not agree on every row")
        return 1
    print(f"independent validation agrees on {actual['row_count']} rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
