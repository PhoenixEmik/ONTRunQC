"""Fast harness logic tests; no R or public input download needed."""
import importlib.util
from pathlib import Path
import sys
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from minionqc_differential import comparison, unverified


class Classification(unittest.TestCase):
    def test_exact_and_rounding(self):
        self.assertEqual(comparison("count",100,100)["status"],"exact_match")
        self.assertEqual(comparison("mean",10.1,10.14,atol=.05)["status"],"rounding_tolerance")

    def test_unexplained_mismatch_never_passes(self):
        self.assertEqual(comparison("bases",100,99)["status"],"suspected_poregauge_defect")
        self.assertEqual(comparison("zero",0,1)["absolute_difference"],1)
        self.assertIsNone(comparison("zero",0,1)["relative_difference"])

    def test_deviations_require_explicit_explanation(self):
        self.assertEqual(comparison("N50",50,100,intentional="strict-half tie")["status"],"intentional_semantic_difference")
        self.assertEqual(comparison("ultra",3,4,upstream="proved endpoint bug")["status"],"upstream_minionqc_defect")
        self.assertEqual(unverified("R","not installed")["status"],"not_yet_verified")


if __name__=="__main__": unittest.main()
