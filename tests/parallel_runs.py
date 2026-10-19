#!/usr/bin/env python3
"""Optional #16 determinism/resource check. Synthetic/public data; no R needed."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import time

REVISION = '54485d586038d8ee36f168dd94f31c423f52f581'
HEADER = 'run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\n'


def fixture(path, run, count):
    with path.open('w') as stream:
        stream.write(HEADER)
        for i in range(count):
            stream.write(f'{run}\t{i%512+1}\t{i%5+1}\t{i%80003}\t1\t{"TRUE" if i%3 else "FALSE"}\t{(i*37)%100003+1}\t{6+(i%17)/3}\tfictional\n')


def snapshot(root):
    # PDF resource order is not byte-deterministic (#15); filenames, other
    # content including scientific JSON/HTML and manifests must match exactly.
    return {str(p.relative_to(root)): None if p.suffix == '.pdf' else hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'execution.log'}


def execute(binary, inputs, output, options, workers):
    output.mkdir(parents=True)
    scratch = output / 'scratch'
    scratch.mkdir()
    done = threading.Event()
    peak = [0]
    def sample():
        while not done.is_set():
            total = 0
            for p in output.rglob('*'):
                if p.is_file() and (p.suffix == '.tmp' or scratch in p.parents):
                    try: total += p.stat().st_size
                    except FileNotFoundError: pass
            peak[0] = max(peak[0], total)
            done.wait(.01)
    monitor = threading.Thread(target=sample)
    monitor.start()
    start = time.perf_counter()
    try:
        with (output/'execution.log').open('wb') as log:
            env = dict(os.environ, TMPDIR=str(scratch.resolve()))
            child = subprocess.Popen([str(binary), 'analyze', *map(str, inputs), *options,
                                      '--processors', str(workers), '-o', str(output)], stdout=log, stderr=log, env=env)
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
        elapsed = time.perf_counter()-start
        assert child.returncode == 0, (output/'execution.log').read_text()
    finally:
        done.set()
        monitor.join()
    assert not list(scratch.iterdir()), 'Leaked scratch files'
    scratch.rmdir()
    assert not list(output.rglob('*.tmp')), 'Leaked staged output'
    reports = [json.loads(p.read_text()) for p in output.rglob('report.json')]
    dense_passes = sum(r['diagnostics']['minion_read_visualization']['additional_input_passes']
                       for r in reports if r['diagnostics'].get('minion_read_visualization'))
    # Exact counts from execution paths for these cases: each dense run has a
    # distinct source file. This is algorithmic accounting, not OS I/O tracing.
    passes = len(inputs)*(1 + (workers > 1 and len(inputs)>1) + ('--minionqc-trends' in options)) + dense_passes
    return dict(wall_seconds=elapsed, user_seconds=usage.ru_utime, system_seconds=usage.ru_stime,
                peak_rss_bytes=usage.ru_maxrss*(1 if os.uname().sysname=='Darwin' else 1024),
                peak_sampled_temporary_bytes=peak[0], temporary_sampling_seconds=.01,
                input_file_passes=passes, input_pass_accounting='code paths, not OS tracing',
                output_bytes=sum(p.stat().st_size for p in output.rglob('*') if p.is_file() and p.name!='execution.log'),
                execution_log=(output/'execution.log').read_text())


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=Path('target/release/poregauge'))
    parser.add_argument('--baseline',type=Path)
    parser.add_argument('--upstream',type=Path)
    parser.add_argument('--output',type=Path,default=Path('benchmark-results/parallel-runs'))
    parser.add_argument('--records',type=int,default=300000)
    args=parser.parse_args()
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=True)
    inputs=root/'inputs';inputs.mkdir(exist_ok=True)
    cases=[]
    for name,count,options in [
        ('large',args.records,['--combined-only']),
        ('dense',4100,['--flowcell-layout','minion-r9.5','--minionqc-trends','--minionqc-density','--format','svg']),
        ('exports',20,['--smallfigures','--yaml','--add-stat'])]:
        files=[]
        for i in range(2):
            path=inputs/f'{name}-{i}.tsv';fixture(path,f'fictional-{i}',count);files.append(path)
        if name=='exports':
            for fmt in ['png','pdf','ps','jpeg','tiff','bmp']:
                cases.append((f'export-{fmt}',files,options+['--format',fmt]))
        else: cases.append((name,files,options))
    if args.upstream:
        upstream=args.upstream.resolve()
        assert subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'],text=True).strip()==REVISION
        cases.append(('public-minion',sorted((upstream/'example_input_minion').glob('*/sequencing_summary.txt')),['--combined-only']))
    results=[]
    for name,paths,options in cases:
        previous=None
        variants=[('one',args.binary,1),('two',args.binary,2)]
        if args.baseline: variants.insert(0,('before',args.baseline,1))
        for label,binary,workers in variants:
            destination=root/name/label
            measurement=execute(binary.resolve(),paths,destination,options,workers)
            content=snapshot(destination)
            if previous is not None: assert content==previous, f'{name}/{label} content changed'
            previous=content
            result=dict(case=name,variant=label,workers=workers,input_bytes=sum(p.stat().st_size for p in paths),**measurement)
            results.append(result)
            (root/'measurements.json').write_text(json.dumps(dict(reference_commit=REVISION,results=results),indent=2)+'\n')
            print(name,label,measurement,flush=True)
    print('All scientific JSON, HTML, non-PDF figures, filenames/manifests match; PDF bytes are intentionally excluded.')


if __name__=='__main__':main()
