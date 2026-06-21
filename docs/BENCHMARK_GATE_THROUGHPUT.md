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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-blocks, ratio 0.5012, encode 18.23us, decode 16.98us (2.0732ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.5021, decode 17.33us (2.1152ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy byte-plane-blocks, ratio 0.5006, encode 35.63us, decode 35.35us (2.1576ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.5010, decode 35.58us (2.1717ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy raw-bits, ratio 1.0007, encode 456.18us, decode 61.88us (5.0362ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 1.0013, decode 56.71us (4.6148ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy raw-bits, ratio 1.0022, encode 114.19us, decode 18.27us (4.4613ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 1.0039, decode 18.86us (4.6052ns/value), exact_bits=true; no-compress bypass selected |
