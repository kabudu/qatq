# Benchmark Gate

- status: `pass`
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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 515.46us, decode 482.60us (1.9637ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 468.36us (1.9058ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 496.94us, decode 480.29us (1.9543ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 472.63us (1.9231ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1035.52us, decode 980.38us (1.9302ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 967.73us (1.9053ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1035.71us, decode 975.65us (1.9209ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 969.56us (1.9089ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1035.51us, decode 973.43us (1.9166ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 961.27us (1.8926ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1037.58us, decode 975.33us (1.9203ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 987.12us (1.9435ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1602.64us, decode 1532.29us (1.9484ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1561.64us (1.9857ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1619.66us, decode 1522.98us (1.9366ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1679.37us (2.1354ns/value), exact_bits=true |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1737.30us, decode 204.43us (4.1590ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 209.44us (4.2612ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1687.24us, decode 208.23us (4.2364ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 211.75us (4.3080ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1743.13us, decode 212.67us (4.3268ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 208.59us (4.2439ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1721.47us, decode 205.45us (4.1798ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 212.34us (4.3200ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1719.76us, decode 207.67us (4.2250ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 208.23us (4.2364ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1722.64us, decode 207.55us (4.2226ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 206.78us (4.2069ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7232.41us, decode 820.61us (4.1738ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 846.46us (4.3053ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7052.34us, decode 822.97us (4.1859ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 855.05us (4.3490ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7361.65us, decode 837.48us (4.2596ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 882.49us (4.4886ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7134.92us, decode 831.39us (4.2287ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 841.91us (4.2822ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7161.95us, decode 822.66us (4.1842ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 861.18us (4.3802ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7166.29us, decode 815.63us (4.1485ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 844.73us (4.2965ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14861.90us, decode 1644.08us (4.1811ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1707.97us (4.3436ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14527.53us, decode 1656.33us (4.2123ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1727.36us (4.3929ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14696.15us, decode 1651.61us (4.2003ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1724.79us (4.3864ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14411.84us, decode 1653.87us (4.2060ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1712.99us (4.3564ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14856.99us, decode 1655.48us (4.2101ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1677.97us (4.2673ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14262.86us, decode 1636.08us (4.1608ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1699.28us (4.3215ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 406.08us, decode 384.68us (1.9566ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 383.82us (1.9522ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 407.98us, decode 383.62us (1.9512ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 381.99us (1.9429ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 405.61us, decode 379.41us (1.9298ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 381.91us (1.9425ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 406.30us, decode 388.78us (1.9774ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 385.63us (1.9614ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 408.09us, decode 385.41us (1.9603ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 384.34us (1.9548ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 409.77us, decode 386.49us (1.9658ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 384.51us (1.9557ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1605.80us, decode 1540.70us (1.9591ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1528.32us (1.9434ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1602.53us, decode 1533.92us (1.9505ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1526.33us (1.9408ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1604.39us, decode 1527.60us (1.9424ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1521.54us (1.9347ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1604.71us, decode 1525.91us (1.9403ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1515.54us (1.9271ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1599.89us, decode 1522.00us (1.9353ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1521.04us (1.9341ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1604.69us, decode 1519.30us (1.9319ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1509.56us (1.9195ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3198.15us, decode 3036.97us (1.9309ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3009.42us (1.9133ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3204.01us, decode 3031.57us (1.9274ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3009.15us (1.9132ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3202.80us, decode 3018.33us (1.9190ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3004.81us (1.9104ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3200.75us, decode 3015.27us (1.9171ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3018.70us (1.9192ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3207.37us, decode 3021.17us (1.9208ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2987.07us (1.8991ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3211.45us, decode 3019.25us (1.9196ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3009.40us (1.9133ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 65.97us, decode 62.74us (1.9146ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.02us (1.9233ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.03us, decode 62.54us (1.9085ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.31us (1.9321ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.13us, decode 62.15us (1.8967ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.30us (1.9319ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.91us, decode 62.13us (1.8962ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.33us (1.9021ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.05us, decode 62.40us (1.9043ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.98us (1.9221ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.00us, decode 62.42us (1.9048ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.77us (1.9156ns/value), exact_bits=true |
