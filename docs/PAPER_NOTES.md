# Paper Notes

Reference: `Quaternion-Augmented TurboQuant: Fusing Algebraic Structure with
Data-Oblivious Vector Quantization for Extreme KV Cache Compression`.

The proposal combines ideas from:

- TurboQuant: random rotation, scalar quantization under coordinate
  distribution assumptions, and a 1-bit QJL residual;
- quaternion-valued representation: treating groups of four channels as a
  quaternion so cross-component structure can be transformed jointly.

Credit TurboQuant to the Google Research / Google DeepMind / NYU work by Amir
Zandieh, Majid Daliri, Majid Hadian, and Vahab Mirrokni. Credit the mathematical
quaternion/Hamilton-product foundation to William Rowan Hamilton, and credit
the neural-network framing of quaternion entities to prior quaternion neural
network work, including Parcollet, Ravanelli, Morchid, Linarès, Trabelsi, De
Mori, and Bengio. Keep `docs/CREDITS.md` aligned with the paper bibliography.

## Implementation Target

The first paper-faithful implementation should support the training-free
variant:

1. group every four consecutive coordinates into a quaternion lane;
2. apply a deterministic unit-quaternion or Haar-style rotation via Hamilton
   product;
3. flatten the rotated coordinates;
4. apply TurboQuant-style scalar quantization;
5. carry a compact QJL/residual side channel;
6. invert the pipeline and measure model/runtime fidelity.

The original seed implementation did not perform this full pipeline; it used a
deterministic signed-int4 approximation to validate migration-style plumbing.

The `turboquant-q4` mode is now the base TurboQuant-style comparator. It uses
deterministic data-oblivious orthogonal rotation, scalar q4 quantization, and a
QJL residual sign estimator without the quaternion overlay. It is not an
official Google implementation.

## Current Implementation Notes

The `phase1-q4` mode implements the training-free pipeline as a deterministic
codec mode:

- four-coordinate quaternion lanes with zero padding on the final partial lane;
- per-lane deterministic unit-quaternion rotation derived from a stored seed;
- Hamilton-product forward and inverse rotation;
- global symmetric signed q4 scalar quantization;
- QJL-inspired residual experiment using one residual sign bit per rotated
  coordinate plus one global mean absolute residual magnitude.

This is suitable for codec-level experiments and white-paper measurements, but
it is not yet a model-quality result.

The `phase2-lossless` mode is the current exact QATQ-family path. It uses Phase
1 prediction plus run-coded XOR residuals when that wins, adjacent-bit
delta-XOR byte-plane residuals for correlated exact streams, and otherwise falls
back to exact raw-bit, byte-RLE, or byte-plane RLE strategies. It is
bit-identical for `f32` payloads, including signed zero, infinities, and NaN
payload bits. The `QATC` container carries large tensors as sequential Phase 2
chunks for CLI and runtime handoff use.

The first real-data paper-refresh step is now complete for Phase 2 exact
transport. The current paper inputs are:

- `docs/PAPER_REFRESH_NOTES.md`
- `docs/WHITEPAPER_RESULTS.md`
- `docs/PAPER_TABLES.md`
- `docs/PUBLIC_BENCHMARKS.md`
- `docs/PUBLIC_PAPER_TABLES.md`

Phase 1 remains a lossy method experiment. Phase 2 is the evidence-backed exact
transport path for refreshed paper claims.
