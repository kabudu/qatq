# Finite-bound applicability

Date: 2026-08-01  
Decision: **GO: finite certificates are non-vacuous for a defined QATQ use case.**

The `GO` is narrow. It is supported by established finite Hamming/Rankin bounds,
not by turning the reviewed asymptotics into finite bounds. Asymptotic output
is restricted to `ASYMPTOTIC_PLANNING` until an explicit finite remainder or a
separate finite LP witness exists.

## Sources and normalization

The qualification reviewed Chapters 1 and 2 of OpenAI's *Ten Advances in
Mathematics and Theoretical Computer Science*, whose source materials refer to
the new asymptotic results as Astra. Credit belongs with that research; the
production Oracle engines in this release use the classical finite Hamming and
Rankin bounds and do not depend on those asymptotic results.
For binary codes, `A_2(n,d)` is the largest subset of `{0,1}^n` having minimum
Hamming distance at least `d`. For spherical codes, `A(n,s)` is the largest subset
of the unit sphere in `R^n` whose distinct inner products are at most `s`.

The rates used by Chapter 2 are

```text
R_2(delta) = limsup[n -> infinity] log2(A_2(n, ceil(delta*n))) / n
R_sph(s)   = limsup[n -> infinity] log2(A(n,s)) / n.
```

## Candidate-theorem qualification

| Result | Exact statement relevant here | Type | Explicit finite constants/remainder | Finite certificate now? | Independent check | Refuse finite use when |
|---|---|---|---|---|---|---|
| Sphere packing, Thm. 1.1 | `LP_d^(1/d) -> sqrt(e/(2*pi))` as `d -> infinity`; hence `Delta_d <= 2^(-(alpha*+o(1))d)` with `alpha*=0.6044...` | Asymptotic | No computable `o(1)` remainder or effective threshold is stated | No | A future finite Cohn–Elkies auxiliary function could be checked directly | Only the limit/exponent is supplied |
| Sign uncertainty, Thm. 1.2 | `A_+(d)/sqrt(d)` and `A_-(d)/sqrt(d)` both tend to `1/pi` | Asymptotic | No effective remainder for dimensions 64–1024 | No | Future finite sign/function witness | A finite radius is inferred from the limit |
| Binary codes, Ch. 2 Thm. 1.1 | For every fixed `0 < delta < 1/2`, `R_2(delta) <= kappa_bin(delta) < M_2(delta)` | Asymptotic rate | Variational formula is explicit, but no finite-`n` remainder/threshold is provided | No | A separate finite Krawtchouk-basis dual witness | Used to bound a particular `A_2(n,d)` directly |
| Spherical codes, Ch. 2 Thm. 1.2 and Cor. 1.3 | For fixed hierarchy parameters with `2 Gamma_r(a,b)>s`, `A(n,s) <= 2^((Phi_r(a,b)+o(1))n)`; optimizing the hierarchy gives a rate strictly below the classical optimized KL exponent | Asymptotic rate | The paper explicitly says `o(1)->0` with the parameters fixed, but gives no finite-`n` remainder/threshold | No | A separate finite Gegenbauer-basis witness with a complete interval sign proof | Used to bound a particular `A(n,s)` directly |
| Finite binary Hamming bound | With `t=floor((d-1)/2)`, `A_2(n,d) * sum(i=0..t) C(n,i) <= 2^n` | Finite | Exact integers | Yes | Recompute binomial sum and integer division | Invalid `n,d`, allocation limit, or unsupported integer size |
| Finite Rankin spherical bounds | `A(n,s) <= floor(1-1/s)` for `s<0`; `A(n,0) <= 2n` | Finite | Exact rational/integer arithmetic | Yes | Gram-matrix/centroid inequality and integer comparison | Vectors are not unit-normalized or separation semantics differ |
| Spherical cap packing | Caps of angular radius `acos(s)/2` are disjoint, so `A(n,s)` is at most reciprocal cap measure | Finite theorem | Requires rigorous special-function enclosure | Not yet | Interval enclosure of regularized incomplete beta | Only floating-point or sampled evaluation is available |

The paper's constructions may inspire finite dual witnesses later. The checker must
validate those witnesses independently; the paper's Lean formalization of an
asymptotic theorem does not supply a certificate for a finite request.

## Kill test at attention-sized dimensions

**Asymptotic-results answer:** no. The published binary and spherical statements contain no
explicit remainder that turns their rate inequalities into certified values at
dimensions 64, 128, 256, 512, or 1024. At every one of these dimensions the asymptotic
column is therefore “planning only,” regardless of how favorable its limiting
exponent looks.

**Established finite-method answer:** yes, for a defined use case. If a normalized
spherical state family requires pairwise nonpositive inner products, Rankin gives
`A(n,0) <= 2n`. Thus 65,536 declared states are impossible already at dimensions
64, 128, 256, 512, and 1024. Similarly, the exact binary Hamming bound gives useful
finite decisions for sufficiently separated bit strings.

