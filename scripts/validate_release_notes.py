#!/usr/bin/env python3
"""Validate GitHub Release notes before publication."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


BLOCK_START = re.compile(r"^(?:#|[-*+] |\d+[.)] |>|\||<|```|~~~)")


def validate(path: Path, product: str, tag: str) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    errors: list[str] = []
    title_prefix = f"# {product} {tag}: "

    if not lines or not lines[0].startswith(title_prefix) or lines[0] == title_prefix:
        errors.append(f"line 1 must be '{title_prefix}<release theme>'")
    if len(lines) < 3 or not any(line.strip() for line in lines[2:]):
        errors.append("release body is empty")

    in_fence = False
    previous_prose_line: int | None = None
    for number, line in enumerate(lines[1:], start=2):
        stripped = line.strip()
        if stripped.startswith(("```", "~~~")):
            in_fence = not in_fence
            previous_prose_line = None
            continue
        if in_fence or not stripped:
            previous_prose_line = None
            continue
        if line[:1].isspace():
            errors.append(
                f"line {number} is an indented continuation; keep each list item on one source line"
            )
            previous_prose_line = None
            continue
        if BLOCK_START.match(line):
            previous_prose_line = None
            continue
        if previous_prose_line is not None:
            errors.append(
                f"lines {previous_prose_line}-{number} hard-wrap one paragraph; keep it on one source line"
            )
        previous_prose_line = number

    if in_fence:
        errors.append("unclosed fenced code block")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", type=Path)
    parser.add_argument("--product", required=True)
    parser.add_argument("--tag", required=True)
    args = parser.parse_args()

    if not args.path.is_file():
        print(f"error: missing curated release notes: {args.path}", file=sys.stderr)
        return 1
    errors = validate(args.path, args.product, args.tag)
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 1
    print(f"release notes valid: {args.path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
