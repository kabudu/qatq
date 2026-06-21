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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1418.65us, decode 466.09us (1.8965ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 468.13us (1.9048ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1388.20us, decode 469.51us (1.9105ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 469.37us (1.9099ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2929.56us, decode 982.76us (1.9349ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 972.06us (1.9139ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2912.67us, decode 977.89us (1.9253ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 969.42us (1.9087ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2919.72us, decode 983.69us (1.9368ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 967.42us (1.9047ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2926.95us, decode 979.50us (1.9285ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 969.25us (1.9083ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4523.91us, decode 1510.73us (1.9210ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1523.70us (1.9375ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4548.56us, decode 1524.45us (1.9384ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1521.94us (1.9352ns/value), exact_bits=true |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1779.80us, decode 203.58us (4.1418ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.1418ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 208.28us (4.2374ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2374ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1779.35us, decode 203.23us (4.1347ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.1347ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 208.39us (4.2397ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2397ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1796.32us, decode 204.21us (4.1546ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.1546ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 206.69us (4.2050ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2050ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1787.10us, decode 201.41us (4.0976ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.0976ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 207.50us (4.2216ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2216ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1772.17us, decode 204.62us (4.1630ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.1630ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 208.07us (4.2332ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2332ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1785.59us, decode 206.38us (4.1988ns/value), exact_bits=true; ratio 1.0002 > 0.9500; decode 4.1988ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 207.34us (4.2183ns/value), exact_bits=true; ratio 1.0003 > 0.9600; decode 4.2183ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7603.84us, decode 807.40us (4.1067ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7603.84us > 5000.00us; decode 4.1067ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 838.92us (4.2669ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.2669ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7460.87us, decode 813.40us (4.1371ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7460.87us > 5000.00us; decode 4.1371ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 841.08us (4.2780ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.2780ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7569.24us, decode 817.01us (4.1555ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7569.24us > 5000.00us; decode 4.1555ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 833.48us (4.2393ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.2393ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7481.72us, decode 814.56us (4.1431ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7481.72us > 5000.00us; decode 4.1431ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 842.58us (4.2856ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.2856ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7651.56us, decode 866.56us (4.4075ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7651.56us > 5000.00us; decode 4.4075ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 844.89us (4.2973ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.2973ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7540.01us, decode 828.52us (4.2140ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7540.01us > 5000.00us; decode 4.2140ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 848.75us (4.3170ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3170ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15617.24us, decode 1636.56us (4.1620ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15617.24us > 5000.00us; decode 4.1620ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1697.94us (4.3181ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3181ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15251.92us, decode 1640.52us (4.1721ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15251.92us > 5000.00us; decode 4.1721ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1703.75us (4.3329ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3329ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15436.07us, decode 1641.59us (4.1748ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15436.07us > 5000.00us; decode 4.1748ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1702.97us (4.3309ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3309ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15175.46us, decode 1640.04us (4.1708ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15175.46us > 5000.00us; decode 4.1708ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1697.66us (4.3174ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3174ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15658.69us, decode 1662.50us (4.2280ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15658.69us > 5000.00us; decode 4.2280ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1719.38us (4.3726ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3726ns/value > 2.2000ns/value |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15020.33us, decode 1651.28us (4.1994ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15020.33us > 5000.00us; decode 4.1994ns/value > 2.1000ns/value |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1710.15us (4.3491ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 4.3491ns/value > 2.2000ns/value |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1159.66us, decode 380.75us (1.9366ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 388.87us (1.9779ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1164.00us, decode 384.84us (1.9574ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 388.09us (1.9739ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1162.11us, decode 382.02us (1.9431ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 389.19us (1.9795ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1162.84us, decode 384.75us (1.9569ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 385.20us (1.9592ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1113.62us, decode 383.46us (1.9504ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 386.65us (1.9666ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1139.00us, decode 385.19us (1.9592ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 380.15us (1.9335ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4551.97us, decode 1537.48us (1.9550ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1555.32us (1.9777ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4602.07us, decode 1550.09us (1.9710ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1580.56us (2.0098ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4639.42us, decode 1560.02us (1.9837ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1690.59us (2.1497ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4695.21us, decode 1560.83us (1.9847ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1562.38us (1.9867ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4639.35us, decode 1556.99us (1.9798ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1560.75us (1.9846ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | pass | values 786432, ratio 0.5000, encode 4531.05us, decode 1548.90us (1.9695ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1556.03us (1.9786ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9277.59us, decode 3117.48us (1.9820ns/value), exact_bits=true; encode 9277.59us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3109.17us (1.9768ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9328.48us, decode 3094.76us (1.9676ns/value), exact_bits=true; encode 9328.48us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3096.97us (1.9690ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9216.36us, decode 3091.69us (1.9656ns/value), exact_bits=true; encode 9216.36us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3148.50us (2.0018ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9274.93us, decode 3102.09us (1.9723ns/value), exact_bits=true; encode 9274.93us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3093.11us (1.9665ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9533.61us, decode 3132.73us (1.9917ns/value), exact_bits=true; encode 9533.61us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3126.36us (1.9877ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9320.72us, decode 3129.37us (1.9896ns/value), exact_bits=true; encode 9320.72us > 5000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 3121.75us (1.9848ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 186.60us, decode 62.70us (1.9136ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 65.17us (1.9889ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 185.45us, decode 63.89us (1.9497ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.86us (1.9794ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 187.07us, decode 63.48us (1.9372ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.04us (1.9543ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 185.41us, decode 63.31us (1.9320ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 65.08us (1.9861ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 186.22us, decode 63.41us (1.9352ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 65.49us (1.9986ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 187.87us, decode 63.78us (1.9464ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 65.26us (1.9916ns/value), exact_bits=true |
