# Fixture Audit

- manifest: `fixtures/public.manifest`
- fixtures: `4`
- total values: `40960`
- total bytes: `163840`

| group | name | values | bytes | fingerprint fnv1a64 | shape | notes | path |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| qatq-public | bf16-kv-ramp-64x8x16 | 8192 | 32768 | 6f42ae7c6e314989 | [tokens=64, heads=8, dim=16] | generated public bfloat16-like KV ramp; low f32 mantissa bytes zeroed; values=8192 | fixtures/generated/bf16-kv-ramp-64x8x16.f32le |
| qatq-public | bf16-kv-wave-128x8x16 | 16384 | 65536 | 260cf16d8eb205f1 | [tokens=128, heads=8, dim=16] | generated public bfloat16-like KV wave; compression-positive phase2 fixture; values=16384 | fixtures/generated/bf16-kv-wave-128x8x16.f32le |
| qatq-public | f32-noisy-pass-through-64x12x16 | 12288 | 49152 | 216f15e2b9eb431e | [tokens=64, heads=12, dim=16] | generated public float32 noisy fixture; expected no-compress pass-through candidate; values=12288 | fixtures/generated/f32-noisy-pass-through-64x12x16.f32le |
| qatq-public | stress-signed-zero-nan-inf | 4096 | 16384 | 8788d5a828e81325 | [values=4096] | generated public exactness stress fixture with signed zero, infinities, and NaN payload bits; values=4096 | fixtures/generated/stress-signed-zero-nan-inf.f32le |
