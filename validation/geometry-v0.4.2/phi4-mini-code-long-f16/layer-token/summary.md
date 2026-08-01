# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 67 | 1024 | Exact | 0.939497 | 16.568104 |
| cache_v_l0:all-heads | Value | 0 | all | 67 | 1024 | Exact | 1.000000 | 16.192588 |
| cache_k_l16:all-heads | Key | 16 | all | 67 | 1024 | Exact | 0.913916 | 18.278091 |
| cache_v_l16:all-heads | Value | 16 | all | 67 | 1024 | Exact | 0.777341 | 26.892202 |
| cache_k_l31:all-heads | Key | 31 | all | 67 | 1024 | Exact | 0.924556 | 15.058630 |
| cache_v_l31:all-heads | Value | 31 | all | 67 | 1024 | Exact | 0.998506 | 1.838522 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
