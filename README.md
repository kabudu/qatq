# QATQ

QATQ is a research-grade Rust project for **Quaternion-Augmented
TurboQuant**: a codec family aimed at compressing LLM KV caches and other
high-dimensional tensor streams used during live agent/runtime migration.

QATQ is standalone. It includes its own deterministic public fixture generator,
public benchmark corpus, CLI, Rust library API, CI workflow, fuzzing scaffold,
and release checklist. External runtime evidence can be attached through
fixture manifests, but no external project is required to build, test, benchmark,
or use QATQ.

## Status

The current implementation provides:

- deterministic public fixture generation with `qatq fixture generate`;
- public CI-ready fixture, benchmark, paper-table, and gate reports;
- the Phase 2 `phase2-lossless` codec as the primary QATQ implementation:
  adaptive exact storage over raw bits, byte-RLE, byte-plane RLE,
  byte-plane zstd entropy coding, reversible quaternion-chain residual coding,
  adjacent-bit delta-XOR byte-plane residuals, or Phase 1 prediction plus coded
  XOR residuals for bit-identical f32 reconstruction;
- a sequential `QATC` chunk container for exact Phase 2 transport of large
  tensors through the CLI;
- production chunk helpers for exact Phase 2 storage decisions and restore;
- an exhaustive Phase 2 encoder variant for research comparisons when payload
  size search is more important than encode latency;
- a deterministic lossy signed-int4 tensor codec retained as a seed baseline;
- a Phase 1 training-free `phase1-q4` codec with quaternion grouping,
  deterministic Hamilton-product rotation, scalar q4 quantization, and a
  compact 1-bit residual-sign side channel, retained as a lossy predictor and
  research comparator rather than the product path;
- a `turboquant-q4` reference baseline for measuring the base random-rotation
  scalar quantization path plus structured QJL residual inner-product estimator
  before the quaternion overlay, not an official Google implementation or the
  default QATQ foundation;
- an exact `lossless-f32` envelope for bit-identical f32 transport while the
  residual-compression design is developed;
- a small CLI for encoding, chunked encoding, and decoding raw f32
  little-endian files;
- tests for payload validation, lossy round trips, exact f32 round trips, Phase
  1 deterministic/configured behavior, production chunk restore, CLI behavior,
  and benchmark gate policy.

`phase2-lossless` and the `QATC` container are the main QATQ product surface.
They are exact by construction and use a fast strategy policy:
the encoder selects the smallest bit-identical Phase 2 candidate, including a
reversible quaternion-chain residual path when it beats simpler byte-plane
transforms. Phase 1 is still lossy and experimental; it is useful as an
internal predictor and comparator, but lossless QATQ claims apply only to Phase
2. The exhaustive encoder remains available for research comparisons.
The generated public fixtures are the default reproducible evidence set. Larger
or private runtime captures can be added as optional external manifests. Current
single payloads are bounded to `67,108,864` f32 values each; larger tensors
should use the Phase 2 `QATC` chunk container.

## Attribution

QATQ is an independent project. TurboQuant should be credited to the Google
Research / Google DeepMind / NYU work by Amir Zandieh, Majid Daliri, Majid
Hadian, and Vahab Mirrokni. The quaternion/Hamilton-product foundation traces
to William Rowan Hamilton, with modern neural-network motivation from prior
quaternion neural-network work such as Parcollet, Ravanelli, Morchid, Linarès,
Trabelsi, De Mori, and Bengio. See [docs/CREDITS.md](docs/CREDITS.md).

## What QATQ Is For

QATQ is designed for structured numeric streams:

- LLM KV caches;
- activation-like tensor blocks;
- vector database and embedding payloads;
- runtime migration packets that can tolerate bounded numeric error or carry a
  residual for exact reconstruction.

It is not a general-purpose byte compressor like zstd or lz4. A QATQ-family
lossless codec is possible, but only by transmitting enough residual
information to reconstruct the original values exactly. The practical question
for this project is whether the QATQ transform makes those residuals smaller
and faster to transmit for KV/tensor workloads.

