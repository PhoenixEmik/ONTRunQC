#!/usr/bin/env python3
"""Bounded evidence from isolated GAM experiments; never changes strict targets.

Usage: assess_stability.py EXPERIMENT_DIRECTORY OUTPUT.json
Large matrices/captures stay in the ignored directory. This summary deliberately
keeps strict failures separate from controlled penalty-alignment experiments.
"""
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import platform
import shutil
import subprocess
import sys

from check_revalidation import verify

FIELDS = ('fitted', 'standard_error', 'lower', 'upper')


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def flat(value):
    if isinstance(value, list):
        return [x for row in value for x in flat(row)]
    return [value]


def maximum(a, b):
    a, b = flat(a), flat(b)
    assert len(a) == len(b) and a, 'Incompatible numerical coordinates'
    assert all(math.isfinite(x) for x in a + b), 'Nonfinite comparison'
    return max(abs(x-y) for x, y in zip(a, b))


def curves(a, b):
    return {f: maximum(a[f], b[f]) for f in FIELDS}


def signatures(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True,
                                     separators=(',', ':')).encode()).hexdigest()


def loaded_libraries(path):
    images, symbols = [], {}
    for line in path.read_text().splitlines():
        row = line.split('\t')
        if row[0] == 'image' and len(row) == 2:
            images.append(row[1])
        elif row[0] == 'symbol' and len(row) == 3:
            binary = Path(row[2]).resolve(strict=True)
            symbols[row[1]] = dict(path=str(binary), sha256=sha(binary))
    assert set(symbols) == {'dgemm_', 'ddot_', 'dsyevr_', 'dsyevd_', 'dpotrf_'}
    relevant = {}
    for image in images:
        p = Path(image)
        if p.is_file() and any(s in image for s in ('lib/R/bin/exec/R','libR.',
                '/stats/libs/','/mgcv/libs/','libgfortran','libopenblas','libblas.','liblapack.','backend_probe')):
            relevant[str(p.resolve())] = sha(p)
    configuration = None
    blas = Path(symbols['dgemm_']['path'])
    if 'openblas' in blas.name:
        code = ('import ctypes,json;lib=ctypes.CDLL('+repr(str(blas))+');'
                'lib.openblas_get_config.restype=ctypes.c_char_p;'
                'lib.openblas_get_corename.restype=ctypes.c_char_p;'
                'print(json.dumps(dict(config=lib.openblas_get_config().decode(),'
                'core=lib.openblas_get_corename().decode())))')
        configuration = json.loads(subprocess.check_output([sys.executable,'-c',code],text=True))
    return dict(image_inventory_sha256=sha(path), image_count=len(images),
                openblas_configuration=configuration,
                relevant_binary_sha256=relevant, symbols=symbols)


def independent(a, b):
    # Different path strings or runtime core names are insufficient.
    return all(a['symbols'][s]['sha256'] != b['symbols'][s]['sha256']
               for s in ('dgemm_', 'dsyevr_'))


def matrix_delta(a, b):
    return {key: maximum(a[key], b[key]) for key in
            ('raw_penalty', 'null_projector', 'normalized_shrink_penalty',
             'null_weight_in_geometric_basis')}


def transpose(a):
    return list(map(list, zip(*a)))


def multiply(a, b):
    return [[sum(x*y for x,y in zip(row, col)) for col in zip(*b)] for row in a]


def inverse(a):
    n = len(a)
    rows = [list(row)+[float(i==j) for j in range(n)] for i,row in enumerate(a)]
    for j in range(n):
        pivot = max(range(j,n), key=lambda i: abs(rows[i][j]))
        rows[j],rows[pivot] = rows[pivot],rows[j]
        divisor = rows[j][j]
        assert divisor != 0
        rows[j] = [v/divisor for v in rows[j]]
        for i in range(n):
            if i != j:
                factor = rows[i][j]
                rows[i] = [v-factor*w for v,w in zip(rows[i],rows[j])]
    return [row[n:] for row in rows]


def common_penalties(model):
    """Compare penalties in nodal coordinates, never arbitrary centered bases.

    gam_audit.shrink_penalty always reconstructs original cs, even for a
    controlled model. Use the actual model's effective penalty instead.
    """
    native = model['rust_audit']
    if native is None:
        return None
    audit = model['variants']['canonical']['audit']
    scale = model['variants']['canonical']['penalty_scale']/audit['raw_eigenvalues'][0]
    r_penalty = [[v*scale for v in row] for row in audit['effective_nodal_penalty']]
    q = native['fit']['transform']
    # Rust's active nine smooth coordinates precede an unused column; R has
    # its intercept first. Embed the same centered quadratic form explicitly.
    transform = [[1.0]+row[1:] for row in q]
    p = native['fit']['centered_penalty']
    penalty = [[0.0]*10]+[[0.0]+row[:9] for row in p[:9]]
    t = inverse(transform)
    native_penalty = multiply(transpose(t), multiply(penalty,t))
    return maximum(r_penalty,native_penalty)


