"""Fast matrix/evidence integrity guards; no R, network or scientific refitting."""
import copy
import unittest

from signoff import GALLERY, REGISTER, ROOT, markdown_table, read, reference_inventory, validate


class SignoffTests(unittest.TestCase):
    def test_matrix_and_generated_summary_cover_all_recorded_report_contexts(self):
        result = validate()
        self.assertEqual(result['report_contexts'],20)
        matrix = read(REGISTER)['compatibility']
        self.assertIn(markdown_table(matrix),(ROOT/'docs/minionqc-comparison.md').read_text())

    def test_signoff_distinguishes_strict_failure_from_audit_success(self):
        record = read(ROOT/'tests/reference/signoff-summary.json')
        self.assertEqual(record['status'],'audit_complete_strict_numerical_parity_failed')
        self.assertEqual(record['strict_reference']['exit_status'],1)
        self.assertEqual(record['strict_reference']['status'],'failed')
        self.assertEqual(record['strict_reference']['regression_guard'],'passed')
        historical = read(ROOT/'tests/reference/validation-summary.json')
        self.assertEqual(record['strict_reference']['status_counts'],historical['status_counts'])
        self.assertEqual(record['strict_reference']['tolerance'],{'absolute':1e-6,'relative':1e-6})
        self.assertEqual(record['issue_acceptance']['8']['state'],'open')
        self.assertEqual(record['issue_acceptance']['12']['state'],'open')
        self.assertFalse(record['issue_acceptance']['8']['contract_change_approved'])

    def test_missing_family_or_dimension_is_not_accepted(self):
        register = read(REGISTER)
        broken = copy.deepcopy(register)
        broken['compatibility']['features'] = [f for f in broken['compatibility']['features']
                                               if f['id']!='plot.single.reads_per_hour']
        with self.assertRaises(AssertionError):
            validate(broken)
        broken = copy.deepcopy(register)
        del broken['compatibility']['features'][0]['dimensions']['statistical']
        with self.assertRaises(AssertionError):
            validate(broken)

    def test_original_gam_failure_cannot_be_called_within_tolerance(self):
        register = read(REGISTER)
        fit = next(f for f in register['compatibility']['features'] if f['id']=='plot.multi.length_by_hour')
        fit['dimensions']['numerical']='verified_with_tolerance'
        with self.assertRaisesRegex(AssertionError,'cannot be relabeled'):
            validate(register)

    def test_stale_source_coordinates_and_changed_card_hashes_fail(self):
        register = read(REGISTER)
        register['compatibility']['features'][0]['native'][0]['line']=1
        with self.assertRaises(AssertionError):
            validate(register)
        measurements = read(GALLERY/'measurements.json')
        measurements['diagnostic_corrections'][0]['changes'][0]['new_card_sha256']='0'*64
        with self.assertRaises(AssertionError):
            validate(measurements=measurements)

    def test_original_inventory_distinguishes_context_and_deduplicates_device_branches(self):
        text = '''single.flowcell <- function() {
ggsave(filename=file.path(output.dir, paste("yield_over_time.", plot_format)))
ggsave(filename=file.path(output.dir, paste("yield_over_time.", plot_format)))
}
multi.plots = function() {
ggsave(filename=file.path(output.dir, paste("yield_over_time.", plot_format)))
}
opt_str = c("-i", "--input")
'''
        families,options = reference_inventory(text)
        self.assertEqual(families,{('single','yield_over_time'),('multi','yield_over_time')})
        self.assertEqual(options,{'input'})


if __name__=='__main__':
    unittest.main()
