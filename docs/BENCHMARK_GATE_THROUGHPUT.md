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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-blocks, ratio 0.5012, encode 23.19us, decode 17.69us (2.1598ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.5021, decode 18.74us (2.2873ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy byte-plane-blocks, ratio 0.5006, encode 38.90us, decode 40.46us (2.4694ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.5010, decode 48.37us (2.9523ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy raw-bits, ratio 1.0007, encode 951.07us, decode 87.34us (7.1079ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 1.0013, decode 56.91us (4.6310ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy raw-bits, ratio 1.0022, encode 118.96us, decode 18.59us (4.5392ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 1.0039, decode 19.13us (4.6715ns/value), exact_bits=true; no-compress bypass selected |
