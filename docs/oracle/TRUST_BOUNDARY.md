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
