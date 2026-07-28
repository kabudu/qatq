# RoPE-Covariant Exact Residual Experiment

## Question

Can QATQ exploit the deterministic positional rotation in cached keys without
changing a single native f16 bit?

This experiment is isolated from the production codec and wire formats. It
compares a complete experimental payload against the public production
`qatq-exact` encoder.

## Candidate

For token row \(t\), head \(h\), and RoPE pair \(j\), the predictor transports
the preceding stored key through one known positional step:

\[
\hat{k}_{t,h,j}=R(\omega_j)k_{t-1,h,j},
\qquad
\omega_j=\theta^{-2j/d}.
\]

The stored correction is:

\[
e_t=\operatorname{bits}(k_t)\mathbin{\mathrm{XOR}}
    \operatorname{bits}(\hat{k}_t).
\]

Decode reconstructs prior rows causally, repeats the prediction, and XORs the
correction to recover the original words. The prediction may be approximate;
the correction remains exact.

The retained evaluation uses the Qwen-family NeoX layout, two 64-dimensional
KV heads, a RoPE base of 1,000,000, byte-planed residuals, and Zstd level 3.
The reported size includes 32 bytes of experimental framing and configuration.
Adjacent-pair RoPE and an identity previous-token predictor are controls.

## Evidence

The real capture is the repository's documented patched-llama.cpp
Qwen2.5-0.5B fixture: 24 layers, 17 active token rows, 128 f16 K or V values
per row. Each candidate is byte-identical after decode.

```sh
cargo run --release --example rope_covariant_exact_experiment -- \
  --input-dir qwen-k:/path/to/k-layers \
  --input-dir qwen-v:/path/to/v-layers
```

| dataset | predictor | qatq-exact bytes | candidate bytes | change | encode change | decode change |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Qwen K | identity control | 97,377 | 97,020 | -0.37% | -63% | +19% |
| Qwen K | adjacent RoPE | 97,377 | 97,217 | -0.16% | -63% | +19% |
| Qwen K | NeoX RoPE | 97,377 | 96,628 | -0.77% | -62% | +19% |
| Qwen V | identity control | 98,001 | 100,044 | +2.08% | -64% | +22% |
| Qwen V | adjacent RoPE | 98,001 | 100,025 | +2.07% | -63% | +23% |
| Qwen V | NeoX RoPE | 98,001 | 100,062 | +2.10% | -61% | +21% |

Timing changes are approximate release-mode measurements over 51 iterations.
Negative encode change is faster. The candidate performs less work because it
evaluates one strategy, whereas production `qatq-exact` adaptively evaluates
several exact strategies.

The NeoX transform contributes only about 0.40 percentage points beyond the
identity cross-token predictor on K. Applying it to V is a negative control:
V does not receive RoPE and grows as expected.

## Gate decision

- **Integrity: pass.** Random native words, exact predictor orbits, both pair
  layouts, and the real captures restore byte-for-byte.
- **Real K compression: fail.** The best result is 0.77%, below the 5%
  production selection margin.
- **Encode speed: pass in the harness.** The single candidate is faster than
  the multi-strategy production encoder.
- **Decode speed: fail.** Even with precomputed rotation coefficients, K decode
  is about 19% slower.
- **Portability: unresolved.** A production format cannot depend on
  platform-specific `sin`, `cos`, or floating-point contraction behavior.
  It would require a specified deterministic fixed-point rotation.

## Conclusion

Do not integrate this candidate into production QATQ. It confirms that
position-aware transport exposes a small amount of additional K structure,
and the correct NeoX layout beats both controls, but the 749-byte saving does
not justify a slower decoder, model-shape metadata, or a new deterministic
rotation specification.

The result narrows the useful direction: future exact K work should retain
shape awareness but seek a cheaper predictor with materially higher residual
entropy reduction. Further tuning of this continuous RoPE predictor against
the same capture is unlikely to cross the production gate.
