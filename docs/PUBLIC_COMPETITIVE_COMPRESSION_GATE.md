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
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 367.75us, decode 61.09us (7.4574ns/value), exact_bits=true; competitive ratio 0.3817 <= best(zstd 0.4665, lz4 0.6901) |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 74.27us (9.0666ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 633.28us, decode 116.94us (7.1373ns/value), exact_bits=true; competitive ratio 0.1153 <= best(zstd 0.2900, lz4 0.4693) |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 125.38us (7.6525ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 691.82us, decode 105.70us (8.6017ns/value), exact_bits=true; competitive ratio 0.6532 <= best(zstd 0.9061, lz4 1.0040) |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 138.26us (11.2520ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 165.68us, decode 28.55us (6.9708ns/value), exact_bits=true; competitive ratio 0.0121 <= best(zstd 0.0413, lz4 0.0673) |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.18us (7.1247ns/value), exact_bits=true |
