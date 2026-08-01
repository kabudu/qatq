# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 7 | 128 | Exact | 0.990343 | 2.174401 |
| cache_v_l0:all-heads | Value | 0 | all | 7 | 128 | Exact | 0.398129 | 4.958673 |
| cache_k_l12:all-heads | Key | 12 | all | 7 | 128 | Exact | 0.918791 | 2.179565 |
| cache_v_l12:all-heads | Value | 12 | all | 7 | 128 | Exact | 0.675842 | 4.542905 |
| cache_k_l23:all-heads | Key | 23 | all | 7 | 128 | Exact | 0.955636 | 3.541565 |
| cache_v_l23:all-heads | Value | 23 | all | 7 | 128 | Exact | 0.706634 | 4.860547 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
