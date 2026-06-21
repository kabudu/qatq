# Runtime Adapter Contract

QATQ is a standalone codec project. Runtime integrations should depend on the
public QATQ API and should not require runtime-specific fixture paths, metadata,
or daemon behavior.

## Production Chunk Flow

Use QATQ's production chunk helpers:

```rust
let encoded = qatq::try_encode_production_chunk(values)?;
send(encoded.metadata.storage_label(), encoded.metadata.raw_f32le_len, encoded.bytes);
```

Restore with the metadata that traveled with the chunk:

```rust
let values = qatq::restore_production_chunk(&metadata, bytes)?;
```

Supported storage labels:

- `qatq-phase2`: `bytes` is a QATQ phase-2 exact payload.
- `raw-f32le-pass-through`: `bytes` is raw little-endian f32.

Adapters must preserve:

- `storage`
- `raw_f32le_len`
- optional phase-2 `strategy`
- byte payload without transcoding

## Required Runtime Behavior

- Reject chunks that omit storage metadata.
- Reject unknown storage labels.
- Restore bytes before committing migrated state.
- Compare or checksum restored values when the runtime has an original tensor.
- Treat pass-through chunks as successful QATQ decisions, not compression
  failures.

## Adapter Examples

The `examples/` directory contains a Rust production-chunk example that can be
used as the reference for MLX, vLLM, llama.cpp, or other runtime-specific
adapters.

Runtime-specific adapters should live in their own repositories unless they are
dependency-free examples that compile as part of QATQ CI.
