# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 30 | 1024 | Exact | 0.941321 | 10.099824 |
| cache_v_l0:all-heads | Value | 0 | all | 30 | 1024 | Exact | 1.000000 | 14.557767 |
| cache_k_l16:all-heads | Key | 16 | all | 30 | 1024 | Exact | 0.924334 | 10.213326 |
| cache_v_l16:all-heads | Value | 16 | all | 30 | 1024 | Exact | 0.748062 | 16.377092 |
| cache_k_l31:all-heads | Key | 31 | all | 30 | 1024 | Exact | 0.922367 | 9.801435 |
| cache_v_l31:all-heads | Value | 31 | all | 30 | 1024 | Exact | 0.993080 | 2.272734 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