def strict_summary(report):
    failed = lambda rows: [r for r in rows if r['status'] == 'suspected_poregauge_defect']
    primary = [r for c in report['cases'] for r in failed(c['checks'])]
    assert all(c['reference_completed'] and c['reference_returncode']==0 for c in report['cases'])
    return dict(
        result='strict parity failed' if primary else 'no primary failures',
        completed_cases=sum(c['reference_completed'] for c in report['cases']),
        status_counts=report['status_counts'],
        failed_fields=dict(Counter(r['metric'].rsplit('.', 1)[-1] for r in primary)),
        intermediate_failures=sum(len(failed(c['gam_diagnostics'])) for c in report['cases']),
        cases=[dict(name=c['name'], reference_capture_sha256=c['reference_capture_sha256'],
                    failures=len(failed(c['checks'])),
                    maximum_errors={f: max((r['absolute_difference'] for r in c['checks']
                        if r['metric'].startswith('fit.') and r['metric'].endswith('.'+f)
                        and r.get('absolute_difference') is not None), default=0)
                        for f in FIELDS}) for c in report['cases']])


def controlled_summary(report):
    models = report['models']
    assert len(models) == 20 and all(m['repeat_identical'] for m in models)
    variants = {}
    for v in ('baseline', 'swapped', 'canonical', 'identical_penalty', 'fixed_lambda'):
        variants[v] = dict(
            curves={f: dict(failed_points=sum(m['errors'][v][f]['failed_points'] for m in models),
                             maximum_error=max(m['errors'][v][f]['maximum_error'] for m in models))
                    for f in FIELDS},
            nodal_coefficient_failures=sum(m['errors'][v]['nodal_coefficient_failures'] for m in models),
            nodal_covariance_failures=sum(m['errors'][v]['nodal_covariance_failures'] for m in models),
            maximum_log_lambda_difference=max(abs(m['errors'][v]['log_lambda_difference']) for m in models))
    for v in ('canonical','identical_penalty','fixed_lambda'):
        result = variants[v]
        assert not any(row['failed_points'] for row in result['curves'].values()), 'Unexplained aligned-penalty curve discrepancy'
        assert result['nodal_coefficient_failures'] == result['nodal_covariance_failures'] == 0, 'Unexplained aligned-penalty coefficient/covariance discrepancy'
    return dict(models=20, variants=variants,
        affected_minion=[dict(quality=m['quality'], rust={k:v for k,v in m['rust'].items() if k!='points'},
            variants={v: dict(errors=m['errors'][v], reml=m['variants'][v]['reml'],
                edf=m['variants'][v]['edf'], variance=m['variants'][v]['variance'])
                for v in variants}) for m in models if m['case']=='minion' and m['selected']])


def stability_summary(report, repeat):
    assert report['driver_sha256'] == repeat['driver_sha256']
    assert report['frozen_sha256'] == repeat['frozen_sha256']
    same = all(report[k] == repeat[k] for k in ('models', 'perturbations'))
    assert same, 'Fresh-process repeat changed; investigate before assessing stability'
    models = []
    for name, m in report['models'].items():
        assert m['repeat_identical'] and m['repeat_prediction_error'] == 0
        matrix = m['matrix']
        models.append(dict(name=name, observations=m['observations'], input_sha256=m['input_sha256'],
            knots=m['knots'], eigenvalues=matrix['eigenvalues'],
            numerical_null_rank=matrix['numerical_null_rank'], positive_null_gap=matrix['positive_null_gap'],
            null_residual=matrix['null_residual'], projector_error=matrix['projector_error'],
            geometric_null_weights=matrix['null_weight_in_geometric_basis'],
            native_raw_penalty_error=maximum(matrix['raw_penalty'], m['rust_audit']['basis']['raw_penalty'])
                if m['rust_audit'] else None,
            native_canonical_effective_nodal_penalty_error=common_penalties(m),
            reversed_shared_errors=curves(m['variants']['shared'], m['reverse_shared'])))
    sweeps = []
    for name in sorted({p['case'] for p in report['perturbations'].values()}):
        baseline = report['models'][name]
        rows = [p for p in report['perturbations'].values() if p['case'] == name]
        knots = [p for p in rows if 'fit' in p]
        matrices = [p for p in rows if 'fit' not in p]
        worst = max(knots, key=lambda p: maximum(p['fit']['fitted'], baseline['variants']['original']['fitted']))
        sweeps.append(dict(case=name, knot_experiments=len(knots), matrix_experiments=len(matrices),
            maximum_projector_error=max(p['matrix']['projector_error'] for p in rows),
            maximum_null_residual=max(p['matrix']['null_residual'] for p in rows),
            maximum_curve_change={f: max(maximum(p['fit'][f], baseline['variants']['original'][f])
                                        for p in knots) for f in FIELDS},
            largest_prediction_change=dict(knot_index=worst['knot_index'], ulp_steps=worst['ulp_steps'],
                maximum_knot_delta=worst['maximum_knot_delta'],
                matrix_changes=matrix_delta(baseline['matrix'], worst['matrix']),
                original_weights=baseline['matrix']['null_weight_in_geometric_basis'],
                perturbed_weights=worst['matrix']['null_weight_in_geometric_basis']),
            maximum_parameter_changes={f: max(abs(p['fit'][f]-baseline['variants']['original'][f])
                                               for p in knots) for f in ('sp','reml','edf','variance')}))
    return dict(fresh_process_repeat_identical=same,
        model_payload_sha256=signatures(report['models']),
        perturbation_payload_sha256=signatures(report['perturbations']),
        frozen_penalties_sha256=report['frozen_sha256'], driver_sha256=report['driver_sha256'],
        environment=report['environment'], models=models, perturbations=sweeps)


