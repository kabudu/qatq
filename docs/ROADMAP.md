# Roadmap

## Phase 0 - Seed

- [x] Split QATQ into its own repository.
- [x] Add a Rust library crate and CLI.
- [x] Preserve the PermeantOS experimental lossy int4 path as a baseline.
- [x] Add exact f32 envelope mode for bit-identical control tests.
- [x] Document that int4 QATQ is lossy and not the full paper implementation.

## Phase 1 - Paper-Faithful Training-Free QATQ

- [ ] Implement quaternion grouping and Hamilton product rotation.
- [ ] Add deterministic rotation seed/configuration handling.
- [ ] Implement TurboQuant-style scalar quantization.
- [ ] Add QJL/residual side-channel experiments.
- [ ] Benchmark against raw, FP8, and existing PermeantOS lossy int4.

## Phase 2 - Lossless QATQ-Family Mode

- [ ] Define exact reconstruction semantics.
- [ ] Implement residual generation from QATQ reconstruction.
- [ ] Entropy-code residuals and compare against zstd/lz4 baselines.
- [ ] Add property tests for bit-identical f32 reconstruction.

## Phase 3 - Runtime and Service Integration

- [ ] Publish a Rust crate API suitable for PermeantOS.
- [ ] Add a standalone codec service binary.
- [ ] Add MLX, vLLM, and llama.cpp adapter examples.
- [ ] Add streaming encode/decode APIs for large KV blocks.

## Phase 4 - Open Release

- [ ] Prepare public repository hygiene.
- [ ] Add CI, coverage, fuzzing, and supply-chain checks.
- [ ] Publish to crates.io when the API is stable.
- [ ] Cut GitHub Releases with binaries once the CLI is useful standalone.

