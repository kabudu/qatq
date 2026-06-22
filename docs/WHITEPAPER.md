# QATQ Technical Whitepaper

## Abstract

QATQ began as Quaternion-Augmented TurboQuant: a proposal to combine
TurboQuant-style data-oblivious vector quantization with quaternion structure for
more aggressive compression of LLM KV-cache tensors. The original direction was
lossy and inference-oriented: group tensor coordinates into quaternion lanes,
rotate them deterministically, quantize them, and use a compact residual signal
to preserve useful inner-product behavior.

The current QATQ implementation has evolved into a sharper initial product:
exact tensor-aware compression for exported LLM KV caches and runtime migration
artifacts. The v0.1.0 product surface is `qatq-exact` plus the sequential `QATC`
container. It preserves native tensor bytes bit-for-bit for `f32`, `f16`, and
`bf16` inputs while selecting the smallest exact representation among raw bits,
byte-run strategies, byte-plane strategies, byte-plane zstd entropy coding,
adjacent-bit delta-XOR residuals, Phase 1 predictor residuals, and a reversible
quaternion-chain residual strategy. Lossy TurboQuant-style modes remain in the
repository as comparators and research scaffolding, not as the launch claim.

This whitepaper explains that evolution, the current method, the evidence
supporting v0.1.0, and the boundaries QATQ should keep before claiming broader
production or model-quality superiority.

## 1. Motivation

LLM runtime migration, checkpoint handoff, and cache archival all move large
numeric tensors. KV-cache tensors are especially interesting because they are
structured, high-dimensional, and often produced in repeated runtime layouts.
Generic compressors such as zstd and lz4 can reduce raw tensor byte streams, but
they do not know the tensor semantics, dtype, lane structure, or exactness
requirements of an LLM handoff path.

QATQ targets a narrower and more auditable problem:

- take exported tensor bytes from a runtime or fixture manifest;
- compress them using tensor-aware exact strategies;
- verify bit-identical reconstruction;
- keep enough metadata for storage or transfer decisions;
- avoid claims about live GPU VRAM reduction unless a runtime integration proves
  paging/offload latency under generation workloads.

For v0.1.0, QATQ is therefore best described as an exact storage and transfer
codec for exported LLM KV caches and related tensor streams.

## 2. Original Foundation

The original Quaternion-Augmented TurboQuant paper direction combined two ideas.

TurboQuant contributes the data-oblivious quantization framing: rotate a vector
to improve coordinate behavior, apply scalar quantization, and carry compact
residual information such as a QJL-style sign signal for inner-product
estimation. QATQ credits this direction to the Google Research, Google DeepMind,
and NYU TurboQuant work by Amir Zandieh, Majid Daliri, Majid Hadian, and Vahab
Mirrokni.

Quaternion representation contributes a structured four-component view of tensor
channels. Consecutive coordinates can be grouped into quaternion lanes, rotated
with Hamilton products, and transformed jointly rather than as unrelated scalar
coordinates. QATQ credits the mathematical foundation to William Rowan Hamilton
and the neural-network motivation to prior quaternion neural-network work,
including Parcollet, Ravanelli, Morchid, Linarès, Trabelsi, De Mori, and
Bengio.

The paper-faithful training-free pipeline was:

1. group every four coordinates into a quaternion lane;
2. derive a deterministic unit quaternion or orthogonal rotation;
3. rotate each lane with Hamilton products;
4. flatten the rotated values;
5. apply TurboQuant-style scalar quantization;
6. carry a compact residual signal;
7. invert the transform and measure runtime or model fidelity.

That pipeline is useful research scaffolding. It is not, by itself, an exact
compression product.

## 3. Why The Product Pivoted To Exact Transport

The project moved from "make the lossy idea exist" to release hardening. During
that shift, three constraints became dominant.

First, runtime migration needs correctness evidence. A migration artifact that
changes KV-cache bytes may still be useful, but it immediately demands model
quality, perplexity, and downstream task proof. Exact transport gives the
project a stronger initial contract: decoded bytes are the original bytes.

Second, QATQ needed to compare against simple baselines honestly. Some early
compatibility paths could be larger than raw tensors. Later tensor-aware exact
strategies beat raw, zstd, and lz4 on the current public compression-positive
fixtures and on scoped real KV matrix runs, but only after the codec became
adaptive about exact representations.

