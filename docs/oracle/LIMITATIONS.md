# Limitations

- Binary production bounds use the finite Hamming bound, not a complete
  Krawtchouk/Delsarte optimizer.
- Spherical production bounds cover only `s<=0`; positive `s` requires a future
  rigorous Gegenbauer or interval-checked certificate.
- The reviewed high-dimensional rate improvements have no explicit finite remainder in
  the reviewed publication and cannot produce finite outcomes.
- Construction search, capture derivation, lossy metric conversion, and KV
  replay are not shipped in this release.
- `UNKNOWN` means only that the enabled finite methods did not decide the
  request. It is not evidence of feasibility.
- External coding-theory review and comparison with a separate mathematical
  system are planned as post-release validation rather than release gates.
