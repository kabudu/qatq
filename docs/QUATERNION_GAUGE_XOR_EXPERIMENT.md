# Quaternion Gauge-Aligned XOR Experiment

## Question

Can QATQ improve exact native f16/bf16 compression by aligning each
four-component word group under the quaternion group \(Q_8\) before XOR
prediction?

This experiment does not change the production QATQ or QATC formats. It
compares a complete experimental payload directly with production
`qatq-exact`.

## Exact transform

For a four-word quaternion \(q=(a,b,c,d)\), the eight elements

\[
Q_8=\{\pm1,\pm i,\pm j,\pm k\}
\]

act as signed lane permutations. Signs are implemented by toggling the IEEE
sign bit, never by floating-point arithmetic. For example:

\[
i(a,b,c,d)=(-b,a,-d,c).
\]

Every action is a bijection over all four 16-bit words, including NaN payloads,
infinities, subnormals and signed zero. Decode applies the inverse group
element.

The performance-shaped candidate selects one orientation for a block of 256
quaternions. It scores four evenly spaced quaternions under all eight
orientations, applies the winning orientation to the block, XORs each aligned
quaternion against the preceding original quaternion, byte-planes the
residuals and compresses them with Zstd level 3.

The complete candidate size includes:

- eight bytes of experimental framing;
- packed three-bit orientation symbols compressed with Zstd;
- the compressed residual byte planes.

A bounded 1,024-quaternion probe rejects unsuitable inputs. Final selection
requires the complete candidate to be at least 5% smaller than production
`qatq-exact`.

## Reproduction

```sh
cargo run --release --example quaternion_gauge_xor_experiment
```

Real native tensor files can be added individually or as a directory:

```sh
cargo run --release --example quaternion_gauge_xor_experiment -- \
  --input-dir qwen-k:/path/to/k-layer-files \
  --input-dir qwen-v:/path/to/v-layer-files
```

## Block-size sweep

The real rows use the documented patched llama.cpp Qwen2.5-0.5B f16 KV
capture, separated into 24 K and 24 V layer tensors. Candidate sizes are
deterministic. Timings are medians of 51 release-mode iterations.

| orientation granularity | Qwen K size change | Qwen V size change | Qwen K encode change | Qwen V encode change |
| --- | ---: | ---: | ---: | ---: |
| one quaternion | +5.08% | +3.30% | +103.83% | +104.61% |
| 16 quaternions | +2.93% | +1.70% | +47.78% | +47.71% |
| 64 quaternions | +2.80% | +1.74% | +33.17% | +33.82% |
| 256 quaternions | +2.66% | +1.71% | +29.86% | +30.39% |
| whole tensor | +2.59% | +1.60% | +27.48% | +30.22% |

No tested granularity improves either real capture.

## Synthetic and integrity results

At the retained 256-quaternion granularity:

| dataset | qatq-exact bytes | gauge bytes | change | selected |
| --- | ---: | ---: | ---: | --- |
| quaternion-orbit runs | 1,123 | 291 | -74.09% | yes |
| smooth native control | 308 | 309 | +0.32% | no |
| random control | 131,108 | 131,125 | +0.01% | no |
| ordered every-u16 stream | 816 | 176 | -78.43% | no, conservative probe |
| Qwen K layers | 97,377 | 99,964 | +2.66% | no |
| Qwen V layers | 98,001 | 99,675 | +1.71% | no |

The orbit fixture proves that the transform captures the symmetry it was
designed for. The every-u16 fixture exposes a conservative false negative in
the inexpensive score-based probe; it is an ordered integrity control rather
than representative KV evidence.

Tests verify all eight group actions and inverses over ordinary and special
16-bit patterns, every packed orientation symbol, partial quaternion lengths,
and byte-identical decode for every evaluated payload.

## Gate decision

**Integrity: pass.** The transform is exactly reversible.

**Designed synthetic structure: pass.** Quaternion-orbit runs are 74.09%
smaller than production `qatq-exact`.

**Real compression: fail.** Qwen K and V grow at every orientation
granularity.

**Encode speed: fail.** Even whole-tensor orientation adds 27–30% on the real
capture; the retained 256-quaternion form adds roughly 30%.

**Decode speed: pass.** Experimental decode is faster in the measured harness,
but this cannot compensate for size and encode failures.

## Conclusion

Do not add Quaternion Gauge-Aligned XOR to production QATQ from this evidence.
The mechanism is sound and highly effective when quaternion-orbit symmetry is
actually present, but the available real KV capture does not exhibit that
symmetry and the bounded orientation search exceeds the encode budget.

A revisit would require real data whose lane semantics are known to transform
under signed quaternion permutations. Without that evidence, further tuning
would optimize a synthetic-only structure.
