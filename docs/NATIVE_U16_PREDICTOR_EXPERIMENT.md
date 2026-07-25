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

The encoder first samples at most 4,096 words in 64 short consecutive windows
distributed across the tensor, without allocating a residual buffer. Consecutive
windows avoid aliasing periodic layouts while retaining broad coverage. Only a
sample with more than half zero residual bytes proceeds to full residual
construction. The full stream must independently meet the same sparsity
threshold before the predictor candidate can be selected. Otherwise the existing
byte-plane Zstd candidate remains available. Raw bits and the other exact
candidates remain available as fallbacks.

This bounded gate prevents the encoder from running two Zstd passes or
allocating a tensor-sized residual buffer for unsuitable inputs. It avoids
selecting the predictor for the tested piecewise, random, and real llama.cpp KV
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
- An exhaustive ordered stream containing every possible 16-bit word round
  trips through the adaptive exact encoder for both native dtypes.
- The predictor fixture verifies explicit strategy selection and rejects a
  truncated payload.
- A dedicated native typed tensor fuzz target exercises both f16 and bf16
  encode/decode.
- The release-mode 4,096-case f32 KV stress matrix remains exact and reports
  7.88 ns/value aggregate decode time.

## Live runtime validation

A fresh pinned llama.cpp Phi-4 Mini matrix covered two token budgets for both
native f16 and bf16 packed KV bundles. Every row restored exactly and retained
the existing `byte-plane-zstd` strategy:

| dtype | raw bytes | QATQ bytes | ratio | selected strategy |
| --- | ---: | ---: | ---: | --- |
| f16 | 2,228,224 | 1,929,497 | 0.8659 | byte-plane-zstd |
| bf16 | 2,228,224 | 1,600,669 | 0.7184 | byte-plane-zstd |

This is non-regression evidence for the bounded preflight on genuine KV data.
It does not claim that the predictor improves these particular captures.

## Conclusion

The result demonstrates additional lossless compression potential for highly
correlated native 16-bit tensors while safely retaining the established strategy
on the tested real captures. The exactness, bounded-memory selection path, fuzz
coverage, and real-capture non-regression evidence make the strategy suitable
for release. Broader model and layout coverage remains necessary before making
a general claim about predictor selection frequency or compression gains.
