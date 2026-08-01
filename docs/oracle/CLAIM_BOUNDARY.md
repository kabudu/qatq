# QATQ Capacity Oracle: claim boundary

Status: production finite-certificate contract for QATQ 0.4.x (2026-08-01).
This contract covers the Hamming and Rankin engines described below. It does not
alter the QATQ/QATC formats.

## Permitted outcomes

The proposed Oracle has exactly four logical outcomes:

- `CONSTRUCTED`: a concrete representation passed every declared constraint and
  carries replayable measurements.
- `INFEASIBLE_UNDER_MODEL`: the required state count is strictly greater than an
  independently checked, finite upper bound for the normalized model.
- `UNKNOWN`: the checked evidence does not decide the request.
- `REFUSED`: the request is malformed, ambiguous, unsupported, or exceeds an
  explicit resource limit.

An upper bound that does not rule out the requested count is `UNKNOWN`, not
`CONSTRUCTED` or “feasible.” Equality with an upper bound is also `UNKNOWN`.

## Model-local meaning

An impossibility statement quantifies only over the declared binary or spherical
code model. It is not a claim about arbitrary KV caches, task quality, codecs, or
distortion measures. In particular:

- binary distance is Hamming distance on fixed-length bit strings;
- spherical separation is maximum inner product between distinct unit vectors;
- a capture-to-model conversion is a separate, explicit scientific assumption;
- an empirical construction says nothing universal about unseen captures; and
- neither CVP hardness nor an asymptotic rate theorem proves that a particular
  finite codec configuration is impossible.

## Evidence classes

| Evidence | May produce |
|---|---|
| Replayable construction satisfying the normalized request | `CONSTRUCTED` |
| Complete finite witness accepted by the independent checker | `INFEASIBLE_UNDER_MODEL` |
| High-dimensional asymptotic rate, numerical optimization, sampled sign check, or heuristic | planning report only |
| Unsupported or exhausted analysis | `UNKNOWN` or `REFUSED` |

The high-dimensional asymptotic results reviewed in
[FINITE_BOUND_APPLICABILITY.md](FINITE_BOUND_APPLICABILITY.md) are classified as
`ASYMPTOTIC_PLANNING`. They must not be adapted into finite impossibility claims
without an explicit finite inequality and independently checkable remainder.

## Stop-ship conditions

A construction and impossibility certificate for the same normalized request is a
soundness incident. Unknown critical certificate fields, arithmetic overflow,
incomplete continuous-domain checking, a failed digest, or exhausted checking
resources must fail closed and can never become a valid certificate.

## Production scope and excluded claims

The production Oracle makes no claim of optimal compression, universal KV-cache
geometry, finite consequences of an `o(n)` term, wire-format changes, or improved
QATQ compression. Its finite-certified scope is exactly:

- the binary Hamming upper bound with exact arbitrary-precision integer checking;
- the spherical Rankin bound for negative maximum inner product; and
- the spherical Rankin orthoplex bound at maximum inner product zero.

Positive-inner-product spherical requests, construction search, capture derivation,
and parameter derivation remain explicit `UNKNOWN` or `REFUSED` paths rather than
product claims. The scoped novelty claim is:

> QATQ Capacity Oracle emits independently checkable finite infeasibility
> certificates under an explicit binary or spherical representation model and
> fails closed outside its declared finite theorem scope.

QATQ v0.4.1 does not broaden this contract. It adds separate SageMath
reproduction evidence for the published rows. “Independently checkable by
QATQ,” “independently reproduced by separate software,” and “externally reviewed
by a person” are distinct statements; see
[`PUBLIC_RELEASE_0_4_1_EVIDENCE.md`](../PUBLIC_RELEASE_0_4_1_EVIDENCE.md).
