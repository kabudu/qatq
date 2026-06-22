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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-zstd, ratio 0.3817, encode 399.83us, decode 69.88us (8.5308ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.3828, decode 77.73us (9.4884ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy quaternion-chain-zstd, ratio 0.1153, encode 731.13us, decode 124.22us (7.5816ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.1158, decode 124.40us (7.5925ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy byte-plane-zstd, ratio 0.6532, encode 805.00us, decode 124.08us (10.0976ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 0.6540, decode 171.62us (13.9664ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy quaternion-chain-zstd, ratio 0.0121, encode 201.27us, decode 32.31us (7.8885ns/value), exact_bits=true |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 0.0143, decode 32.86us (8.0216ns/value), exact_bits=true |
