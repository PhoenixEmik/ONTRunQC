"""Classification safeguards for the optional numerical assessment summary."""
import json
from pathlib import Path
import tempfile
import unittest

from assess_fits import summarize


class Assessment(unittest.TestCase):
    def test_controlled_experiment_never_hides_strict_failures_or_regressions(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            check=dict(metric='fit.source_001.length_by_hour.Q>=7.79.fitted',
                       status='suspected_poregauge_defect',absolute_difference=1.7,
                       tolerance=dict(absolute=1e-6,relative=1e-6))
            nonfit=dict(metric='yield',status='exact_match',reference=100,poregauge=100,
                        tolerance=dict(absolute=0,relative=0))
            report=dict(reference_commit='pinned',source_sha256='source',
                        reference_environment={},poregauge_binary_sha256='binary',
                        status_counts=dict(suspected_poregauge_defect=1,exact_match=1),
                        cases=[dict(name='minion',reference_completed=True,
                                    reference_capture_sha256='capture',
                                    checks=[check,nonfit],gam_diagnostics=[])])
            paths=[]
            for folder in ('before','after'):
                path=root/folder/'comparison.json'
                path.parent.mkdir()
                path.write_text(json.dumps(report))
                capture=path.parent/'minion/capture.json'
                capture.parent.mkdir()
                capture.write_text('{"plots":[]}')
                (capture.parent/'source-population.json').write_text('{"reference_sources":[]}')
                paths.append(path)
            controlled=root/'controlled.json'
            controlled.write_text('{"models":[],"environment":{},"driver_sha256":"driver"}')
            sensitivity=root/'sensitivity.json';sensitivity.write_text('{}')
            result=summarize(*paths,controlled,sensitivity)
            self.assertEqual(result['status'],'Partial')
            self.assertEqual(result['suite_exit_code'],1)
            self.assertEqual(result['primary_failed_fields'],{'fitted':1})
            self.assertEqual(result['unchanged_non_fit_checks'],1)
            report['cases'][0]['checks'][0]['tolerance']['relative']=1
            paths[1].write_text(json.dumps(report))
            with self.assertRaisesRegex(AssertionError,'Changed tolerance'):
                summarize(*paths,controlled,sensitivity)
            report['cases'][0]['checks'][0]['tolerance']['relative']=1e-6
            report['cases'][0]['checks'][1]['poregauge']=99
            paths[1].write_text(json.dumps(report))
            with self.assertRaisesRegex(AssertionError,'Non-fit regression'):
                summarize(*paths,controlled,sensitivity)
