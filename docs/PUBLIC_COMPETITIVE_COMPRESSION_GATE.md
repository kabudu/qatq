# Benchmark Gate

- status: `pass`
- policy: `competitive-compression`
- readiness role: `competitive compression regression gate; compression-positive Phase 2 rows must beat zstd/lz4 raw-f32le baselines`
- evaluated fixtures: `8`
- external fixtures: `4`
- scope: `external`
- max phase2 ratio: `unset`
- max phase2 encode us: `unset`
- max phase2 decode us: `unset`

- max phase2 decode ns/value: `unset`

- max phase2 container ratio: `unset`
- max phase2 container decode us: `unset`

- max phase2 container decode ns/value: `unset`

| dataset | check | status | details |
| --- | --- | --- | --- |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 402.57us, decode 65.64us (8.0125ns/value), exact_bits=true; competitive ratio 0.3817 <= best(zstd 0.4665, lz4 0.6901) |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.3828, decode 78.34us (9.5629ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 717.52us, decode 134.74us (8.2237ns/value), exact_bits=true; competitive ratio 0.1153 <= best(zstd 0.2900, lz4 0.4693) |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.1158, decode 125.65us (7.6690ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 845.42us, decode 116.29us (9.4637ns/value), exact_bits=true; competitive ratio 0.6532 <= best(zstd 0.9061, lz4 1.0040) |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 0.6540, decode 152.89us (12.4425ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 207.74us, decode 31.19us (7.6139ns/value), exact_bits=true; competitive ratio 0.0121 <= best(zstd 0.0413, lz4 0.0673) |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 0.0143, decode 30.87us (7.5372ns/value), exact_bits=true |
