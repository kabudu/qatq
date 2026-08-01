# Capacity Oracle evaluation plan

## Phase gates

1. Preserve the Phase 0 claim boundary and reproduce every table with an
   independent exact-arithmetic script or coding-theory system.
2. Add strict versioned contracts, deterministic normalization, limits, CLI
   skeleton, and only `UNKNOWN`/`REFUSED` behavior.
3. Implement finite binary witness production and a smaller checker that does not
   share optimizer logic.
4. Add positive-separation spherical bounds only after a complete continuous sign
   proof using rational root isolation, certified intervals, or checked SOS.
5. Add capture derivation and construction replay without changing QATQ/QATC bytes.

## Phase 0 reproduction

Binary rows are reproduced with arbitrary-precision integers:

```text
t = floor((d-1)/2)
volume = sum(binomial(n,i), i=0..t)
U_hamming = floor(2^n / volume)
```

At `d=n/2`, also check the finite Plotkin/orthoplex bound `U=2n` and take the
smaller applicable upper bound. Separately reproduce representative cases in
SageMath, Mathematica, or established coding-theory software.

For the spherical screen, exact Rankin rows require rational arithmetic only.
Positive-`s` cap estimates are deliberately non-decisive until two independent
interval implementations agree and the emitted enclosure can be checked without
rerunning search.

## Conformance matrix

- known small values and published upper bounds for `A_2(n,d)`;
- monotonicity checks while respecting which parameter direction is stronger;
- `required == upper` returns `UNKNOWN`;
- `required == upper+1` returns `INFEASIBLE_UNDER_MODEL`;
- exact outward integer rounding;
- malformed/oversized dimensions, counts, degrees, and coefficients;
- changed request digest, truncated/duplicated witness fields, denominator zero,
  false objective, unsupported theorem, and false decisive inequality;
- roots at interval endpoints and near the spherical threshold;
- exact f32/f16/bf16 replay, signed zero and NaN payloads, stride hints, and raw
  pass-through once the bridge exists;
- regression comparison proving existing QATQ/QATC encodings are unchanged.

## Representative QATQ-shaped scenarios

Evaluate dimensions 64, 128, 256, 512, and 1024; binary relative distances 1/8,
1/4, 3/8, and 1/2; spherical `s` values -0.25, 0, 0.25, 0.5, and 0.75; and required
counts `2^16`, `2^32`, `2^48`, and `2^64`. Record `UNKNOWN` explicitly wherever a
finite checked bound does not decide the request.

## Release requirements

No impossibility feature ships until the built-in independent checker validates
every emitted finite witness, arithmetic and resource use fail closed,
adversarial tests pass, published small bounds reproduce, and reports distinguish
proof, planning, and construction. External mathematician and separate
SageMath/Mathematica verification are post-release evidence and do not block the
production release. A contradiction between a valid construction and a valid
upper-bound certificate blocks release.
