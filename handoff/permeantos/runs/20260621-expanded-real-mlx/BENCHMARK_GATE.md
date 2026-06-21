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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1545.64us, decode 490.52us (1.9959ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 478.23us (1.9459ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, ratio 0.5000, encode 1437.21us, decode 476.30us (1.9381ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 483.76us (1.9684ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2962.69us, decode 999.80us (1.9685ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 995.47us (1.9600ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2964.73us, decode 996.20us (1.9614ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 1031.89us (2.0317ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | fail | values 507904, ratio 0.5000, encode 2956.59us, decode 1009.18us (1.9870ns/value), exact_bits=true; decode 1009.18us > 1000.00us |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 993.23us (1.9556ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, ratio 0.5000, encode 2911.47us, decode 999.09us (1.9671ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 993.07us (1.9552ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4779.73us, decode 1559.77us (1.9834ns/value), exact_bits=true; decode 1559.77us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1560.22us (1.9839ns/value), exact_bits=true; decode 1560.22us > 1200.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4645.29us, decode 1542.11us (1.9609ns/value), exact_bits=true; decode 1542.11us > 1000.00us |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1551.77us (1.9732ns/value), exact_bits=true; decode 1551.77us > 1200.00us |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1792.90us, decode 212.45us (4.3222ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 212.21us (4.3174ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1808.95us, decode 208.78us (4.2477ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 210.79us (4.2885ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1813.07us, decode 208.55us (4.2430ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 212.82us (4.3299ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1814.45us, decode 203.97us (4.1499ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 220.01us (4.4761ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1838.45us, decode 212.79us (4.3292ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 213.44us (4.3425ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | fail | values 49152, ratio 1.0002, encode 1809.43us, decode 208.64us (4.2447ns/value), exact_bits=true; ratio 1.0002 > 0.9500 |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | fail | values 49152, ratio 1.0003, decode 211.84us (4.3099ns/value), exact_bits=true; ratio 1.0003 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7741.71us, decode 831.61us (4.2298ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7741.71us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 857.01us (4.3590ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7499.68us, decode 840.16us (4.2733ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7499.68us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 848.23us (4.3143ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7652.89us, decode 840.10us (4.2730ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7652.89us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 861.94us (4.3840ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7692.49us, decode 852.77us (4.3374ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7692.49us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 862.92us (4.3890ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7661.20us, decode 839.79us (4.2714ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7661.20us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 880.35us (4.4777ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | fail | values 196608, ratio 1.0000, encode 7939.37us, decode 915.67us (4.6573ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 7939.37us > 5000.00us |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | fail | values 196608, ratio 1.0002, decode 865.54us (4.4024ns/value), exact_bits=true; ratio 1.0002 > 0.9600 |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15787.30us, decode 1749.90us (4.4502ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15787.30us > 5000.00us; decode 1749.90us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1737.59us (4.4189ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1737.59us > 1200.00us |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15473.57us, decode 1687.69us (4.2920ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15473.57us > 5000.00us; decode 1687.69us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1726.92us (4.3918ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1726.92us > 1200.00us |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15679.18us, decode 1659.25us (4.2197ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15679.18us > 5000.00us; decode 1659.25us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1690.49us (4.2991ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1690.49us > 1200.00us |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15405.58us, decode 1646.65us (4.1877ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15405.58us > 5000.00us; decode 1646.65us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1693.23us (4.3061ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1693.23us > 1200.00us |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15731.59us, decode 1632.50us (4.1517ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15731.59us > 5000.00us; decode 1632.50us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1733.88us (4.4095ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1733.88us > 1200.00us |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | fail | values 393216, ratio 1.0000, encode 15270.33us, decode 1676.66us (4.2640ns/value), exact_bits=true; ratio 1.0000 > 0.9500; encode 15270.33us > 5000.00us; decode 1676.66us > 1000.00us |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | fail | values 393216, ratio 1.0002, decode 1720.32us (4.3750ns/value), exact_bits=true; ratio 1.0002 > 0.9600; decode 1720.32us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1170.07us, decode 389.33us (1.9802ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 389.66us (1.9819ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1173.40us, decode 381.41us (1.9400ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 386.83us (1.9675ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1165.39us, decode 383.02us (1.9481ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 385.61us (1.9613ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1154.30us, decode 402.77us (2.0486ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 386.94us (1.9681ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1163.88us, decode 383.67us (1.9514ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 384.15us (1.9539ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, ratio 0.5001, encode 1142.96us, decode 383.98us (1.9530ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 386.01us (1.9633ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4569.79us, decode 1540.76us (1.9592ns/value), exact_bits=true; decode 1540.76us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1536.00us (1.9531ns/value), exact_bits=true; decode 1536.00us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4738.68us, decode 1548.53us (1.9691ns/value), exact_bits=true; decode 1548.53us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1539.92us (1.9581ns/value), exact_bits=true; decode 1539.92us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4574.49us, decode 1543.14us (1.9622ns/value), exact_bits=true; decode 1543.14us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1546.97us (1.9671ns/value), exact_bits=true; decode 1546.97us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4558.87us, decode 1537.78us (1.9554ns/value), exact_bits=true; decode 1537.78us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1532.95us (1.9493ns/value), exact_bits=true; decode 1532.95us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4549.73us, decode 1518.30us (1.9306ns/value), exact_bits=true; decode 1518.30us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1525.49us (1.9398ns/value), exact_bits=true; decode 1525.49us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | fail | values 786432, ratio 0.5000, encode 4564.13us, decode 1546.48us (1.9665ns/value), exact_bits=true; decode 1546.48us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | fail | values 786432, ratio 0.5002, decode 1545.73us (1.9655ns/value), exact_bits=true; decode 1545.73us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9239.91us, decode 3110.11us (1.9774ns/value), exact_bits=true; encode 9239.91us > 5000.00us; decode 3110.11us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3103.15us (1.9729ns/value), exact_bits=true; decode 3103.15us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9289.66us, decode 3120.50us (1.9840ns/value), exact_bits=true; encode 9289.66us > 5000.00us; decode 3120.50us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3110.39us (1.9775ns/value), exact_bits=true; decode 3110.39us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9301.45us, decode 3104.70us (1.9739ns/value), exact_bits=true; encode 9301.45us > 5000.00us; decode 3104.70us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3090.75us (1.9650ns/value), exact_bits=true; decode 3090.75us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9205.02us, decode 3089.09us (1.9640ns/value), exact_bits=true; encode 9205.02us > 5000.00us; decode 3089.09us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3083.03us (1.9601ns/value), exact_bits=true; decode 3083.03us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9171.41us, decode 3095.06us (1.9678ns/value), exact_bits=true; encode 9171.41us > 5000.00us; decode 3095.06us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3088.97us (1.9639ns/value), exact_bits=true; decode 3088.97us > 1200.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | fail | values 1572864, ratio 0.5000, encode 9187.08us, decode 3091.78us (1.9657ns/value), exact_bits=true; encode 9187.08us > 5000.00us; decode 3091.78us > 1000.00us |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | fail | values 1572864, ratio 0.5002, decode 3070.83us (1.9524ns/value), exact_bits=true; decode 3070.83us > 1200.00us |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 185.15us, decode 63.02us (1.9233ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.35us (1.9639ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 183.49us, decode 61.97us (1.8913ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.65us (1.9729ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 184.43us, decode 61.57us (1.8790ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 63.97us (1.9521ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 183.40us, decode 62.53us (1.9083ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.55us (1.9699ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, ratio 0.5003, encode 181.71us, decode 62.47us (1.9065ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.91us (1.9810ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, ratio 0.5003, encode 184.49us, decode 62.26us (1.9000ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 64.22us (1.9597ns/value), exact_bits=true |