Third, the quaternion theme had to earn its place. A reversible quaternion
transform only helps exact compression when it is bit-for-bit reversible, lowers
entropy, costs less metadata than it saves, and beats simpler byte-plane
transforms on real or reproducible fixtures. QATQ now includes exactly that kind
of reversible quaternion-chain candidate inside `qatq-exact`; it is selected
only when it wins the size search.

The result is not a rejection of the original paper. It is a product-oriented
evolution: lossy quaternion TurboQuant remains a comparator, while the launch
surface uses reversible quaternion-backed exact compression when the data
supports it.

## 4. Current Architecture

QATQ has three main layers.

The codec core is pure Rust. It owns payload headers, exact strategy selection,
checksums, decode limits, and typed tensor byte handling.

The `qatq-exact` mode is the primary exact product mode. It stores enough
metadata to reconstruct the original tensor bytes exactly and validates that
reconstruction with checksums. For native typed inputs, QATQ supports `f32`,
`f16`, and `bf16` byte streams without widening `f16` or `bf16` to `f32`.

The `QATC` container is a sequential large-tensor wrapper around complete
`qatq-exact` payload chunks. It is suitable for files, runtime handoff
artifacts, and bounded library integrations. It is not a random-access cache
service and does not claim live VRAM paging behavior.

The architecture diagram is maintained as
[`assets/qatq-architecture.svg`](../assets/qatq-architecture.svg).

## 5. Exact Strategy Selection

`qatq-exact` chooses the smallest bit-identical representation from a strategy
set. The current strategy family includes:

- raw bit storage as the universal exact fallback;
- byte-run and byte-plane run coding;
- direct byte-plane block layouts;
- byte-plane zstd entropy coding;
- adjacent-bit delta-XOR byte-plane residual coding;
- Phase 1 predictor plus coded XOR residuals;
- reversible quaternion-chain residual coding followed by byte-plane zstd.

The reversible quaternion-chain path groups values into four-component lanes and
stores wrapping residuals within and across lanes. It is exact because decode
replays the residual chain and checksum validation verifies the final byte
stream. It is not selected for branding reasons; it is selected only when its
encoded representation is smaller than simpler exact candidates.

On the generated public fixture corpus, byte-plane zstd wins on the bfloat16-like
ramp and noisy fixtures, while quaternion-chain zstd wins on the bfloat16-like
wave and signed-zero/NaN/infinity stress fixtures.

## 6. Production Decision API

QATQ separates "can encode exactly" from "should store the compressed payload".
The production decision API can return:

- `Compressed`: store or transmit a QATQ exact payload;
- `PassThroughRaw`: store or transmit raw tensor bytes with metadata because
  compression would not help.

The generated public corpus currently selects compressed QATQ exact storage for
every row. Pass-through remains part of the public production contract because
future tensors may be compression-negative.

## 7. Evidence Summary

QATQ v0.1.0 is backed by reproducible public evidence and scoped runtime
evidence.

The public fixture corpus contains four generated tensors:

| fixture class | purpose |
| --- | --- |
| bfloat16-like KV ramp | compression-positive typed KV-like structure |
| bfloat16-like KV wave | structured wave pattern where quaternion-chain wins |
| noisy f32 pass-through stress | harder byte distribution with exact fallback pressure |
| signed-zero/NaN/Inf stress | exact bit preservation across special f32 values |

The current public exact-compression summary is:

| metric | result |
| --- | ---: |
| generated public fixtures | 4 |
| compressed decisions | 4 |
| raw pass-through decisions | 0 |
| average direct compressed ratio | 0.2906 |
| average direct compressed reduction | 70.94% |
| maximum direct decode throughput | 10.0976 ns/value |
| maximum QATC decode throughput | 13.9664 ns/value |

Every QATQ exact and QATC row in the public evidence bundle reports exact bit
reconstruction. The competitive compression gate requires every
compression-positive QATQ exact row to be at or below the best zstd/lz4
raw-f32le baseline for the same fixture.

The real-runtime evidence is intentionally scoped:

- local model-output score tensors were ingested as runtime fixtures and QATQ
  exact preserved raw top-1 decisions;
- a patched llama.cpp exporter path captured direct KV tensors and benchmarked
  exact QATQ against zstd/lz4;
- a broader llama.cpp KV matrix reports QATQ wins across selected Qwen and
  Qwen Coder prompt/dtype cases;