## CLI

Encode a raw f32 little-endian tensor with QATQ exact reconstruction:

```sh
cargo run -- encode --mode phase2-lossless input.f32le output.qatq
```

Decode back to raw f32 little-endian:

```sh
cargo run -- decode output.qatq restored.f32le
```

Use an explicit seed for reproducible Phase 2 sweeps:

```sh
cargo run -- encode --mode phase2-lossless --seed 0x51415451 input.f32le output.qatq
```

For large tensors, write a Phase 2 chunk container so each embedded payload
stays inside the decoder safety bound while preserving bit-identical
reconstruction across chunk boundaries:

```sh
cargo run -- encode-chunked --max-values-per-chunk 65536 input.f32le output.qatc
```

`encode-chunked` reads and encodes one raw `.f32le` chunk at a time, so the CLI
does not need to hold the full tensor in memory while building the `QATC`
artifact. `QATC` uses the version `2` container header with an aggregate
checksum over the ordered chunk length/payload stream.

The normal decode command accepts both `QATQ` single payloads and `QATC`
containers. CLI encode and decode writes go through a temporary file and replace
the target only after the payload has been fully produced or validated and
written successfully:

```sh
cargo run -- decode output.qatc restored.f32le
```

Use exact f32 envelope transport as a control baseline:

```sh
cargo run -- encode --mode lossless-f32 input.f32le output.qatq
```

Use the reference base TurboQuant-style q4 path only for lossy comparator
experiments:

```sh
cargo run -- encode --mode turboquant-q4 input.f32le output.qatq
```

Use the Phase 1 quaternion path as a lossy predictor/comparator experiment:

```sh
cargo run -- encode --mode phase1-q4 input.f32le output.qatq
```

Generate the public fixture corpus:

```sh
cargo run --bin qatq -- fixture generate \
  --manifest fixtures/public.manifest \
  --dir fixtures/generated
```

Run the public benchmark report:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --output docs/PUBLIC_BENCHMARKS.md \
  --paper-output docs/PUBLIC_PAPER_TABLES.md \
  --manifest fixtures/public.manifest
```

The short release-facing compression table is maintained in
[`docs/PUBLIC_COMPRESSION_SUMMARY.md`](docs/PUBLIC_COMPRESSION_SUMMARY.md).

Run the public quality-proxy report:

```sh
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --quality-output docs/PUBLIC_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
```

Run the public retrieval task-quality report. This verifies that Phase 2 exact
transport preserves top-1 retrieval decisions on the public fixture corpus and
keeps lossy comparator rows separate:

```sh
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --task-quality-output docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
```

Add optional raw f32 little-endian fixtures from any runtime:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --input runtime-kv:path/to/kv-cache.f32le
```

Use a fixture manifest and generate paper-ready summary tables:

```sh
cargo run --release --bin qatq-bench -- \
  --output docs/BENCHMARKS.md \
  --paper-output docs/PAPER_TABLES.md \
  --quality-output docs/PUBLIC_QUALITY_EXPERIMENTS.md \
  --task-quality-output docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
```

Add `--no-synthetic` when you want an external-fixture-only smoke run or gate.
The benchmark harness preflights external fixture paths and raw `.f32le` byte
lengths before running timing loops, so missing or malformed captures fail
before report outputs are replaced.

See [docs/FIXTURES.md](docs/FIXTURES.md) for the manifest format.

Append a validated runtime/KV fixture entry:

```sh
cargo run -- fixture add \
  --manifest fixtures/runtime.manifest \
  --group runtime-kv \
  --name llama-layer12-k \
  --path captures/llama-layer12-k.f32le \
  --shape "[layers=1, heads=32, tokens=128, dim=128]" \
  --notes "runtime source capture"
```

Verify a manifest and write an audit report:

```sh
cargo run -- fixture verify \
  --manifest fixtures/public.manifest \
  --output docs/PUBLIC_FIXTURE_AUDIT.md
```

