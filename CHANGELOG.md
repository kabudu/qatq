# Changelog

All notable changes to QATQ are recorded here.

## Unreleased

## 0.1.1 - 2026-06-23

### Fixed

- Removed local absolute llama.cpp/model paths from scripts and documentation
  included in the crates.io source package. Runtime benchmark scripts now use
  documented environment variables for local model locations.

## 0.1.0 - 2026-06-22

### Added

- Added a standalone generated public fixture corpus and manifest so QATQ can
  benchmark and validate itself without external runtime captures.
- Added explicit benchmark gate policies for production KV throughput readiness
  competitive compression, and fixed-us latency-budget analysis.
- Added production chunk metadata/restore helpers for runtime integrations.
- Added native exact f16 and bf16 tensor byte support.
- Added direct external KV/tensor benchmark adapters and runtime evidence
  documentation for exported LLM cache migration artifacts.
- Added deterministic KV stress coverage and scheduled fuzzing workflow
  scaffolding.
- Added cargo-dist GitHub Release automation with cross-platform archives,
  checksums, shell installers, and signed/notarized macOS release artifacts.
- Added manual crates.io publication workflow guarded by the `crates-io`
  environment and an explicit expected-version check.
- Added a technical whitepaper connecting the original quaternion TurboQuant
  foundation to the current exact QATQ/QATC product surface.
- Added open-source readiness files, issue/PR templates, Dependabot
  configuration, and QATQ brand assets.

### Changed

- Made `qatq-exact` and the `QATC` container the primary exact QATQ product
  surface.
- Moved the crate to Rust 2024 edition with an explicit MSRV/toolchain record.
- Scoped lossless claims to QATQ exact and QATC, with lossy Phase 1 and
  TurboQuant paths retained as research/baseline comparators.
- Recorded API/CLI freeze status for the initial public release.
