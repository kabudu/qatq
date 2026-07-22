# QATQ 0.1.2 Release Evidence

QATQ 0.1.2 adds an opaque `u32` integration surface and bounded QATC metadata
inspection. It does not change the QATC v2 wire format, exact strategy
selection, tensor decoding, llama.cpp adapter, or runtime claims.

## Required Gates

| gate | result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo check --all-targets --locked` | pass |
| `cargo test --locked` | pass, 121 tests; one separately run stress test ignored by default |
| locked metadata and duplicate dependency check | pass; no duplicate dependencies |
| `cargo audit` | pass, no known vulnerabilities |
| line coverage | pass, 84.95% against a 75% floor |
| public fixture verification | pass |
| production KV benchmark gate | pass for all 8 exact/container rows |
| competitive compression gate | pass for all 8 exact/container rows |
| deterministic KV stress matrix | pass, 4,096 cases and 8,499,064 values |
| crate package verification | pass |

The stress matrix restored all values exactly. Its aggregate QATQ exact size
was 4,900,031 bytes for 33,996,256 raw bytes, a ratio of 0.1441. The larger
container aggregate was 8,407,068 bytes.

## Runtime Matrix Inheritance

The release checklist's patch-release inheritance rule applies. Compared with
tag `v0.1.1`, neither `adapters/llama-cpp/` nor
`scripts/llama_cpp_kv_matrix.py` changed. The previously accepted real-model
matrix remains scoped to the same claims and is recorded in
`docs/LLAMA_CPP_KV_MATRIX.md`.

- adapter patch SHA-256:
  `88963e6ff635f373e44538acff435d2fe75d2c57c128b3eec6ed020671ed8f65`
- matrix report SHA-256:
  `35b681c1b42cb8d4066c1a80ca3a2a160ab321239e4d50a4a5108433fdd719c2`

The 12 recorded packed all-KV cases cover two real Qwen2.5 models and f16,
bf16, and f32 cache representations. All decode checks were exact, and QATQ
was smaller than the recorded zstd and lz4 baselines in every case. QATQ 0.1.2
does not broaden that result into a live-VRAM or general-purpose compression
claim.
