#!/usr/bin/env python3
"""Optional #19 transport/pass measurement, reusing existing child accounting.

Input must be a stable single-run synthetic summary; no inputs are generated or
copied. Results stay outside Git. Run after builds/other validation have stopped.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gzip_inputs import execute


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input',type=Path)
    parser.add_argument('--gzip',type=Path)
    parser.add_argument('--binary',type=Path,default=Path('target/release/ontrunqc'))
    parser.add_argument('--output',type=Path,default=Path('benchmark-results/bin-performance'))
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    variants=[('native',args.input,[],1),
              ('reference',args.input,['--minionqc-plot-bins'],2),
              ('reference-sha',args.input,['--minionqc-plot-bins','--verify-input-content'],3)]
    if args.gzip: variants.append(('reference-gzip',args.gzip,['--minionqc-plot-bins'],2))
    baseline=None;results=[]
    for name,source,options,passes in variants:
        destination=args.output/name
        measurement=execute(args.binary.resolve(),source.resolve(),destination,options,passes)
        report=json.loads((destination/'report.json').read_text())
        report.pop('minionqc_plot_bins',None)
        if baseline is None: baseline=report
        else: assert report==baseline, 'Canonical report changed'
        results.append(dict(case=name,**measurement))
        print(name,measurement,flush=True)
    evidence=dict(system=platform.platform(),machine=platform.machine(),
        binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        note='One execution/configuration, uncontrolled caches/load; 10-ms temp samples are lower bounds. Extra passes are code-path accounting. All canonical JSON values match.',results=results)
    (args.output/'measurements.json').write_text(json.dumps(evidence,indent=2)+'\n')


if __name__=='__main__':main()
