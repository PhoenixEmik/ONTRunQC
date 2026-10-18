#!/usr/bin/env python3
"""Summarize strict paired checks and separately controlled R experiments.

Never relabel a strict failure as a pass. Inputs are ignored validation artifacts,
not raw reads. Usage: assess_fits.py BEFORE AFTER CONTROLLED SENSITIVITY OUTPUT
"""
from collections import Counter
import hashlib
import json
import math
import os
from pathlib import Path
import sys
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def counts(rows):
    return dict(Counter(c['status'] for c in rows))


def field(metric):
    return metric.rsplit('.', 1)[-1]


def fit_failures(case):
    return [c for c in case['checks'] if c['metric'].startswith('fit.')
            and c['status'] == 'suspected_poregauge_defect']


def canonical_penalties(directory, case):
    """Algebraic reconstruction from captured R eigenpairs, not a new R target."""
    reference=json.loads((directory/case/'capture.json').read_text())
    probe=json.loads((directory/case/'source-population.json').read_text())
    sources={s['source_id']:s for s in probe['reference_sources']}
    rows=[]
    for plot in reference['plots']:
        family=plot['file'].rsplit('.',1)[0]
        if family not in ('length_by_hour','q_by_hour'):
            continue
        combined=plot['directory']=='combinedQC'
        panels={str(p['PANEL']):p for p in plot['panels']}
        legends=dict(zip(plot['colours'],plot['colour_labels']))
        predictions=iter(plot['smooth_predictions'])
        for layer in plot['layers']:
            if layer['stat']!='StatSmooth':
                continue
            groups={}
            for r in layer['data']:
                groups.setdefault((str(r['PANEL']),r['group']),[]).append(r)
            for (panel,_),group in groups.items():
                r=next(predictions,None)
                if not r or r['method']!='gam':
                    continue
                label=legends[group[0]['colour']]
                source=label if combined else plot['directory']
                selected=(panels[panel]['Q_cutoff'] if combined else label)!='All reads'
                quality=family=='q_by_hour'
                single=not combined and not selected and quality
                d=next(a for a in sources[source]['gam_audit'] if a['quality']==quality
                       and a['population'].startswith('qscore')==selected and a['single']==single)
                k=r['knots'];ra=r['audit']
                linear=[x-k[0] for x in k]
                norm=math.sqrt(sum(x*x for x in linear));linear=[x/norm for x in linear]
                projection=sum(linear);constant=[1-projection*x for x in linear]
                norm=math.sqrt(sum(x*x for x in constant));constant=[x/norm for x in constant]
                eig=ra['raw_eigenvalues'];vectors=ra['raw_eigenvectors']
                penalty=[[sum(vectors[i][t]*eig[t]*vectors[j][t] for t in range(8))/eig[0]
                          +eig[7]/eig[0]*(.1*linear[i]*linear[j]+.01*constant[i]*constant[j])
                          for j in range(10)] for i in range(10)]
                expected=d['basis']['normalized_shrink_penalty']
                errors=[abs(a-b) for ar,br in zip(penalty,expected) for a,b in zip(ar,br)]
                failed=sum(abs(a-b)>1e-12+1e-10*abs(a) for ar,br in zip(penalty,expected) for a,b in zip(ar,br))
                rows.append(dict(case=case,context=plot['directory'],source=source,
                                 family=family,selected=selected,cells=100,
                                 maximum_error=max(errors),failed_cells=failed))
    return rows


def backend_evidence(environment):
    """Inspect only the existing native installation; never install/swap BLAS."""
    library=Path(environment.get('blas',''))
    if not library.is_file():
        return dict(status='unverified',reason='Native BLAS path unavailable on this host.')
    prefix=library.parent.parent
    paths=[library,prefix/'bin/Rscript',prefix/'lib/R/lib/libR.dylib',
           prefix/'lib/R/library/stats/libs/stats.so',prefix/'lib/R/library/mgcv/libs/mgcv.so']
    aliases={p.name:str(p.resolve()) for p in library.parent.glob('libopenblas*.dylib')}
    probes=[]
    for requested in ('','ARMV8','VORTEX'):
        code=('import ctypes,json;lib=ctypes.CDLL('+repr(str(library))+');'
              'lib.openblas_get_corename.restype=ctypes.c_char_p;'
              'lib.openblas_get_config.restype=ctypes.c_char_p;'
              'print(json.dumps(dict(core=lib.openblas_get_corename().decode(),'
              'config=lib.openblas_get_config().decode())))')
        result=subprocess.run([sys.executable,'-c',code],env=dict(os.environ,OPENBLAS_CORETYPE=requested),
                              capture_output=True,text=True)
        probes.append(dict(requested_core=requested or 'default',returncode=result.returncode,
                           result=json.loads(result.stdout) if result.returncode==0 else None))
    return dict(scope='Existing native pinned installation; not a system-wide backend search.',
                binary_sha256={str(p):digest(p) for p in paths if p.is_file()},
                library_aliases=aliases,core_requests=probes,
                independent_second_environment='not_verified; alias/core requests do not establish a distinct backend')


