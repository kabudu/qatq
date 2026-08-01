# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 30 | 128 | Exact | 0.995404 | 3.179410 |
| cache_v_l0:all-heads | Value | 0 | all | 30 | 128 | Exact | 1.000000 | 7.964796 |
| cache_k_l12:all-heads | Key | 12 | all | 30 | 128 | Exact | 0.963394 | 8.160774 |
| cache_v_l12:all-heads | Value | 12 | all | 30 | 128 | Exact | 0.673612 | 16.171502 |
| cache_k_l23:all-heads | Key | 23 | all | 30 | 128 | Exact | 0.983044 | 9.589416 |
| cache_v_l23:all-heads | Value | 23 | all | 30 | 128 | Exact | 0.776227 | 18.019739 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
