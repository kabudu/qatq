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
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless | pass | values 8192, strategy byte-plane-blocks, ratio 0.5012, encode 19.26us, decode 17.15us (2.0934ns/value), exact_bits=true |
| qatq-public / bf16-kv-ramp-64x8x16 | phase2-lossless-container | pass | values 8192, ratio 0.5021, decode 17.54us (2.1410ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless | pass | values 16384, strategy byte-plane-blocks, ratio 0.5006, encode 35.34us, decode 33.45us (2.0414ns/value), exact_bits=true |
| qatq-public / bf16-kv-wave-128x8x16 | phase2-lossless-container | pass | values 16384, ratio 0.5010, decode 34.13us (2.0832ns/value), exact_bits=true |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless | pass | values 12288, strategy raw-bits, ratio 1.0007, encode 423.95us, decode 55.47us (4.5140ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / f32-noisy-pass-through-64x12x16 | phase2-lossless-container | pass | values 12288, ratio 1.0013, decode 54.40us (4.4272ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless | pass | values 4096, strategy raw-bits, ratio 1.0022, encode 108.43us, decode 17.23us (4.2058ns/value), exact_bits=true; no-compress bypass selected |
| qatq-public / stress-signed-zero-nan-inf | phase2-lossless-container | pass | values 4096, ratio 1.0039, decode 17.86us (4.3614ns/value), exact_bits=true; no-compress bypass selected |
