# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 30 | 1024 | Exact | 0.941401 | 10.100009 |
| cache_v_l0:all-heads | Value | 0 | all | 30 | 1024 | Exact | 1.000000 | 14.558973 |
| cache_k_l16:all-heads | Key | 16 | all | 30 | 1024 | Exact | 0.927420 | 10.217339 |
| cache_v_l16:all-heads | Value | 16 | all | 30 | 1024 | Exact | 0.759866 | 16.433978 |
| cache_k_l31:all-heads | Key | 31 | all | 30 | 1024 | Exact | 0.922262 | 9.802850 |
| cache_v_l31:all-heads | Value | 31 | all | 30 | 1024 | Exact | 0.992878 | 2.282653 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
