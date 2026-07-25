# Cross-Chunk 16-bit XOR Predictor Experiment

## Goal

Test whether reversible cross-token or cross-row prediction can reduce native
f16/bf16 payload size without weakening bit integrity or materially reducing
encode and decode throughput.

The typed tensor API does not carry shape metadata. Automatic stride discovery
added measurable work to every native tensor, including unsuitable inputs. The
final experiment therefore accepts an explicit stride from a shape-aware
runtime adapter and leaves the default encoder unchanged.

For a selected stride \(s\), it stores:

\[
r_i =
\begin{cases}
u_i, & i < s \\
u_i \oplus u_{i-s}, & i \ge s.
\end{cases}
\]

The two residual byte planes are compressed with Zstd level 3. The four-byte
stride is stored in the payload, so decoding is deterministic and does not need
external shape information.

## Shape-aware one-pass selection

The experiment adds
`try_encode_qatq_exact_tensor_le_with_stride_hint(bytes, dtype, stride_elements)`.
It accepts a validated caller-provided row or chunk width. Invalid, zero,
out-of-range, and f32 stride hints are rejected.

The hinted path first requires more than half of sampled residual bytes to be
zero. Perfectly repeated rows are conservatively returned to ordinary
byte-plane Zstd, which was smaller in the measured fixture. For other candidates,
it compresses identically distributed 1,024-word samples of ordinary byte
planes and strided residual planes. The residual sample must be at most 90% of
the ordinary sample before QATQ runs one full strided Zstd pass. Otherwise it
runs one ordinary byte-plane Zstd pass.

The default API performs no stride work and retains its existing behavior. The
hinted API retains raw and run-coded fallbacks, but the sample
classifier is not an exhaustive proof that its chosen Zstd transform is globally
smallest.

## Method

The experiment executable is:

```text
cargo run --release --example cross_chunk_predictor_experiment
```

Each fixture contains 65,536 native bf16 words. The harness performs eight
warmups and 80 measured encode/decode iterations, verifying exact restored bytes
after every decode. Results below are medians of three process runs compared
with untouched v0.2.1 at commit `27c4187` on the same host.

## Initial two-pass results

| Fixture | v0.2.1 bytes | Experiment bytes | Size change | v0.2.1 encode ns/value | Experiment encode ns/value | v0.2.1 decode ns/value | Experiment decode ns/value |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| repeated token rows | 319 | 319 | 0.0% | 12.590 | 15.508 | 3.369 | 3.704 |
| slowly drifting token rows | 1,025 | 484 | -52.8% | 12.880 | 13.975 | 3.459 | 3.407 |
| adjacent smooth | 1,261 | 1,261 | 0.0% | 14.134 | 13.998 | 3.309 | 3.283 |
| random bits | 131,108 | 131,108 | 0.0% | 12.010 | 11.942 | 2.366 | 2.356 |

The existing native predictor's `piecewise-kv` fixture also changed from 26,965
bytes to 15,450 bytes, a 42.7% reduction. Its median encode time increased from
13.808 to 15.729 ns/value, while median decode time changed from 3.688 to 3.766
ns/value.

## Final one-pass results

The final measurements compare the normal and hinted paths in the same process.
Each number is the median of three process runs.

| Fixture | Normal bytes | Hinted bytes | Size change | Normal encode ns/value | Hinted encode ns/value | Hinted decode ns/value |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| repeated token rows | 319 | 319 | 0.0% | 12.402 | 12.526 | 3.344 |
| slowly drifting token rows | 1,025 | 484 | -52.8% | 12.472 | 12.394 | 3.276 |
| piecewise KV | 26,965 | 15,450 | -42.7% | 12.343 | 12.592 | 3.596 |
| adjacent smooth | 1,261 | 1,261 | 0.0% | 13.778 | 13.515 | 3.331 |
| random bits | 131,108 | 131,108 | 0.0% | 11.583 | 11.696 | 2.227 |

The selected drifting-row case is faster to encode. Piecewise KV pays a 2.0%
encode difference, which is within the experiment's 3% run-to-run noise budget,
while reducing payload size by 42.7%. Non-selected hinted fixtures remain within
the same 3% budget. The default non-hinted path executes no new predictor work.

## Integrity and regression validation

- Every new selected-strategy decode is byte-identical to the native input.
- Tests reject zero-stride metadata, truncated metadata, truncated payloads, and
  checksum-detected residual corruption.
- Arbitrary f16 and bf16 patterns and the ordered stream containing every u16
  pattern remain exact.
- The complete all-target test suite passes.
- The release-mode 4,096-case KV stress matrix passes with 8,499,064 values and
  bit-identical reconstruction.
- Random and adjacent-smooth fixtures retain their established strategies and
  payload sizes.

## Gate decision

**Compression: pass on correlated strided fixtures.**

**Integrity: pass.**

**Decode speed: pass on the measured fixtures.**

**Encode speed: pass within a 3% run-to-run noise budget.**

**Default-path non-regression: pass by construction.** The normal typed encoder
does not perform stride discovery or sample classification.

The opt-in, shape-aware one-pass design passes the production gates for release
as an exact API and CLI capability. Its compression claim remains scoped to the
tested correlated fixtures until real native KV captures with trustworthy
row/head layout metadata select the strategy. The classifier's conservative
threshold deliberately does not claim that the hinted path always finds the
globally smallest ordinary or strided byte-plane payload.