- an external Rust live-migration proof transferred a scoped MLX-to-AWS-vLLM
  runtime state with QATQ exact bytes smaller than raw, zstd, and lz4 baselines
  for the same streamed block artifacts.

These results support exact storage and transfer claims. They do not prove live
VRAM reduction, language-model quality improvement, or universal superiority
over every runtime-native compression system.

## 8. Comparator Modes

QATQ retains lossy and control modes because the original paper still matters.

`turboquant-q4` is a local TurboQuant-style comparator. It uses deterministic
data-oblivious rotation, scalar q4 quantization, and structured QJL residual
signs for inner-product estimation. It is not Google's implementation.

`phase1-q4` is the quaternion overlay comparator. It groups values into
quaternion lanes, applies deterministic Hamilton-product rotations, quantizes
with signed q4, and carries a compact residual-sign side channel. It is useful
for codec-level experiments but is not the default product path.

`lossless-f32` is an exact envelope/control mode. It exists to separate generic
exact wrapping from the stronger tensor-aware QATQ exact strategy selection.

These modes should appear in papers and reports as baselines or research
context. Lossless QATQ claims are scoped to `qatq-exact` and `QATC`.

## 9. Security And Robustness

QATQ's exact container hardening focuses on corrupt or malicious inputs:

- reserved header bytes must be zero;
- unknown modes and strategies are rejected;
- oversized counts are rejected before allocation;
- chunk counts and chunk lengths are bounded;
- QATC v2 verifies an aggregate checksum before decode callbacks;
- decode writes through temporary files so failed decodes do not overwrite
  existing outputs;
- fuzzing and deterministic KV stress tests are wired into repository
  validation.

The project still treats broader production hardening as ongoing work. Scheduled
fuzzing must remain green, release owners should review corrupt-input coverage
before tags, and runtime adapters should impose their own size and resource
limits around untrusted artifacts.

## 10. Release Positioning

The correct v0.1.0 claim is:

> QATQ provides exact tensor-aware compression for exported LLM KV caches and
> runtime migration artifacts, with native f32/f16/bf16 byte preservation,
> adaptive exact strategy selection, a sequential QATC container, public
> reproducibility fixtures, and scoped real KV evidence.

The claim should not be:

- QATQ is a drop-in live VRAM reducer;
- QATQ improves model quality;
- QATQ is universally superior to TurboQuant, FP8, or every runtime codec;
- QATC is a random-access runtime cache service;
- the local `turboquant-q4` comparator is an official Google implementation.

## 11. Roadmap

The next research and product work should separate three tracks.

The release track should publish signed GitHub binaries and, once the release
owner approves, the crates.io package.

The evidence track should broaden real KV benchmarks across more model
families, dtypes, prompts, context lengths, and packed chunk sizes, while keeping
zstd/lz4 and runtime-native baselines visible.

The runtime track should explore live VRAM reduction only as an experimental
goal. That work requires runtime KV paging or offload hooks, cold-page
scheduling, partial decode or service semantics, and latency evidence under
active generation.

## 12. Source Documents

This whitepaper is derived from the current repository evidence:

- [`PAPER_NOTES.md`](PAPER_NOTES.md)
- [`PAPER_REFRESH_NOTES.md`](PAPER_REFRESH_NOTES.md)
- [`WHITEPAPER_RESULTS.md`](WHITEPAPER_RESULTS.md)
- [`PUBLIC_COMPRESSION_SUMMARY.md`](PUBLIC_COMPRESSION_SUMMARY.md)
- [`PUBLIC_BENCHMARKS.md`](PUBLIC_BENCHMARKS.md)
- [`PUBLIC_COMPARATIVE_BASELINES.md`](PUBLIC_COMPARATIVE_BASELINES.md)
- [`PUBLIC_TASK_QUALITY_EXPERIMENTS.md`](PUBLIC_TASK_QUALITY_EXPERIMENTS.md)
- [`LLAMA_CPP_KV_COMPRESSION_REPORT.md`](LLAMA_CPP_KV_COMPRESSION_REPORT.md)
- [`LLAMA_CPP_KV_MATRIX.md`](LLAMA_CPP_KV_MATRIX.md)
- [`EXTERNAL_RUNTIME_EVIDENCE.md`](EXTERNAL_RUNTIME_EVIDENCE.md)
- [`CREDITS.md`](CREDITS.md)
