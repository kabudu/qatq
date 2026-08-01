# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 30 | 128 | Exact | 0.995400 | 3.180176 |
| cache_v_l0:all-heads | Value | 0 | all | 30 | 128 | Exact | 1.000000 | 7.964223 |
| cache_k_l12:all-heads | Key | 12 | all | 30 | 128 | Exact | 0.963923 | 8.157170 |
| cache_v_l12:all-heads | Value | 12 | all | 30 | 128 | Exact | 0.675968 | 16.191591 |
| cache_k_l23:all-heads | Key | 23 | all | 30 | 128 | Exact | 0.983686 | 9.536221 |
| cache_v_l23:all-heads | Value | 23 | all | 30 | 128 | Exact | 0.790488 | 17.886752 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
