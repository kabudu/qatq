# Certificate schema

Schema version 1 binds an impossibility certificate to the complete normalized
request and its SHA-256 digest. Large integers are canonical unsigned decimal
strings.

The checker verifies:

1. strict JSON structure and supported schema/theorem identifiers;
2. the normalized request digest;
3. exact witness arithmetic from the model parameters;
4. equality between the recomputed and claimed upper bounds; and
5. the decisive strict inequality `required_states > claimed_upper_bound`.

The binary witness carries the correction radius, exact Hamming-ball volume,
and `2^n` ambient-space size. The spherical negative witness carries the exact
decimal separation as an integer numerator and power-of-ten denominator. The
orthoplex witness requires exactly `s=0` and recomputes `2d`.

Unknown fields, truncated JSON, changed requests, mismatched witness kinds,
floating-point arithmetic profiles, false bounds, equality-only requests, and
oversized certificates fail closed.
