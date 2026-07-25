# QATQ 0.3.0 Release Evidence

QATQ 0.3.0 adds an opt-in, shape-aware strided-XOR exact strategy for native
f16 and bf16 tensors. Runtime adapters provide a trusted row or chunk width in
elements. Existing no-hint encoding performs no strided probing and retains its
v0.2.1 behavior.

## Compatibility

- The QATQ envelope version remains `1`.
- The QATC container version remains `2`.
- Existing QATQ and QATC payloads decode unchanged.
- New strided payloads use exact strategy identifier `10`.
- Older decoders reject identifier `10` instead of silently misdecoding it.
- The additive public enum and error variants make this a semver-minor release.

## Classifier and integrity

The hinted encoder:

1. validates a nonzero stride smaller than the tensor element count;
2. confirms that sampled cross-stride XOR residuals are sparse;
3. compares 1,024-word ordinary and residual byte-plane samples;
4. selects strided Zstd only when its sample is at most 90% of the ordinary
   sample;
5. performs one full Zstd pass;
6. validates restored native bytes with the existing payload checksum.

Tests cover f16 and bf16 exact reconstruction, arbitrary and exhaustive u16
patterns, invalid and corrupt stride metadata, truncated streams, residual
corruption, checksum failure, CLI round trips, conservative fallback, and
default-path non-regression.

## Measured experiment

The deterministic 65,536-word fixtures report:

| fixture | ordinary bytes | hinted bytes | payload change |
| --- | ---: | ---: | ---: |
| slowly drifting rows | 1,025 | 484 | -52.8% |
| piecewise KV | 26,965 | 15,450 | -42.7% |
| repeated rows | 319 | 319 | 0.0% |
| adjacent smooth | 1,261 | 1,261 | 0.0% |
| random bits | 131,108 | 131,108 | 0.0% |

Encode measurements remained within the experiment's 3% run-to-run noise
budget. The default no-hint path contains no classifier or predictor work.
See `docs/CROSS_CHUNK_XOR_EXPERIMENT.md` for the complete method and historical
two-pass comparison.

## Fresh llama.cpp matrix

A fresh patched llama.cpp matrix used the official Apache-2.0
Qwen2.5-0.5B-Instruct Q4_0 GGUF, f16 KV export, token budgets 16 and 64, and a
64-element native stride hint. Both packed cases restored exactly and retained
`byte-plane-zstd`, demonstrating conservative fallback on these real captures:

| token budget | raw bytes | QATQ bytes | QATQ ratio | zstd bytes | strategy |
| ---: | ---: | ---: | ---: | ---: | --- |
| 16 | 208,896 | 190,243 | 0.9107 | 196,517 | byte-plane-zstd |
| 64 | 208,896 | 190,243 | 0.9107 | 196,517 | byte-plane-zstd |

This matrix is exact non-regression evidence. It does not claim that the new
strategy improves these captures. The compression wins above remain scoped to
the deterministic correlated fixtures until broader runtime layouts select the
strided strategy.

## Release claim

Supported: shape-aware native f16/bf16 callers can opt into a reversible
cross-row predictor that substantially improves the tested correlated layouts
without changing the default encoder.

Not supported: universal payload improvement, automatic inference of semantic
tensor shape, transparent live VRAM compression, or guaranteed global
optimality between both complete Zstd transforms.
