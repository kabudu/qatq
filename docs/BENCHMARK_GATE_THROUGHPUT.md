# Benchmark Gate

- status: `pass`
- evaluated fixtures: `16`
- external fixtures: `8`
- scope: `external`
- max phase2 ratio: `0.9500`
- max phase2 encode us: `5000.0000`
- max phase2 decode us: `unset`

- max phase2 decode ns/value: `2.1000`

- max phase2 container ratio: `0.9600`
- max phase2 container decode us: `unset`

- max phase2 container decode ns/value: `2.2000`

| dataset | check | status | details |
| --- | --- | --- | --- |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1381.77us, decode 448.62us (1.8255ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 461.30us (1.8770ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1390.16us, decode 451.83us (1.8385ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.37us (1.8814ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2835.12us, decode 945.75us (1.8621ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.05us (1.8804ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2849.80us, decode 945.54us (1.8616ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 953.16us (1.8767ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2870.18us, decode 942.75us (1.8562ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.28us (1.8808ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2875.87us, decode 950.96us (1.8723ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.25us (1.8808ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4421.24us, decode 1472.44us (1.8723ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1481.16us (1.8834ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4430.03us, decode 1481.00us (1.8832ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1485.03us (1.8883ns/value), exact_bits=true |
