"""Fast harness logic tests; no R or public input download needed."""
from pathlib import Path
import sys
import unittest
import tempfile
import json
from types import SimpleNamespace

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

    def test_checkpoints_preserve_unverified_cases_and_exact_counts(self):
        from minionqc_differential import save_comparison
        with tempfile.TemporaryDirectory() as directory:
            report={"cases":[{"checks":[comparison("bases",100,100),unverified("band","absent")]}]}
            counts=save_comparison(Path(directory),report)
            self.assertEqual(counts["not_yet_verified"],1)
            self.assertEqual(json.loads((Path(directory)/"comparison.json").read_text()),report)
            self.assertFalse((Path(directory)/"comparison.json.tmp").exists())

    def test_reference_command_preserves_paths_and_preflight(self):
        from minionqc_differential import reference_command
        args=SimpleNamespace(docker=None,rscript="/R installation/Rscript")
        self.assertEqual(reference_command(args,["capture.R"],{},{}),[args.rscript,"capture.R"])
        args.docker="pinned-reference"
        command=reference_command(args,["capture.R"],{"PG_REFERENCE_PREFLIGHT":"true"},
                                  {Path("/input with spaces"):False,Path("/output"):True})
        self.assertIn("/input with spaces:/input with spaces:ro",command)
        self.assertIn("PG_REFERENCE_PREFLIGHT=true",command)



class NumericalLayers(unittest.TestCase):
    def test_uncensored_smooth_predictions_and_gam_uncertainty_are_compared(self):
        from minionqc_differential import fitted_comparisons
        row=dict(x=0,y=-1,se=.1,ymin=-1.2,ymax=-.8)
        reference={"plots":[dict(file="length_by_hour.pdf",directory="source_001",panels=[{"PANEL":"1"}],
            colours=["indigo"],colour_labels=["All reads"],smooth_predictions=[dict(observations=5,data=[row])],
            layers=[dict(stat="StatSmooth",method="gam",data=[dict(PANEL="1",group=1,colour="indigo",x=0,y=None)])])]}
        curve=dict(population="all_summary_records",metric="length_bp",method="gam",observations=5,
                   confidence_interval="pointwise_95_percent_bayesian_normal",points=[dict(hours=0,value=-1,standard_error=.1,lower=-1.2,upper=-.8)])
        probe={"reference_sources":[dict(source_id="source_001",fitted_trends={"single":[curve]})]}
        checks=fitted_comparisons(reference,probe)
        self.assertTrue(all(c["status"]=="exact_match" for c in checks))
        curve["points"][0]["standard_error"]=.2
        checks=fitted_comparisons(reference,probe)
        self.assertTrue(any(c["metric"].endswith("standard_error") and c["status"]=="suspected_poregauge_defect" for c in checks))

    def test_censored_reference_prediction_is_not_a_false_match_or_defect(self):
        from minionqc_differential import fitted_comparisons
        reference={"plots":[dict(file="length_by_hour.pdf",directory="source_001",
            panels=[{"PANEL":"1"}],colours=["indigo"],colour_labels=["All reads"],
            layers=[dict(stat="StatSmooth",method="loess",data=[dict(PANEL="1",group=1,colour="indigo",x=0,y=None)])])]}
        probe={"reference_sources":[dict(source_id="source_001",fitted_trends={"single":[dict(
            population="all_summary_records",metric="length_bp",method="loess",points=[dict(hours=0,value=1e-14)])]})]}
        checks=fitted_comparisons(reference,probe)
        self.assertEqual(next(c["status"] for c in checks if c["metric"].endswith(".fitted")),"not_yet_verified")

    def test_density_comparison_checks_coordinates_values_and_integral(self):
        from minionqc_differential import density_comparisons
        reference={"plots":[{"file":"q_distributions.pdf","panels":[{"PANEL":"1","Q_cutoff":"All reads"}],"colours":["indigo"],"colour_labels":["source_001"],"layers":[{"stat":"StatDensity","data":[{"PANEL":"1","group":1,"colour":"indigo","x":0,"density":0.5},{"PANEL":"1","group":1,"colour":"indigo","x":1,"density":0.5}]}]}]}
        probe={"reference_sources":[{"source_id":"source_001","density":{"qscore":{"points":[{"x":0,"density":0.5},{"x":1,"density":0.5}]}}}]}
        reference["plots"][0]["density_parameters"]=[dict(bandwidth=.25,observations=4,
            **{"from":0,"to":1,"kernel":"gaussian","grid_points":2})]
        probe["reference_sources"][0]["density"]["qscore"].update(bandwidth=.25,observations=4)
        checks=density_comparisons(reference,probe)
        self.assertTrue(all(c["status"]=="exact_match" for c in checks))
        self.assertEqual(next(c["reference"] for c in checks if c["metric"].endswith("integral_on_plotted_domain")),0.5)
        probe["reference_sources"][0]["density"]["qscore"]["points"][0]["density"]=0.8
        self.assertTrue(any(c["status"]=="suspected_poregauge_defect" for c in density_comparisons(reference,probe)))
        reference["plots"][0].pop("density_parameters")
        self.assertTrue(any(c["status"]=="not_yet_verified" for c in density_comparisons(reference,probe)))

if __name__=="__main__": unittest.main()
