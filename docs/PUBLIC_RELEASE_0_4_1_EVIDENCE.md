# QATQ 0.4.1 Release Evidence

QATQ 0.4.1 independently reproduces the complete published finite-certificate
corpus with separate SageMath software. It does not add a theorem engine,
change certificate semantics, or broaden the v0.4.0 claim boundary.

## Differential result

| corpus | rows | QATQ checker | separate SageMath reproduction |
|---|---:|---|---|
| binary Hamming | 14 | 14 valid | 14 agree |
| spherical Rankin, `s = 0` | 5 | 5 valid | 5 agree |
| spherical Rankin, `s < 0` | 8 | 8 valid | 8 agree |
| **total** | **27** | **27 valid** | **27 agree** |

The independent program recomputes the witness, exact integer upper bound, and
decisive inequality from public certificate JSON. It imports no QATQ source or
expected numeric answers. The run uses SageMath 10.6 from an image pinned by
SHA-256 digest with networking disabled.

Pinned image: `sagemath/sagemath:10.6@sha256:19995db6194f4a4bab18ce9a88556fd15b9ed5e916b4504fefe618a7796ddbdb`.

Machine-readable evidence, requests, certificates, hashes, raw Sage output,
and environment versions are published in
[`validation/oracle-v0.4.1/evidence`](../validation/oracle-v0.4.1/evidence).
CI regenerates the full corpus and compares its semantic results with the
published record.

## Release validation

| gate | result |
|---|---|
| format, all-target/all-feature check, and all-feature tests | pass |
| separate SageMath focused tests | pass; 5 tests including false witness and false objective rejection |
| 27-row QATQ/SageMath differential corpus | pass; every witness, upper bound, and decisive inequality agrees |
| all-target line coverage | pass; 84.89% overall |
| RustSec audit | pass; no known vulnerabilities in 40 locked dependencies |
| duplicate dependency check | pass; no duplicates reported |
| deterministic KV stress matrix | pass; 4,096 cases and 8,499,064 values, exact restore |
| public production KV gate | pass; all eight checks below 50 ns/value decode ceilings |
| public competitive compression gate | pass; all compression-positive exact rows beat zstd/lz4 |
| crate package and publish dry run | pass |

The codec, QATC container, llama.cpp adapter patch, pinned matrix harness, public
fixtures, and `Cargo.lock` are unchanged from v0.4.0. Under the documented patch
release exception, the fresh v0.4.0 llama.cpp matrix is inherited rather than
rerun. The release candidate reruns the full exact KV stress and both public
compression gates and makes no new runtime or compression claim.

- adapter patch SHA-256: `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`
- matrix harness SHA-256: `97b19a36e8971ce58711c95d6325e1551150d8e8fb43641582aa8e4d4416697e`
- inherited report SHA-256: `133de6ddacd64f2ccf20999447006fed67694370a817c893482acbac36394492`

## Validation levels

| statement | v0.4.1 status | meaning |
|---|---|---|
| independently checkable by QATQ | complete | the production checker recomputed and accepted every certificate |
| independently reproduced by separate software | complete | SageMath independently computed the same witness and upper bound for all 27 rows |
| externally reviewed by a person | not complete | no attributable human coding-theory review is recorded |

The machine-readable human-review status is
[`external-review.json`](../validation/oracle-v0.4.1/external-review.json).
QATQ does not describe v0.4.1 as externally reviewed.

## Compatibility and claim boundary

QATQ payload version 1, QATC container version 2, Oracle request schema 1,
certificate schema 1, theorem identifiers, codec selection, tensor decoding,
and runtime behavior are unchanged from v0.4.0.

Supported: QATQ's published binary Hamming and spherical Rankin `s <= 0`
certificate rows are independently reproduced by a separate SageMath
implementation.

Not supported: arbitrary KV-cache impossibility, positive-`s` spherical
certificates, asymptotic-to-finite inference, concrete construction search,
automatic distortion conversion, universal compression-capacity claims, or a
claim of completed external human review.
