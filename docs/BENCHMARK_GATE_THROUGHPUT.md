# Benchmark Gate

- status: `pass`
- policy: `production-kv`
- readiness role: `production readiness for mixed-size external KV tensors; use throughput-normalized decode ns/value ceilings`
- evaluated fixtures: `8`
- external fixtures: `4`
- scope: `external`
- max exact ratio: `0.9600`
- max exact encode us: `5000.0000`
- max exact decode us: `unset`

- max exact decode ns/value: `50.0000`

- max exact container ratio: `0.9700`
- max exact container decode us: `unset`

- max exact container decode ns/value: `50.0000`

| dataset | check | status | details |
| --- | --- | --- | --- |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 380.62us, decode 62.00us (7.5684ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 74.39us (9.0802ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 639.17us, decode 118.15us (7.2113ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 127.34us (7.7724ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 693.02us, decode 107.03us (8.7101ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 140.81us (11.4591ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 168.99us, decode 28.79us (7.0289ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.57us (7.2192ns/value), exact_bits=true |
