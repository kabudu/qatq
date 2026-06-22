<p align="center">
  <img src="assets/qatqLogoFinal.png" alt="QATQ logo" width="520">
</p>

# QATQ

**QATQ makes live LLM memory smaller, exact, and portable.**

QATQ is a Rust codec toolkit for **Quaternion-Augmented TurboQuant**: exact
tensor-aware compression for exported LLM KV caches and other high-dimensional
tensor streams used during live agent/runtime migration.

QATQ is standalone. It includes its own deterministic public fixture generator,
public benchmark corpus, CLI, Rust library API, CI workflow, fuzzing scaffold,
and release checklist. External runtime evidence can be attached through
fixture manifests, but no external project is required to build, test, benchmark,
or use QATQ.

The initial release target is exact storage and transfer compression for
exported KV/tensor bytes. Live GPU VRAM reduction is a separate experimental
roadmap goal that would require runtime KV paging/offload integration and
latency proof before it becomes a product claim.

## Status

The current implementation provides:

- deterministic public fixture generation with `qatq fixture generate`;
- public CI-ready fixture, benchmark, paper-table, and gate reports;
- the QATQ exact `qatq-exact` codec as the primary QATQ implementation:
  adaptive exact storage over raw bits, byte-RLE, byte-plane RLE,
  byte-plane zstd entropy coding, reversible quaternion-chain residual coding,
  adjacent-bit delta-XOR byte-plane residuals, or Phase 1 prediction plus coded
  XOR residuals for bit-identical f32 reconstruction;
- a sequential `QATC` chunk container for exact QATQ exact transport of large
  tensors through the CLI;
- production chunk helpers for exact QATQ exact storage decisions and restore;
- an exhaustive QATQ exact encoder variant for research comparisons when payload
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
- a small CLI for encoding, chunked encoding, and decoding raw f32, f16, and
  bf16 little-endian tensor files;
- tests for payload validation, lossy round trips, exact f32 round trips, Phase
  1 deterministic/configured behavior, production chunk restore, CLI behavior,
  and benchmark gate policy.

`qatq-exact` and the `QATC` container are the main QATQ product surface.
They are exact by construction and use a fast strategy policy:
the encoder selects the smallest bit-identical QATQ exact candidate, including a
reversible quaternion-chain residual path when it beats simpler byte-plane
transforms. Phase 1 is still lossy and experimental; it is useful as an
internal predictor and comparator, but lossless QATQ claims apply only to QATQ
exact and QATC. The exhaustive encoder remains available for research
comparisons.
The generated public fixtures are the default reproducible evidence set. Larger
or private runtime captures can be added as optional external manifests. Current
single payloads are bounded to `67,108,864` tensor values each; larger tensors
should use the QATQ exact `QATC` chunk container.

The current source-release API/CLI freeze record is in
[`docs/API_CLI_FREEZE.md`](docs/API_CLI_FREEZE.md). Production-readiness status
and remaining gates are tracked in
[`docs/PRODUCTION_READINESS.md`](docs/PRODUCTION_READINESS.md).
Runtime-agnostic external integration evidence is summarized in
[`docs/EXTERNAL_RUNTIME_EVIDENCE.md`](docs/EXTERNAL_RUNTIME_EVIDENCE.md).
Release packaging and publication are documented in
[`docs/RELEASE_CHECKLIST.md`](docs/RELEASE_CHECKLIST.md): GitHub Release
binaries are built by cargo-dist from annotated tags, while crates.io
publication is a separate manually approved workflow.

## Installation

Until the first public release is tagged, build from source:

```sh
cargo install --path .
```

After a GitHub Release exists, install the prebuilt CLI with the generated
cargo-dist installer:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/kabudu/qatq/releases/download/v0.1.0/qatq-installer.sh | sh
```

On Windows:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/kabudu/qatq/releases/download/v0.1.0/qatq-installer.ps1 | iex"
```

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

For v0.1, QATQ is about exported-state compression: checkpoints, migration
payloads, fixture captures, and cold storage for typed tensor bytes. It is not a
transparent layer between an arbitrary LLM runtime and GPU memory. Live VRAM
reduction would need a runtime adapter that compresses cold KV pages, restores
them on demand, and proves lower peak VRAM without unacceptable token-latency or
behavior regressions.

It is not a general-purpose byte compressor like zstd or lz4. The practical
question for this project is whether QATQ's tensor-aware exact strategy makes
KV/tensor payloads smaller and fast enough to transmit for runtime migration and
storage workloads.

## CLI

Encode a raw f32 little-endian tensor with QATQ exact reconstruction:

```sh
cargo run -- encode --mode qatq-exact input.f32le output.qatq
```

Decode back to raw f32 little-endian:

```sh
cargo run -- decode output.qatq restored.f32le
```

Encode native raw bf16 or f16 little-endian tensors without widening to f32:

```sh
cargo run -- encode --mode qatq-exact --dtype bf16 input.bf16le output.qatq
cargo run -- encode --mode qatq-exact --dtype f16 input.f16le output.qatq
```

Decode writes the same native little-endian tensor bytes that were encoded:

```sh
cargo run -- decode output.qatq restored.bf16le
```

Use an explicit seed for reproducible QATQ exact sweeps:

```sh
cargo run -- encode --mode qatq-exact --seed 0x51415451 input.f32le output.qatq
```

For large tensors, write a QATQ exact chunk container so each embedded payload
stays inside the decoder safety bound while preserving bit-identical
reconstruction across chunk boundaries:

```sh
cargo run -- encode-chunked --max-values-per-chunk 65536 input.f32le output.qatc
```

For native half-precision captures, pass `--dtype` to the same chunked path:

