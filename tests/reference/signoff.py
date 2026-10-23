#!/usr/bin/env python3
"""Focused compatibility-record guards and optional retained-artifact audit.

No statistical recalculation or reference reclassification. Run --help for the
optional pinned-source/capture checks. The matrix lives in the existing register.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from check_revalidation import verify

ROOT = Path(__file__).resolve().parents[2]
GALLERY = ROOT/'docs/figures/minionqc-comparison'
REGISTER = GALLERY/'discrepancies.json'
AXES = ('functional','statistical','numerical','visual','cli_output')
CLASSES = ('verified_exact','verified_with_tolerance','verified_bounded',
           'functional_with_difference','intentional_correction','strict_failure',
           'not_verified','not_applicable')
CODES = dict(zip(CLASSES, ('E','T','B','D','I','X','U','—')))
REVISION = '54485d586038d8ee36f168dd94f31c423f52f581'
REFERENCE_SHA = '841253a28a785b9efdc09c855c163646882e01152e26eda509adae2732c6651d'


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reference_inventory(text):
    """Enumerate every literal ggsave output in its original function context."""
    context = None
    families = set()
    for line in text.splitlines():
        for name, value in [('single.flowcell','single'),('combined.flowcell','pooled'),('multi.plots','multi')]:
            if re.match(re.escape(name)+r'\s*(?:<-|=)\s*function',line):
                context = value
        if 'ggsave(' in line:
            match = re.search(r'''file\.path\(output\.dir,\s*(?:paste\()?\s*["']([^"']+)''',line)
            assert match and context, 'Unmapped original output: '+line
            families.add((context,match[1].split('.')[0]))
    options = set(re.findall(r'opt_str\s*=\s*c\("-[a-z]",\s*"--([^"]+)"\)',text))
    return families, options


def validate(register=None, measurements=None, upstream=None):
    register = register if register is not None else read(REGISTER)
    measurements = measurements if measurements is not None else read(GALLERY/'measurements.json')
    matrix = register['compatibility']
    assert register['reference_commit'] == measurements['reference_commit'] == REVISION
    assert matrix['reference_source_sha256'] == measurements['reference_source_sha256'] == REFERENCE_SHA
    assert tuple(matrix['axes']) == AXES
    features = matrix['features']
    ids = {f['id'] for f in features}
    assert len(ids) == len(features)
    findings = {f['id'] for f in register['findings']}
    assert len(findings) == len(register['findings'])
    evidence = matrix['evidence_registry']
    for entry in evidence.values():
        assert (ROOT/entry['path']).is_file(), entry
    expected = {('single' if p['scope']!='combined' else 'pooled' if p['family'].startswith('combined_') else 'multi',p['family']) for p in measurements['plots']}
    mapped = {(f['report_key']['context'],f['report_key']['family']) for f in features if 'report_key' in f}
    assert mapped == expected and len(mapped) == 20, 'Incomplete report/context coverage'
    for f in features:
        assert set(f['dimensions']) == set(AXES), f['id']
        assert set(f['dimensions'].values()) <= set(CLASSES), f['id']
        assert f['evidence'] and set(f['evidence']) <= set(evidence)
        assert set(f['finding_ids']) <= findings
        assert f['acceptance_status'] != 'Complete'
        if f['dimensions']['numerical']=='strict_failure':
            assert f['acceptance_status']=='open_strict_numerical_gate'
        if f['related_issue']==12 and f.get('report_key'):
            historical = read(ROOT/'tests/reference/validation-summary.json')
            if historical['status_counts'].get('suspected_poregauge_defect',0):
                assert f['dimensions']['numerical']=='strict_failure', 'Original GAM failures cannot be relabeled by an audit'
        reference = f['reference']
        assert 1 <= reference['lines'][0] <= reference['lines'][1] <= matrix['reference_line_count']
        for source in f['native']:
            lines = (ROOT/source['path']).read_text().splitlines()
            assert 1 <= source['line'] <= len(lines), source
            assert re.search(r'\b'+re.escape(source['symbol'].split('::')[-1])+r'\b',lines[source['line']-1]), source
        for test in f['tests']:
            assert re.search(r'\b(?:fn|def) '+re.escape(test['symbol'])+r'\b',(ROOT/test['path']).read_text()),test
    declared_cards = {p['card'] for p in measurements['plots'] if p.get('card')}
    actual_cards = {p.name for p in GALLERY.glob('*.png')}
    assert actual_cards == declared_cards, 'Missing or orphaned gallery images'
    for name in declared_cards:
        assert (GALLERY/name).read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
    for correction in measurements.get('diagnostic_corrections',[]):
        for change in correction['changes']:
            assert digest(GALLERY/change['card']) == change['new_card_sha256']
    if upstream:
        script = upstream/'MinIONQC.R'
        assert digest(script) == REFERENCE_SHA
        assert subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'],text=True).strip() == REVISION
        assert not subprocess.check_output(['git','-C',str(upstream),'status','--porcelain'])
        text = script.read_text()
        assert len(text.splitlines()) == matrix['reference_line_count']
        families,options = reference_inventory(text)
        assert families == mapped
        assert options == {f['original_option'] for f in features if f['id'].startswith('cli.')}
    return dict(features=len(features),report_contexts=len(mapped),cards=len(declared_cards))


def markdown_table(matrix):
    lines = ['<!-- compatibility-matrix:start -->',
        '| Feature / explicit comparison scope | F | S | N | V | C | Evidence |',
        '| --- | --- | --- | --- | --- | --- | --- |']
    for f in matrix['features']:
        values = ' | '.join(CODES[f['dimensions'][axis]] for axis in AXES)
        evidence = ', '.join(f['evidence'])
        lines.append(f"| `{f['id']}` · {f['label']} | {values} | {evidence} |")
    lines.append('<!-- compatibility-matrix:end -->')
    return '\n'.join(lines)


def check_gallery_artifacts(directory, measurements):
    """Retained figures are historical original-R executions, never fresh runs."""
    records = []
    captures = {}
    for case in measurements['cases']:
        path = directory/case['case']/'capture.json'
        assert digest(path) == case['capture_sha256']
        captures[case['case']] = read(path)
        stamp = read(path.with_name('stamp.json'))
        assert stamp['inputs'] == case['input_sha256'] and stamp['cutoff'] == case['qscore_cutoff']
        assert stamp['source_sha256'] == REFERENCE_SHA
        for file,expected in stamp['native_sources'].items():
            if digest(ROOT/file) != expected:
                # Prior test-only additions do not invalidate production plots.
                assert file == 'src/trends.rs', 'Historical native source changed: '+file
                original = subprocess.check_output(['git','show','49fcc45:'+file],text=True)
                assert hashlib.sha256(original.encode()).hexdigest() == expected
                marker = '\n#[cfg(test)]\nmod tests {'
                assert original.split(marker)[0] == (ROOT/file).read_text().split(marker)[0]
    for row in measurements['plots']:
        case = directory/row['case']
        scope = 'combinedQC' if row['scope']=='combined' else row['scope']
        reference = case/'minionqc'/scope/row['reference_plot']
        native = case/'native'/row['scope']/'figures'/row['native_figure']
        report = case/'native'/row['scope']/'report.json'
        assert reference.is_file() and native.is_file() and report.is_file()
        records.append(dict(case=row['case'],scope=row['scope'],family=row['family'],
            original_figure_sha256=digest(reference),native_figure_sha256=digest(native),
            native_report_sha256=digest(report),card_sha256=digest(GALLERY/row['card']) if row['card'] else None))
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream',type=Path)
    parser.add_argument('--paired',type=Path,help='Fresh original comparison.json; strict failure stays a failure')
    parser.add_argument('--gallery-artifacts',type=Path,help='Retained plot_audit directory; not a fresh R execution')
    parser.add_argument('--render-matrix',action='store_true',help='Update only the marked comparison table')
    parser.add_argument('--output',type=Path,help='Write bounded evidence guard results, not a new reference target')
    args = parser.parse_args()
    measurements = read(GALLERY/'measurements.json')
    matrix = read(REGISTER)['compatibility']
    result = dict(audit_guard='passed',strict_parity='not_executed_by_this_guard',**validate(upstream=args.upstream))
    if args.paired:
        report = read(args.paired)
        verify(report,read(Path(__file__).with_name('revalidation-summary.json')))
        failures = [(c['name'],r['metric']) for c in report['cases'] for r in c['checks'] if r['status']=='suspected_poregauge_defect']
        result['strict_parity'] = 'failed' if failures else 'no_primary_failures'
        result['strict'] = dict(comparison_sha256=digest(args.paired),completed_R_cases=len(report['cases']),
            status_counts=report['status_counts'],failed_identity_sha256=hashlib.sha256(json.dumps(failures,separators=(',',':')).encode()).hexdigest(),
            failure_count=len(failures),regression_guard='all original check signatures reproduced')
    if args.gallery_artifacts:
        result['retained_original_gallery'] = check_gallery_artifacts(args.gallery_artifacts,measurements)
    text = (ROOT/'docs/minionqc-comparison.md').read_text()
    table = markdown_table(matrix)
    if args.render_matrix:
        updated,count = re.subn(r'<!-- compatibility-matrix:start -->.*?<!-- compatibility-matrix:end -->',lambda _:table,text,flags=re.S)
        assert count == 1, 'Missing matrix markers'
        (ROOT/'docs/minionqc-comparison.md').write_text(updated)
    else:
        assert table in text, 'Stale human-readable matrix; use --render-matrix'
    if args.output:
        args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k!='retained_original_gallery'}))


if __name__=='__main__':
    main()