| dimension | asymptotic binary/spherical result | Rankin `s=0` upper bound | `2^16` ruled out? |
|---:|---|---:|---|
| 64 | asymptotic planning only | 128 | yes |
| 128 | asymptotic planning only | 256 | yes |
| 256 | asymptotic planning only | 512 | yes |
| 512 | asymptotic planning only | 1,024 | yes |
| 1024 | asymptotic planning only | 2,048 | yes |

This use case is mathematically meaningful but intentionally narrow; a bridge must
not silently assert that real KV states require nonpositive pairwise inner product.

## Exact binary prototype calculations

The table uses the smaller of the exact Hamming bound and, at relative distance
`1/2`, the finite Plotkin/orthoplex value `2n`. `log2 U` is descriptive only; the
certificate inequality uses the integer `U`.

| `n` | `d` | separation `d/n` | certified `U` | `log2 U` | requested bit counts ruled out |
|---:|---:|---:|---:|---:|---|
| 64 | 8 | 1/8 | 421,688,057,462,785 | 48.583 | 64 |
| 64 | 16 | 1/4 | 26,184,380,591 | 34.608 | 48, 64 |
| 64 | 24 | 3/8 | 19,883,522 | 24.245 | 32, 48, 64 |
| 64 | 32 | 1/2 | 128 | 7.000 | 16, 32, 48, 64 |
| 128 | 16 | 1/8 | 3,395,184,828,163,349,608,402,497,490 | 91.456 | none |
| 128 | 32 | 1/4 | 22,396,032,652,922,403,638 | 64.280 | none |
| 128 | 48 | 3/8 | 19,391,329,499,178 | 44.140 | 48, 64 |
| 128 | 64 | 1/2 | 256 | 8.000 | 16, 32, 48, 64 |
| 256 | 32 | 1/8 | 162,372,326,214,067,081,141,448,504,628,655,429,554,456,941,526,568,067 | 176.761 | none |
| 256 | 64 | 1/4 | 12,071,934,357,881,141,822,097,984,615,890,981,597 | 123.183 | none |
| 256 | 96 | 3/8 | 13,683,528,021,137,272,041,170,604 | 83.501 | none |
| 256 | 128 | 1/2 | 512 | 9.000 | 16, 32, 48, 64 |

For example, at `(n,d)=(128,48)`, `t=23` and exact integer arithmetic gives

```text
U = floor(2^128 / sum(i=0..23) C(128,i))
  = 19,391,329,499,178.
```

Since `2^48 > U`, a 48-bit state requirement is infeasible under this binary
model. `2^32 <= U` is merely unsettled.

## Spherical prototype calculations

The exact Rankin rows are certificate-ready. Positive-`s` rows show the classical
cap bound's approximate `log2` value as a usefulness screen only; they are **not**
certificates until interval arithmetic encloses the cap measure outward.

| `d` | max inner product `s` | method | upper bound / approximate `log2 U` | 16/32/48/64-bit requests ruled out |
|---:|---:|---|---:|---|
| 64 | -0.25 | Rankin | 5 | all |
| 64 | 0 | Rankin | 128 | all |
| 64 | 0.25 | cap screen | ~48.568 bits | 64 only (planning) |
| 64 | 0.50 | cap screen | ~67.120 bits | none |
| 64 | 0.75 | cap screen | ~98.727 bits | none |
| 128 | -0.25 | Rankin | 5 | all |
| 128 | 0 | Rankin | 256 | all |
| 128 | 0.25 | cap screen | ~94.345 bits | none |
| 128 | 0.50 | cap screen | ~131.619 bits | none |
| 128 | 0.75 | cap screen | ~195.228 bits | none |
| 256 | -0.25 | Rankin | 5 | all |
| 256 | 0 | Rankin | 512 | all |
| 256 | 0.25 | cap screen | ~185.406 bits | none |
| 256 | 0.50 | cap screen | ~260.119 bits | none |
| 256 | 0.75 | cap screen | ~387.729 bits | none |

For `s<0`, the centroid/Gram argument gives `M <= 1-1/s`; at `s=-1/4`,
`M<=5`. For `s=0`, Rankin's orthoplex bound gives `M<=2d`. These calculations
require no asymptotic inference.

## Conclusion and implementation authorization

The first kill test passes only through established finite methods. A useful,
independently checkable bound exists for explicit QATQ-shaped models, authorizing
the M1 contracts plus the exact Hamming and `s<=0` Rankin production engines. This
does not authorize a general binary or spherical LP engine. The following remain
unauthorized:

- any finite outcome derived only from asymptotic rate theorems;
- a positive-`s` spherical certificate without complete interval/root checking;
- a general sphere-packing, CVP, Ehrhart, or decoder-complexity engine; and
- any assertion that observed KV distortion automatically supplies code separation.
