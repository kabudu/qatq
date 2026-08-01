# QATQ v0.4.1 independent reproduction corpus

This directory contains the preregistered finite-certificate corpus and the
machine-readable evidence used for QATQ v0.4.1.

The reproduction program is
[`scripts/oracle_validation/reproduce.py`](../../scripts/oracle_validation/reproduce.py).
It runs under the pinned SageMath 10.6 container, imports no QATQ code, and
consumes only public certificate JSON. For each row it independently computes:

- the binary Hamming correction radius, ball volume, ambient-space size, and
  integer upper bound;
- the spherical Rankin orthoplex upper bound at `s = 0`;
- the exact decimal ratio and negative Rankin upper bound for `s < 0`; and
- the strict inequality between the reproduced upper bound and required states.

The corpus contains 27 rows: 14 binary rows, five orthoplex rows, and eight
negative-`s` rows. It includes Hamming odd/even-radius boundaries and negative
Rankin ratios whose integer division is not exact.
[`evidence/results.json`](evidence/results.json) is the
top-level differential result. The requests, certificates, their hashes, raw
SageMath results, and QATQ/Sage comparison are all committed below `evidence/`.

Regenerate into a new directory (the script refuses to overwrite evidence):

```sh
scripts/run_oracle_independent_validation.sh /tmp/qatq-oracle-evidence
```

The Docker invocation is network-disabled and pins the SageMath image by
digest. On arm64 hosts it uses Docker's amd64 emulation because the upstream
SageMath 10.6 image does not publish an arm64 manifest.

## Evidence terminology

- **Independently checkable by QATQ** means the production `qatq-oracle check`
  path recomputed and accepted the certificate.
- **Independently reproduced by separate software** means the SageMath program
  computed the same witness and upper bound without importing QATQ.
- **Externally reviewed by a person** requires an identified reviewer, a
  relevant competence basis, a dated scope, and a public review reference.

The first two statements are supported for all rows in this corpus. The third
is supported only when [`external-review.json`](external-review.json) records a
completed review; an open request or an automated review is not sufficient.
