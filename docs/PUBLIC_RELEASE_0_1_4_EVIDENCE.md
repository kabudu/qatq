# QATQ 0.1.4 Release Evidence

QATQ 0.1.4 adds byte-oriented exact QATC wrappers for protocols that already
authenticate their decoded byte length. It does not change QATC v2 bytes,
exact strategy selection, tensor decoding, the llama.cpp adapter, or runtime
claims.

## Required Gates

| Gate | Result |
| --- | --- |
| format and all-target check | pass |
| default test suite | pass, 125 tests |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| RustSec audit | pass, no known vulnerabilities |
| line coverage | pass, 85.40% against a 75% floor |
| public fixture verification | pass |
| production KV benchmark gate | pass for all 8 exact/container rows |
| competitive compression gate | pass for all 8 exact/container rows |
| deterministic KV stress matrix | pass, 4,096 cases and 8,499,064 values |
| crate package | pass, 62 files and 300.6 KiB compressed |
| crates.io publication dry run | pass |

The stress matrix restored all values exactly. Its aggregate QATQ exact size
was 4,900,031 bytes for 33,996,256 raw bytes, a ratio of 0.1441. The QATC
aggregate was 8,407,068 bytes.

## Runtime Matrix Inheritance

The patch-release inheritance rule applies. Compared with `v0.1.3`, neither
the pinned llama.cpp adapter nor matrix harness changed. The existing real-model
matrix remains scoped to the same exact exported-state transport claims.

- adapter patch SHA-256:
  `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`
- matrix report SHA-256:
  `35b681c1b42cb8d4066c1a80ca3a2a160ab321239e4d50a4a5108433fdd719c2`
- matrix harness SHA-256:
  `a91a00d231ff0c239d4d4b52cecacb653639258bac982aef4b3f3147e3416886`

The new byte APIs delegate to the opaque `u32` exact codec using canonical
little-endian packing. Tests require wire identity with the existing word API,
exact recovery for every tail length, complete validation before callbacks,
and rejection of non-zero final-word padding.
