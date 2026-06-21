# Benchmark Gate

- status: `fail`
- evaluated fixtures: `16`
- external fixtures: `8`
- scope: `external`
- max phase2 ratio: `0.9500`
- max phase2 encode us: `5000.0000`
- max phase2 decode us: `1000.0000`

- max phase2 decode ns/value: `unset`

- max phase2 container ratio: `0.9600`
- max phase2 container decode us: `1200.0000`

- max phase2 container decode ns/value: `unset`

| dataset | check | status | details |
| --- | --- | --- | --- |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1375.96us, decode 448.77us (1.8260ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 461.76us (1.8789ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1364.92us, decode 448.52us (1.8250ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.31us (1.8811ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2853.52us, decode 930.53us (1.8321ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 954.98us (1.8802ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2857.21us, decode 934.16us (1.8393ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.36us (1.8810ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2858.35us, decode 933.58us (1.8381ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.10us (1.8805ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2862.48us, decode 936.88us (1.8446ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.03us (1.8803ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4451.93us, decode 1454.16us (1.8491ns/value), exact_bits=true; decode 1454.16us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1478.52us (1.8800ns/value), exact_bits=true; decode 1478.52us > 1200.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4401.93us, decode 1471.51us (1.8711ns/value), exact_bits=true; decode 1471.51us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1479.24us (1.8809ns/value), exact_bits=true; decode 1479.24us > 1200.00us |
