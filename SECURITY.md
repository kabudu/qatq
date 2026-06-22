# Security Policy

## Supported Versions

QATQ is pre-1.0. Security fixes target the latest source release and the
default development branch.

## Reporting A Vulnerability

Do not open a public issue for a suspected vulnerability.

Report security issues through GitHub private vulnerability reporting if it is
enabled for the repository. If it is not enabled yet, contact the repository
owner directly and include:

- affected commit or release;
- input file or reproduction steps, if safe to share;
- expected and actual behavior;
- impact assessment, especially for decoder crashes, corrupt output, excessive
  allocation, path traversal, or unsafe temporary-file behavior.

## Security-Relevant Scope

Security-sensitive areas include:

- `QATQ` and `QATC` decoder validation;
- chunk count, chunk length, and total value limits;
- checksum enforcement;
- atomic CLI writes;
- fixture ingestion paths;
- fuzz targets and scheduled fuzzing;
- dependency advisories.

Release candidates must keep `cargo audit`, coverage, fuzzing, and corrupt-input
tests green.
