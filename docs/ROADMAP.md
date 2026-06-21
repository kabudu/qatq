# Roadmap

## Phase 0 - Seed

- [x] Split QATQ into its own repository.
- [x] Add a Rust library crate and CLI.
- [x] Preserve the original lossy int4 path as a seed baseline.
- [x] Add exact f32 envelope mode for bit-identical control tests.
- [x] Document that int4 QATQ is lossy and not the full paper implementation.

## Phase 1 - Paper-Faithful Training-Free QATQ

- [x] Implement quaternion grouping and Hamilton product rotation.
- [x] Add deterministic rotation seed/configuration handling.
- [x] Implement TurboQuant-style scalar quantization.
- [x] Add a base `turboquant-q4` comparator before the quaternion overlay.
- [x] Add QJL/residual side-channel experiments.
- [x] Benchmark against raw, zstd, lz4, FP8, base TurboQuant-style q4, and the
      seed lossy int4 baseline.

Phase 1 is implemented as the `phase1-q4` mode. The QJL/residual side channel is
currently a compact global residual-magnitude plus per-coordinate sign-bit
experiment. It is useful for measurement but does not claim lossless
reconstruction.

The `turboquant-q4` mode is the current base reference path: deterministic
data-oblivious orthogonal rotation, scalar q4 quantization, and QJL residual
signs for query-side inner-product estimation using a structured signed-Hadamard
projection. It is included so the quaternion overlay can be measured against a
non-quaternion baseline.

## Phase 2 - Lossless QATQ-Family Mode

- [x] Define exact reconstruction semantics.
- [x] Implement residual generation from QATQ reconstruction.
- [x] Entropy-code residuals and compare against the exact f32 envelope.
- [x] Add tests for bit-identical f32 reconstruction.

Phase 2 is implemented as `phase2-lossless`. It adaptively stores raw f32 bits,
byte-RLE, byte-plane RLE, adjacent-bit delta-XOR byte-plane residuals, or the
Phase 1 predictor plus run-coded XOR residuals and verifies final reconstruction
with the payload checksum. zstd/lz4 comparison rows are included in all-codec
benchmark reports as general-purpose byte-compression baselines over raw f32le.

## Phase 3 - Runtime and Service Integration

- [x] Add production chunk APIs suitable for generic runtime adapters.
- [ ] Add a standalone codec service binary.
- [x] Add a generic runtime adapter contract and Rust production-chunk example.
- [ ] Add MLX, vLLM, and llama.cpp adapter examples as optional external
      integration examples.
- [x] Add chunked exact encode/decode APIs for large KV blocks.
- [x] Add a sequential Phase 2 chunk container for large tensor files.
- [x] Validate the phase-2 production storage-decision API with generated public
      fixtures.
- [ ] Add random-access metadata and a true streaming container/service
      protocol.

The current QATQ implementation is usable for exact phase-2 runtime transfer
experiments, but the broader project is not complete until service adapters,
release hygiene, and comparative paper baselines are finished.

## Phase 4 - Open Release

- [x] Prepare public repository hygiene around generated public fixtures.
- [x] Add CI and fuzzing scaffold.
- [ ] Add coverage and supply-chain checks.
- [ ] Publish to crates.io when the API is stable.
- [ ] Cut GitHub Releases with binaries once the CLI is useful standalone.
