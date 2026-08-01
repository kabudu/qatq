# Changelog

All notable changes to QATQ are recorded here.

## Unreleased

## 0.4.1 - 2026-08-01

### Added

- Added a pinned, network-disabled SageMath 10.6 implementation that separately
  reproduces all 27 published binary Hamming and spherical Rankin certificate
  rows without importing QATQ code or expected numeric answers.
- Added a deterministic differential corpus with requests, certificates,
  SHA-256 manifests, exact per-row comparisons, environment versions, and a CI
  gate against the published machine-readable results.

### Changed

- Distinguished certificates that are checkable by QATQ, reproduced by
  separate software, and externally reviewed by a person. No completed external
  human review is claimed.

## 0.4.0 - 2026-08-01

### Added

- Added the feature-gated `qatq-oracle` binary and `qatq::oracle` library API for
  strict binary/spherical capacity requests, deterministic normalization, and
  SHA-256 request binding.
- Added independently checkable finite impossibility certificates for the exact
  binary Hamming bound and spherical Rankin bounds at maximum inner product
  `s <= 0`.
- Added fail-closed certificate checking, adversarial schema/arithmetic tests,
  logical exit statuses, and atomically published evidence bundles.

### Changed

- Cargo-dist production archives now enable the `oracle` feature and include the
  separate `qatq-oracle` executable without changing QATQ or QATC wire formats.

## 0.3.0 - 2026-07-25

### Added

- Added production shape-aware strided-XOR byte-plane Zstd encoding for native
  f16/bf16 tensors through
  `try_encode_qatq_exact_tensor_le_with_stride_hint` and CLI
  `--stride-elements`.
- Added exact strategy identifier `10`,
  `QatqExactStrategy::StridedXorBytePlaneZstd`, validated stride metadata,
  corruption coverage, deterministic experiment fixtures, and release
  evidence.

### Changed

- The shape-aware path uses a conservative sample-compression classifier to
  select ordinary byte-plane or strided-XOR Zstd before one full compression
  pass. The default no-hint encoder performs no strided probing.

## 0.2.1 - 2026-07-25

### Changed

- `qatq encode` now defaults to the production `qatq-exact` mode when `--mode`
  is omitted. Explicit comparator and research modes remain available through
  `--mode`.

## 0.2.0 - 2026-07-25

### Added

- Added a sparsity-gated adjacent-XOR byte-plane Zstd strategy for native f16
  and bf16 tensors. A bounded sample of 4,096 words across 64 consecutive
  windows avoids full residual allocation for unsuitable inputs; the reversible
  predictor is selected only when both the sample and complete residual stream
  contain more than half zero bytes.
  Original bytes remain exact and the existing byte-plane Zstd path remains the
  fallback.
- Added a native f16/bf16 exact round-trip fuzz target and exhaustive ordered
  coverage of every 16-bit word pattern.

## 0.1.5 - 2026-07-22

### Fixed

- Preflight the caller-authenticated decoded byte length against QATC aggregate
  limits before the exact-byte decode API attempts its output allocation.
- Reject empty default llama.cpp model paths as missing configuration instead
  of misreading the repository directory as a model.

## 0.1.4 - 2026-07-22

### Added

- Added exact opaque byte QATC encode, decode and bounded chunk-visitor APIs.
  They preserve the existing QATC v2 wire format, require the surrounding
  protocol's authenticated byte length, and reject non-canonical padding.

## 0.1.3 - 2026-07-22

### Fixed

- Prevented the crates.io publication path-scrub guard from matching its own
  workflow source on runners whose search includes hidden directories.

### Added

- Added an opaque `u32` QATC chunk visitor that validates the complete
  container before callbacks and materialises only one decoded chunk at a time.

## 0.1.2 - 2026-07-22

### Added

- Added exact opaque `u32` QATC encode/decode helpers for non-tensor protocols.
- Added public, resource-bounded QATC chunk metadata inspection so integrations
  can validate decoded counts and canonical chunk layout before body decoding.

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
