#!/usr/bin/env python3
"""Optional actual-R/actual-Rust plot audit. No third-party Python packages.

Requires pinned R packages, ImageMagick and the normal Rust static-figures build.
Outputs full reports outside Git; only bounded gallery panels/measurements are
copied into docs. Never changes the strict fitting/density comparison targets.
"""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tests'))
from minionqc_differential import cases, digest
from minionqc_reference import REVISION
from plot_checks import checks as plot_checks, curve_measurements, input_rows

# Every source plot family has an actual renderer mapping, not a redrawn proxy.
FIGURES = {
    'length_histogram': 'length-counts',
    'q_histogram': 'quality-histogram',
    'flowcell_overview': 'minion-flowcell-overview',
    'yield_over_time': 'qcutoff-yield-time',
    'yield_by_length': 'yield-minimum-length',
    'length_by_hour': 'fitted-false-false',
    'q_by_hour': 'fitted-true-false',
    'reads_per_hour': 'qcutoff-reads-time',
    'length_vs_q': 'minion-length-quality',
    'channel_summary': 'channel-summary-read_count',
    'gb_per_channel_overview': 'spatial-total-yield',
    'combined_length_histogram': 'length-counts',
    'combined_q_histogram': 'quality-histogram',
    'combined_yield_by_length': 'yield-minimum-length',
    'length_distributions': 'combined-distribution-false-false',
    'q_distributions': 'combined-distribution-true-false',
}
MULTI = {
    'yield_over_time': 'combined-yield-time-false',
    'yield_by_length': 'combined-yield-length-false',
    'length_by_hour': 'fitted-false-false',
    'q_by_hour': 'fitted-true-false',
}


def write_gallery_index(gallery, evidence):
    """Organize existing comparison cards without regenerating scientific evidence."""
    lines = [
        '<a id="actual-minionqc--ontrunqc-comparison-gallery"></a>', '',
        '# MinIONQC / ONTRunQC scientific comparison gallery', '',
        '[Scientific comparison](../../minionqc-comparison.md) · '
        '[Documentation index](../../README.md) · '
        '[Reference commands](../../../tests/reference/README.md)', '',
        '## How to read the figures', '',
        '| Panel | Content | Interpretation |',
        '| --- | --- | --- |',
        '| A | Original MinIONQC | Actual pinned R output, not a native reconstruction. |',
        '| B | Actual ONTRunQC | Canonical native figures; related All/Q panels may be stacked. |',
        '| C | Numerical diagnostic | Units, populations and differences identified in the caption; first panel/group unless stated. |', '',
        'Each card uses identical source bytes and Q cutoff. File-scoped validation '
        "matches R's grouping; source labels are not instrument run IDs. Different "
        'histogram grids are not treated as commensurable residuals. Functional '
        'coverage, numerical layer agreement and visual equivalence are separate claims.', '',
        '**Strict GAM parity remains Partial:** the original-R comparison retains '
        '1,542 fit/uncertainty discrepancies. These images do not replace that evidence.', '',
        '## Browse by analysis', '',
        '- [Single MinION flowcell](#single-minion-flowcell)',
        '- [Device and legacy checks](#device-and-legacy-checks)',
        '- [Pooled and per-source comparisons](#pooled-and-per-source-comparisons)',
        '- [Reproduction and provenance](#reproduction-and-provenance)', '',
    ]
    groups = [
        ('Single MinION flowcell', lambda row: row['case'] == 'minion',
         'Single-source distributions, temporal views, channel summaries and physical/read-level plots.'),
        ('Device and legacy checks', lambda row: row['case'] not in ('minion', 'combined'),
         'PromethION, short-run, 1D2 and direct-RNA examples exercise distinct schemas and plot populations.'),
        ('Pooled and per-source comparisons', lambda row: row['case'] == 'combined',
         'Pooled distributions and per-source comparisons are labeled separately; pooled medians/N50 are not averages.'),
    ]
    for title, matches, description in groups:
        lines += [f'## {title}', '', description, '']
        for row in evidence['plots']:
            if not row.get('card') or not matches(row):
                continue
            lines += [
                f'<a id="{row["case"]}--{row["family"]}--{row["scope"]}"></a>', '',
                f"### {row['family']} · {row['case']} · {row['scope']}", '',
                row['description'], '',
                f"Q cutoff: {row['qscore_cutoff']}; input SHA-256 prefixes: "
                f"{', '.join(value[:16] for value in row['input_sha256'])}. "
                'Axes, units and populations follow the A/B labels and C caption.', '',
                f"![{row['case']} {row['family']}: original, native and numerical diagnostic]({row['card']})", '',
            ]
    versions = evidence['reference_versions']
    lines += [
        '## Reproduction and provenance', '',
        f"- Source commit: `{evidence['reference_commit']}`.",
        f"- Reference environment: R {versions['r']}, ggplot2 {versions['ggplot2']}, mgcv {versions['mgcv']}.",
        '- Full input hashes, numerical checks, audit-script hashes and measurements: [measurements.json](measurements.json).',
        '- Generate figures with `python3 tests/reference/plot_audit.py --help`; prerequisites and commands are in the [reference guide](../../../tests/reference/README.md#formula-and-visual-layer-audit-18).',
        '- Full captures/reports remain outside Git in `benchmark-results/plot-audit/`.', '',
        evidence['render_note'], '',
        'This index is generated from recorded measurements; restructuring it does '
        'not rerun R or alter reference curves, images or discrepancy classifications.', '',
        "Public fixture figures derive from Rob Lanfear's MinIONQC (MIT). See the "
        '[upstream license](https://github.com/roblanf/minion_qc/blob/'
        f"{evidence['reference_commit']}/LICENSE).", '',
    ]
    (Path(gallery) / 'README.md').write_text('\n'.join(lines))


