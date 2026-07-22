# QATQ 0.1.5 Release Evidence

QATQ 0.1.5 hardens the byte-oriented exact QATC decoder by rejecting an
authenticated decoded length outside aggregate policy before output
allocation. It preserves QATC v2 bytes, exact strategy selection, tensor
decoding, the llama.cpp adapter, and runtime claims. It also accepts the
maintained `lz4_flex` 0.14.0 baseline update already merged on master.

## Required Gates

| Gate | Result |
| --- | --- |
| format and all-target check | pass |
| default test suite | pass, 126 tests; one separately run stress test ignored by default |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| RustSec audit | pass, no known vulnerabilities |
| line coverage | pass, 85.40% against a 75% floor |
| public fixture verification | pass |
| production KV benchmark gate | pass for all 8 exact/container rows |
| competitive compression gate | pass for all 8 exact/container rows |
| deterministic KV stress matrix | pass, 4,096 cases and 8,499,064 values |
| pinned llama.cpp adapter matrix | pass, 2 fresh Phi-4 Mini cases |
| crate package | pass, 63 files and 299.1 KiB compressed |
| crates.io publication dry run | pass |

The stress matrix restored all values exactly. Its aggregate QATQ exact size
was 4,900,031 bytes for 33,996,256 raw bytes, a ratio of 0.1441. The QATC
aggregate was 8,407,068 bytes.

## Fresh Runtime Matrix

The pinned llama.cpp commit `7992aa7c8` was rebuilt after applying QATQ's owned
adapter patch. Two Phi-4 Mini f16 cases, at token budgets 16 and 64, exported
and exact-decoded 2,228,224 raw bytes each. QATQ stored each case in 1,929,497
bytes, ratio 0.8659, compared with 2,075,661 zstd bytes and 2,236,777 LZ4 bytes.

- adapter patch SHA-256:
  `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`
- matrix report SHA-256:
  `6a9d180085a4afc2a974c772fa252679a24d4d1fc625b41dd1011eae8924d27a`

This fresh matrix validates the maintained LZ4 baseline update and the existing
exported-state transport path. It does not broaden QATQ's claims to universal
compression superiority or live GPU-memory reduction.
