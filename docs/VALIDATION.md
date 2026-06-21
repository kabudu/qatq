# Validation

This file records local validation for the current Phase 1 and Phase 2
implementation.

## 2026-06-21 Phase 1/2 Implementation

Environment:

- OS/arch: `macos` / `aarch64`
- Rust crate: `qatq 0.1.0`

Commands run:

```sh
cargo fmt
cargo test rejects_nonzero_reserved_header_bytes
cargo test byte_plane
cargo test phase2_lossless_container
cargo test --test cli
cargo test --test bench
cargo check
cargo test
cargo run --release --bin qatq-bench -- --output docs/BENCHMARKS.md --paper-output docs/PAPER_TABLES.md
cargo fmt --check
```

Results:

- `cargo fmt`: passed.
- `cargo test rejects_nonzero_reserved_header_bytes`: passed.
- `cargo test byte_plane`: passed.
- `cargo test phase2_lossless_container`: passed.
- `cargo test --test cli`: passed.
- `cargo test --test bench`: passed.
- `cargo check`: passed.
- `cargo test`: passed.
- `cargo run --release --bin qatq-bench -- --output docs/BENCHMARKS.md --paper-output docs/PAPER_TABLES.md`: passed.
- `cargo fmt --check`: passed.
- Tests: 79 passed, 0 failed.
  - library tests: 57 passed.
  - benchmark integration tests: 8 passed.
  - CLI integration tests: 14 passed.
- Benchmark report: regenerated at [BENCHMARKS.md](BENCHMARKS.md).
- Paper table report: regenerated at [PAPER_TABLES.md](PAPER_TABLES.md).

Coverage added:

- original `lossy-i4` round trip and payload rejection;
- top-level `QATQ` reserved header byte rejection;
- exact `lossless-f32` bit preservation and corruption detection;
- `phase1-q4` round trip shape preservation and compression ratio;
- deterministic Phase 1 seed/config behavior;
- empty Phase 1 tensor handling;
- partial quaternion-lane handling;
- Phase 1 body magic validation;
- Phase 1 truncated-body validation.
- CLI encode/decode smoke coverage for `phase1-q4 --seed`.
- `phase2-lossless` bit-identical reconstruction including signed zero,
  infinities, and NaN payload bits;
- `phase2-lossless` deterministic seed/config behavior;
- adaptive Phase 2 raw-bit, byte-RLE, and byte-plane RLE strategy selection;
- adjacent-bit Phase 2 delta-XOR byte-plane RLE strategy selection for
  correlated exact bitstreams;
- public Phase 2 strategy inspection for encoded exact payloads;
- bounded Phase 2 RLE strategy probing for incompressible streams;
- direct Phase 2 byte-plane RLE strategy probing without materializing the
  plane buffer, with byte-for-byte equivalence against the former materialized
  path;
- direct Phase 2 delta-XOR byte-plane RLE strategy probing without materializing
  the delta buffer, with byte-for-byte equivalence against the materialized
  path;
- bounded Phase 2 delta-XOR byte-plane probing for incompressible streams;
- allocation-reduced Phase 2 byte-RLE decode and byte-plane assembly paths;
- preallocated Phase 2 byte-plane run decode buffer from bounded payload
  metadata;
- fast Phase 2 exact encoding with compression-positive byte-plane
  short-circuiting;
- exhaustive Phase 2 exact encoding for deeper strategy comparison;
- chunked Phase 2 exact encode/decode round trip across partial chunk
  boundaries;
- chunked Phase 2 empty-input handling and invalid chunk-size rejection;
- sequential `QATC` Phase 2 container exact round trip through top-level
  `decode`;
- sequential `QATC` container empty-input handling and invalid chunk-size
  rejection;
- sequential `QATC` container rejection for nonzero reserved bytes, truncated
  chunk bodies, zero chunk count, total-value mismatches, and trailing data;
- sequential `QATC` encode appends chunk payloads directly to the final
  container buffer instead of staging all encoded chunks separately;
- sequential `QATC` decode pre-indexes embedded chunk headers and verifies the
  total value count before allocating the output vector;
- sequential `QATC` payload visitor preserves chunk order and validates the full
  container layout before invoking callbacks;
- Phase 2 body magic validation;
- Phase 2 reserved-prefix validation;
- Phase 2 oversized-header rejection;
- Phase 2 malformed run rejection for zero-length runs, unknown run tags,
  truncated repeat runs, and trailing run data;