def run(command, log, env=None):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env,
                            capture_output=True, text=True)
    Path(log).write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{command[0]} exited {result.returncode}: {log}')


def finite(v):
    return isinstance(v, (int, float)) and math.isfinite(v)


def rows_of(layer):
    audit = layer.get('audit') or {}
    return audit.get('data') or layer.get('data') or audit.get('sample') or []


def first_group(rows):
    if not rows:
        return []
    panel, group = rows[0].get('PANEL'), rows[0].get('group')
    return [r for r in rows if r.get('PANEL') == panel and r.get('group') == group]


def histogram_points(bins, length=False):
    result = []
    for b in bins:
        lo = b.get('lower_inclusive_bp', b.get('lower_inclusive', 0))
        hi = b.get('upper_exclusive_bp', b.get('upper_exclusive'))
        # Final overflow bin remains a count, NOT an invented finite boundary.
        x = lo if hi is None else (lo + hi) / 2
        result.append([math.log10(max(1, x)) if length else x, b['read_count']])
    return result


def layer_points(plot):
    layers = [l for l in plot['layers'] if l['stat'] != 'StatIdentity' or
              l.get('audit', {}).get('geom') not in ('GeomVline',)]
    if not layers:
        return [], None
    l = layers[0]
    rows = first_group(rows_of(l))
    return [[r['x'], r.get('count', r['y'])] for r in rows
            if finite(r.get('x')) and finite(r.get('count', r.get('y')))], l


