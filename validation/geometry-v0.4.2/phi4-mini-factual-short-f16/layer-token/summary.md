# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 7 | 1024 | Exact | 0.938087 | 2.476284 |
| cache_v_l0:all-heads | Value | 0 | all | 7 | 1024 | Exact | 0.758178 | 4.675858 |
| cache_k_l16:all-heads | Key | 16 | all | 7 | 1024 | Exact | 0.887711 | 2.276788 |
| cache_v_l16:all-heads | Value | 16 | all | 7 | 1024 | Exact | 0.625162 | 4.404874 |
| cache_k_l31:all-heads | Key | 31 | all | 7 | 1024 | Exact | 0.910087 | 2.562790 |
| cache_v_l31:all-heads | Value | 31 | all | 7 | 1024 | Exact | 0.981939 | 1.252057 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
