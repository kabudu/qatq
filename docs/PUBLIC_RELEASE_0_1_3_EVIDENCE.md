# QATQ 0.1.3 Release Evidence

QATQ 0.1.3 adds bounded, opaque `u32` chunk visitation and fixes the crates.io
path-scrub workflow self-match discovered while publishing 0.1.2. It does not
change QATC v2 bytes, exact strategy selection, tensor decoding, the llama.cpp
adapter, or runtime claims.

## Required Gates

| gate | result |
| --- | --- |
| format and all-target check | pass |
| default test suite | pass, 123 tests |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| RustSec audit | pass, no known vulnerabilities |
| line coverage | pass, 85.00% against a 75% floor |
| public fixture verification | pass |
| production KV benchmark gate | pass for all 8 exact/container rows |
| competitive compression gate | pass for all 8 exact/container rows |
| deterministic KV stress matrix | pass, 4,096 cases and 8,499,064 values |

The stress matrix restored all values exactly. Its aggregate QATQ exact size
was 4,900,031 bytes for 33,996,256 raw bytes, a ratio of 0.1441. The QATC
aggregate was 8,407,068 bytes.

## Runtime Matrix Inheritance

The patch-release inheritance rule applies. Compared with `v0.1.2`, neither
the pinned llama.cpp adapter nor matrix harness changed. The existing real-model
matrix remains scoped to the same exact exported-state transport claims.

- adapter patch SHA-256:
  `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`
- matrix report SHA-256:
  `35b681c1b42cb8d4066c1a80ca3a2a160ab321239e4d50a4a5108433fdd719c2`

The new visitor delegates decoding to the existing exact codec after complete
container prevalidation. Tests prove callback suppression on an invalid policy,
bounded chunk sizes, ordered reconstruction, and preservation of arbitrary
32-bit patterns.
