#!/usr/bin/env python3
"""Optional synthetic single/multi-run temporal resources; not a CI benchmark.

Reuses child CPU/RSS and sampled scratch accounting. Generation happens before
measurement. Never reads research inputs. Run after builds/audits have stopped.
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


def canonical(report):
    report.pop('minionqc_time_series',None)
    for run in report.get('collection',{}).get('runs',[]): run.pop('minionqc_time_series',None)
    return report


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--records',type=int,default=1_000_000,help='Records per independent source')
    p.add_argument('--binary',type=Path,default=Path('target/release/ontrunqc'))
    p.add_argument('--output',type=Path,default=Path('benchmark-results/time-performance'))
    a=p.parse_args();assert a.records>0
    a.output.mkdir(parents=True)
    inputs=a.output/'inputs';inputs.mkdir()
    plain=inputs/'sequencing_summary_a.tsv';compressed=inputs/'sequencing_summary_b.tsv.gz'
    for source,stream in [('fictional-a',plain.open('wb')),('fictional-b',gzip.open(compressed,'wb'))]:
        with stream:
            stream.write(HEADER)
            for i in range(a.records):
                length=i*37%10000+1
                stream.write(f'{source}\t{i%512+1}\t{i%5+1}\t{i*137%260000}\t1\tTRUE\t{length}\t{6+i%17/3}\tsynthetic\t{length*2}\n'.encode())
    cases=[('single-native',plain,[],1,'report.json','single'),
           ('single-reference',plain,['--minionqc-time-series'],2,'report.json','single'),
           ('gzip-native',compressed,[],1,'report.json','gzip'),
           ('gzip-reference',compressed,['--minionqc-time-series'],2,'report.json','gzip'),
           ('multi-native',inputs,['--combined-only'],1,'combined/report.json','multi'),
           ('multi-reference-1',inputs,['--combined-only','--minionqc-time-series'],2,'combined/report.json','multi'),
           ('multi-reference-2',inputs,['--combined-only','--minionqc-time-series','--processors','2'],3,'combined/report.json','multi'),
           ('multi-reference-sha',inputs,['--combined-only','--minionqc-time-series','--verify-input-content'],3,'combined/report.json','multi'),
           ('multi-reference-svg',inputs,['--combined-only','--minionqc-time-series','--format','svg'],2,'combined/report.json',None)]
    baselines={};full=None;results=[]
    for name,source,args,passes,relative,key in cases:
        destination=a.output/name
        m=execute(a.binary.resolve(),source.resolve(),destination,args,passes,relative)
        if source.is_dir():
            m.pop('decompression_passes',None)
            m['input_passes_total']=passes*2
            m['input_passes_by_source']={'sequencing_summary_a.tsv':passes,'sequencing_summary_b.tsv.gz':passes}
            m['decompression_passes_by_source']={'sequencing_summary_a.tsv':0,'sequencing_summary_b.tsv.gz':passes}
        m['publication_verification_passes']=int('--verify-input-content' in args)
        report=json.loads((destination/relative).read_text())
        if name=='multi-reference-1':full=json.loads(json.dumps(report))
        if name=='multi-reference-2':assert report==full,'Worker count changed JSON'
        if key:
            c=canonical(report)
            if key in baselines:assert c==baselines[key],'Canonical statistics changed'
            else:baselines[key]=c
        phases=[]
        for line in (destination/'execution.log').read_text().splitlines():
            if line.startswith('Reference time-series '):phases.append(line)
        results.append(dict(case=name,**m,temporal_phase_logs=phases))
        print(name,m,flush=True)
    (a.output/'measurements.json').write_text(json.dumps(dict(system=platform.platform(),machine=platform.machine(),
        binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),records_per_source=a.records,
        note='Single executions, uncontrolled caches/load. Temp peaks sampled at 10 ms are lower bounds; pass counts are per-source code paths, not syscall traces. Phase timing separates guarded reread/chunk sorting from final merge/curve passes; remaining wall time includes initial aggregation and rendering/publication.',results=results),indent=2)+'\n')


if __name__=='__main__':main()
