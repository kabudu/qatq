# Roadmap

## Phase 0 - Seed

- [x] Split QATQ into its own repository.
- [x] Add a Rust library crate and CLI.
- [x] Preserve the original lossy int4 path as a seed baseline.
- [x] Add exact f32 envelope mode for bit-identical control tests.
- [x] Document that int4 QATQ is lossy and not the full paper implementation.

## Phase 1 - Lossy Predictor And Comparator Research

- [x] Implement quaternion grouping and Hamilton product rotation.
- [x] Add deterministic rotation seed/configuration handling.
- [x] Implement TurboQuant-style scalar quantization.
- [x] Add a base `turboquant-q4` comparator before the quaternion overlay.
- [x] Add QJL/residual side-channel experiments.
- [x] Benchmark against raw, zstd, lz4, FP8, base TurboQuant-style q4, and the
      seed lossy int4 baseline.

Phase 1 is implemented as the `phase1-q4` mode. It is retained as a lossy
predictor and measurement path, not as the main QATQ product surface. The
QJL/residual side channel is currently a compact global residual-magnitude plus
per-coordinate sign-bit experiment. It is useful for measurement but does not
claim lossless reconstruction.

The `turboquant-q4` mode is a local base reference path: deterministic
data-oblivious orthogonal rotation, scalar q4 quantization, and QJL residual
signs for query-side inner-product estimation using a structured signed-Hadamard
projection. It is included so QATQ can measure against a non-quaternion lossy
baseline. It is not an official Google implementation and is not the default
QATQ foundation.

## QATQ exact - Primary QATQ Exact Mode

- [x] Define exact reconstruction semantics.
- [x] Implement residual generation from QATQ reconstruction.
- [x] Entropy-code residuals and compare against the exact f32 envelope.
- [x] Add tests for bit-identical f32 reconstruction.

QATQ exact is implemented as `qatq-exact` and is the primary QATQ
implementation. It adaptively stores raw f32 bits, byte-RLE, byte-plane RLE,
byte-plane zstd, reversible quaternion-chain zstd, adjacent-bit delta-XOR
byte-plane residuals, or the Phase 1 predictor plus run-coded XOR residuals and
verifies final reconstruction with the payload checksum. Lossless QATQ claims
are scoped to QATQ exact and its `QATC` container. zstd/lz4 comparison rows are
included in benchmark reports as general-purpose byte-compression baselines over
raw f32le, and the competitive compression gate rejects public fixture
regressions against those baselines.

## Phase 3 - Runtime and Service Integration

- [x] Add production chunk APIs suitable for generic runtime adapters.
- [ ] Add a standalone codec service binary.
- [x] Add a generic runtime adapter contract and Rust production-chunk example.
- [ ] Add MLX, vLLM, and llama.cpp adapter examples as optional external
      integration examples.
- [x] Add chunked exact encode/decode APIs for large KV blocks.
- [x] Add a sequential QATQ exact chunk container for large tensor files.
- [x] Validate the QATQ exact production storage-decision API with generated public
      fixtures.
- [ ] Add random-access metadata and a true streaming container/service
      protocol.

The current QATQ implementation is usable for exact QATQ exact runtime transfer
experiments, but the broader project is not complete until service adapters,
release hygiene, and comparative paper baselines are finished.

The initial public release should stay focused on storage and transfer of
exported KV/tensor bytes: checkpoints, migration artifacts, runtime captures,
and fixture bundles. QATQ should not claim transparent live GPU VRAM reduction
for v0.1.

## Experimental Track - Live KV Paging and VRAM Reduction

- [ ] Define a runtime KV page/offload adapter contract.
- [ ] Choose one paged/offload-capable runtime target for the first experiment.
- [ ] Compress only cold/inactive KV pages and restore them before attention
      needs them.
- [ ] Measure peak VRAM, tokens/sec, first-token latency, and per-token latency
      against the runtime's native KV cache, FP8/KV quantization, CPU offload,
      zstd, and lz4.
- [ ] Prove byte-exact page restore for exact mode and task/output preservation
      across long-context generation.
- [ ] Keep the feature behind experimental docs or feature flags until it beats
      simpler runtime offload strategies under realistic workloads.

This track is deliberately separate from the v0.1 release goal. QATQ can
already compress exported KV/tensor state for storage and transfer; live VRAM
reduction requires participation in a runtime's KV allocator or page scheduler,
not just access to exported tensors.

## Phase 4 - Open Release

- [x] Prepare public repository hygiene around generated public fixtures.
- [x] Add CI and fuzzing scaffold.
- [x] Add scheduled longer fuzzing for decoder and QATQ exact round-trip targets.
- [x] Add a public end-to-end retrieval task-quality experiment.
- [x] Add a local Ollama model-output task harness for runtime fixture
      ingestion and task-decision preservation.
- [x] Add direct live KV-cache extraction from at least one runtime that exposes
      internal transformer KV tensors.
- [x] Add an owned, version-pinned llama.cpp adapter patch and direct KV matrix
      runner.
- [x] Record one scoped external Rust live-migration proof where standalone QATQ
      preserved exact continuation behavior and beat raw, zstd, and lz4
      transfer-size baselines.
- [x] Freeze source-release API/CLI names before crates.io publish.
- [ ] Add coverage and supply-chain checks.
- [ ] Publish to crates.io when the API is stable.
- [ ] Cut GitHub Releases with binaries once the CLI is useful standalone.
