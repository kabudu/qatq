# QATQ

QATQ is a new research-grade Rust project for **Quaternion-Augmented
TurboQuant**: a codec family aimed at compressing LLM KV caches and other
high-dimensional tensor streams used during live agent/runtime migration.

This repository is intentionally separate from PermeantOS. PermeantOS proved
that an experimental QATQ-inspired transfer codec can move agent state across a
real MLX-to-vLLM AWS migration path. This project exists to turn the codec
itself into a serious standalone library, CLI, and eventually a service that
any runtime can adopt.

## Status

This is a private seed repository. The current implementation provides:

- a deterministic lossy signed-int4 tensor codec compatible with the first
  PermeantOS experiments;
- an exact `lossless-f32` envelope for bit-identical f32 transport while the
  residual-compression design is developed;
- a small CLI for encoding and decoding raw f32 little-endian files;
- tests for payload validation, lossy round trips, and exact f32 round trips.

It does **not** yet implement the full paper-faithful QATQ pipeline. The paper
roadmap is tracked in [docs/ROADMAP.md](docs/ROADMAP.md).

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

Encode a raw f32 little-endian tensor:

```sh
cargo run -- encode --mode lossy-i4 input.f32le output.qatq
```

Decode back to raw f32 little-endian:

```sh
cargo run -- decode output.qatq restored.f32le
```

Use exact f32 transport:

```sh
cargo run -- encode --mode lossless-f32 input.f32le output.qatq
```

## Library

```rust
use qatq::{decode, encode, CodecMode};

let values = [0.25_f32, -0.5, 1.0, 2.0];
let payload = encode(&values, CodecMode::LossyI4);
let decoded = decode(&payload)?;
# Ok::<(), qatq::QatqError>(())
```

## Relationship To PermeantOS

PermeantOS currently treats `qatq` as an experimental transfer codec. Once this
project matures, PermeantOS should depend on the `qatq` crate instead of owning
codec internals directly.

