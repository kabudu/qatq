# Trust boundary

Trusted code is limited to request normalization, exact integer primitives,
SHA-256 binding, schema/version rules, finite witness production, and the
independent checker path.

Reports, imported requests, asymptotic planning, future solver output, capture
metadata, and construction search are untrusted or advisory. The checker
recomputes finite arithmetic and never trusts a producer's claimed objective.

The checker is bounded by certificate bytes, dimension, schema, and arithmetic
policy. It uses no floating point and no LP solver. Unsupported theorems and
resource exhaustion cannot become `VALID`.

QATQ/QATC encoding and decoding do not depend on Oracle code unless the Cargo
`oracle` feature is explicitly enabled. The cargo-dist production build enables
that feature so release archives contain `qatq-oracle`; the codec wire formats
remain unchanged.

QATQ v0.4.1 adds a separate evidence layer outside this production checker
boundary. A pinned SageMath implementation consumes public certificate JSON and
reproduces the finite witness and upper bound without importing QATQ code. Its
agreement is evidence about the checker and producer; SageMath is not a runtime
dependency and is not added to the trusted production path.

Automated agreement is not human review. A claim of external review additionally
requires the attributable record defined in
[`validation/oracle-v0.4.1/external-review.json`](../../validation/oracle-v0.4.1/external-review.json).
