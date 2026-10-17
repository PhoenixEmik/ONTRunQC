"""Fast harness logic tests; no R or public input download needed."""
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



class NumericalLayers(unittest.TestCase):
    def test_density_comparison_checks_coordinates_values_and_integral(self):
        from minionqc_differential import density_comparisons
        reference={"plots":[{"file":"q_distributions.pdf","panels":[{"PANEL":"1","Q_cutoff":"All reads"}],"colours":["indigo"],"colour_labels":["source_001"],"layers":[{"stat":"StatDensity","data":[{"PANEL":"1","group":1,"colour":"indigo","x":0,"density":0.5},{"PANEL":"1","group":1,"colour":"indigo","x":1,"density":0.5}]}]}]}
        probe={"reference_sources":[{"source_id":"source_001","density":{"qscore":{"points":[{"x":0,"density":0.5},{"x":1,"density":0.5}]}}}]}
        checks=density_comparisons(reference,probe)
        self.assertTrue(all(c["status"]=="exact_match" for c in checks))
        self.assertEqual(next(c["reference"] for c in checks if c["metric"].endswith("integral_on_plotted_domain")),0.5)
        probe["reference_sources"][0]["density"]["qscore"]["points"][0]["density"]=0.8
        self.assertTrue(any(c["status"]=="suspected_poregauge_defect" for c in density_comparisons(reference,probe)))

if __name__=="__main__": unittest.main()
