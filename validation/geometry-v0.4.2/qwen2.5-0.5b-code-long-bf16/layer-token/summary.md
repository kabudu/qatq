# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 67 | 128 | Exact | 0.995548 | 3.575373 |
| cache_v_l0:all-heads | Value | 0 | all | 67 | 128 | Exact | 1.000000 | 7.110042 |
| cache_k_l12:all-heads | Key | 12 | all | 67 | 128 | Exact | 0.957912 | 11.822136 |
| cache_v_l12:all-heads | Value | 12 | all | 67 | 128 | Exact | 0.947016 | 18.745692 |
| cache_k_l23:all-heads | Key | 23 | all | 67 | 128 | Exact | 0.980621 | 13.105018 |
| cache_v_l23:all-heads | Value | 23 | all | 67 | 128 | Exact | 0.953857 | 24.842534 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