```sh
cargo run -- encode-chunked --max-values-per-chunk 65536 \
  --dtype bf16 input.bf16le output.qatc
```

`encode-chunked` reads and encodes one raw tensor chunk at a time, so the CLI
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

For runtime KV-cache capture work, the preferred route is a llama.cpp adapter
that exports internal K/V cache tensors as raw `.f16le`, `.bf16le`, or `.f32le`
files and then compresses them with QATQ exact. See
[`docs/LLAMA_CPP_KV_CAPTURE.md`](docs/LLAMA_CPP_KV_CAPTURE.md).

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
  --exact-only \
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

Run the public retrieval task-quality report. This verifies that QATQ exact
transport preserves top-1 retrieval decisions on the public fixture corpus and
keeps lossy comparator rows separate:

```sh
cargo run --release --bin qatq-bench -- \
  --no-synthetic \
  --task-quality-output docs/PUBLIC_TASK_QUALITY_EXPERIMENTS.md \
  --manifest fixtures/public.manifest
```

Add optional raw f32 little-endian fixtures from any runtime. Native f16/bf16
runtime captures should use the QATQ CLI `--dtype` path and carry dtype metadata
in their runtime adapter manifest:

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
  --exact-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy production-kv \
  --max-exact-ratio 0.96 \
  --max-exact-encode-us 5000 \
  --max-exact-decode-ns-per-value 50.00 \
  --max-exact-container-ratio 0.97 \
  --max-exact-container-decode-ns-per-value 50.00
```

Run the competitive compression gate. This refuses regressions where a
compression-positive QATQ exact row is larger than the best of the `zstd-raw-f32le`
and `lz4-raw-f32le` baselines over the same public raw f32 fixture:

```sh
cargo run --release --bin qatq-bench -- \
  --exact-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/PUBLIC_COMPETITIVE_COMPRESSION_GATE.md \
  --gate-require-external \
  --gate-policy competitive-compression
```

Run the deterministic KV-cache stress matrix. This ignored integration test
generates thousands of KV-shaped tensors, verifies QATQ exact bit identity through
single payloads, production chunk decisions, `QATC` containers, dispatch decode,
sampled corruption rejection, and default-vs-exhaustive payload size checks:

```sh
cargo test --test kv_stress -- --ignored --nocapture
```

Run the fixed absolute-latency gate only as service-budget analysis for small
tensors or deployment-specific envelopes. It is intentionally not the
large-tensor production readiness signal:

```sh
cargo run --release --bin qatq-bench -- \
  --exact-only \
  --no-synthetic \
  --manifest fixtures/public.manifest \
  --gate-output docs/BENCHMARK_GATE.md \
  --gate-require-external \
  --gate-policy latency-budget \
  --max-exact-ratio 0.95 \
  --max-exact-encode-us 5000 \
  --max-exact-decode-us 1000 \
  --max-exact-container-ratio 0.96 \
  --max-exact-container-decode-us 1200
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
`try_encode_phase1_q4_with_config`, `try_encode_qatq_exact_with_config`,
and `try_encode_qatq_exact_exhaustive_with_config`. Use the chunk APIs or
`QATC` container for larger tensors. Use `qatq_exact_strategy` to inspect
which exact QATQ exact strategy was selected for an encoded payload.

Chunk exact QATQ exact payloads for large tensor blocks:

```rust
use qatq::{decode_qatq_exact_chunks, encode_qatq_exact_chunks};

let values = vec![0.0_f32; 1_000_000];
let chunks = encode_qatq_exact_chunks(&values, 65_536)?;
let decoded = decode_qatq_exact_chunks(chunks.iter().map(Vec::as_slice))?;
assert_eq!(decoded, values);
# Ok::<(), qatq::QatqError>(())
```

Use the sequential QATQ exact container when a single file artifact is needed:

```rust
use qatq::{decode, encode_qatq_exact_container};

let values = vec![0.0_f32; 1_000_000];
let payload = encode_qatq_exact_container(&values, 65_536)?;
let decoded = decode(&payload)?;
assert_eq!(decoded, values);
# Ok::<(), qatq::QatqError>(())
```

Use the prevalidated container visitor when an integration wants to process
chunks without allocating the full decoded tensor:

```rust
use qatq::{
    decode_qatq_exact, encode_qatq_exact_container,
    for_each_qatq_exact_container_payload_with_limits, QatcDecodeLimits,
};

let payload = encode_qatq_exact_container(&[1.0_f32, 2.0, 3.0], 2)?;
let mut decoded_count = 0;
let limits = QatcDecodeLimits {
    max_total_values: 1_000_000,
    ..QatcDecodeLimits::default()
};

for_each_qatq_exact_container_payload_with_limits(&payload, limits, |chunk| {
    let values = decode_qatq_exact(chunk)?;
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

The strongest current external evidence is a 2026-06-22 Rust live-migration run
that built standalone `qatq v0.1.0` from this repository on the target host,
preserved exact continuation behavior through 128 tokens, and transferred
14,004,990 QATQ bytes for the same streamed block artifacts that measured
50,331,648 raw bytes, 20,405,381 zstd bytes, and 28,739,217 lz4 bytes. See
[`docs/EXTERNAL_RUNTIME_EVIDENCE.md`](docs/EXTERNAL_RUNTIME_EVIDENCE.md).

Run the local Ollama model-output task harness when Ollama is available. This
captures model-produced task tensors under ignored `captures/`, ingests them
through the fixture manifest path, QATQ-encodes and decodes them, and writes
[`docs/RUNTIME_TASK_QUALITY_EXPERIMENTS.md`](docs/RUNTIME_TASK_QUALITY_EXPERIMENTS.md):

```sh
python3 scripts/ollama_task_quality.py --model phi4-mini:latest
```