def summarize(root):
    manifest = load(root/'baseline-manifest.json')
    current = load(root/'paired-verified/comparison.json')
    frozen = load(Path('tests/reference/revalidation-summary.json'))
    verify(current, frozen)
    binary = Path('target/release/ontrunqc')
    assert sha(binary) == current['poregauge_binary_sha256'], 'Native release binary changed after differential execution'
    if (root/'baseline/comparison.json').exists():
        baseline = load(root/'baseline/comparison.json')
        assert all(a[k] == b[k] for a,b in zip(current['cases'], baseline['cases'])
                   for k in ('checks','gam_diagnostics','loess_diagnostics'))
    # Only cfg(test) additions are permitted in the production solver source.
    marker = '\n#[cfg(test)]\nmod tests {'
    old = (root/'baseline/src/trends.rs').read_text().split(marker)[0]
    new = Path('src/trends.rs').read_text().split(marker)[0]
    assert old == new, 'Production solver changed during assessment'
    immutable = [p for p in manifest['protected_sha256'] if p.startswith('tests/')
                 or p=='src/trend_knots.rs']
    assert all(sha(Path(p)) == manifest['protected_sha256'][p] for p in immutable)
    backends = {}
    documents = {}
    for name, filename in [('openblas','stability-openblas-final.json'), ('netlib','stability-netlib.json')]:
        report = load(root/filename)
        documents[name] = report
        repeat = load(root/f'stability-{name}-repeat.json')
        backends[name] = stability_summary(report, repeat)
        backends[name]['loaded_libraries'] = loaded_libraries(root/(filename+'.loaded.tsv'))
        backends[name]['strict_suite'] = strict_summary(load(root/('paired-verified' if name=='openblas' else 'netlib-paired')/'comparison.json'))
        backends[name]['controlled_models'] = controlled_summary(load(root/f'controlled-{name}.json'))
    assert independent(*(b['loaded_libraries'] for b in backends.values())), 'Backend aliases are not independent'
    secondary = load(root/'netlib-paired/comparison.json')
    assert [c['name'] for c in current['cases']] == [c['name'] for c in secondary['cases']]
    for key in ('reference_commit','source_sha256'):
        assert current[key] == secondary[key]
    assert current['poregauge_binary_sha256'] == secondary['poregauge_binary_sha256']
    for key,value in frozen['reference_versions'].items():
        assert secondary['reference_environment'][key] == value
    for a,b in zip(current['cases'],secondary['cases']):
        assert a['name'] == b['name']
        assert [(r['metric'],r['tolerance']) for r in a['checks']] == [(r['metric'],r['tolerance']) for r in b['checks']]
        failures = lambda c: {r['metric'] for r in c['checks'] if r['status']=='suspected_poregauge_defect'}
        assert failures(a) == failures(b), 'Experimental backend changed the primary failure set'
    cross = []
    a, b = documents.values()
    assert a['frozen_sha256'] == b['frozen_sha256']
    for name, x in a['models'].items():
        y = b['models'][name]
        assert x['knots'] == y['knots'] and x['observations'] == y['observations']
        assert x['matrix']['raw_penalty'] == y['matrix']['raw_penalty'], 'Backend raw penalty changed; isolate before attribution'
        assert x['rust_audit'] == y['rust_audit'], 'Native comparator changed between backends'
        cross.append(dict(name=name, matrices=matrix_delta(x['matrix'], y['matrix']),
            frozen_raw=matrix_delta(x['frozen_matrix'], y['frozen_matrix']),
            variants={v: dict(curves=curves(x['variants'][v], y['variants'][v]),
                parameters={f: abs(x['variants'][v][f]-y['variants'][v][f])
                            for f in ('sp','reml','edf','variance')},
                nodal_covariance=maximum(x['variants'][v]['nodal_covariance'],y['variants'][v]['nodal_covariance']))
                for v in ('original','canonical','shared')}))
    inputs = sorted([root/'baseline-manifest.json', root/'cs-sensitivity-openblas.json',
        *[root/f for f in ('stability-openblas-final.json','stability-netlib.json',
            'stability-openblas-repeat.json','stability-netlib-repeat.json',
            'controlled-openblas.json','controlled-netlib.json',
            'paired-verified/comparison.json','netlib-paired/comparison.json')]])
    packages = {}
    for name in ('blas','lapack'):
        info = root/f'netlib/{name}-info/info'
        if info.is_dir():
            packages[name] = dict(index=load(info/'index.json'),
                recipe_sha256={str(p.relative_to(info)):sha(p)
                    for p in sorted((info/'recipe').rglob('*')) if p.is_file()},
                build_metadata_note='Recipe specifies clang 17, gfortran 13, macOS SDK 11.0; '
                    'shared CMake build and -fno-optimize-sibling-calls. Complete expanded compiler flags not retained.')
    cpu = subprocess.run(['sysctl','-n','machdep.cpu.brand_string'],capture_output=True,text=True) if sys.platform=='darwin' else None
    return dict(assessment='Strict parity failed; no portable solver correction justified by these experiments.',
        native_baseline_commit=manifest['commit'], reference_commit=current['reference_commit'],
        native_binary_sha256=current['poregauge_binary_sha256'], native_binary_unchanged=True,
        reference_source_sha256=current['source_sha256'],
        tolerance=dict(absolute=1e-6,relative=1e-6),
        protected_numerical_artifacts_unchanged=True,
        production_solver_sha256=hashlib.sha256(new.encode()).hexdigest(),
        host=dict(platform=platform.platform(), architecture=platform.machine(),
                  cpu=cpu.stdout.strip() if cpu and cpu.returncode==0 else
                      (platform.processor() if not cpu else 'Not verified: '+cpu.stderr.strip())),
        provenance_sha256={str(p.relative_to(root)):sha(p) for p in inputs},
        netlib_package_metadata=packages,
        sensitivity=load(root/'cs-sensitivity-openblas.json'),
        independent_backend_verified=True, backends=backends, cross_backend=cross,
        limits=['One host/architecture; native Rust cross-platform execution not verified.',
                'Finite fixtures and perturbations cannot prove equivalence for every input.',
                'Canonical/shared penalties are diagnostic models, never replacement original-R targets.',
                'Backend observations do not justify globally widening the strict tolerance.'])


