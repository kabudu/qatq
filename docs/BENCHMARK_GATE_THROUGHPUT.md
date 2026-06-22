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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 393.40us, decode 67.03us (8.1827ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.3828, decode 78.35us (9.5637ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 726.45us, decode 119.47us (7.2920ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.1158, decode 126.12us (7.6977ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 843.33us, decode 119.17us (9.6977ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 0.6540, decode 145.07us (11.8055ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 200.90us, decode 33.89us (8.2730ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 0.0143, decode 30.29us (7.3953ns/value), exact_bits=true |
