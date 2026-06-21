# Architecture

QATQ is split into three layers:

1. **Codec core**: pure Rust encode/decode primitives with deterministic payload
   framing and validation.
2. **Runtime adapters**: future integrations for MLX, vLLM, llama.cpp, and other
   runtimes that can export/import KV tensors.
3. **Service mode**: a future binary service for runtimes that cannot link the
   Rust crate directly.

## Payload Envelope

The seed format is intentionally small:

- magic: `QATQ`
- version: `1`
- mode: `lossy-i4` or `lossless-f32`
- value count
- scale field
- checksum of the original f32 bitstream
- mode-specific payload

The lossy mode validates structure and length. The exact mode validates the
checksum during decode.

## Lossless Strategy

Signed int4 quantization cannot be numerically lossless on its own. The lossless
track therefore needs a residual:

1. transform and quantize the values;
2. reconstruct the approximate values;
3. compute an exact residual against the original f32 bit pattern or a canonical
   integer representation;
4. entropy-code that residual;
5. verify bit-identical reconstruction.

The current `lossless-f32` mode is an honest baseline envelope. It should remain
as a control while residual codecs are evaluated.

## General Compression Scope

QATQ should be evaluated beyond KV caches only where the input is numeric and
structured. It is not expected to beat mature byte compressors on arbitrary
files. A useful generalization target would be "tensor compression" rather than
"general compression."

