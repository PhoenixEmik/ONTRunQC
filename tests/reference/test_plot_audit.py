"""Fast population/classification checks; no R, browser or real reads required."""
import json
from pathlib import Path
import tempfile
import unittest

from plot_audit import FIGURES, MULTI, first_group, histogram_points, write_gallery_index
from plot_checks import checks, result, compatible_bin_checks, compatible_time_checks, display_at, yield_display_measurements


class PlotAuditTests(unittest.TestCase):
    def test_retained_yield_duplicate_coordinates_are_one_sided_steps(self):
        points=[dict(minimum_length_bp=x,retained_yield_bp=y) for x,y in [(0,50),(10,50),(10,40),(20,40),(20,0),(21,0)]]
        self.assertEqual(display_at(points,10),(50,40))
        self.assertEqual(display_at(points,15),(40,40))
        self.assertEqual(display_at(points,20),(40,0))
        self.assertEqual(display_at(points,21),(0,0))

    def test_false_display_bound_is_not_accepted(self):
        display=dict(points=[dict(minimum_length_bp=0,retained_yield_bp=20),dict(minimum_length_bp=21,retained_yield_bp=0)],
            observations=1,error_bound_bp=0,maximum_inclusive_error_bp=0,maximum_one_sided_error_bp=0)
        report=dict(comparison=dict(yield_by_minimum_length=dict(display=dict(all=display,qscore_selected=display))))
        checks=yield_display_measurements(report,[(20,7,1)],7)
        self.assertEqual(checks['all']['status'],'unexplained_difference')

    def test_only_matching_panel_and_group_are_compared(self):
        rows=[dict(PANEL='1',group=1),dict(PANEL='1',group=2),dict(PANEL='2',group=1)]
        self.assertEqual(first_group(rows),rows[:1])
        self.assertEqual(first_group([]),[])

    def test_overflow_has_no_invented_finite_upper_boundary(self):
        points=histogram_points([dict(lower_inclusive_bp=100,upper_exclusive_bp=None,read_count=3)],True)
        self.assertEqual(points,[[2,3]])

    def test_unknown_mismatch_is_not_reclassified_as_acceptable(self):
        self.assertEqual(result('count',3,2)['status'],'unexplained_difference')
        self.assertEqual(result('count',3,3)['status'],'exact_match')

    def test_original_families_have_actual_export_mappings(self):
        self.assertEqual(len(FIGURES),16)  # 11 single +3 pooled +2 densities
        self.assertEqual(set(MULTI),{'yield_over_time','yield_by_length','length_by_hour','q_by_hour'})

    def test_zero_length_q_population_difference_is_named(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'input.tsv'
            path.write_text('channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t0\t0\n1\t1\t100\t1\n')
            bins=[dict(PANEL='1',group=1,x=0,xmin=-.5,xmax=.5,y=1,count=1),
                  dict(PANEL='1',group=1,x=1,xmin=.5,xmax=1.5,y=1,count=1)]
            capture=dict(plots=[dict(file='q_histogram.png',directory='source_001',
                panels=[dict(PANEL='1',Q_cutoff='All reads')],
                layers=[dict(stat='StatBin',audit=dict(data=bins))])])
            reports={'source_001':dict(quality=dict(histogram=[dict(read_count=1)]))}
            measured=checks(capture,reports,[path],7)
            by_name={r['metric'].rsplit('.',1)[1]:r for r in measured}
            self.assertEqual(by_name['independent_bin_L1']['absolute_difference'],0)
            self.assertEqual(by_name['population_count_difference']['status'],'intentional_population_difference')
            self.assertEqual(by_name['population_count_difference']['signed_difference'],-1)
            self.assertEqual(by_name['bin_count_difference']['status'],'different_aggregation')
            capture['plots'][0]['layers'][0]['audit']['data'][1]['count']=99
            self.assertTrue(any(c['status']=='unexplained_difference' for c in checks(capture,reports,[path],7)))

    def test_unfaceted_promethion_bin2d_uses_all_reads(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'input.tsv'
            path.write_text('channel\tstart_time\tsequence_length_template\tmean_qscore_template\n600\t0\t100\t1\n600\t1\t200\t9\n')
            capture=dict(plots=[dict(file='length_vs_q.png',directory='source_001',
                panels=[dict(PANEL='1')],layers=[dict(stat='StatBin2d',audit=dict(
                    data=[dict(PANEL='1',group=1,x=2,y=1,count=2)]))])])
            measured=checks(capture,{'source_001':{}},[path],7)
            self.assertEqual(measured[0]['status'],'exact_match')
            self.assertEqual(measured[0]['reference'],2)

    def test_reference_bin_mismatches_are_not_hidden_by_total_conservation(self):
        bins=[dict(x=0,xmin=-.5,xmax=.5,count=2),dict(x=1,xmin=.5,xmax=1.5,count=1)]
        capture=dict(plots=[dict(file='q_histogram.png',directory='source_001',panels=[dict(PANEL=1,Q_cutoff='All reads')],layers=[dict(stat='StatBin',audit=dict(data=[dict(PANEL=1,**b) for b in bins]))])])
        native=json.loads(json.dumps(bins))
        reports={'source_001':dict(minionqc_plot_bins=dict(quality=dict(all=native)))}
        self.assertEqual(compatible_bin_checks(capture,reports)[0]['status'],'exact_match')
        native[0]['count']=1;native[1]['count']=2
        failed=compatible_bin_checks(capture,reports)[0]
        self.assertEqual(failed['status'],'unexplained_difference')
        self.assertEqual(failed['count_L1'],2)
        native[0]['count']=2;native[1]['count']=1;native[0]['xmin']-=.01
        self.assertEqual(compatible_bin_checks(capture,reports)[0]['failed_coordinates'],1)

    def test_channel_count_exception_requires_independent_formula_proof(self):
        def bin(x,count): return dict(x=x,xmin=x-.5,xmax=x+.5,count=count)
        reference=[bin(1,1),bin(3,1)]
        native=[bin(1,1),bin(2,1)]
        capture=dict(plots=[dict(file='channel_summary.png',directory='source_001',panels=[dict(PANEL=1,Q_cutoff='All reads',variable='Number of reads per channel')],layers=[dict(stat='StatBin',audit=dict(data=[dict(PANEL=1,**b) for b in reference]))])])
        reports={'source_001':dict(minionqc_plot_bins=dict(channels=[dict(metric='read_count',histogram=dict(all=native))]))}
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'summary.tsv'
            path.write_text('channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t10\t9\n1\t1\t20\t9\n2\t2\t30\t9\n')
            row=compatible_bin_checks(capture,reports,[path],7)[0]
            self.assertEqual(row['status'],'intentional_upstream_defect_difference')
            self.assertEqual(set(row['upstream_defect_proof'].values()),{0})
            native[0]['count']=3
            self.assertEqual(compatible_bin_checks(capture,reports,[path],7)[0]['status'],'unexplained_difference')

    def test_temporal_hours_reject_count_swaps_and_missing_zero_hours(self):
        rows=[dict(x=0,y=2,group=1,PANEL='1',colour='blue'),dict(x=1,y=0,group=1,PANEL='1',colour='blue')]
        capture=dict(plots=[dict(file='reads_per_hour.png',directory='source_001',panels=[dict(PANEL='1')],colour_labels=['All reads'],colours=['blue'],layers=[dict(audit=dict(geom='GeomLine',data=rows))])])
        model=dict(observations=2,hourly=[dict(hour=0,read_count=2),dict(hour=1,read_count=0)])
        reports={'source_001':dict(minionqc_time_series=dict(all=model))}
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'exact_match')
        model['hourly'][0]['read_count']=1;model['hourly'][1]['read_count']=1
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'unexplained_difference')
        model['hourly']=[dict(hour=0,read_count=2)]
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'unexplained_difference')

    def test_temporal_bound_never_waives_terminal_or_coordinate_failure(self):
        c=dict(candidate='source_001:all',reference_observations=10,native_observations=10,
            maximum_coordinate_error_hours=0,unmatched_native_coordinates=0,maximum_endpoint_coordinate_error_hours=0,reference_terminal_yield_bp=100,native_terminal_yield_bp=100,maximum_envelope_error_bp=5,maximum_original_polyline_error_bp=5)
        capture=dict(plots=[dict(file='yield_over_time.png',directory='source_001',panels=[dict(PANEL='1')],colour_labels=['All reads'],colours=['blue'],layers=[dict(audit=dict(geom='GeomLine',temporal_comparisons=[dict(panel='1',colour='blue',comparisons=[c])]))])])
        reports={'source_001':dict(minionqc_time_series=dict(all=dict(observations=10,total_yield_bp=100,cumulative=dict(yield_error_bound_bp=5,omitted_points=2,maximum_linear_interpolation_error_bp=5))))}
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'bounded_display_difference')
        c['native_terminal_yield_bp']=99
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'unexplained_difference')
        c['native_terminal_yield_bp']=100;c['maximum_endpoint_coordinate_error_hours']=.0001
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'unexplained_difference')
        c['maximum_endpoint_coordinate_error_hours']=0;c['maximum_envelope_error_bp']=6
        self.assertEqual(compatible_time_checks(capture,reports)[0]['status'],'unexplained_difference')

    def test_committed_gallery_has_resolvable_cards_and_original_hashes(self):
        directory=Path(__file__).resolve().parents[2]/'docs/figures/minionqc-comparison'
        if not (directory/'measurements.json').exists():
            self.skipTest('Optional gallery has not been generated')
        evidence=json.loads((directory/'measurements.json').read_text())
        self.assertEqual(evidence['reference_commit'],'54485d586038d8ee36f168dd94f31c423f52f581')
        for plot in evidence['plots']:
            self.assertEqual(len(plot['input_sha256'][0]),64)
            if plot['card']:
                self.assertEqual((directory/plot['card']).read_bytes()[:8],b'\x89PNG\r\n\x1a\n')

    def test_gallery_index_groups_all_cards_without_modifying_evidence(self):
        directory=Path(__file__).resolve().parents[2]/'docs/figures/minionqc-comparison'
        raw=(directory/'measurements.json').read_bytes()
        evidence=json.loads(raw)
        original=json.dumps(evidence,sort_keys=True)
        with tempfile.TemporaryDirectory() as temporary:
            write_gallery_index(temporary,evidence)
            generated=(Path(temporary)/'README.md').read_text()
            self.assertEqual(generated,(directory/'README.md').read_text())
            for title in ['Single MinION flowcell','Device and legacy checks','Pooled and per-source comparisons']:
                self.assertIn('## '+title,generated)
            for plot in evidence['plots']:
                if plot.get('card'):
                    self.assertEqual(generated.count(']('+plot['card']+')'),1)
        self.assertEqual(json.dumps(evidence,sort_keys=True),original)
        self.assertEqual((directory/'measurements.json').read_bytes(),raw)


if __name__=='__main__':unittest.main()
