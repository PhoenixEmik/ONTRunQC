#!/usr/bin/env python3
"""Optional #21 before/after resources on identical fictional plain/gzip runs.

Build the baseline from the recorded commit separately. No timing assertion in
CI. Reuses existing wait4/10-ms scratch accounting; includes report publication.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import platform
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gzip_inputs import execute, HEADER


def scientific(report):
    report['comparison']['yield_by_minimum_length'].pop('display',None)
    for run in report.get('collection',{}).get('runs',[]):
        run['yield_by_minimum_length'].pop('display',None)
    return report


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--before',type=Path,required=True)
    p.add_argument('--after',type=Path,default=Path('target/release/ontrunqc'))
    p.add_argument('--records',type=int,default=1_000_000)
    p.add_argument('--output',type=Path,default=Path('benchmark-results/yield-performance'))
    a=p.parse_args();assert a.records>0
    a.output.mkdir(parents=True)
    inputs=a.output/'inputs';inputs.mkdir()
    plain=inputs/'sequencing_summary_a.tsv';compressed=inputs/'sequencing_summary_b.tsv.gz'
    for source,path in [('fictional-a',plain),('fictional-b',compressed)]:
        # mtime=0 + empty gzip filename make compressed source bytes reproducible.
        with path.open('wb') as raw:
            stream=gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0) if path.suffix=='.gz' else raw
            stream.write(HEADER)
            for i in range(a.records):
                length=10_000_000 if i==0 else i*37%100_000+1
                stream.write(f'{source}\t{i%512+1}\t{i%5+1}\t{i*137%260000}\t1\tTRUE\t{length}\t{6+i%17/3}\tsynthetic\t{length*2}\n'.encode())
            if stream is not raw:stream.close()
    results=[];parallel=None
    for case,source,options,passes,relative in [
        ('plain-svg',plain,['--format','svg'],1,'report.json'),
        ('gzip',compressed,[],1,'report.json'),
        ('mixed-1',inputs,['--combined-only'],1,'combined/report.json'),
        ('mixed-2',inputs,['--combined-only','--processors','2'],2,'combined/report.json')]:
        expected=None
        for mode,binary in [('before',a.before),('after',a.after)]:
            destination=a.output/(case+'-'+mode)
            measured=execute(binary.resolve(),source.resolve(),destination,options,passes,relative)
            report=json.loads((destination/relative).read_text())
            if mode=='after':
                if case=='mixed-1':parallel=json.loads(json.dumps(report))
                if case=='mixed-2':assert report==parallel,'Worker order changed JSON'
                display=report['comparison']['yield_by_minimum_length']['display']
                measured['display']={k:{n:v for n,v in d.items() if n!='points'} | dict(vertices=len(d['points'])) for k,d in display.items() if isinstance(d,dict)}
            if source.is_dir():
                measured['passes_per_source']=passes
                measured['input_passes_total']=passes*2
                measured['decompression_passes']=passes
            model=scientific(report)
            if expected is None:expected=model
            else:assert model==expected,'Scientific JSON changed'
            measured['html_bytes']=(destination/relative).with_suffix('.html').stat().st_size
            measured['json_bytes']=(destination/relative).stat().st_size
            measured['svg_bytes']=sum(f.stat().st_size for f in destination.rglob('*.svg'))
            results.append(dict(case=case,mode=mode,**measured))
            print(case,mode,measured,flush=True)
    (a.output/'measurements.json').write_text(json.dumps(dict(system=platform.platform(),
        baseline_commit='876bf65',records_per_source=a.records,
        binaries={k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in [('before',a.before),('after',a.after)]},
        input_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [plain,compressed]},
        note='Single executions, uncontrolled caches/load; generation/build excluded. Scratch/staged peaks sampled every 10ms are lower bounds. Zero additional source passes/scratch for yield display; existing two-worker discovery rereads both sources. Every non-display JSON field compared exactly.',results=results),indent=2)+'\n')


if __name__=='__main__':main()
