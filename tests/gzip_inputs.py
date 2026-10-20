#!/usr/bin/env python3
"""Optional streaming gzip equivalence/resource measurement; Python stdlib only.

Generates synthetic input, never private data. Not part of routine CI.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import time

HEADER = b'run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\tnum_events_template\n'


def snapshot(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'execution.log'}


def execute(binary, source, output, options, passes, report_relative='report.json'):
    output.mkdir(parents=True)
    scratch = output / 'scratch'
    scratch.mkdir()
    done = threading.Event()
    peak = [0]
    def sample():
        while not done.is_set():
            total = 0
            for p in output.rglob('*'):
                if p.suffix == '.tmp' or scratch in p.parents:
                    try:
                        if p.is_file(): total += p.stat().st_size
                    except FileNotFoundError: pass
            peak[0] = max(peak[0], total)
            done.wait(.01)
    monitor = threading.Thread(target=sample)
    monitor.start()
    begin = time.perf_counter()
    try:
        with (output/'execution.log').open('wb') as log:
            child = subprocess.Popen([str(binary), 'analyze', str(source), *options, '-o', str(output)],
                                     stdout=log, stderr=log,
                                     env=dict(os.environ, TMPDIR=str(scratch.resolve())))
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
        elapsed = time.perf_counter() - begin
        assert child.returncode == 0, (output/'execution.log').read_text()
    finally:
        done.set()
        monitor.join()
    assert not list(scratch.iterdir()), 'Leaked scratch files'
    scratch.rmdir()
    assert not list(output.rglob('*.tmp')), 'Leaked staged output'
    report = json.loads((output/report_relative).read_text())
    input_size = sum(p.stat().st_size for p in source.rglob('*') if p.is_file()) if source.is_dir() else source.stat().st_size
    return dict(input_bytes=input_size, wall_seconds=elapsed,
                user_seconds=usage.ru_utime, system_seconds=usage.ru_stime,
                peak_rss_bytes=usage.ru_maxrss*(1 if os.uname().sysname == 'Darwin' else 1024),
                records=report['metrics']['total_records'], input_passes=passes,
                decompression_passes=passes if source.suffix == '.gz' else 0,
                input_pass_accounting='code paths, not OS read tracing',
                peak_sampled_temporary_bytes=peak[0], temporary_sampling_seconds=.01,
                output_bytes=sum(p.stat().st_size for p in output.rglob('*')
                                 if p.is_file() and p.name != 'execution.log'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/ontrunqc'))
    parser.add_argument('--records', type=int, default=1_000_000)
    parser.add_argument('--output', type=Path, default=Path('benchmark-results/gzip/measurement'))
    args = parser.parse_args()
    assert args.records > 0
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=True)
    plain = root/'synthetic.tsv'
    # A bounded 10,000-row pattern makes generation reproducible and streaming.
    rows = [f'fictional-gzip\t{i%512+1}\t{i%5+1}\t{i*17}\t1\t{"TRUE" if i%3 else "FALSE"}\t{i*37+1}\t{6+i%17/3}\tfictional\t{(i*37+1)*2}\n'.encode()
            for i in range(10_000)]
    with plain.open('wb') as stream:
        stream.write(HEADER)
        for start in range(0, args.records, len(rows)):
            stream.write(b''.join(rows[:min(len(rows), args.records-start)]))
    compressed = plain.with_suffix('.tsv.gz')
    # Independent standard-library zlib encoder, also checks cross-encoder input.
    with compressed.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as encoded, plain.open('rb') as source:
        while block := source.read(1024*1024): encoded.write(block)
    results = []
    for name, options, passes in [
        ('single-pass', [], 1),
        ('fits-and-verification', ['--minionqc-trends', '--minionqc-density', '--verify-input-content'], 3),
        ('dense-minion-and-verification', ['--flowcell-layout', 'minion-r9.5', '--verify-input-content'], 3),
    ]:
        expected = None
        for kind, source in [('plain', plain), ('gzip', compressed)]:
            out = root/name/kind
            measured = execute(args.binary.resolve(), source, out, options, passes)
            assert measured['records'] == args.records
            content = snapshot(out)
            if expected is None: expected = content
            else: assert content == expected, f'{name}: output changed with compression'
            results.append(dict(case=name, compression=kind, **measured))
            print(name, kind, measured, flush=True)
    report = dict(binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  note='Single executions, uncontrolled cache/load. Temporary peaks sampled every 10 ms are lower bounds and include staged reports. No full TSV extraction; additional passes decompress again.',
                  results=results)
    (root/'measurements.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Every JSON/HTML output is byte-identical between plain and Python-generated gzip.')


if __name__ == '__main__': main()
