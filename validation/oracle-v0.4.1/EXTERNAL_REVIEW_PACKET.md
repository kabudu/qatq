# External coding-theory review packet

This packet is for a reviewer who is independent of the QATQ implementation
and competent in coding theory, spherical codes, or rigorous computational
mathematics.

## Requested review

1. Confirm that the implemented binary expression is the finite Hamming
   sphere-packing upper bound with radius `floor((d-1)/2)`.
2. Confirm that the `s = 0` spherical expression is the Rankin orthoplex bound
   `2d` and that the stated domain conditions are sufficient.
3. Confirm that the `s < 0` expression and integer flooring used by QATQ and the
   SageMath reproduction match the applicable finite Rankin bound.
4. Inspect the SageMath implementation for shared assumptions or arithmetic
   errors that could cause false agreement with QATQ.
5. Inspect the 27-row corpus for missing boundary cases material to the three
   v0.4.0 theorem identifiers.
6. State any limitation that should narrow the release claim.

The relevant files are:

- `docs/oracle/FINITE_BOUND_APPLICABILITY.md`
- `docs/oracle/CLAIM_BOUNDARY.md`
- `src/oracle/checker.rs`
- `scripts/oracle_validation/reproduce.py`
- `validation/oracle-v0.4.1/corpus.json`
- `validation/oracle-v0.4.1/evidence/results.json`

## Recording a completed review

Update `external-review.json` only from a public, attributable review. Record
the reviewer's name, relevant competence basis, review date, exact scope, and a
stable public reference. Do not translate silence, an open request, CI success,
or an AI-generated review into `REVIEWED`.
