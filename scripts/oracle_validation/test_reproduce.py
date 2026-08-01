#!/usr/bin/env sage -python
"""Focused tests for the independent SageMath arithmetic implementation."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("reproduce.py")
SPEC = importlib.util.spec_from_file_location("oracle_sage_reproduce", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def binary_certificate() -> dict[str, object]:
    return {
        "theorem": "binary-hamming-bound-v1",
        "normalized_request": {
            "model": {
                "kind": "binary",
                "dimension": 8,
                "minimum_hamming_distance": 3,
            }
        },
        "witness": {
            "kind": "binary_hamming",
            "correction_radius": 1,
            "hamming_ball_volume": "9",
            "ambient_space_size": "256",
        },
        "required_states": "29",
        "claimed_upper_bound": "28",
    }


class ReproductionTests(unittest.TestCase):
    def test_binary_row_agrees(self) -> None:
        self.assertEqual(MODULE.reproduce(binary_certificate())["status"], "AGREE")

    def test_corrupt_witness_disagrees(self) -> None:
        certificate = copy.deepcopy(binary_certificate())
        certificate["witness"]["hamming_ball_volume"] = "10"
        self.assertEqual(MODULE.reproduce(certificate)["status"], "DISAGREE")

    def test_false_upper_bound_disagrees(self) -> None:
        certificate = binary_certificate()
        certificate["claimed_upper_bound"] = "29"
        self.assertEqual(MODULE.reproduce(certificate)["status"], "DISAGREE")

    def test_negative_rankin_uses_exact_floor(self) -> None:
        certificate = {
            "theorem": "spherical-rankin-negative-v1",
            "normalized_request": {
                "model": {
                    "kind": "spherical",
                    "ambient_dimension": 128,
                    "maximum_inner_product": "-0.3",
                }
            },
            "witness": {
                "kind": "spherical_rankin_negative",
                "separation_magnitude_numerator": "3",
                "separation_denominator": "10",
            },
            "required_states": "5",
            "claimed_upper_bound": "4",
        }
        self.assertEqual(MODULE.reproduce(certificate)["status"], "AGREE")

    def test_orthoplex_row_agrees(self) -> None:
        certificate = {
            "theorem": "spherical-rankin-orthoplex-v1",
            "normalized_request": {
                "model": {
                    "kind": "spherical",
                    "ambient_dimension": 64,
                    "maximum_inner_product": "0",
                }
            },
            "witness": {"kind": "spherical_rankin_orthoplex"},
            "required_states": "129",
            "claimed_upper_bound": "128",
        }
        self.assertEqual(MODULE.reproduce(certificate)["status"], "AGREE")


if __name__ == "__main__":
    unittest.main()
