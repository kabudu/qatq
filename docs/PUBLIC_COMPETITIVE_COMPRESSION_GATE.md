# Benchmark Gate

- status: `pass`
- policy: `competitive-compression`
- readiness role: `competitive compression regression gate; compression-positive QATQ exact rows must beat zstd/lz4 raw-f32le baselines`
- evaluated fixtures: `8`
- external fixtures: `4`
- scope: `external`
- max exact ratio: `unset`
- max exact encode us: `unset`
- max exact decode us: `unset`

- max exact decode ns/value: `unset`

- max exact container ratio: `unset`
- max exact container decode us: `unset`

- max exact container decode ns/value: `unset`

| dataset | check | status | details |
| --- | --- | --- | --- |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 366.87us, decode 61.15us (7.4651ns/value), exact_bits=true; competitive ratio 0.3817 <= best(zstd 0.4665, lz4 0.6901) |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 74.14us (9.0505ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 628.24us, decode 111.05us (6.7781ns/value), exact_bits=true; competitive ratio 0.1153 <= best(zstd 0.2900, lz4 0.4693) |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 119.43us (7.2895ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 674.59us, decode 105.79us (8.6092ns/value), exact_bits=true; competitive ratio 0.6532 <= best(zstd 0.9061, lz4 1.0040) |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 138.39us (11.2619ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 166.57us, decode 28.78us (7.0252ns/value), exact_bits=true; competitive ratio 0.0121 <= best(zstd 0.0413, lz4 0.0673) |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.34us (7.1643ns/value), exact_bits=true |
