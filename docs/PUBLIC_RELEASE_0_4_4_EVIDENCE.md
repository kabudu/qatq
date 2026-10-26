# QATQ 0.4.4 Release Evidence

Validated on 26 October 2026 with Rust 1.97.0 on macOS arm64.

## Scope and compatibility

This maintenance release updates the locked Zstd version from 0.14.2 to 0.14.3, including the fuzz workspace lockfile. QATQ and QATC formats, public APIs, Capacity Oracle certificate semantics, and research geometry contracts remain unchanged. No new compression-performance, runtime-integration, or capacity claim is introduced.

## Validation

- Formatting, locked all-target/all-feature compilation, and all 169 ordinary tests pass.
- All three fuzz targets compile against the updated locked dependencies.
- The full 4,096-case deterministic KV stress matrix passes across 8,499,064 values.
- Line coverage is 83.95%, above the 75% gate.
- RustSec audit reports no vulnerabilities; the dependency tree reports no duplicate versions.
- Public fixture regeneration reproduces every committed f32le file byte for byte; fixture verification passes.
- Baseline, exact, quality, and task-quality reports were regenerated. Exact ratios remain 0.3817, 0.1153, 0.6532, and 0.0121, so the public compression summary remains applicable.
- Production and competitive compression gates pass all eight exact/container checks.
- The pinned, network-disabled SageMath 10.6 image independently reproduces all 27 published certificate rows, with semantic comparison passing.
- All 36 published geometry observation profiles verify.
- Cargo package creation and verification pass.
- Curated release notes validation and the repository typography scan pass.

## Fresh live KV-cache validation

The patched llama.cpp exporter at commit `7992aa7c8` was rerun with adapter patch SHA-256 `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`. The local Phi4-mini model SHA-256 is `3c168af1dea0a414299c7d9077e100ac763370e5a98b3c53801a958a47f0a5db`.

Two CPU-only f16 smoke cases, with token budgets of 16 and 64, exported real KV tensors and passed exact decode checks. See the [release-specific live matrix](LLAMA_CPP_KV_MATRIX_0_4_4.md). This is a two-case smoke check, with no expanded performance or model-quality claim. Raw models and private captures remain outside the source package.

## Evidence links

- [Production gate](PUBLIC_BENCHMARK_GATE.md)
- [Competitive compression gate](PUBLIC_COMPETITIVE_COMPRESSION_GATE.md)
- [Public compression summary](PUBLIC_COMPRESSION_SUMMARY.md)
- [Oracle corpus](../validation/oracle-v0.4.1)
- [Geometry corpus](../validation/geometry-v0.4.2)