def summarize(before_path, after_path, controlled_path, sensitivity_path):
    before, after, controlled, sensitivity = [json.loads(p.read_text()) for p in
        (before_path, after_path, controlled_path, sensitivity_path)]
    previous = {c['name']: c for c in before['cases']}
    nonfit_equal = 0
    cases = []
    for c in after['cases']:
        old = previous[c['name']]
        nonfit = lambda case: [r for r in case['checks'] if not r['metric'].startswith('fit.')]
        assert nonfit(c) == nonfit(old), 'Non-fit regression in ' + c['name']
        nonfit_equal += len(nonfit(c))
        old_rows = {r['metric']: r for r in old['checks']}
        for row in c['checks']:
            old_row = old_rows.get(row['metric'])
            if old_row is not None:
                assert old_row['tolerance'] == row['tolerance'], 'Changed tolerance: ' + row['metric']
        maxima = {}
        for population, case in [('before', old), ('after', c)]:
            maxima[population] = {}
            for family in ('length_by_hour', 'q_by_hour'):
                maxima[population][family] = {
                    f: max((r['absolute_difference'] for r in case['checks']
                            if '.' + family + '.' in r['metric'] and field(r['metric']) == f
                            and isinstance(r.get('absolute_difference'),(float,int))), default=None)
                    for f in ('fitted', 'standard_error', 'lower', 'upper')}
        cases.append(dict(name=c['name'],reference_completed=c['reference_completed'],
            reference_capture_sha256=c['reference_capture_sha256'],
            status_counts=counts(c['checks']),
            failed_fields=dict(Counter(field(r['metric']) for r in fit_failures(c))),
            maximum_errors=maxima,
            gam_diagnostic_counts=counts(c['gam_diagnostics']),
            loess_diagnostic_counts=counts(c.get('loess_diagnostics', []))))

    models = []
    for m in controlled['models']:
        errors = m['errors']
        variants = {}
        for name, values in m['variants'].items():
            variants[name] = {f: values[f] for f in
                ('sp', 'penalty_scale', 'reml', 'edf', 'variance')}
            variants[name]['endpoint'] = values['fitted'][-1]
            variants[name]['errors'] = errors[name]
        models.append(dict(case=m['case'],source=m['source'],selected=m['selected'],
            quality=m['quality'],observations=m['observations'],input_sha256=m['input_sha256'],
            repeat_identical=m['repeat_identical'],repeat_maximum_error=m['repeat_maximum_error'],
            variants=variants))
    # Compare actual original-reference captures between independent full runs.
    # Added LOESS audit metadata is deliberately excluded from this GAM check.
    repeats = []
    for c in after['cases']:
        name = c['name']
        paths = [p.parent/name/'capture.json' for p in (before_path, after_path)]
        a, b = [json.loads(p.read_text()) for p in paths]
        def gam_models(capture):
            return [r for p in capture['plots'] for r in p.get('smooth_predictions', [])
                    if r.get('method') == 'gam']
        a, b = gam_models(a), gam_models(b)
        if not a and not b:
            continue
        assert len(a) == len(b)
        differences = Counter()
        for ar, br in zip(a, b):
            for f in ('knots', 'smoothing_parameter', 'residual_variance', 'data', 'audit'):
                if ar[f] != br[f]:
                    differences[f] += 1
        repeats.append(dict(case=name,models=len(a),identical=not differences,
            changed_fields=dict(differences),before_capture_sha256=digest(paths[0]),
            after_capture_sha256=digest(paths[1])))

    failures = [r for c in after['cases'] for r in fit_failures(c)]
    diagnostics = [r for c in after['cases'] for r in c['gam_diagnostics']]
    loess = [r for c in after['cases'] for r in c.get('loess_diagnostics', [])]
    penalties=[r for c in after['cases'] for r in canonical_penalties(after_path.parent,c['name'])]
    return dict(
        status='Partial',strict_comparison_preserved=True,
        reference_commit=after['reference_commit'],reference_source_sha256=after['source_sha256'],
        validation_driver_sha256={p.name:digest(p) for p in
            [Path(__file__).resolve(),Path(__file__).with_name('capture.R'),
             Path(__file__).with_name('gam_assessment.R'),Path(__file__).with_name('loess_reference.R'),
             Path(__file__).resolve().parents[1]/'minionqc_differential.py']},
        reference_environment=after['reference_environment'],
        poregauge_binary_sha256=after['poregauge_binary_sha256'],
        artifacts={str(p): digest(p) for p in (before_path, after_path, controlled_path, sensitivity_path)},
        completed_cases=sum(c['reference_completed'] for c in after['cases']),
        suite_exit_code=1 if after['status_counts'].get('suspected_poregauge_defect') else
            2 if after['status_counts'].get('not_yet_verified') else 0,
        before_status_counts=before['status_counts'],status_counts=after['status_counts'],
        unchanged_non_fit_checks=nonfit_equal,
        primary_failed_fields=dict(Counter(field(r['metric']) for r in failures)),
        primary_failure_contexts=dict(Counter(c['name']+' | '+r['metric'].rsplit('.',2)[0]
                                             for c in after['cases'] for r in fit_failures(c))),
        gam_diagnostic_counts=counts(diagnostics),
        gam_failed_quantities=dict(Counter(r['metric'].split('.')[5] for r in diagnostics
                                         if r['status']=='suspected_poregauge_defect')),
        loess_diagnostic_counts=counts(loess),
        original_reference_repeat_runs=repeats,
        controlled_environment=controlled['environment'],controlled_driver_sha256=controlled['driver_sha256'],
        backend_evidence=backend_evidence(controlled['environment']),
        controlled_models=models,cs_sensitivity=sensitivity,cases=cases,
        canonical_penalty_reconstruction=penalties,
        compatibility_policy='Exact core QC semantics; strict pinned-R numerical checks retained. '
            'A separately named deterministic cs null-space convention is supported for display. '
            'Controlled canonical/swapped fits explain discrepancies, not replacements for original-reference targets. '
            'No cross-BLAS equivalence claimed without an independent measured backend.')


if __name__ == '__main__':
    if len(sys.argv) != 6:
        raise SystemExit(__doc__)
    paths = list(map(Path, sys.argv[1:]))
    result = summarize(*paths[:4])
    paths[4].write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
