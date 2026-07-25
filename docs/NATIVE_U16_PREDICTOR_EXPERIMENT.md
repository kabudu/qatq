# Native 16-bit Adjacent-XOR Predictor Experiment

## Goal

Test whether a reversible adjacent-word XOR transform can improve native bf16
compression without weakening bit integrity or materially reducing encode and
decode throughput.

For canonical 16-bit words \(u_i\), the transform stores:

\[
r_0 = u_0,\qquad r_i = u_i \oplus u_{i-1}.
\]

Decode applies the same XOR recurrence. The residual bytes are transposed into
two byte planes and compressed with Zstd level 3.

## Selection policy

The encoder constructs the residual planes in one pass and counts zero bytes.
It uses the predictor candidate only when more than half of the residual bytes
are zero. Otherwise it retains the existing byte-plane Zstd candidate. Raw bits
and the other exact candidates remain available as fallbacks.

This gate is deliberately cheap. It prevents the encoder from running two Zstd
passes and avoids selecting the predictor for the tested piecewise and random
fixtures.

## Method

The experiment executable is:

```text
cargo run --release --example native_bf16_predictor_experiment
```

Each fixture contains 65,536 native bf16 words. The harness performs eight
warmups followed by 80 measured encode/decode iterations and verifies the
restored bytes after every decode. The table reports the median of three process
runs on both untouched `origin/master` at `984683b` and the experiment branch.

## Results

| Fixture | Baseline ratio | Predictor ratio | Payload change | Baseline encode ns/value | Predictor encode ns/value | Baseline decode ns/value | Predictor decode ns/value |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| smooth wave | 0.230362 | 0.200645 | -12.9% | 16.938 | 16.249 | 4.628 | 4.290 |
| slow ramp | 0.040863 | 0.011024 | -73.0% | 77.378 | 75.139 | 4.492 | 3.861 |
| piecewise KV | 0.205727 | 0.205727 | 0.0% | 15.613 | 15.901 | 4.353 | 4.280 |
| random bits | 1.000275 | 1.000275 | 0.0% | 14.417 | 14.831 | 2.728 | 2.731 |

The two correlated fixtures select `adjacent-xor-byte-plane-zstd`. The piecewise
fixture retains `byte-plane-zstd`, and random bits retain the raw fallback.
Non-selected encode differences of roughly 2-3% are small enough to require
confirmation on real runtime captures.

## Integrity and validation

- Native f16 and bf16 arbitrary-pattern round trips remain byte-identical.
- The predictor fixture verifies explicit strategy selection and rejects a
  truncated payload.
- The complete test suite passes: 93 library tests, 14 benchmark integration
  tests, and 21 CLI integration tests.
- The release-mode 4,096-case f32 KV stress matrix remains exact and reports
  7.88 ns/value aggregate decode time.

## Conclusion

The result demonstrates additional lossless compression potential for correlated
native 16-bit tensors. On this deterministic corpus it improves size without a
measured throughput regression. Real f16/bf16 KV captures across multiple
models and layouts are still required before treating the sparsity threshold as
production-calibrated.
