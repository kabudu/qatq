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
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 366.56us, decode 61.11us (7.4592ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | qatq-exact-container | pass | values 8192, ratio 0.3828, decode 73.32us (8.9503ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 627.09us, decode 113.45us (6.9243ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | qatq-exact-container | pass | values 16384, ratio 0.1158, decode 121.83us (7.4358ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 667.75us, decode 105.07us (8.5509ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | qatq-exact-container | pass | values 12288, ratio 0.6540, decode 141.21us (11.4916ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 167.23us, decode 28.47us (6.9514ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | qatq-exact-container | pass | values 4096, ratio 0.0143, decode 29.97us (7.3177ns/value), exact_bits=true |
