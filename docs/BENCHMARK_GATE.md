# Benchmark Gate

- status: `pass`
- policy: `latency-budget`
- readiness role: `service-budget analysis; fixed absolute microsecond ceilings are not the large-tensor production readiness gate`
- evaluated fixtures: `8`
- external fixtures: `4`
- scope: `external`
- max exact ratio: `0.9500`
- max exact encode us: `5000.0000`
- max exact decode us: `1000.0000`

- max exact decode ns/value: `unset`

- max exact container ratio: `0.9600`
- max exact container decode us: `1200.0000`

- max exact container decode ns/value: `unset`

| dataset | check | status | details |
| --- | --- | --- | --- |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 378.77us, decode 62.44us (7.6218ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 75.57us (9.2250ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 639.62us, decode 119.41us (7.2883ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 127.74us (7.7966ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 697.29us, decode 107.00us (8.7077ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 140.27us (11.4154ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 169.21us, decode 29.17us (7.1211ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.35us (7.1659ns/value), exact_bits=true |
