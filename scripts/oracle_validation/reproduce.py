#!/usr/bin/env sage -python
"""Independent SageMath reproduction of QATQ finite certificate rows.

This program intentionally does not import QATQ source, generated bindings, or
expected numeric answers. It consumes only the public certificate JSON format.
"""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import sys

from sage.all import Integer, binomial, version as sage_version


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def decimal_magnitude_ratio(value):
    if not isinstance(value, str) or not value.startswith("-"):
        raise ValueError("negative Rankin row requires a negative decimal string")
    magnitude = value[1:]
    if "." in magnitude:
        whole, fractional = magnitude.split(".", 1)
    else:
        whole, fractional = magnitude, ""
    if not whole.isdigit() or (fractional and not fractional.isdigit()):
        raise ValueError("invalid decimal separation")
    numerator = Integer((whole + fractional) or "0")
    denominator = Integer(10) ** len(fractional)
    # QATQ's certificate witness preserves the canonical decimal's base-10
    # numerator and denominator.  The quotient below is mathematically
    # invariant under reduction, but matching the unreduced witness detects a
    # distinct class of certificate-serialization defects.
    return numerator, denominator


def reproduce(certificate):
    theorem = certificate["theorem"]
    model = certificate["normalized_request"]["model"]
    witness = certificate["witness"]
    required = Integer(certificate["required_states"])

    if theorem == "binary-hamming-bound-v1":
        if model["kind"] != "binary" or witness["kind"] != "binary_hamming":
            raise ValueError("binary theorem/model/witness mismatch")
        dimension = Integer(model["dimension"])
        distance = Integer(model["minimum_hamming_distance"])
        radius = (distance - 1) // 2
        volume = sum(binomial(dimension, index) for index in range(radius + 1))
        ambient = Integer(2) ** dimension
        upper = ambient // volume
        witness_matches = (
            Integer(witness["correction_radius"]) == radius
            and Integer(witness["hamming_ball_volume"]) == volume
            and Integer(witness["ambient_space_size"]) == ambient
        )
        reproduction = {
            "correction_radius": str(radius),
            "hamming_ball_volume": str(volume),
            "ambient_space_size": str(ambient),
        }
    elif theorem == "spherical-rankin-orthoplex-v1":
        if model["kind"] != "spherical" or witness["kind"] != "spherical_rankin_orthoplex":
            raise ValueError("orthoplex theorem/model/witness mismatch")
        if model["maximum_inner_product"] != "0":
            raise ValueError("orthoplex theorem requires canonical s = 0")
        upper = Integer(2) * Integer(model["ambient_dimension"])
        witness_matches = True
        reproduction = {"ambient_dimension": str(model["ambient_dimension"])}
    elif theorem == "spherical-rankin-negative-v1":
        if model["kind"] != "spherical" or witness["kind"] != "spherical_rankin_negative":
            raise ValueError("negative Rankin theorem/model/witness mismatch")
        numerator, denominator = decimal_magnitude_ratio(model["maximum_inner_product"])
        if numerator <= 0 or numerator > denominator:
            raise ValueError("negative Rankin separation must satisfy -1 <= s < 0")
        upper = Integer(1) + (denominator // numerator)
        witness_matches = (
            Integer(witness["separation_magnitude_numerator"]) == numerator
            and Integer(witness["separation_denominator"]) == denominator
        )
        reproduction = {
            "separation_magnitude_numerator": str(numerator),
            "separation_denominator": str(denominator),
        }
    else:
        raise ValueError(f"unsupported theorem: {theorem}")

    claimed = Integer(certificate["claimed_upper_bound"])
    certificate_agrees = claimed == upper
    decisive = required > upper
    return {
        "theorem": theorem,
        "reproduced_upper_bound": str(upper),
        "claimed_upper_bound": str(claimed),
        "required_states": str(required),
        "witness_matches": bool(witness_matches),
        "certificate_agrees": bool(certificate_agrees),
        "decisive_inequality_holds": bool(decisive),
        "reproduction_witness": reproduction,
        "status": "AGREE" if witness_matches and certificate_agrees and decisive else "DISAGREE",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    rows = []
    for entry in manifest["rows"]:
        certificate_path = args.root / entry["certificate"]
        if sha256(certificate_path) != entry["certificate_sha256"]:
            raise SystemExit(f"{entry['id']}: certificate digest mismatch")
        certificate = json.loads(certificate_path.read_text())
        result = reproduce(certificate)
        rows.append({"id": entry["id"], "certificate_sha256": entry["certificate_sha256"], **result})

    output = {
        "schema_version": 1,
        "implementation": "independent-sagemath-finite-bounds-v1",
        "implementation_sha256": sha256(Path(__file__)),
        "environment": {
            "sagemath": str(sage_version()),
            "python": platform.python_version(),
            "platform": platform.platform(),
        },
        "row_count": len(rows),
        "all_rows_agree": all(row["status"] == "AGREE" for row in rows),
        "rows": rows,
    }
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return 0 if output["all_rows_agree"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
