# Benchmark Gate

- status: `fail`
- evaluated fixtures: `100`
- external fixtures: `50`
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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 456.45us, decode 447.63us (1.8214ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.24us (1.8809ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 456.68us, decode 446.97us (1.8187ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.04us (1.8801ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 965.70us, decode 924.01us (1.8193ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 967.06us (1.9040ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 986.45us, decode 935.33us (1.8416ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 986.45us (1.9422ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 980.47us, decode 926.50us (1.8242ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 959.17us (1.8885ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1002.29us, decode 933.42us (1.8378ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 956.52us (1.8833ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1555.71us, decode 1437.77us (1.8282ns/value), exact_bits=true; decode 1437.77us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1482.07us (1.8845ns/value), exact_bits=true; decode 1482.07us > 1200.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1526.23us, decode 1435.07us (1.8248ns/value), exact_bits=true; decode 1435.07us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1480.76us (1.8829ns/value), exact_bits=true; decode 1480.76us > 1200.00us |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1684.59us, decode 194.56us (3.9584ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.63us (4.0208ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1689.23us, decode 196.32us (3.9942ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.67us (4.0217ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 2051.89us, decode 210.94us (4.2917ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 216.04us (4.3953ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 2226.69us, decode 211.79us (4.3088ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 219.30us (4.4618ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 2300.55us, decode 214.46us (4.3633ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 217.46us (4.4243ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 2288.73us, decode 223.87us (4.5546ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 223.28us (4.5427ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 8998.27us, decode 891.40us (4.5339ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 866.67us (4.4081ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7091.43us, decode 851.06us (4.3287ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 887.60us (4.5146ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7181.16us, decode 838.26us (4.2636ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 862.64us (4.3876ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7387.52us, decode 846.99us (4.3080ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 863.77us (4.3934ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7175.27us, decode 842.51us (4.2852ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 861.33us (4.3809ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7370.68us, decode 859.27us (4.3705ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 863.15us (4.3902ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14949.46us, decode 1662.27us (4.2274ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1732.62us (4.4063ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14474.69us, decode 1681.41us (4.2761ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1775.14us (4.5144ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14760.24us, decode 1682.84us (4.2797ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1729.43us (4.3982ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14421.34us, decode 1684.50us (4.2839ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1744.96us (4.4377ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14901.21us, decode 1714.50us (4.3602ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1730.47us (4.4008ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14316.23us, decode 1684.92us (4.2850ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1733.66us (4.4089ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 419.55us, decode 399.09us (2.0299ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 398.46us (2.0267ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 408.16us, decode 391.53us (1.9914ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 399.81us (2.0335ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 408.11us, decode 389.21us (1.9796ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 393.71us (2.0025ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 408.45us, decode 390.28us (1.9851ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 391.47us (1.9911ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 407.23us, decode 390.25us (1.9849ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 396.57us (2.0171ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 408.03us, decode 387.38us (1.9703ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 400.76us (2.0384ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1612.00us, decode 1565.63us (1.9908ns/value), exact_bits=true; decode 1565.63us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1582.13us (2.0118ns/value), exact_bits=true; decode 1582.13us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1612.86us, decode 1607.29us (2.0438ns/value), exact_bits=true; decode 1607.29us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1596.23us (2.0297ns/value), exact_bits=true; decode 1596.23us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1610.67us, decode 1572.49us (1.9995ns/value), exact_bits=true; decode 1572.49us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1579.70us (2.0087ns/value), exact_bits=true; decode 1579.70us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1616.10us, decode 1562.41us (1.9867ns/value), exact_bits=true; decode 1562.41us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1581.92us (2.0115ns/value), exact_bits=true; decode 1581.92us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1609.47us, decode 1559.12us (1.9825ns/value), exact_bits=true; decode 1559.12us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1578.84us (2.0076ns/value), exact_bits=true; decode 1578.84us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1612.86us, decode 1565.73us (1.9909ns/value), exact_bits=true; decode 1565.73us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1580.69us (2.0100ns/value), exact_bits=true; decode 1580.69us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3237.17us, decode 3125.05us (1.9869ns/value), exact_bits=true; decode 3125.05us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3128.47us (1.9890ns/value), exact_bits=true; decode 3128.47us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3208.48us, decode 3113.10us (1.9793ns/value), exact_bits=true; decode 3113.10us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3108.10us (1.9761ns/value), exact_bits=true; decode 3108.10us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3207.16us, decode 3100.65us (1.9713ns/value), exact_bits=true; decode 3100.65us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3086.90us (1.9626ns/value), exact_bits=true; decode 3086.90us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3213.58us, decode 3099.16us (1.9704ns/value), exact_bits=true; decode 3099.16us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3076.94us (1.9563ns/value), exact_bits=true; decode 3076.94us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3206.49us, decode 3078.72us (1.9574ns/value), exact_bits=true; decode 3078.72us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3059.62us (1.9453ns/value), exact_bits=true; decode 3059.62us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3203.10us, decode 3077.36us (1.9565ns/value), exact_bits=true; decode 3077.36us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3056.10us (1.9430ns/value), exact_bits=true; decode 3056.10us > 1200.00us |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.84us, decode 62.92us (1.9201ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.29us (1.9620ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.83us, decode 61.76us (1.8847ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.43us (1.9664ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 68.46us, decode 63.34us (1.9330ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.43us (1.9664ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.71us, decode 63.21us (1.9291ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.02us (1.9537ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 68.33us, decode 62.98us (1.9220ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.12us (1.9569ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 68.38us, decode 63.23us (1.9296ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.25us (1.9302ns/value), exact_bits=true |
