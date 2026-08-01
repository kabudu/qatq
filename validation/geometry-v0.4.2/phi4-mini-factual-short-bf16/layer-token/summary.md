# QATQ KV Geometry Profile

STATUS: `APPROXIMATE`

| group | kind | layer | head | vectors | dimension | pair mode | max cosine | effective rank |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| cache_k_l0:all-heads | Key | 0 | all | 7 | 1024 | Exact | 0.938081 | 2.476454 |
| cache_v_l0:all-heads | Value | 0 | all | 7 | 1024 | Exact | 0.758314 | 4.676174 |
| cache_k_l16:all-heads | Key | 16 | all | 7 | 1024 | Exact | 0.886734 | 2.265092 |
| cache_v_l16:all-heads | Value | 16 | all | 7 | 1024 | Exact | 0.633367 | 4.396598 |
| cache_k_l31:all-heads | Key | 31 | all | 7 | 1024 | Exact | 0.908266 | 2.565650 |
| cache_v_l31:all-heads | Value | 31 | all | 7 | 1024 | Exact | 0.982237 | 1.259563 |

## Claim boundary

- Observed capture geometry is not an application-required geometry.
- Observed vector count is not a required capacity.
- Binary mappings are descriptive and do not prove semantic distinguishability.
- This profiler does not emit Capacity Oracle verdicts.