def diagnostic(plot, report, family, multi=False):
    r, layer = layer_points(plot)
    p = []
    description = 'First actual reference panel/group; ONTRunQC All population. Units as source axes.'
    counts = {'reference_layer_rows': layer['rows'] if layer else 0}
    if layer:
        counts['reference_finite_xy'] = layer['audit']['finite_xy']
        counts['reference_nonfinite_xy'] = layer['audit']['nonfinite_xy']
        counts['reference_groups'] = layer['audit']['groups']
    source = report
    if multi and report.get('collection'):
        source = report['collection']['runs'][0]
    if 'length_histogram' in family:
        p = histogram_points(report['read_length_distribution'], True)
        description = 'Counts on DIFFERENT grids: R 300 automatic log10 bins; ONTRunQC fixed bp intervals (centres shown). No binwise residual is defined.'
    elif 'q_histogram' in family:
        p = histogram_points(report['quality']['histogram'])
        description = 'Counts on DIFFERENT grids: R 300 automatic Q bins; ONTRunQC fixed Q intervals. Zero-length Q population differs.'
    elif family == 'reads_per_hour':
        p = [[(b['start_seconds'] + 450)/3600, b['basecalled_reads']] for b in source['time_bins']]
        description = 'R counts per integer hour; ONTRunQC counts per 15 minutes. Heights are NOT rates with equal exposure.'
    elif family == 'yield_over_time':
        p = [[b['end_seconds']/3600, b['cumulative_yield_bp']/1e9] for b in source['time_bins']]
        if layer and layer['audit'].get('cumulative_queries'):
            queries = next(iter(layer['audit']['cumulative_queries'].values()))['data'] if isinstance(layer['audit']['cumulative_queries'], dict) else layer['audit']['cumulative_queries'][0]['data']
            r = [[v['x'], v['y']] for v in queries]
        description = 'R exact cumulative-at-read-start queried every 15 min; ONTRunQC bin-end values. Within-bin paths differ; terminal bp conserved.'
    elif 'yield_by_length' in family:
        distribution = source.get('yield_by_minimum_length', report['comparison']['yield_by_minimum_length'])
        p = [[v['minimum_length_bp'], v['all_yield_bp']/1e9] for v in distribution['curve']]
        description = 'R cropped per-read cumulative path; ONTRunQC exact inclusive minimum-length queries. Ties, crop and bounded display are distinct.'
    elif family in ('length_by_hour', 'q_by_hour'):
        metric = 'length_bp' if family == 'length_by_hour' else 'qscore'
        trends = source.get('fitted_trends') or {}
        key = 'combined' if multi else 'single'
        curves = [c for c in trends.get(key, []) if c['metric'] == metric and c['population'] == 'all_summary_records']
        if curves:
            p = [[v['hours'], v['value']] for v in curves[0]['points'] if finite(v['value'])]
        description = 'Actual original-R fitted layer vs native Rust curve; original strict GAM failures remain failures in the separate differential suite.'
    elif family in ('length_distributions', 'q_distributions'):
        estimates = source.get('minionqc_density') or {}
        key = 'length_log10' if family == 'length_distributions' else 'qscore'
        density = estimates.get(key) or {}
        p = [[v['x'], v['density']] for v in density.get('points', [])]
        description = 'Actual ggplot StatDensity vs opt-in R-grid Rust estimator; log-length KDE is computed AFTER log10 transform.'
    elif family == 'channel_summary':
        distributions = report['comparison']['channel_distributions']
        d = next(d for d in distributions if d['metric'] == 'read_count')
        p = [[(b['lower_inclusive'] + (b['upper_exclusive'] or d['axis_maximum']))/2, b['all_summary_channels']] for b in d['bins']]
        # Original facets put bases first; explicitly select total.reads facet.
        panels = [v for v in plot['panels'] if v.get('variable') == 'Number of reads per channel' and v.get('Q_cutoff') == 'All reads']
        if panels and layer:
            rows = [v for v in rows_of(layer) if str(v['PANEL']) == str(panels[0]['PANEL'])]
            r = [[v['x'],v['count']] for v in rows if finite(v.get('x'))]
        description = 'Original read-count statistic is n(n+1)/2 (BUG); Rust uses true n. Both show channel frequency, with different bin boundaries.'
    elif family == 'length_vs_q':
        h = report['diagnostics']['length_quality_histogram']
        p = [[math.log10(max(1,h['length_lower_bounds_bp'][i])), sum(row[i] for row in h['all_read_counts'])] for i in range(len(h['length_lower_bounds_bp']))]
        if layer and layer['stat'] == 'StatBin2d':
            counts['reference_count_sum'] = sum(v.get('count',0) for v in rows_of(layer))
            counts['native_count_sum'] = sum(v[1] for v in p)
            columns = {}
            for v in rows_of(layer):
                if finite(v.get('x')): columns[v['x']] = columns.get(v['x'],0) + v['count']
            r = sorted([x,n] for x,n in columns.items())
            description = 'C sums each length column over Q: R automatic log10 bin2d versus fixed log2 length intervals. Different boundaries; A/B show actual 2D geometry. Native final overflow is plotted at its lower bound.'
        else:
            view = report['diagnostics'].get('minion_read_visualization') or {}
            counts['native_eligible_reads'] = view.get('eligible_reads')
            grid = layer['audit']['grid']
            lower,upper = grid['x_range']
            reference_counts = [sum(grid['counts'][x::64]) for x in range(64)]
            native_counts = [0]*64
            if view.get('scatter'):
                for index,n in view['scatter']['counts']:
                    native_counts[(index % 512)//8] += n
            else:
                for point in view.get('points',[]):
                    fraction=(math.log10(point['length_bp'])-lower)/(upper-lower) if upper>lower else .5
                    native_counts[min(63,max(0,int(fraction*64)))] += 1
            r = [[lower+(upper-lower)*(i+.5)/64,n] for i,n in enumerate(reference_counts)]
            p = [[lower+(upper-lower)*(i+.5)/64,n] for i,n in enumerate(native_counts)]
            counts['full_count_length_marginal_L1'] = sum(abs(a-b) for a,b in zip(reference_counts,native_counts))
            description = 'C: complete 64-column log-length marginal from ALL finite R scatter coordinates versus coarsened native full-count pixels (or exact points). This is a diagnostic aggregation, not an original R histogram or independent point-by-point rendering.'
    elif family == 'flowcell_overview':
        view = report['diagnostics'].get('minion_read_visualization') or {}
        p = [[v['channel'],v['eligible_reads']] for v in view.get('channels', [])]
        counts['native_eligible_reads'] = view.get('eligible_reads')
        coordinates = {(v['row']+1,v['column']+1):v['channel'] for v in report['diagnostics']['physical_layout']['channel_coordinates']}
        panels = {str(v['PANEL']):coordinates[(int(v['row']),int(v['col']))] for v in plot['panels']}
        r = sorted([panels[v['panel']],v['finite_xy']] for v in layer['audit']['groups'])
        description = 'C compares complete finite R panel counts against native positive-length counts/channel. R overview grid includes every finite XY row; native counts/raster conservation separately tested. A/B layouts are exact R9.5.'
    elif family == 'gb_per_channel_overview':
        p = [[v['channel'],v['total_yield_bp']/1e9] for v in report['channels']]
        r = sorted([v['channel'],v['value']/1e9] for v in plot.get('source_tiles',[]) if v['Q_cutoff']=='All reads')
        description = 'C compares exact yield/channel; source spatial cells and panel/population totals retained. A/B share verified mapping, different missing-cell styling and palette.'
    residual = []
    if (family in ('length_by_hour','q_by_hour','length_distributions','q_distributions','yield_over_time','gb_per_channel_overview','flowcell_overview') or (family=='length_vs_q' and layer and layer['stat']=='StatIdentity')) and r and p:
        # No interpolation across unlike histogram bins. Only compare matching X.
        reference_x = {round(x,10): y for x,y in r}
        residual = [[x,y-reference_x[round(x,10)]] for x,y in p if round(x,10) in reference_x]
        if residual:
            counts['matched_curve_coordinates'] = len(residual)
            counts['maximum_absolute_curve_difference'] = max(abs(v[1]) for v in residual)
    counts.update(native_display_points=len(p), reference_diagnostic_points=len(r),
                  native_total_records=report['metrics']['total_records'],
                  native_basecalled_reads=report['metrics']['basecalled_reads'])
    histogram = 'histogram' in family or family == 'channel_summary' or (family=='length_vs_q' and layer and layer['stat']=='StatBin2d')
    if 'length_histogram' in family or family in ('length_vs_q','length_distributions'):
        xlabel = 'log10(read length / bp)'
    elif 'q_histogram' in family or family=='q_distributions': xlabel = 'Mean read Q-score'
    elif 'yield_by_length' in family: xlabel = 'Minimum read length (bp)'
    elif family=='channel_summary': xlabel = 'Reads per channel (upstream count bug)'
    elif family in ('gb_per_channel_overview','flowcell_overview'): xlabel = 'Channel ID'
    else: xlabel = 'Time from run start (hours)'
    ylabel = ('Count per bin' if histogram else 'Density per X unit' if 'distributions' in family else
              'Yield (Gb)' if 'yield' in family or family=='gb_per_channel_overview' else
              'Read length (bp)' if family=='length_by_hour' else 'Mean read Q-score' if family=='q_by_hour' else 'Reads')
    return dict(reference=r, native=p, residual=residual, histogram=histogram, x_label=xlabel,y_label=ylabel, description=description, measurements=counts)


def rna_missing_labels(args, upstream, output):
    """Minimized same-byte legacy population counterexample, not a private run."""
    case=output/'rna-missing';source=case/'inputs/source_001/sequencing_summary.txt'
    source.parent.mkdir(parents=True,exist_ok=True)
    source.write_text('channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\tcalibration_strand_genome_template\n1\t0\t10\t9\t20\t\n1\t1\t20\t9\t40\tNA\n1\t2\t30\t9\t60\tno_match\n1\t3\t40\t9\t80\tYHR174W\n')
    env=dict(os.environ,TZ='UTC',LC_ALL='C',PG_REFERENCE_SOURCE=str(upstream/'MinIONQC.R'),
             PG_REFERENCE_CAPTURE=str(case/'capture.json'),PG_REFERENCE_RENDER='false',PG_PLOT_AUDIT='false')
    run([args.rscript,Path(__file__).with_name('capture.R'),'-i',source,'-o',case/'minionqc','-q','7','-m','8','-s','TRUE'],case/'R.log',env)
    run([args.binary.resolve(),'analyze',source,'--output',case/'native'],case/'native.log')
    reference=json.loads((case/'capture.json').read_text())
    native=json.loads((case/'native/report.json').read_text())
    assert reference['error'] is None
    assert (reference['populations']['All reads']['reads'],reference['populations']['All reads']['bases'])==(1,30)
    assert (native['metrics']['total_records'],native['metrics']['total_yield_bp'])==(3,60)
    return dict(input_sha256=digest(source),capture_sha256=digest(case/'capture.json'),
        reference_reads=1,reference_yield_bp=30,native_reads=3,native_yield_bp=60,
        status='intentional_population_difference',
        definition='Pinned R subset drops missing calibration labels; native legacy exact-name control exclusion retains unknown labels.',
        synthetic_tsv=source.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('upstream', type=Path)
    parser.add_argument('--rscript', default='Rscript')
    parser.add_argument('--binary',type=Path,default=ROOT/'target/release/ontrunqc')
    parser.add_argument('--output', type=Path, default=ROOT/'benchmark-results/plot-audit')
    parser.add_argument('--gallery', type=Path, default=ROOT/'docs/figures/minionqc-comparison')
    parser.add_argument('--case', action='append', choices=['minion','promethion','short','1d2','rna','combined'])
    parser.add_argument('--reuse', action='store_true', help='Reuse only hash-matching captured reports, not cached conclusions')
    args = parser.parse_args()
    upstream=args.upstream.resolve(); output=args.output.resolve(); gallery=args.gallery.resolve()
    assert subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'],text=True).strip()==REVISION
    assert not subprocess.check_output(['git','-C',str(upstream),'status','--porcelain'],text=True)
    output.mkdir(parents=True,exist_ok=True); gallery.mkdir(parents=True,exist_ok=True)
    scripts = Path(__file__).resolve().parent
    inventory=[]; numerical_checks=[]; case_evidence=[]
    for name, paths, cutoff in cases(upstream, output):
        if name not in (args.case or ['minion','promethion','short','1d2','rna','combined']): continue
        case=output/name; case.mkdir(exist_ok=True)
        copied=[]
        for i,path in enumerate(paths):
            dest=case/'inputs'/f'source_{i+1:03}'/'sequencing_summary.txt'
            dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,dest);copied.append(dest)
        stamp=dict(inputs=[digest(p) for p in paths],cutoff=cutoff,reference_commit=REVISION,
                   source_sha256=digest(upstream/'MinIONQC.R'),capture_sha256=digest(scripts/'capture.R'),
                   helper_sha256=digest(scripts/'plot_layers.R'),probe_sha256=digest(scripts/'probe.rs'),
                   native_sources={str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'src').glob('*')) if p.is_file()},
                   lockfile=digest(ROOT/'Cargo.lock'))
        stampfile=case/'stamp.json'
        previous=json.loads(stampfile.read_text()) if stampfile.exists() else {}
        same_native = all(previous.get(k)==v for k,v in stamp.items() if k not in ('capture_sha256','helper_sha256'))
        reuse_native = args.reuse and same_native
        reuse_reference = args.reuse and all(previous.get(k)==stamp[k] for k in ('inputs','cutoff','reference_commit','source_sha256','capture_sha256','helper_sha256'))
        if not reuse_reference:
            rdir=case/'minionqc';rdir.mkdir(exist_ok=True)
            env=dict(os.environ,TZ='UTC',LC_ALL='C',PG_REFERENCE_SOURCE=str(upstream/'MinIONQC.R'),
                     PG_REFERENCE_CAPTURE=str(case/'capture.json'),PG_REFERENCE_RENDER='true',
                     PG_PLOT_AUDIT='true',PG_PLOT_AUDIT_HELPER=str(scripts/'plot_layers.R'),PG_REFERENCE_DPI='60')
            run([args.rscript,scripts/'capture.R','-i',case/'inputs' if len(paths)>1 else copied[0],
                 '-o',rdir,'-q',cutoff,'-p','1','-m','8','-f','png','-s','TRUE'],case/'R.log',env)
        if not reuse_native:
            env=dict(os.environ,PG_PROBE_INPUTS=json.dumps(list(map(str,copied))),PG_PROBE_CUTOFF=str(cutoff),
                     PG_PROBE_OUTPUT=str(case/'probe.json'),PG_PROBE_CURVES='1',
                     PG_PROBE_FIGURES=str(case/'native'),PG_PROBE_LAYOUT='promethion-3000' if name=='promethion' else 'minion-r9.5')
            run(['cargo','test','--release','--locked','reference_probe::export_report','--','--ignored','--exact'],case/'Rust.log',env)
        stampfile.write_text(json.dumps(stamp,indent=2)+'\n')
        assert [digest(p) for p in copied]==stamp['inputs'], 'Copied audit input changed during processing'
        capture=json.loads((case/'capture.json').read_text())
        if capture.get('error'): raise RuntimeError(capture['error'])
        reports = {p.parent.name:json.loads(p.read_text()) for p in (case/'native').glob('*/report.json')}
        checked = plot_checks(capture,reports,copied,cutoff)
        numerical_checks.extend(dict(case=name,**c) for c in checked)
        case_evidence.append(dict(case=name,input_sha256=stamp['inputs'],qscore_cutoff=cutoff,
            capture_sha256=digest(case/'capture.json'),session=capture['session'],
            all_summary_records=reports['combined']['metrics']['total_records'],
            basecalled_reads=reports['combined']['metrics']['basecalled_reads'],
            yield_bp=reports['combined']['metrics']['total_yield_bp'],
            display_differences=curve_measurements(reports['combined'],list(input_rows(copied)))))
        for plot in capture['plots']:
            family=plot['file'].rsplit('.',1)[0]
            multi=plot['directory']=='combinedQC' and not family.startswith('combined_')
            directory='combined' if plot['directory']=='combinedQC' else plot['directory']
            report=json.loads((case/'native'/directory/'report.json').read_text())
            target=(MULTI if multi and family in MULTI else FIGURES).get(family)
            if name=='promethion' and family=='length_vs_q': target='length-quality-all'
            candidates=list((case/'native'/directory/'figures').glob('*.png'))
            png=case/'native'/directory/'figures'/f'{target}.png'
            if not png.exists():
                raise RuntimeError(f'Missing actual native figure {png}; available {[p.stem for p in candidates]}')
            data=diagnostic(plot,report,family,multi)
            m=data['measurements']
            if 'full_count_length_marginal_L1' in m or family in ('flowcell_overview','gb_per_channel_overview'):
                error=m.get('full_count_length_marginal_L1',m.get('maximum_absolute_curve_difference',0))
                numerical_checks.append(dict(case=name,metric=directory+'.'+family+'.native_reference_count_or_yield_difference',
                    reference=0,ontrunqc_or_independent=error,absolute_difference=abs(error),signed_difference=error,
                    tolerance=0,status='exact_match' if error==0 else 'unexplained_difference',explanation='Full finite populations; no sampled equality claim.'))
            identity=f'{name}-{directory}-{family}'
            diagnostic_file=case/f'{identity}.json';diagnostic_file.write_text(json.dumps(data,allow_nan=False))
            run([args.rscript,scripts/'plot_diagnostic.R',diagnostic_file,case/f'{identity}-difference.png'],case/f'{identity}-diagnostic.log')
            # Keep one readable A/B/C per family, plus distinct legacy/device cases.
            selected=(name=='minion' or (name=='combined' and directory=='combined') or
                      (name in ('promethion','1d2') and family=='length_vs_q') or
                      (name=='rna' and family=='q_histogram') or (name=='short' and family=='reads_per_hour'))
            card=None
            if selected:
                companions = []
                if 'length_histogram' in family: companions = ['qcutoff-length-counts']
                elif 'q_histogram' in family: companions = ['qcutoff-quality']
                elif family == 'gb_per_channel_overview': companions = ['spatial-qcutoff-yield']
                elif family == 'channel_summary': companions = ['channel-summary-'+m for m in ['total_yield_bp','mean_read_length_bp','median_read_length_bp']]
                elif multi and family in ('length_distributions','q_distributions','length_by_hour','q_by_hour'):
                    companions = [target.rsplit('-',1)[0]+'-true']
                native_panel = png
                if companions:
                    native_panel = case/f'{identity}-native-panels.png'
                    run(['magick',png,*[png.with_name(c+'.png') for c in companions],'-append',native_panel],case/'image.log')
                images=[case/'minionqc'/plot['directory']/plot['file'],native_panel,case/f'{identity}-difference.png']
                panels=[]
                for i,(image,label) in enumerate(zip(images,['A  Original MinIONQC','B  Actual ONTRunQC','C  Numerical diagnostic'])):
                    panel=case/f'{identity}-{i}.png';panels.append(panel)
                    run(['magick',image,'-background','white','-alpha','remove','-resize','720x500',
                         '-gravity','center','-extent','740x540','-gravity','north','-font',ROOT/'assets/fonts/LiberationSans-Regular.ttf',
                         '-pointsize','19','-annotate','+0+7',label,panel],case/'image.log')
                card=f'{identity}.png'
                run(['magick',*panels,'+append','-strip','-dither','None','-colors','256','-define','png:compression-level=9',gallery/card],case/'image.log')
            inventory.append(dict(case=name,family=family,scope=directory,reference_plot=plot['file'],
                native_figure=png.name,diagnostic=data,card=card,input_sha256=stamp['inputs'],qscore_cutoff=cutoff))
        print(name,len(capture['plots']),'actual figure families captured',flush=True)
    edge = rna_missing_labels(args,upstream,output) if not args.case or 'rna' in args.case else None
    result=dict(reference_commit=REVISION,reference_source_sha256=digest(upstream/'MinIONQC.R'),
                reference_versions=dict(r='4.4.1',ggplot2='3.5.1',mgcv='1.9-1'),
                native_probe_sha256=digest(scripts/'probe.rs'),capture_script_sha256=digest(scripts/'capture.R'),
                audit_script_sha256=digest(Path(__file__)),lockfile_sha256=digest(ROOT/'Cargo.lock'),
                render_note='Original R smallfigures TRUE, PNG 60 DPI for audit readability; native canonical figures 300 DPI; gallery resized and palette-reduced to 256 colours only for documentation. Original files remain unchanged.',
                limitations='A/B have independently defined grids/populations; no pixel-equivalence claim. C first panel/group unless stated; strict fitting comparisons remain separate.',
                cases=case_evidence,rna_missing_labels=edge,numerical_checks=numerical_checks,plots=inventory)
    (output/'audit.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    # Store bounded evidence without duplicate XY arrays/full reports in Git.
    compact={**result,'plots':[{k:v for k,v in row.items() if k!='diagnostic'} | dict(
        description=row['diagnostic']['description'],measurements=row['diagnostic']['measurements']) for row in inventory]}
    (gallery/'measurements.json').write_text(json.dumps(compact,indent=2,allow_nan=False)+'\n')
    write_gallery_index(gallery, compact)
    unexplained=[c for c in numerical_checks if c['status']=='unexplained_difference']
    print(f'{len(numerical_checks)} layer checks; {len(unexplained)} unexplained differences',flush=True)
    return bool(unexplained)


if __name__=='__main__': sys.exit(main())
