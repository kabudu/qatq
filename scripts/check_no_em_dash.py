#!/usr/bin/env python3
"""Reject Unicode U+2014 from repository text files."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


FORBIDDEN = chr(0x2014)


def tracked_paths() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        check=True,
        capture_output=True,
    )
    return [Path(raw.decode("utf-8")) for raw in result.stdout.split(b"\0") if raw]


def violations(paths: list[Path]) -> list[str]:
    found: list[str] = []
    for path in paths:
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for number, line in enumerate(text.splitlines(), start=1):
            if FORBIDDEN in line:
                found.append(f"{path}:{number}: Unicode U+2014 is forbidden")
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()

    found = violations(args.paths or tracked_paths())
    for message in found:
        print(f"error: {message}", file=sys.stderr)
    if found:
        return 1
    print("typography valid: no Unicode U+2014 found")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
