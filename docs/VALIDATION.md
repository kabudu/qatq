# Validation

This file records local validation for the current Phase 1 and QATQ exact
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
cargo test qatq_exact_container
cargo test exact_decision
cargo test byte_plane_blocks
cargo test specialized_two_high_raw_two_low_zero_encoder_matches_general_blocks
cargo test --test cli
cargo test --test bench
cargo check --all-targets
cargo test
cargo run --example production_chunk
cargo run --bin qatq -- fixture generate --manifest fixtures/public.manifest --dir fixtures/generated
cargo run --bin qatq -- fixture verify --manifest fixtures/public.manifest --output docs/PUBLIC_FIXTURE_AUDIT.md
cargo run --release --bin qatq-bench -- --no-synthetic --output docs/PUBLIC_COMPARATIVE_BASELINES.md --paper-output docs/PUBLIC_COMPARATIVE_TABLES.md --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- --exact-only --no-synthetic --output docs/PUBLIC_BENCHMARKS.md --paper-output docs/PUBLIC_PAPER_TABLES.md --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- --no-synthetic --quality-output docs/PUBLIC_QUALITY_EXPERIMENTS.md --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- --no-synthetic --task-quality-output docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md --manifest fixtures/public.manifest
cargo run --release --bin qatq-bench -- --exact-only --no-synthetic --manifest fixtures/public.manifest --gate-output docs/PUBLIC_BENCHMARK_GATE.md --gate-require-external --gate-policy production-kv --max-exact-ratio 0.96 --max-exact-encode-us 5000 --max-exact-decode-ns-per-value 50.00 --max-exact-container-ratio 0.97 --max-exact-container-decode-ns-per-value 50.00
cargo run --release --bin qatq-bench -- --exact-only --no-synthetic --manifest fixtures/public.manifest --gate-output docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md --gate-require-external --gate-policy competitive-compression
cargo test --test kv_stress -- --ignored --nocapture
cargo test --release --test kv_stress -- --ignored --nocapture
python3 scripts/ollama_task_quality.py --model phi4-mini:latest
cargo fmt --check
cargo check --manifest-path fuzz/Cargo.toml
cargo package --allow-dirty
cargo package --list --allow-dirty
```

Results:

- `cargo fmt`: passed.
- `cargo test rejects_nonzero_reserved_header_bytes`: passed.
- `cargo test byte_plane`: passed.
- `cargo test qatq_exact_container`: passed.
- `cargo test exact_decision`: passed.
- `cargo test byte_plane_blocks`: passed.
- `cargo test specialized_two_high_raw_two_low_zero_encoder_matches_general_blocks`: passed.
- `cargo test --test cli`: passed.
- `cargo test --test bench`: passed.
- `cargo check --all-targets`: passed.
- `cargo test`: passed.
- `cargo run --example production_chunk`: passed.
- public fixture generation and audit: passed.
- public comparative baseline report: passed.
- public benchmark and paper reports: passed.
- public quality-proxy report: passed.
- public retrieval task-quality report: passed.
- latency-budget gate: failed as expected on large-tensor fixed decode ceilings; exactness, ratio, and encode checks passed. This gate is service-budget analysis, not the large-tensor production readiness signal.
- public production KV throughput gate: passed with the split `production-kv` policy and portable `50.00 ns/value` direct/container decode ceilings.
- public competitive compression gate: passed; every compression-positive public QATQ exact row is at or below the best zstd/lz4 raw-f32 baseline for the same fixture.
- deterministic KV stress matrix: passed locally across 4,096 generated
  KV-shaped cases and 8,499,064 f32 values; exact single-payload, dispatch,
  production chunk, and `QATC` container round trips all preserved bit identity.
  Sampled payload/container mutations were rejected, bounded container limits
  rejected oversized decode attempts, and the default encoder was checked
  against exhaustive encoding on the first 512 eligible cases.
- deterministic KV stress matrix in release mode: passed across the same 4,096
  cases with aggregate ratio `0.1441`, encode throughput `62.09 ns/value`, and
  decode throughput `7.25 ns/value`.
- local Ollama model-output task harness: passed with `phi4-mini:latest`.
  Ollama embedding endpoints were unavailable for this installed model, so the
  harness used deterministic text generation to produce a 12-query by
  24-document relevance-score tensor, ingested it as a runtime fixture, encoded
  and decoded it through QATQ exact, preserved exact f32 bits, compressed the
  tensor to ratio `0.1476`, and preserved raw top-1 score decisions `12/12`.
- `cargo fmt --check`: passed.
- `cargo check --manifest-path fuzz/Cargo.toml`: passed.
- `cargo package --allow-dirty`: passed; package verification compiled the crate from the archive.
- `cargo package --list --allow-dirty`: passed.
- Tests: 109 passed, 0 failed.
  - library tests: 78 passed.
  - benchmark integration tests: 14 passed.
  - CLI integration tests: 17 passed.
- Public benchmark report: regenerated at [PUBLIC_BENCHMARKS.md](PUBLIC_BENCHMARKS.md).
- Public paper table report: regenerated at [PUBLIC_PAPER_TABLES.md](PUBLIC_PAPER_TABLES.md).
- Public production KV throughput gate report: regenerated at [PUBLIC_BENCHMARK_GATE.md](PUBLIC_BENCHMARK_GATE.md).
- Public competitive compression gate report: regenerated at [PUBLIC_COMPETITIVE_COMPRESSION_GATE.md](PUBLIC_COMPETITIVE_COMPRESSION_GATE.md).

Coverage added:

- original `lossy-i4` round trip and payload rejection;
- top-level `QATQ` reserved header byte rejection;
- exact `lossless-f32` bit preservation and corruption detection;
- `phase1-q4` round trip shape preservation and compression ratio;
- `turboquant-q4` reference baseline round trip shape preservation and
  compression ratio;
- `turboquant-q4` deterministic seed/config behavior;
- `turboquant-q4` QJL inner-product estimator consistency with the
  QJL-corrected decoded vector;
- `turboquant-q4` query-length mismatch rejection for inner-product estimates;
- `turboquant-q4` invalid residual-norm rejection;
- deterministic Phase 1 seed/config behavior;
- empty Phase 1 tensor handling;
- partial quaternion-lane handling;
- Phase 1 body magic validation;
- Phase 1 truncated-body validation.
- CLI encode/decode smoke coverage for `phase1-q4 --seed`.
- CLI encode/decode smoke coverage for `turboquant-q4 --seed`.
- `qatq-exact` bit-identical reconstruction including signed zero,
  infinities, and NaN payload bits;
- `qatq-exact` deterministic seed/config behavior;
- adaptive QATQ exact strategy selection across raw-bit, byte-RLE,
  byte-plane RLE, byte-plane zstd, and reversible quaternion-chain zstd
  candidates;
- byte-plane block strategy selection for repetitive whole-plane f32 byte
  layouts in bfloat16-derived runtime captures;
- adjacent-bit QATQ exact delta-XOR byte-plane RLE strategy selection for
  correlated exact bitstreams;
- public QATQ exact strategy inspection for encoded exact payloads;
- public QATQ exact storage-decision APIs that return either a compressed QATQ
  payload or raw f32le pass-through bytes when the selected exact strategy is
  `raw-bits`;
- bounded QATQ exact RLE strategy probing for incompressible streams;
- direct QATQ exact byte-plane RLE strategy probing without materializing the
  plane buffer, with byte-for-byte equivalence against the former materialized
  path;
- direct QATQ exact delta-XOR byte-plane RLE strategy probing without materializing
  the delta buffer, with byte-for-byte equivalence against the materialized
  path;
- bounded QATQ exact delta-XOR byte-plane probing for incompressible streams;
- allocation-reduced QATQ exact byte-RLE decode and byte-plane assembly paths;
- direct QATQ exact byte-plane block decode with fused checksum validation for
  large exact tensors;
- direct QATQ exact byte-plane block encode for the common
  `raw, raw, zero, zero` bfloat16-derived KV layout, with fused checksum
  calculation and byte-for-byte equivalence against the general block encoder;
- preallocated QATQ exact byte-plane run decode buffer from bounded payload
  metadata;
- fast QATQ exact encoding with compression-positive selection across
  byte-plane and reversible quaternion-chain entropy-coded candidates;
- exhaustive QATQ exact encoding for deeper strategy comparison;
- ignored deterministic KV stress matrix covering thousands of generated
  bfloat16-like, low-rank, sparse, repeated-token, raw-noise,
  non-finite/signed-zero, delta-bit, and quaternion-chain-friendly KV cases;
- local Ollama model-output task harness that captures generated relevance
  score tensors, validates fixture ingestion, encodes/decodes through QATQ, and
  verifies task-decision preservation;
- chunked QATQ exact encode/decode round trip across partial chunk
  boundaries;
- chunked QATQ exact empty-input handling and invalid chunk-size rejection;
- sequential `QATC` QATQ exact container exact round trip through top-level
  `decode`;
- sequential `QATC` container empty-input handling and invalid chunk-size
  rejection;
- sequential `QATC` container rejection for nonzero reserved bytes, truncated
  chunk bodies, zero chunk count, total-value mismatches, and trailing data;
- sequential `QATC` version `2` container checksum verification and legacy
  version rejection;
- sequential `QATC` configurable decode limits for total values, chunks, encoded
  bytes, and chunk bytes;
- sequential `QATC` encode appends chunk payloads directly to the final
  container buffer instead of staging all encoded chunks separately;
- sequential `QATC` decode pre-indexes embedded chunk headers and verifies the
  total value count before allocating the output vector;
- sequential `QATC` payload visitor preserves chunk order and validates the full
  container layout before invoking callbacks;
- QATQ exact body magic validation;
- QATQ exact reserved-prefix validation;
- QATQ exact oversized-header rejection;
- QATQ exact malformed run rejection for zero-length runs, unknown run tags,
  truncated repeat runs, and trailing run data;
- QATQ exact delta-XOR byte-plane stream truncation rejection;
- QATQ exact truncated residual stream validation;
- QATQ exact checksum/corruption detection;
- public `try_encode` and seeded QATQ exact `try_encode_*_with_config` APIs return
  `QatqError` on validation failure while preserving bit-identical normal
  round trips.
- direct public `try_encode_lossy_i4` and `try_encode_lossless_f32` APIs provide
  non-panicking mode-specific single-payload encoding; lossless direct encoding
  preserves exact f32 bits.
- generic `try_encode` dispatch is covered across every single-payload mode.
- public single-payload value-count validation rejects oversized inputs without
  requiring an oversized tensor allocation.
- CLI encode/decode smoke coverage for exact `qatq-exact --seed`.
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
- CLI `QATC` decode streams the container input chunk by chunk and preserves an
  existing output file when the version `2` aggregate checksum fails.
- CLI `QATC` decode uses a streaming file reader so it avoids loading the full
  container or decoded tensor while retaining atomic output replacement.
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
  fixture residency for large runtime-scale manifests.
- Benchmark harness smoke coverage for fixture manifests and `--paper-output`.
- Benchmark gate pass/fail behavior for QATQ exact ratio/latency thresholds.
- Benchmark gate pass/fail behavior for `QATC` container ratio thresholds.
- Benchmark exact-only mode for faster readiness gates.
- Benchmark gate pass/fail behavior for throughput-normalized decode
  thresholds.
- Benchmark report, paper-table report, and gate report output preservation when
  fixture loading fails before report generation.
- Benchmark report, paper-table report, and gate report output preservation when
  malformed raw `.f32le` input is rejected before report generation.
- Benchmark report, paper-table report, and gate report output preservation when
  a later manifest fixture fails during external metadata preflight.
- Benchmark harness rows and paper tables for `qatq-exact-container`
  overhead, exactness, and decode time.
- Benchmark harness rows and paper tables include selected QATQ exact strategy
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
- Debug builds of `qatq-bench` use a shorter timing loop so `cargo test` cannot
  hang on expensive exact strategy probes; release benchmark runs keep the
  documented 200 iterations and 3 timing samples.

Known validation limits:

- Benchmarks include deterministic public tensors and a local Ollama
  model-output task tensor, not direct live runtime KV-cache captures.
- The FP8 comparison is a local finite-value software E4M3 baseline, not a
  hardware/runtime FP8 path.
- Phase 1 quality metrics are codec reconstruction metrics only. QATQ exact
  metrics prove bit-identical f32 reconstruction locally, the public retrieval
  proxy verifies top-1 task parity on generated fixtures, and the Ollama harness
  verifies a local model-output task tensor. These do not yet measure language
  model perplexity, agent migration fidelity, latency inside external runtimes,
  residual entropy on live KV tensors, or direct KV-cache extraction from a
  runtime.
