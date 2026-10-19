"""A repeated strict failure must remain a failure; changed evidence is rejected."""
import unittest
from check_revalidation import signature, verify


class RevalidationTests(unittest.TestCase):
    def fixture(self):
        case = dict(name='synthetic', reference_completed=True, reference_returncode=0,
                    checks=[dict(metric='fitted', reference=1, poregauge=2,
                                 tolerance=.001, status='suspected_poregauge_defect')],
                    gam_diagnostics=[], loess_diagnostics=[])
        report = dict(reference_commit='pinned', source_sha256='original',
                      reference_environment={'r': '4.4.1'},
                      status_counts={'suspected_poregauge_defect': 1}, cases=[case])
        summary = dict(reference_commit='pinned', reference_source_sha256='original',
                       reference_versions={'r': '4.4.1'}, status_counts=report['status_counts'].copy(),
                       cases={'synthetic': {g: signature(case[g]) for g in ['checks', 'gam_diagnostics', 'loess_diagnostics']}})
        return report, summary

    def test_original_failure_is_preserved(self):
        report, summary = self.fixture()
        self.assertEqual(verify(report, summary), 1)
        self.assertEqual(report['status_counts'], {'suspected_poregauge_defect': 1})

    def test_changed_values_tolerances_statuses_and_incomplete_execution_fail(self):
        for key, value in [('poregauge', 1), ('tolerance', 5), ('status', 'exact_match')]:
            report, summary = self.fixture()
            report['cases'][0]['checks'][0][key] = value
            with self.assertRaises(AssertionError): verify(report, summary)
        report, summary = self.fixture()
        report['cases'][0]['reference_completed'] = False
        with self.assertRaises(AssertionError): verify(report, summary)
        report, summary = self.fixture()
        report['reference_environment']['r'] = 'different'
        with self.assertRaises(AssertionError): verify(report, summary)
