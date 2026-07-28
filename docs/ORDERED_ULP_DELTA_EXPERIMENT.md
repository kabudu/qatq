# Ordered-ULP Delta Experiment

## Overlooked structure

QATQ's native f16/bf16 predictors compare raw IEEE words with XOR. XOR captures
shared bit prefixes but is not a metric on numeric distance: mantissa carries,
exponent transitions, and sign boundaries can make nearby values produce large
bit residuals.

This experiment first applies the standard bijective float-flip transform:

\[
F(b)=
\begin{cases}
b\oplus 0x8000 & \text{when the sign bit is zero}\\
\neg b          & \text{when the sign bit is one}
\end{cases}
\]

The resulting unsigned integers preserve IEEE numerical order for ordinary
values while remaining a permutation over all 65,536 bit patterns. For a
shape-provided stride \(s\), the candidate stores:

\[
z_i=\operatorname{zigzag}_{16}\left(F(b_i)-F(b_{i-s})\pmod {2^{16}}\right).
\]

The residuals are byte-planed and compressed with Zstd level 3. Decode uses
only complement, XOR, wrapping addition, shifts, and the inverse ZigZag
mapping. There is no floating-point arithmetic, model state, learned table, or
platform-dependent operation.

## Evidence

The retained evidence uses the documented 24-layer Qwen2.5-0.5B llama.cpp f16
KV capture with 17 rows and a 128-element shape stride. The complete candidate
includes 24 bytes of framing and is compared against the public
shape-aware `qatq-exact` encoder.

```sh
cargo run --release --example ordered_ulp_delta_experiment -- \
  --input-dir qwen-k:/path/to/k-layers \
  --input-dir qwen-v:/path/to/v-layers
```

| dataset | predictor | qatq-exact bytes | candidate bytes | change | encode change | decode change |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Qwen K | adjacent | 97,377 | 100,700 | +3.41% | -83% | +2% |
| Qwen K | stride 128 | 97,377 | 96,340 | **-1.06%** | **-80%** | **-44%** |
| Qwen K | stride 128, second order | 97,377 | 100,377 | +3.08% | -80% | -4% |
| Qwen V | stride 128 | 98,001 | 100,843 | +2.90% | -82% | -41% |

Negative timing change is faster. The standalone candidate performs one Zstd
pass, while production `qatq-exact` adaptively evaluates multiple candidates;
the timing comparison therefore proves a cheap candidate, not an expected 80%
whole-codec speedup after integration.

A 16-way bit transpose was also measured and rejected: the strided K candidate
grew by 0.19% and scalar decode slowed by 19%. The byte-plane form is retained.

## Gate decision

- **Integrity: pass.** Exhaustive tests prove that float-flip and ZigZag are
  bijections over every 16-bit word; arbitrary multi-layer words round-trip
  through all predictors.
- **Model independence: pass.** The transform depends only on dtype and a
  caller-supplied shape stride already supported by QATQ.
- **Compression: promising but below the current production margin.** K saves
  1.06%; V must be rejected by sampled selection.
- **Performance: pass for the candidate.** The operations are constant-time
  integer transforms and the measured selected path is faster in both
  directions.

## Conclusion

Ordered strided ULP delta is a real model-independent opportunity that QATQ
does not currently evaluate. It is the first tested candidate in this sequence
to improve real K size, encode time, and decode time together. It should not be
promoted from one capture or bypass QATQ's conservative 5% margin.

The next evidence gate is a multi-model, multi-prompt f16/bf16 KV matrix. A
production proposal is justified only if sample gating reliably selects gains,
never enlarges output, and the aggregate benefit pays for the additional wire
strategy and maintenance surface.
