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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-blocks, ratio 0.5012, encode 18.21us, decode 18.76us (2.2905ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.5021, decode 17.51us (2.1374ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy byte-plane-blocks, ratio 0.5006, encode 35.57us, decode 34.10us (2.0812ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.5010, decode 34.56us (2.1091ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy raw-bits, ratio 1.0007, encode 422.96us, decode 56.80us (4.6221ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 1.0013, decode 56.47us (4.5952ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy raw-bits, ratio 1.0022, encode 113.22us, decode 18.49us (4.5141ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 1.0039, decode 18.37us (4.4837ns/value), exact_bits=true; no-compress bypass selected |
