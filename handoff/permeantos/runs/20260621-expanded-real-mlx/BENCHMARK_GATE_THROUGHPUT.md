# Benchmark Gate

- status: `fail`
- evaluated fixtures: `100`
- external fixtures: `50`
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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 1414.90us, decode 482.04us (1.9614ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 482.53us (1.9634ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 1402.36us, decode 478.49us (1.9470ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 479.24us (1.9500ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 2900.15us, decode 1005.16us (1.9790ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 993.88us (1.9568ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 2958.14us, decode 1003.53us (1.9758ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 1003.05us (1.9749ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 2902.04us, decode 1002.12us (1.9730ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 998.13us (1.9652ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 2859.35us, decode 993.07us (1.9552ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 993.14us (1.9554ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4477.35us, decode 1550.51us (1.9716ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1544.73us (1.9642ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4505.94us, decode 1549.64us (1.9705ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1543.20us (1.9623ns/value), exact_bits=true |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1788.25us, decode 209.22us (4.2566ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 208.13us (4.2344ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1763.80us, decode 206.43us (4.1998ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 205.90us (4.1889ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1760.56us, decode 207.67us (4.2250ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 210.00us (4.2725ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1788.07us, decode 205.54us (4.1818ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 207.79us (4.2274ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1772.45us, decode 206.83us (4.2079ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 212.93us (4.3322ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1785.78us, decode 207.53us (4.2222ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 210.75us (4.2877ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7531.88us, decode 817.78us (4.1594ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 845.73us (4.3016ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7374.88us, decode 819.83us (4.1699ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 847.05us (4.3083ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7434.88us, decode 821.28us (4.1772ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 853.80us (4.3427ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7437.80us, decode 822.87us (4.1853ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 851.97us (4.3334ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7516.04us, decode 822.85us (4.1852ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 848.74us (4.3169ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7441.41us, decode 825.17us (4.1970ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 860.88us (4.3786ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 15520.62us, decode 1656.47us (4.2126ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1711.51us (4.3526ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 15138.38us, decode 1653.01us (4.2038ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1708.18us (4.3441ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 15317.65us, decode 1652.92us (4.2036ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1711.70us (4.3531ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 15106.57us, decode 1648.98us (4.1936ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1714.78us (4.3609ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 15508.47us, decode 1651.16us (4.1991ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1712.06us (4.3540ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14909.72us, decode 1648.64us (4.1927ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1710.23us (4.3493ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1124.95us, decode 386.14us (1.9640ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 388.36us (1.9753ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1122.14us, decode 387.13us (1.9690ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 389.71us (1.9822ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1125.75us, decode 383.58us (1.9510ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 387.57us (1.9713ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1121.90us, decode 384.27us (1.9545ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 387.61us (1.9715ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1124.75us, decode 385.67us (1.9616ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 385.38us (1.9602ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 1126.96us, decode 391.48us (1.9911ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 385.92us (1.9629ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4511.35us, decode 1545.30us (1.9650ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1549.71us (1.9706ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4514.35us, decode 1550.57us (1.9716ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1546.35us (1.9663ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4474.73us, decode 1540.15us (1.9584ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1534.03us (1.9506ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4507.56us, decode 1537.49us (1.9550ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1538.24us (1.9560ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4502.28us, decode 1543.99us (1.9633ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1534.40us (1.9511ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 4471.77us, decode 1539.96us (1.9582ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1537.11us (1.9545ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 8986.86us, decode 3095.89us (1.9683ns/value), exact_bits=true; encode 8986.86us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3063.03us (1.9474ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 9035.42us, decode 3084.68us (1.9612ns/value), exact_bits=true; encode 9035.42us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3079.78us (1.9581ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 8935.33us, decode 3087.91us (1.9632ns/value), exact_bits=true; encode 8935.33us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3058.29us (1.9444ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 9069.94us, decode 3084.07us (1.9608ns/value), exact_bits=true; encode 9069.94us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3066.00us (1.9493ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 9035.07us, decode 3076.22us (1.9558ns/value), exact_bits=true; encode 9035.07us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3060.99us (1.9461ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 8999.09us, decode 3082.94us (1.9601ns/value), exact_bits=true; encode 8999.09us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3044.07us (1.9354ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 185.04us, decode 62.12us (1.8957ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.29us (1.9620ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 184.13us, decode 62.17us (1.8972ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.08us (1.9555ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 181.86us, decode 60.55us (1.8477ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.03us (1.9540ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 184.07us, decode 62.15us (1.8968ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.83us (1.9478ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 182.29us, decode 62.55us (1.9088ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.45us (1.9667ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 180.95us, decode 62.39us (1.9039ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.77us (1.9463ns/value), exact_bits=true |
