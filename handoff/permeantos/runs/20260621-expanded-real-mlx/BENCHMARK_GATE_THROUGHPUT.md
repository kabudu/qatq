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
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 461.27us, decode 446.94us (1.8186ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer0-key | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 461.93us (1.8796ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless | pass | values 245760, strategy byte-plane-blocks, ratio 0.5000, encode 457.47us, decode 447.38us (1.8204ns/value), exact_bits=true |
| permeantos-kv / qwen25-05b-long-1920-layer23-value | phase2-lossless-container | pass | values 245760, ratio 0.5002, decode 462.06us (1.8801ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 954.99us, decode 923.94us (1.8191ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.90us (1.8820ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 961.24us, decode 924.90us (1.8210ns/value), exact_bits=true |
| permeantos-kv / tinyllama-11b-1984-layer21-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.97us (1.8822ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 971.57us, decode 924.77us (1.8208ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer0-key | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.83us (1.8819ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless | pass | values 507904, strategy byte-plane-blocks, ratio 0.5000, encode 960.41us, decode 924.98us (1.8212ns/value), exact_bits=true |
| permeantos-kv / qwen25-15b-1984-layer27-value | phase2-lossless-container | pass | values 507904, ratio 0.5002, decode 955.71us (1.8817ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1503.66us, decode 1433.90us (1.8233ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1482.35us (1.8849ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1504.04us, decode 1433.62us (1.8229ns/value), exact_bits=true |
| permeantos-kv / phi35-mini-256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1481.08us (1.8833ns/value), exact_bits=true |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1673.34us, decode 194.62us (3.9595ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.69us (4.0220ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1671.16us, decode 194.61us (3.9594ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer0-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.99us (4.0281ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1678.82us, decode 194.65us (3.9601ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.50us (4.0182ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1672.03us, decode 194.57us (3.9585ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer6-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.83us (4.0249ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1711.59us, decode 194.61us (3.9593ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-key | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.37us (4.0156ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless | pass | values 49152, strategy raw-bits, ratio 1.0002, encode 1703.40us, decode 195.94us (3.9865ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq64-layer11-value | phase2-lossless-container | pass | values 49152, ratio 1.0003, decode 197.05us (4.0089ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7114.37us, decode 781.27us (3.9737ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 792.47us (4.0307ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 6999.54us, decode 782.59us (3.9805ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer0-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 793.15us (4.0342ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7081.75us, decode 781.96us (3.9773ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 792.54us (4.0311ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7042.71us, decode 782.61us (3.9806ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer6-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 798.11us (4.0594ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7165.32us, decode 782.65us (3.9808ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-key | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 797.53us (4.0564ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless | pass | values 196608, strategy raw-bits, ratio 1.0000, encode 7128.27us, decode 782.65us (3.9807ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq256-layer11-value | phase2-lossless-container | pass | values 196608, ratio 1.0002, decode 800.07us (4.0694ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14826.36us, decode 1609.55us (4.0933ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1609.05us (4.0920ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14429.07us, decode 1610.03us (4.0945ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer0-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1620.74us (4.1218ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14642.86us, decode 1609.97us (4.0944ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1624.08us (4.1303ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14387.52us, decode 1610.32us (4.0953ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer6-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1631.19us (4.1483ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14840.83us, decode 1611.56us (4.0984ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-key | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1638.61us (4.1672ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless | pass | values 393216, strategy raw-bits, ratio 1.0000, encode 14241.04us, decode 1610.89us (4.0967ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / gpt2-seq512-layer11-value | phase2-lossless-container | pass | values 393216, ratio 1.0002, decode 1632.02us (4.1504ns/value), exact_bits=true; no-compress bypass selected |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 398.42us, decode 368.60us (1.8748ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 372.65us (1.8954ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 398.98us, decode 367.70us (1.8702ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 371.31us (1.8886ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 399.26us, decode 374.60us (1.9053ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 372.03us (1.8922ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 395.64us, decode 374.11us (1.9028ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer16-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 370.45us (1.8842ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 397.81us, decode 378.19us (1.9236ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-key | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 371.96us (1.8919ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless | pass | values 196608, strategy byte-plane-blocks, ratio 0.5001, encode 396.19us, decode 377.21us (1.9186ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq64-layer31-value | phase2-lossless-container | pass | values 196608, ratio 0.5002, decode 377.58us (1.9205ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1580.25us, decode 1495.53us (1.9017ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1485.12us (1.8884ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1582.81us, decode 1493.39us (1.8989ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer0-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1494.16us (1.8999ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1587.24us, decode 1485.54us (1.8890ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1484.22us (1.8873ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1587.64us, decode 1492.48us (1.8978ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer16-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1483.63us (1.8865ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1584.78us, decode 1490.87us (1.8957ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-key | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1484.12us (1.8872ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless | pass | values 786432, strategy byte-plane-blocks, ratio 0.5000, encode 1574.29us, decode 1487.72us (1.8917ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq256-layer31-value | phase2-lossless-container | pass | values 786432, ratio 0.5002, decode 1485.52us (1.8889ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3169.82us, decode 2966.93us (1.8863ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2970.25us (1.8884ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3167.83us, decode 2977.68us (1.8932ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer0-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2978.44us (1.8936ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3165.05us, decode 2980.83us (1.8952ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2971.83us (1.8894ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3154.68us, decode 2982.23us (1.8960ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer16-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2968.78us (1.8875ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3161.83us, decode 2973.10us (1.8902ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-key | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2975.41us (1.8917ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless | pass | values 1572864, strategy byte-plane-blocks, ratio 0.5000, encode 3160.87us, decode 2974.96us (1.8914ns/value), exact_bits=true |
| permeantos-kv / microsoft-phi-3-5-mini-instruct-seq512-layer31-value | phase2-lossless-container | pass | values 1572864, ratio 0.5002, decode 2973.33us (1.8904ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.61us, decode 61.44us (1.8750ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.03us (1.8930ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.90us, decode 59.54us (1.8171ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer0-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.23us (1.8991ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.39us, decode 61.26us (1.8694ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 61.62us (1.8804ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.19us, decode 61.19us (1.8672ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer14-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 61.52us (1.8776ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.27us, decode 61.63us (1.8809ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-key | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 62.60us (1.9104ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless | pass | values 32768, strategy byte-plane-blocks, ratio 0.5003, encode 66.62us, decode 61.88us (1.8884ns/value), exact_bits=true |
| permeantos-kv / qwen-qwen2-5-7b-instruct-seq64-layer27-value | phase2-lossless-container | pass | values 32768, ratio 0.5005, decode 61.97us (1.8910ns/value), exact_bits=true |
