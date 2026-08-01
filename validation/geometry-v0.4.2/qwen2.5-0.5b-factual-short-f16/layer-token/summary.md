# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 7 | 128 | Exact | 0.990354 | 2.175121 |
| cache_v_l0:all-heads | Value | 0 | all | 7 | 128 | Exact | 0.397673 | 4.959888 |
| cache_k_l12:all-heads | Key | 12 | all | 7 | 128 | Exact | 0.918165 | 2.177495 |
| cache_v_l12:all-heads | Value | 12 | all | 7 | 128 | Exact | 0.677767 | 4.566127 |
| cache_k_l23:all-heads | Key | 23 | all | 7 | 128 | Exact | 0.955258 | 3.557187 |
| cache_v_l23:all-heads | Value | 23 | all | 7 | 128 | Exact | 0.700718 | 4.880798 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
