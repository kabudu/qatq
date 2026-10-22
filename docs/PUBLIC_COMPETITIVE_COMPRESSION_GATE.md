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
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 404.27us, decode 65.22us (7.9608ns/value), exact_bits=true; competitive ratio 0.3817 <= best(zstd 0.4665, lz4 0.6901) |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 75.65us (9.2343ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 910.75us, decode 151.96us (9.2746ns/value), exact_bits=true; competitive ratio 0.1153 <= best(zstd 0.2900, lz4 0.4693) |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 157.00us (9.5824ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 702.85us, decode 107.43us (8.7431ns/value), exact_bits=true; competitive ratio 0.6532 <= best(zstd 0.9061, lz4 1.0040) |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 141.63us (11.5261ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 172.88us, decode 28.97us (7.0732ns/value), exact_bits=true; competitive ratio 0.0121 <= best(zstd 0.0413, lz4 0.0673) |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.79us (7.2733ns/value), exact_bits=true |
