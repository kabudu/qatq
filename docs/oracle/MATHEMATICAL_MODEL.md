# Oracle mathematical model

## Binary model

A request `(n,d,M)` asks whether there is a set `C` contained in `{0,1}^n` such
that `|C| >= M` and every distinct `x,y` in `C` satisfy

```text
HammingDistance(x,y) >= d.
```

The normalized domain is `n>=1`, `1<=d<=n`, and decimal-string `M>=1`. The model
does not imply a codec, decoding radius, quantization error, or tensor metric.

For the Phase 0 finite calculation, let `t=floor((d-1)/2)`. Disjoint Hamming
balls prove

```text
A_2(n,d) <= floor(2^n / sum(i=0..t) binomial(n,i)).
```

All quantities are exact integers. `M` is impossible under the model iff it is
strictly greater than the checked upper bound.

## Spherical model

A request `(n,s,M)` asks whether there is a set `C` of unit vectors in `R^n` such
that `|C|>=M` and every distinct `x,y` satisfy

```text
inner_product(x,y) <= s.
```

The normalized domain is `n>=2`, `-1<=s<1`, and decimal-string `M>=1`. Unit-L2
normalization is part of the model. Angular separation is equivalently at least
`acos(s)`.

Exact Phase 0 bounds are:

```text
s < 0: A(n,s) <= floor(1 - 1/s)
s = 0: A(n,0) <= 2n.
```

For `0<s<1`, a finite cap-packing theorem exists, but evaluating its cap measure
must use rigorous outward rounding before it can support a certificate. A grid or
ordinary floating-point estimate is not a proof.

## Required state counts

Counts use decimal strings in JSON. The standard test requirements are
`2^16=65536`, `2^32=4294967296`, `2^48=281474976710656`, and
`2^64=18446744073709551616`. A storage budget does not itself establish that all
`2^b` states are required; the requirement source must be user-declared or derived
by a documented bridge rule.

## Normalization and identity

Future normalization must reject unknown critical fields, duplicate JSON object
keys, noncanonical large integers, NaN/infinity, impossible ranges, and dimensions
outside resource policy. A normalized request digest binds the schema version,
request identifier, model, count, separation, and all critical assumptions.

No distortion-to-distance conversion belongs in these core models. Such a bridge
must identify its artifact digest, partitioning, normalization, metric, sampling,
requirement source, separation source, and assumptions.
