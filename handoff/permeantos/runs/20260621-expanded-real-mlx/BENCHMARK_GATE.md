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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 490.64us, decode 456.17us (1.8562ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.39us (1.8815ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 488.17us, decode 459.89us (1.8713ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.90us (1.8836ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1010.28us, decode 954.50us (1.8793ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 956.31us (1.8829ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1012.04us, decode 954.78us (1.8798ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 956.82us (1.8839ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1010.44us, decode 954.89us (1.8801ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.96us (1.8822ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 1013.27us, decode 951.21us (1.8728ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.47us (1.8812ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1569.85us, decode 1480.84us (1.8830ns/value), exact_bits=true; decode 1480.84us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1480.40us (1.8824ns/value), exact_bits=true; decode 1480.40us > 1200.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1576.81us, decode 1481.41us (1.8837ns/value), exact_bits=true; decode 1481.41us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1483.20us (1.8860ns/value), exact_bits=true; decode 1483.20us > 1200.00us |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1704.71us, decode 195.44us (3.9762ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.34us (4.0150ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1713.40us, decode 196.28us (3.9934ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.92us (4.0267ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1773.53us, decode 195.18us (3.9709ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 199.66us (4.0621ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1706.53us, decode 194.71us (3.9613ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 199.65us (4.0620ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1734.03us, decode 200.41us (4.0773ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 200.52us (4.0796ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1708.86us, decode 196.70us (4.0019ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 199.84us (4.0658ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7221.37us, decode 790.24us (4.0194ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 815.57us (4.1482ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7015.52us, decode 786.01us (3.9979ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 816.05us (4.1506ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7124.53us, decode 793.45us (4.0357ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 818.90us (4.1651ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7085.66us, decode 793.66us (4.0368ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 816.41us (4.1525ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7194.17us, decode 799.86us (4.0683ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 823.60us (4.1891ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7137.43us, decode 795.93us (4.0483ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 821.21us (4.1769ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14878.17us, decode 1619.56us (4.1188ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1656.82us (4.2135ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14439.82us, decode 1615.82us (4.1092ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1660.12us (4.2219ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14663.93us, decode 1621.88us (4.1246ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1661.29us (4.2249ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14417.17us, decode 1624.05us (4.1302ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1666.16us (4.2373ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14842.54us, decode 1619.11us (4.1176ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1670.68us (4.2488ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14264.27us, decode 1620.37us (4.1208ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1675.18us (4.2602ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 406.68us, decode 377.41us (1.9196ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 376.69us (1.9159ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 404.13us, decode 376.58us (1.9154ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 378.63us (1.9258ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 403.15us, decode 378.78us (1.9266ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 380.35us (1.9346ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 401.88us, decode 381.81us (1.9420ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 375.30us (1.9089ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 403.86us, decode 380.38us (1.9347ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 379.04us (1.9279ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 406.84us, decode 374.27us (1.9036ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 377.98us (1.9225ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1604.03us, decode 1517.23us (1.9293ns/value), exact_bits=true; decode 1517.23us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1509.40us (1.9193ns/value), exact_bits=true; decode 1509.40us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1601.30us, decode 1517.74us (1.9299ns/value), exact_bits=true; decode 1517.74us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1496.06us (1.9023ns/value), exact_bits=true; decode 1496.06us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1605.73us, decode 1510.74us (1.9210ns/value), exact_bits=true; decode 1510.74us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1503.88us (1.9123ns/value), exact_bits=true; decode 1503.88us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1599.72us, decode 1510.68us (1.9209ns/value), exact_bits=true; decode 1510.68us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1495.83us (1.9020ns/value), exact_bits=true; decode 1495.83us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1603.18us, decode 1509.99us (1.9200ns/value), exact_bits=true; decode 1509.99us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1515.03us (1.9265ns/value), exact_bits=true; decode 1515.03us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | fail | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1599.25us, decode 1503.22us (1.9114ns/value), exact_bits=true; decode 1503.22us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1497.09us (1.9036ns/value), exact_bits=true; decode 1497.09us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3190.53us, decode 3006.91us (1.9117ns/value), exact_bits=true; decode 3006.91us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 2992.36us (1.9025ns/value), exact_bits=true; decode 2992.36us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3196.89us, decode 3005.33us (1.9107ns/value), exact_bits=true; decode 3005.33us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 2987.97us (1.8997ns/value), exact_bits=true; decode 2987.97us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3187.16us, decode 3008.96us (1.9130ns/value), exact_bits=true; decode 3008.96us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3000.96us (1.9080ns/value), exact_bits=true; decode 3000.96us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3194.06us, decode 3003.17us (1.9094ns/value), exact_bits=true; decode 3003.17us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 2999.17us (1.9068ns/value), exact_bits=true; decode 2999.17us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3199.34us, decode 3009.49us (1.9134ns/value), exact_bits=true; decode 3009.49us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 2974.36us (1.8910ns/value), exact_bits=true; decode 2974.36us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | fail | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3209.47us, decode 3006.58us (1.9115ns/value), exact_bits=true; decode 3006.58us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 2981.59us (1.8956ns/value), exact_bits=true; decode 2981.59us > 1200.00us |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.78us, decode 62.30us (1.9011ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.67us (1.9431ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 65.88us, decode 61.93us (1.8900ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.72us (1.9140ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.65us, decode 62.32us (1.9019ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.27us (1.9307ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.39us, decode 62.17us (1.8973ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 61.98us (1.8915ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 67.37us, decode 61.89us (1.8888ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.05us (1.9241ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 65.70us, decode 62.51us (1.9076ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.85us (1.9179ns/value), exact_bits=true |