def snapshot(root, upstream):
    """Preserve pre-experiment sources; fail rather than overwrite an old run."""
    target = root/'baseline-manifest.json'
    assert not target.exists(), 'Use a new experimental directory'
    commit = subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    reference = subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'],text=True).strip()
    assert reference == '54485d586038d8ee36f168dd94f31c423f52f581'
    assert not subprocess.check_output(['git','-C',str(upstream),'status','--porcelain'])
    reference_sha = sha(upstream/'MinIONQC.R')
    assert reference_sha == '841253a28a785b9efdc09c855c163646882e01152e26eda509adae2732c6651d'
    files = ['src/trends.rs','src/trend_knots.rs','tests/minionqc_differential.py',
        'tests/minionqc_reference.py','docs/minionqc-parity.md','docs/minionqc-comparison.md','docs/metrics.md']
    files += ['tests/reference/'+f for f in ('capture.R','gam_assessment.R','cs_sensitivity.R',
        'assess_fits.py','check_revalidation.py','gam-golden.json','gam-canonical-golden.json',
        'validation-summary.json','revalidation-summary.json')]
    for file in files:
        destination = root/'baseline'/file
        destination.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(file,destination)
    target.write_text(json.dumps(dict(commit=commit,reference_commit=reference,
        reference_source_sha256=reference_sha,reference_clean=True,
        protected_sha256={f:sha(Path(f)) for f in files}),indent=2)+'\n')


if __name__ == '__main__':
    if sys.argv[1] == '--snapshot':
        snapshot(*map(Path,sys.argv[2:]))
    else:
        root, output = map(Path, sys.argv[1:])
        output.write_text(json.dumps(summarize(root), indent=2, allow_nan=False)+'\n')
        print('Assessment recorded. Original strict GAM failures remain failures.')