Run the production KV readiness gate. This is the primary gate for mixed-size
external KV tensors because decode is judged by throughput-normalized
nanoseconds per value:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-phase2-ratio 0.96 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-ns-per-value 50.00 \
  --max-phase2-container-ratio 0.97 \
  --max-phase2-container-decode-ns-per-value 50.00
```

Run the competitive compression gate. This refuses regressions where a
compression-positive Phase 2 row is larger than the best of the `zstd-raw-f32le`
and `lz4-raw-f32le` baselines over the same public raw f32 fixture:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md \
  --gate-require-external \
  --gate-policy competitive-compression
```

Run the fixed absolute-latency gate only as service-budget analysis for small
tensors or deployment-specific envelopes. It is intentionally not the
large-tensor production readiness signal:

```sh
cargo run --release --bin qatq-bench -- \
  --phase2-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy latency-budget \
  --max-phase2-ratio 0.95 \
  --max-phase2-encode-us 5000 \
  --max-phase2-decode-us 1000 \
  --max-phase2-container-ratio 0.96 \
  --max-phase2-container-decode-us 1200
```

## Library

```rust
use qatq::{decode, try_encode, CodecMode};

let values = [0.25_f32, -0.5, 1.0, 2.0];
let payload = try_encode(&values, CodecMode::Phase1Q4)?;
let decoded = decode(&payload)?;
# Ok::<(), qatq::QatqError>(())
```

Runtime integrations should prefer `try_encode` for single-payload artifacts so
oversized inputs return `QatqError::ValueCountTooLarge` instead of panicking.
Direct fallible helpers are also available for mode-specific callers:
`try_encode_lossy_i4`, `try_encode_lossless_f32`,
`try_encode_phase1_q4_with_config`, `try_encode_phase2_lossless_with_config`,
and `try_encode_phase2_lossless_exhaustive_with_config`. Use the chunk APIs or
`QATC` container for larger tensors. Use `phase2_lossless_strategy` to inspect
which exact Phase 2 strategy was selected for an encoded payload.

Chunk exact Phase 2 payloads for large tensor blocks:

```rust
use qatq::{decode_phase2_lossless_chunks, encode_phase2_lossless_chunks};

let values = vec![0.0_f32; 1_000_000];
let chunks = encode_phase2_lossless_chunks(&values, 65_536)?;
let decoded = decode_phase2_lossless_chunks(chunks.iter().map(Vec::as_slice))?;
assert_eq!(decoded, values);
# Ok::<(), qatq::QatqError>(())
```

Use the sequential Phase 2 container when a single file artifact is needed:

```rust
use qatq::{decode, encode_phase2_lossless_container};

let values = vec![0.0_f32; 1_000_000];
let payload = encode_phase2_lossless_container(&values, 65_536)?;
let decoded = decode(&payload)?;
assert_eq!(decoded, values);
# Ok::<(), qatq::QatqError>(())
```

Use the prevalidated container visitor when an integration wants to process
chunks without allocating the full decoded tensor:

```rust
use qatq::{
    decode_phase2_lossless, encode_phase2_lossless_container,
    for_each_phase2_lossless_container_payload_with_limits, QatcDecodeLimits,
};

let payload = encode_phase2_lossless_container(&[1.0_f32, 2.0, 3.0], 2)?;
let mut decoded_count = 0;
let limits = QatcDecodeLimits {
    max_total_values: 1_000_000,
    ..QatcDecodeLimits::default()
};

for_each_phase2_lossless_container_payload_with_limits(&payload, limits, |chunk| {
    let values = decode_phase2_lossless(chunk)?;
    decoded_count += values.len();
    Ok(())
})?;
assert_eq!(decoded_count, 3);
# Ok::<(), qatq::QatqError>(())
```

## External Validation

QATQ does not depend on any external runtime. Historical runtime-integration
evidence can be kept outside this repository as optional validation provenance.
New runtime integrations should follow
[docs/RUNTIME_ADAPTERS.md](docs/RUNTIME_ADAPTERS.md) and provide ordinary
fixture manifests rather than runtime-specific project coupling.
