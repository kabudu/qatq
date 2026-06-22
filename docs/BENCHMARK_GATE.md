# Benchmark Gate

- status: `pass`
- policy: `production-kv`
- readiness role: `production readiness for mixed-size external KV tensors; use throughput-normalized decode ns/value ceilings`
- evaluated fixtures: `8`
- external fixtures: `4`
- scope: `external`
- max phase2 ratio: `0.9600`
- max phase2 encode us: `5000.0000`
- max phase2 decode us: `unset`

- max phase2 decode ns/value: `50.0000`

- max phase2 container ratio: `0.9700`
- max phase2 container decode us: `unset`

- max phase2 container decode ns/value: `50.0000`

| dataset | check | status | details |
| --- | --- | --- | --- |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 405.96us, decode 77.34us (9.4412ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.3828, decode 77.61us (9.4733ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 698.28us, decode 119.46us (7.2914ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.1158, decode 127.50us (7.7823ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 833.57us, decode 111.87us (9.1042ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 0.6540, decode 150.22us (12.2248ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 220.62us, decode 31.22us (7.6210ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 0.0143, decode 39.97us (9.7572ns/value), exact_bits=true |
