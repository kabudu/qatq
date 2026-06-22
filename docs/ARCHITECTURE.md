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
- mode: `lossy-i4`, `lossless-f32`, `phase1-q4`, or `qatq-exact`
- reserved bytes, currently required to be zero
- value count
- scale field
- checksum of the original f32 bitstream
- mode-specific payload

The decoder rejects nonzero reserved header bytes so future protocol extensions
cannot be silently misread by older code. The lossy mode validates structure and
length. The exact mode validates the checksum during decode.

## Chunk Container

Large QATQ exact tensors can be stored in the sequential `QATC` container. This is
a file-level wrapper around normal `QATQ` QATQ exact payloads, not a replacement
for the per-payload codec envelope.

The current and only supported container format is `QATC` version `2`.

The version `2` container header stores:

- magic: `QATC`
- version: `2`
- mode: `qatq-exact`
- reserved bytes
- total decoded f32 value count
- chunk count
- aggregate checksum over the ordered chunk length/payload stream

Each chunk stores a big-endian `u32` byte length followed by one complete
`QATQ` `qatq-exact` payload. The decoder validates the container header,
rejects nonzero reserved bytes, rejects truncated chunks, rejects trailing data,
verifies the version `2` aggregate checksum before visitor callbacks or full
decode allocation, decodes each embedded QATQ exact payload through the normal
checksum path, and verifies that the decoded chunk totals match the container
total. The top-level `decode` function accepts both `QATQ` single payloads and
`QATC` containers. Container decode first indexes embedded chunk headers,
verifies the declared total value count, applies configurable resource limits,
then allocates the output vector once and decodes chunks. This avoids repeated
growth on valid large files while rejecting bogus totals before allocation.

This container is intentionally sequential. It is suitable for runtime handoff
artifacts, CLI round trips, and bounded library integrations, but it does not
provide random-access indexes or a long-lived service protocol.

The CLI writes encoded and decoded artifacts through a temporary file and
renames it only after the full payload has been produced or decoded
successfully. For `QATC`, CLI decode reads and writes one embedded chunk at a
time, keeping peak container-input and decoded-output memory bounded to one
chunk. For all formats, failed
checksum or structural validation leaves any existing output file untouched.
The benchmark harness uses the same temporary-file replacement behavior for
benchmark, paper-table, and gate reports.

## Phase 1 Quaternion Predictor Path

The `phase1-q4` mode is a training-free lossy predictor and research comparator.
It is implemented as a separate mode so the original seed-baseline `lossy-i4`
baseline remains stable for comparison and so QATQ exact can reuse the predictor
when exact residual coding makes it worthwhile. It is not the main QATQ product
surface.

Encoding:

1. group every four consecutive coordinates into a quaternion lane, padding the
   final lane with zeros when needed;
2. derive a deterministic unit quaternion from the stored seed and lane index;
3. rotate each lane with Hamilton product `r * q * conjugate(r)`;
4. flatten the rotated coordinates;
5. apply symmetric signed q4 scalar quantization using one global scale;
6. store a compact residual side channel: one sign bit per rotated coordinate
   plus one global mean absolute residual magnitude.

Decoding:

1. validate the Phase 1 body magic, reserved bytes, quantized length, and
   residual-bit length;
2. dequantize the rotated coordinates;
3. apply the residual sign correction;
4. invert the Hamilton rotation with `conjugate(r) * q * r`;
5. truncate any padded tail coordinates.

The Phase 1 body stores:

- body magic: `P1Q4`
- deterministic rotation seed
- residual magnitude scale
- reserved bytes for future flags
- packed q4 coordinates
- packed residual sign bits

This path is intentionally lossy. The side channel is a QJL-inspired experiment
for reducing reconstruction error and measuring residual structure. It is not
the QATQ exact bit-identical residual codec, and lossless QATQ claims do not apply
to this mode.

## Lossless Strategy

Signed int4 quantization cannot be numerically lossless on its own. The lossless
track therefore needs a residual:

1. transform and quantize the values;
2. reconstruct the approximate values;
3. compute an exact residual against the original f32 bit pattern or a canonical
   integer representation;
4. entropy-code that residual;
5. verify bit-identical reconstruction.

The `qatq-exact` mode is the primary QATQ implementation and the only mode
that carries lossless QATQ claims. The default encoder selects the smallest
bit-identical candidate from byte-level, byte-plane, reversible
quaternion-chain, delta-XOR, and predictor-residual strategies. Runtime KV-cache
captures exposed a common exact pattern where the high two f32 byte planes vary
and the low two byte planes are all zero, so QATQ exact also has a
`byte-plane-blocks` strategy that stores each byte plane as raw, repeated, or
zero. The `encode_qatq_exact_exhaustive` path keeps the deeper
full-candidate strategy search available for research comparisons.

QATQ exact can store:

- raw f32 bits;
- byte-level zero/raw run coding over the original f32 bitstream;
- byte-plane run coding, which groups the first, second, third, and fourth byte
  of each f32 into separate planes before run coding;
- byte-plane block coding, which stores each f32 byte plane as zero, repeated,
  or raw without run metadata when whole-plane structure is stronger than local
  runs;
- byte-plane zstd entropy coding, which first groups f32 bytes by plane and
  then entropy-codes the tensor-aware stream;
- reversible quaternion-chain zstd coding, which groups values into
  four-coordinate quaternion lanes and stores exact wrapping residuals
  `[a - previous_d, b - a, c - b, d - c]` before byte-plane entropy coding;
- adjacent-bit delta-XOR byte-plane run coding, which stores the first f32 bit
  pattern followed by `current_bits ^ previous_bits` residuals in byte-plane
  order;
- Phase 1 prediction plus run-coded XOR residuals.

The predictor strategy:

1. build the same deterministic Phase 1 prediction used by `phase1-q4`;
2. reconstruct the approximate f32 values from the stored Phase 1 body;
3. compute one XOR residual per original f32 bit pattern:
   `original.to_bits() ^ predicted.to_bits()`;
4. code the residual stream as zero runs and raw nonzero runs;
5. reconstruct by applying the XOR residual to the predicted bits;
6. validate the final f32 bitstream with the payload checksum.

The QATQ exact body stores:

- body magic: `QEX1`
- strategy byte and reserved bytes;
- strategy-specific exact payload:
  - raw f32 bits;
  - byte-level run stream;
  - byte-plane run stream;
  - byte-plane zstd stream;
  - reversible quaternion-chain zstd stream;
  - byte-plane block stream;
  - adjacent-bit delta-XOR byte-plane run stream;
  - or deterministic rotation seed, residual magnitude scale, packed q4
    coordinates, packed Phase 1 residual sign bits, and run-coded XOR residuals.

This is bit-identical for f32 payloads, including signed zero, infinities, and
NaN payload bits. Fast selection prevents the predictor path from dominating
encode latency when a byte-plane candidate already compresses exactly.
Exhaustive selection can still be used when smallest payload search matters more
than encode time. QATQ exact is compression-positive on the current real
runtime KV fixtures, but the fixture set is still too small for broad
production claims. The `lossless-f32` mode remains the exact envelope control.

Production callers should use `encode_qatq_exact_decision` or
`try_encode_qatq_exact_decision_with_config` when deciding what to store.
These APIs make the benchmark gate policy first-class:

- `Compressed` returns a normal `QATQ` QATQ exact payload, the selected
  `QatqExactStrategy`, and the original raw f32le byte length.
- `PassThroughRaw` returns raw little-endian f32 bytes when QATQ exact would choose
  the `raw-bits` strategy. That is an explicit instruction to bypass QATQ/QATC
  storage for that tensor rather than persist a compression-negative exact
  envelope.

The existing `encode_qatq_exact*` APIs still always return a valid `QATQ`
payload for research, inspection, and compatibility tests.

