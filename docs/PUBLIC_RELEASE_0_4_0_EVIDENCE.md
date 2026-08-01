# QATQ 0.4.0 Release Evidence

QATQ 0.4.0 adds the first production Capacity Oracle scope without changing
QATQ payload version 1, QATC container version 2, codec selection, tensor
decoding, or runtime adapter behavior.

## Finite certificate scope

| model | production theorem | arithmetic | decisive condition |
|---|---|---|---|
| binary | Hamming sphere-packing bound | arbitrary-precision integers | required states exceed `floor(2^n / V(n,t))` |
| spherical, `s<0` | Rankin negative bound | exact decimal rational and integers | required states exceed `floor(1-1/s)` |
| spherical, `s=0` | Rankin orthoplex bound | exact integers | required states exceed `2d` |

The checker recomputes request identity, witness arithmetic, the integer upper
bound, and the strict decisive inequality. Positive-`s` spherical requests,
asymptotic rate results, construction search, and KV constraint derivation do not
produce finite impossibility outcomes in this release.

## Validation

| gate | result |
|---|---|
| format and all-target/all-feature check | pass |
| all-feature test suite | pass; 161 tests, one long stress test intentionally ignored in the ordinary suite |
| focused Oracle conformance/adversarial/CLI tests | pass; 25 tests |
| release KV stress matrix | pass; 4,096 cases and 8,499,064 values, exact restore |
| all-target line coverage | pass; 84.89% overall, including 12 example tests |
| RustSec audit | pass; no known vulnerabilities in 40 locked dependencies |
| duplicate dependency check | pass; no duplicates reported |
| public production KV gate | pass; all eight checks below 50 ns/value decode ceilings |
| public competitive compression gate | pass; all compression-positive exact rows beat zstd/lz4 |
| package build and verification | pass |

The binary conformance suite reproduces nine exact Hamming rows at dimensions
64, 128, and 256. Adversarial tests cover request mutation, false objectives,
false decisive inequalities, truncated and oversized certificates, unknown
schemas and theorems, unknown critical witness fields, altered arithmetic
profiles, and enormous coefficients.

## Fresh llama.cpp smoke matrix

The unchanged pinned adapter was rerun with the official Apache-2.0
Qwen2.5-0.5B-Instruct Q4_0 GGUF and the existing patched `llama-simple` binary.
Both f16 packed cases restored exactly and retained the expected
`byte-plane-zstd` strategy:

| token budget | raw bytes | QATQ bytes | QATQ ratio | zstd bytes | result |
|---:|---:|---:|---:|---:|---|
| 16 | 208,896 | 190,243 | 0.9107 | 196,517 | exact, QATQ smaller |
| 64 | 208,896 | 190,243 | 0.9107 | 196,517 | exact, QATQ smaller |

Adapter patch SHA-256:
`88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`.
The full fresh report is in `docs/LLAMA_CPP_KV_MATRIX.md`.

## Independent verification sequencing

The built-in checker is part of the production soundness boundary and is a
release gate. External mathematician review and comparison against a separate
SageMath, Mathematica, or coding-theory implementation are explicitly scheduled
as post-release evidence and do not block QATQ 0.4.0 publication.

## Release claim

Supported: QATQ can emit and independently check finite infeasibility
certificates under the exact binary Hamming and spherical Rankin `s<=0` models.

Not supported: arbitrary KV-cache impossibility, positive-`s` spherical
certificates, asymptotic-to-finite inference, concrete construction search,
automatic distortion conversion, or universal compression-capacity claims.
