#!/usr/bin/env python3
"""Create a reproducible patched llama.cpp checkout for QATQ evidence runs.

The script is intentionally small and fail-closed: it pins the upstream commit,
applies the checked-in QATQ adapter patch, runs the structural adapter audit,
and optionally builds the llama.cpp binaries used by the evidence harnesses.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LLAMA_REPO = "https://github.com/ggml-org/llama.cpp.git"
DEFAULT_LLAMA_COMMIT = "7992aa7c8e21ea2eb7a5e4802da56eec7b376036"
DEFAULT_PATCH = REPO_ROOT / "adapters" / "llama-cpp" / "qatq-kv-export-7992aa7c8.patch"
DEFAULT_WORK_DIR = Path("/private/tmp/qatq-llama.cpp")
DEFAULT_TARGETS = ("llama-simple", "llama-server")


@dataclass
class CommandResult:
    argv: list[str]
    returncode: int | None
    dry_run: bool


class BootstrapError(RuntimeError):
    pass


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Clone, patch, audit, and optionally build the pinned QATQ llama.cpp adapter."
    )
    parser.add_argument("--repo-url", default=DEFAULT_LLAMA_REPO)
    parser.add_argument("--commit", default=DEFAULT_LLAMA_COMMIT)
    parser.add_argument("--patch-file", type=Path, default=DEFAULT_PATCH)
    parser.add_argument("--work-dir", type=Path, default=DEFAULT_WORK_DIR)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--cmake-build-type", default="Release")
    parser.add_argument("--target", action="append", dest="targets")
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) - 1))
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-audit", action="store_true")
    parser.add_argument("--force", action="store_true", help="Remove an existing work dir before cloning.")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--output", type=Path, help="Optional JSON bootstrap report path.")
    args = parser.parse_args()

    try:
        report = bootstrap(args)
    except BootstrapError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0


def bootstrap(args: argparse.Namespace) -> dict[str, object]:
    patch = args.patch_file.resolve()
    require(patch.exists(), f"patch file does not exist: {patch}")
    require(args.jobs > 0, "--jobs must be greater than zero")
    require(args.commit and len(args.commit) >= 7, "--commit must name a pinned llama.cpp commit")

    work_dir = args.work_dir.resolve()
    build_dir = (args.build_dir or work_dir / "build-qatq").resolve()
    targets = tuple(args.targets or DEFAULT_TARGETS)
    require(targets, "at least one --target is required")

    runner = CommandRunner(args.dry_run)
    assume_removed = False
    if work_dir.exists() and args.force:
        if args.dry_run:
            runner.record(["rm", "-rf", str(work_dir)])
            assume_removed = True
        else:
            shutil.rmtree(work_dir)

    if assume_removed or not work_dir.exists():
        runner.run(["git", "clone", "--filter=blob:none", args.repo_url, str(work_dir)])
    else:
        require((work_dir / ".git").exists(), f"existing work dir is not a git checkout: {work_dir}")
        require(
            not args.force,
            "--force should have removed the work dir before this point; refusing to continue",
        )

    git = ["git", "-C", str(work_dir)]
    runner.run(git + ["fetch", "--depth", "1", "origin", args.commit])
    runner.run(git + ["checkout", "--detach", args.commit])
    runner.run(git + ["reset", "--hard", args.commit])
    runner.run(git + ["clean", "-fdx"])
    runner.run(git + ["apply", "--check", str(patch)])
    runner.run(git + ["apply", str(patch)])
    runner.run(git + ["diff", "--check"])

    audit_report = work_dir / "qatq-adapter-audit.json"
    if not args.skip_audit:
        runner.run(
            [
                sys.executable,
                str(REPO_ROOT / "scripts" / "llama_cpp_live_vram_adapter_audit.py"),
                "--llama-cpp",
                str(work_dir),
                "--require-live-paging",
                "--require-runtime-security",
                "--output",
                str(audit_report),
            ]
        )

    built_binaries: list[str] = []
    if not args.skip_build:
        cmake_args = [
            "cmake",
            "-S",
            str(work_dir),
            "-B",
            str(build_dir),
            f"-DCMAKE_BUILD_TYPE={args.cmake_build_type}",
        ]
        if platform.system() == "Darwin":
            cmake_args.extend(["-DGGML_METAL=ON", "-DGGML_METAL_EMBED_LIBRARY=ON"])
        runner.run(cmake_args)
        for target in targets:
            runner.run(["cmake", "--build", str(build_dir), "--target", target, "-j", str(args.jobs)])
            built_binaries.append(str(build_dir / "bin" / target))

    return {
        "format": "qatq-llama-cpp-adapter-bootstrap-v1",
        "dry_run": args.dry_run,
        "repo_url": args.repo_url,
        "commit": args.commit,
        "patch_file": str(patch),
        "work_dir": str(work_dir),
        "build_dir": str(build_dir),
        "targets": list(targets),
        "skip_build": args.skip_build,
        "skip_audit": args.skip_audit,
        "audit_report": None if args.skip_audit else str(audit_report),
        "built_binaries": built_binaries,
        "commands": [result.__dict__ for result in runner.results],
    }


class CommandRunner:
    def __init__(self, dry_run: bool) -> None:
        self.dry_run = dry_run
        self.results: list[CommandResult] = []

    def record(self, argv: list[str]) -> None:
        self.results.append(CommandResult(argv=argv, returncode=None, dry_run=True))

    def run(self, argv: list[str]) -> None:
        if self.dry_run:
            self.record(argv)
            return
        result = subprocess.run(argv, check=False)
        self.results.append(CommandResult(argv=argv, returncode=result.returncode, dry_run=False))
        if result.returncode != 0:
            raise BootstrapError(f"command failed with exit code {result.returncode}: {quote_command(argv)}")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BootstrapError(message)


def quote_command(argv: list[str]) -> str:
    return " ".join(json.dumps(part) if any(ch.isspace() for ch in part) else part for part in argv)


if __name__ == "__main__":
    raise SystemExit(main())