- Phase 2 delta-XOR byte-plane stream truncation rejection;
- Phase 2 truncated residual stream validation;
- Phase 2 checksum/corruption detection;
- public `try_encode` and seeded Phase 2 `try_encode_*_with_config` APIs return
  `QatqError` on validation failure while preserving bit-identical normal
  round trips.
- direct public `try_encode_lossy_i4` and `try_encode_lossless_f32` APIs provide
  non-panicking mode-specific single-payload encoding; lossless direct encoding
  preserves exact f32 bits.
- generic `try_encode` dispatch is covered across every single-payload mode.
- public single-payload value-count validation rejects oversized inputs without
  requiring an oversized tensor allocation.
- CLI encode/decode smoke coverage for exact `phase2-lossless --seed`.
- CLI encode and `encode-chunked` raw `.f32le` input loading streams directly
  into `f32` values instead of retaining a second full byte buffer.
- CLI single-payload encode rejects oversized raw `.f32le` inputs from file
  metadata before loading tensor values and preserves an existing output file.
- CLI single-payload `QATQ` encode writes through a temporary file and preserves
  an existing output file when input validation fails.
- CLI single-payload `QATQ` decode writes through a temporary file and preserves
  an existing output file when a corrupt payload fails validation.
- CLI `encode-chunked` plus normal `decode` exact byte-for-byte round trip for
  `QATC` containers.
- CLI `encode-chunked` streams raw `.f32le` input one chunk at a time instead of
  loading the full tensor before container construction.
- CLI `QATC` encode writes through a temporary file and preserves an existing
  output file when chunk configuration validation fails.
- CLI `QATC` decode writes through a temporary file and preserves an existing
  output file when a corrupt later chunk fails validation.
- CLI `QATC` decode uses the prevalidated container payload visitor so it avoids
  building a chunk-index vector while retaining atomic output replacement.
- CLI fixture manifest entry creation for validated raw f32le captures.
- CLI fixture manifest appends use temporary-file replacement and preserve an
  existing manifest when fixture validation fails.
- CLI fixture rejection for files whose byte length is not divisible by four.
- CLI fixture audit report generation with stable per-file fingerprints.
- CLI fixture audit report writes use temporary-file replacement and preserve an
  existing audit report when fixture verification fails.
- CLI fixture audit streams each tensor file for fingerprinting instead of
  loading the full capture into memory.
- CLI fixture audit rejection for missing fixture files.
- Benchmark harness smoke coverage for external `--input name:path.f32le`
  fixtures.
- Benchmark harness external fixture loading streams raw `.f32le` bytes directly
  into `f32` values instead of retaining a second full byte buffer.
- Benchmark harness processes external fixture datasets one at a time and keeps
  only dataset metadata plus result rows after each benchmark, bounding peak
  fixture residency for PermeantOS-scale manifests.
- Benchmark harness smoke coverage for fixture manifests and `--paper-output`.
- Benchmark gate pass/fail behavior for Phase 2 exact ratio/latency thresholds.
- Benchmark gate pass/fail behavior for `QATC` container ratio thresholds.
- Benchmark report, paper-table report, and gate report output preservation when
  fixture loading fails before report generation.
- Benchmark report, paper-table report, and gate report output preservation when
  malformed raw `.f32le` input is rejected before report generation.
- Benchmark report, paper-table report, and gate report output preservation when
  a later manifest fixture fails during external metadata preflight.
- Benchmark harness rows and paper tables for `phase2-lossless-container`
  overhead, exactness, and decode time.
- Benchmark harness rows and paper tables include selected Phase 2 strategy
  labels for exact payload evidence.
- Benchmark harness synthetic controls include a `bit-delta` dataset that
  exercises the adjacent-bit delta-XOR byte-plane RLE strategy.
- Benchmark harness supports `--no-synthetic` for external-fixture-only smoke
  runs and gates.
- Benchmark harness preflights external fixture metadata before timing loops, so
  missing or malformed captures fail before report replacement.
- Benchmark timing uses three samples of 200 iterations and reports the best
  sample mean to reduce scheduler-noise sensitivity while keeping local
  validation bounded.

Known validation limits:

- Benchmarks use deterministic synthetic tensors, not live PermeantOS KV-cache
  captures.
- The FP8 comparison is a local finite-value software E4M3 baseline, not a
  hardware/runtime FP8 path.
- Phase 1 quality metrics are codec reconstruction metrics only. Phase 2 exact
  metrics prove bit-identical f32 reconstruction locally, but they do not yet
  measure model perplexity, agent migration fidelity, latency inside
  PermeantOS, residual entropy on real KV tensors, or downstream task success.