Decoder safety bounds:

- payload headers are rejected above `67,108,864` f32 values per payload;
- single-payload `try_encode*` APIs enforce the same bound before writing a
  header and return `QatqError::ValueCountTooLarge` instead of panicking;
- f32 byte lengths and Phase 1 padded coordinate counts are checked before use;
- QATQ exact reserved prefix bytes, unknown strategy bytes, unknown run tags,
  zero-length runs, truncated runs, and trailing run data are rejected;
- run decoders grow output only as validated runs are consumed, avoiding large
  upfront allocations for malformed streams.
- QATQ exact byte-RLE strategy probes are bounded to the raw f32 bitstream size, so
  incompressible byte streams are abandoned before they can become selected
  candidates.
- QATQ exact byte-plane strategy probes run directly over plane order without
  materializing a full byte-plane buffer, and they use the same bounded-abandon
  rule as byte-RLE probes.
- QATQ exact delta-XOR byte-plane probes apply the same bounded run coding directly
  to adjacent f32 bit residuals without materializing a full delta buffer,
  giving correlated tensors another exact residual path before falling back to
  the heavier QATQ predictor.
- Encoded QATQ exact payloads expose their selected exact strategy through
  `qatq_exact_strategy`, and benchmark reports include that label so paper
  evidence can distinguish raw fallback, byte-plane coding, delta-XOR coding,
  and predictor fallback.
- Public QATQ exact storage-decision APIs expose the production compress vs raw
  pass-through decision without requiring callers to parse benchmark reports.
- QATQ exact byte-RLE decode writes f32 values directly from validated runs instead
  of materializing an intermediate byte stream.
- QATQ exact byte-plane decode writes validated plane runs into a preallocated word
  buffer and then converts those words to f32 values without rebuilding an
  interleaved byte stream.
- QATQ exact byte-plane block decode has direct fast paths for the common
  `raw, raw, zero, zero` and `raw, raw, raw, raw` plane layouts; it fuses
  checksum validation with f32 reconstruction to avoid a second pass over large
  bfloat16-derived tensors.
- QATQ exact byte-plane block encode has a direct fast path for the common
  `raw, raw, zero, zero` layout seen in bfloat16-derived runtime KV
  captures. It builds the two raw high-byte planes directly from f32 values and
  fuses checksum calculation, avoiding the full raw-bit staging buffer.
- QATC container decode rejects zero-chunk containers and pre-validates chunk
  lengths, declared value counts, configurable decode limits, and version `2`
  aggregate checksums before allocating the output vector.
- QATC container payload visiting pre-validates the complete chunk layout before
  invoking callbacks, so malformed later chunks cannot cause partial visitor
  side effects.
- QATC container encode writes each QATQ exact chunk directly into the final
  container buffer instead of staging a `Vec<Vec<u8>>` of encoded chunks.
- CLI `encode-chunked` streams raw `.f32le` input into one QATQ exact chunk at a
  time, then writes each payload into the `QATC` artifact through the atomic
  output path.
- The benchmark harness can run with `--no-synthetic` for external-fixture-only
  smoke checks, and it preflights external fixture metadata before timing work
  so missing or malformed captures fail before report replacement.
- The benchmark harness can run with `--exact-only` for readiness gates that
  only need `qatq-exact` and QATC rows.
- Gate reports can enforce either absolute decode microsecond ceilings or
  normalized decode ns/value ceilings. The normalized policy is a better fit
  for comparing captures with substantially different value counts.

Large real tensors can be split with `encode_qatq_exact_chunks` and
reassembled with `decode_qatq_exact_chunks`, or stored as a single
sequential file with `encode_qatq_exact_container`. This is exact across
chunk boundaries. Random-access metadata remains future runtime-integration
work.

## General Compression Scope

QATQ should be evaluated beyond KV caches only where the input is numeric and
structured. It is not expected to beat mature byte compressors on arbitrary
files. A useful generalization target would be "tensor compression" rather than
"general compression."
