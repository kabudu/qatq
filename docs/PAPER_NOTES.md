# Paper Notes

Reference: `Quaternion-Augmented TurboQuant: Fusing Algebraic Structure with
Data-Oblivious Vector Quantization for Extreme KV Cache Compression`.

The proposal combines ideas from:

- TurboQuant: random rotation, scalar quantization under coordinate
  distribution assumptions, and a 1-bit QJL residual;
- quaternion-valued representation: treating groups of four channels as a
  quaternion so cross-component structure can be transformed jointly.

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

The PermeantOS seed implementation did not perform this full pipeline; it used
a deterministic signed-int4 approximation to validate real migration plumbing.

