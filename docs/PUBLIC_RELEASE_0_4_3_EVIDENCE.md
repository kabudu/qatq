# QATQ 0.4.3 Release Evidence

Validated on 22 October 2026 with Rust 1.97.0 on macOS arm64.

## Scope and compatibility

This maintenance release updates Zstd to 0.14.2, num-bigint to 0.5.1, SHA-2 to 0.11.1, serde_json to 1.0.154, and the release artifact actions. The SHA-2 upgrade includes equivalent explicit lowercase hexadecimal digest formatting in the Oracle and geometry tools. Independent certificate reproduction and the published geometry corpus verify the resulting digests.

QATQ and QATC wire formats, public interfaces, Oracle certificate schemas and theorem families remain unchanged. No new performance, runtime-integration, or capacity claim is introduced. The fuzz lockfile now matches the released crate version and Zstd dependency family.

## Validation

- Formatting, locked all-target/all-feature compilation, and all 169 ordinary tests pass.
- All three fuzz targets compile with the updated locked dependencies.
- The 4,096-case deterministic KV stress matrix passes across 8,499,064 values.
- Line coverage is 83.95%, above the 75% gate.
- RustSec audit reports no vulnerabilities; the dependency tree reports no duplicate versions.
- Public fixture regeneration reproduces every committed f32le file byte for byte, and fixture verification passes.
- Public baseline, exact, quality, and task-quality reports were regenerated. Exact compression ratios remain 0.3817, 0.1153, 0.6532, and 0.0121; the public compression summary remains applicable.
- Production and competitive compression gates pass all eight exact/container checks.
- The pinned, network-disabled SageMath 10.6 image independently reproduces all 27 certificate rows, with semantic comparison against the published results passing.
- All 36 published geometry observation profiles verify.
- Cargo package creation and verification pass.
- Curated release notes validation and the repository typography scan pass.

## Fresh live KV-cache validation

The pinned llama.cpp exporter was rebuilt at commit `7992aa7c8` with the committed adapter patch, SHA-256 `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`. The local Phi4-mini model SHA-256 is `3c168af1dea0a414299c7d9077e100ac763370e5a98b3c53801a958a47f0a5db`.

Two CPU-only f16 cases, with 16 and 64 token budgets, successfully exported and exactly compressed real KV tensors. See [the release-specific live matrix](LLAMA_CPP_KV_MATRIX_0_4_3.md). This is a two-case smoke check, not a new broad model-quality or performance study. Raw model files and private captures are excluded from the release source.

## Evidence links

- [Production gate](PUBLIC_BENCHMARK_GATE.md)
- [Competitive compression gate](PUBLIC_COMPETITIVE_COMPRESSION_GATE.md)
- [Public compression summary](PUBLIC_COMPRESSION_SUMMARY.md)
- [Oracle corpus](../validation/oracle-v0.4.1)
- [Geometry corpus](../validation/geometry-v0.4.2)
