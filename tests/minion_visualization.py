#!/usr/bin/env python3
"""Optional full-data MinION validation/measurements; standard library only.

Requires the pinned public MinIONQC checkout, a release binary and existing R.
Never modifies the scientific differential suite or downloads research data.
"""
import argparse
import base64
from collections import Counter
import csv
import hashlib
from html.parser import HTMLParser
import json
import math
import os
from pathlib import Path
import subprocess
import time

REVISION='54485d586038d8ee36f168dd94f31c423f52f581'


class Inventory(HTMLParser):
    def __init__(self):
        super().__init__();self.tags=Counter();self.images=[]
    def handle_starttag(self,tag,attrs):
        self.tags[tag]+=1
        for key,value in attrs:
            if key=='href':
                assert value.startswith(('#','data:image/png;base64,')),value
                if value.startswith('data:'):self.images.append(value)
    def handle_startendtag(self,tag,attrs):self.handle_starttag(tag,attrs)


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def coordinates(channel):
    for right,starts in [(False,[33,481,417,353,289,225,161,97]),(True,[1,449,385,321,257,193,129,65])]:
        for block,start in enumerate(starts):
            if start<=channel<start+32:
                n=channel-start
                return block*4+n//8,15-n%8 if right else n%8
    raise AssertionError(channel)


def observations(path):
    with path.open() as stream:
        for row in csv.DictReader(stream,delimiter='\t'):
            if row.get('calibration_strand_genome_template')=='YHR174W':continue
            two='sequence_length_2d' in row
            length=int(row['sequence_length_2d' if two else 'sequence_length_template'])
            yield int(row['channel']),float(row['start_time']),length,float(row['mean_qscore_2d' if two else 'mean_qscore_template']),row.get('num_events_template') if not two else None


def independent_counts(path,view,reference):
    overview,scatter=Counter(),Counter();channels=Counter();missing=zero=excluded=0
    times=[];lengths=[];qualities=[] # only minima/maxima below; never retain reads
    def fraction(x,lo,hi):return .5 if lo==hi else (x-lo)/(hi-lo)
    def pixel(v,lo,hi,size):return min(size-1,max(0,math.floor(fraction(v,lo,hi)*size)))
    bounds=view['bounds'];ln=[math.log10(x) for x in bounds['length_bp']]
    for ch,t,length,q,event in observations(path):
        if length==0:excluded+=1;continue
        channels[ch]+=1;missing+=event in (None,'','NA');zero+=event not in (None,'','NA') and int(event)==0
        for values,v in ((times,t),(lengths,length),(qualities,q)):
            if not values:values.extend([v,v])
            values[0]=min(values[0],v);values[1]=max(values[1],v)
        ix=pixel(t,*bounds['start_seconds'],64);iy=39-pixel(math.log10(length),*ln,40)
        row,col=coordinates(ch);overview[(row*40+iy)*1024+col*64+ix]+=1
        ix=pixel(math.log10(length),*ln,512);iy=255-pixel(q,*bounds['qscore'],256)
        scatter[iy*512+ix]+=1
    assert times==bounds['start_seconds'];assert lengths==bounds['length_bp'];assert qualities==bounds['qscore']
    assert sum(channels.values())==view['eligible_reads']
    assert excluded==view['zero_length_records'];assert missing==view['missing_events_reads'];assert zero==view['zero_events_reads']
    assert {c['channel']:c['eligible_reads'] for c in view['channels'] if c['eligible_reads']}==dict(channels)
    if view['method']=='exact_points':
        assert len(view['points'])==sum(channels.values())
        expected=Counter((ch,t,length,q,None if event in (None,'','NA') else int(event)/length)
                         for ch,t,length,q,event in observations(path) if length>0)
        actual=Counter((p['channel'],p['start_seconds'],p['length_bp'],p['qscore'],p['events_per_base']) for p in view['points'])
        assert actual==expected,'Small-input points must contain every original observation'
    for field,family,expected in [('overview','flowcell_overview',overview),('scatter','length_vs_q',scatter)]:
        assert dict(reference['plots'][family]['counts'])==dict(expected),family+' R pixel disagreement'
        if view[field] is not None:
            assert dict(view[field]['counts'])==dict(expected),field+' pixel disagreement'
            assert view[field]['total_reads']==sum(expected.values())
    return dict(reads=sum(channels.values()),channels=len(channels),overview_occupied=len(overview),scatter_occupied=len(scatter))


def measure(binary,path,output):
    output.mkdir(parents=True,exist_ok=True)
    command=[str(binary),'analyze',str(path),'--split-runs','--flowcell-layout','minion-r9.5','--output',str(output)]
    # wait4 measures this specific child without /usr/bin/time's macOS sysctl
    # query (blocked by some sandboxes), or a process-wide cumulative maximum.
    start=time.perf_counter()
    with (output/'execution.log').open('wb') as log:
        child=subprocess.Popen(command,stdout=subprocess.DEVNULL,stderr=log)
        _,status,usage=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status)
    elapsed=time.perf_counter()-start
    assert child.returncode==0,(output/'execution.log').read_text()
    rss=usage.ru_maxrss*(1 if os.uname().sysname=='Darwin' else 1024)
    reports=[]
    for html in sorted(output.rglob('report.html')):
        text=html.read_text();inv=Inventory();inv.feed(text)
        report_json=html.with_suffix('.json')
        reports.append(dict(path=str(html.relative_to(output)),html_bytes=html.stat().st_size,
                            json_bytes=report_json.stat().st_size,
                            elements=sum(inv.tags.values()),svg_elements=inv.tags['svg'],
                            circles=inv.tags['circle'],embedded_images=len(inv.images),
                            embedded_png_bytes=sum(len(base64.b64decode(uri.split(',',1)[1])) for uri in inv.images)))
    return dict(wall_seconds=elapsed,user_seconds=usage.ru_utime,system_seconds=usage.ru_stime,
                peak_rss_bytes=rss,reports=reports)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('upstream',type=Path);p.add_argument('--binary',type=Path,default=Path('target/release/poregauge'))
    p.add_argument('--baseline',type=Path);p.add_argument('--rscript',default='Rscript')
    p.add_argument('--output',type=Path,default=Path('benchmark-results/minion-visualization'))
    p.add_argument('--render-reference',action='store_true');args=p.parse_args()
    upstream=args.upstream.resolve();output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
    assert subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'],text=True).strip()==REVISION
    assert subprocess.check_output(['git','-C',str(upstream),'status','--porcelain'],text=True)==''
    synthetic=output/'synthetic.tsv'
    with synthetic.open('w') as stream:
        stream.write('channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\n')
        for i in range(150000):
            length=50+i*7919%50000;q=3+(i%240)/10
            events='NA' if i%11==0 else '0' if i%13==0 else str(length*(i%10+1))
            stream.write(f'{i%512+1}\t{i*13%129600}\t{length}\t{q}\t{events}\n')
        stream.write('512\t129600\t1000000\t100\t2000000\n33\t0\t1\t-3\t1\n1\t0\t0\t0\t0\n')
    inputs=[('synthetic',synthetic),
            ('public-minion',upstream/'example_input_minion/RB7_A2/sequencing_summary.txt'),
            ('public-1d2',upstream/'testfiles/1D2/sequencing_summary.txt'),
            ('public-short',upstream/'testfiles/short_run/sequencing_summary.txt'),
            ('public-rna',upstream/'testfiles/directRNA/Direct_RNA_sequencing_summary.txt')]
    results=[]
    for name,path in inputs:
        dest=output/name;dest.mkdir(exist_ok=True)
        after=measure(args.binary.resolve(),path,dest/'after')
        before=measure(args.baseline.resolve(),path,dest/'before') if args.baseline else None
        if before:
            for old in (dest/'before').rglob('report.json'):
                a=json.loads(old.read_text());b=json.loads((dest/'after'/old.relative_to(dest/'before')).read_text())
                for v in (a,b):
                    v['diagnostics'].pop('minion_sample',None);v['diagnostics'].pop('minion_read_visualization',None)
                assert a==b,'Validated QC changed'
        probe=dest/'source-visualization.json'
        env=dict(os.environ,PG_MINION_INPUT=str(path),PG_MINION_OUTPUT=str(probe))
        checked=subprocess.run(['cargo','test','--release','--locked','minion::tests::export_public_visualization','--','--ignored','--exact'],env=env,capture_output=True,text=True)
        assert checked.returncode==0,checked.stderr
        data=json.loads(probe.read_text());view=data['visualization']
        capture=dest/'minionqc.json';routput=dest/'minionqc';routput.mkdir(exist_ok=True)
        env=dict(os.environ,PG_MINION_SOURCE=str(upstream/'MinIONQC.R'),PG_MINION_CAPTURE=str(capture),
                 PG_MINION_RENDER=str(args.render_reference).lower())
        run=subprocess.run([args.rscript,'tests/reference/minion_plots.R','-i',str(path),'-o',str(routput),'-q','7','-p','1','-f','png','-s','TRUE','-m','8'],env=env,capture_output=True,text=True)
        (dest/'reference.log').write_text(run.stdout+run.stderr);assert run.returncode==0,run.stderr
        reference=json.loads(capture.read_text());assert reference['error'] is None
        coverage=independent_counts(path,view,reference)
        for family in ['flowcell_overview','length_vs_q']:
            r=reference['plots'][family]
            assert r['eligible_reads']==view['eligible_reads'];assert r['zero_length_records']==view['zero_length_records']
            for c in r['channels']:assert coordinates(c['channel'])==(c['row'],c['col'])
        assert reference['plots']['flowcell_overview']['alpha']==.35
        assert reference['plots']['length_vs_q']['alpha']==.05
        color=reference['plots']['length_vs_q']
        if view['bounds']['positive_events_per_base']:
            assert color['color_transform'].startswith('log-2.718')
            assert all(abs(a-b)<1e-12 for a,b in zip(color['color_transform_test'],[0,1]))
        else:
            assert color['color_transform']=='identity' # Upstream's no-events path.
        assert reference['plots']['flowcell_overview']['x_transform']=='identity'
        assert reference['plots']['flowcell_overview']['y_transform']=='log-10'
        assert reference['plots']['length_vs_q']['x_transform']=='log-10'
        assert reference['plots']['length_vs_q']['y_transform']=='identity'
        results.append(dict(case=name,input_sha256=sha(path),input_bytes=path.stat().st_size,coverage=coverage,before=before,after=after,
                            visualization_timings=data['timings'],reference_capture_sha256=sha(capture)))
        print(name,coverage,after['wall_seconds'],after['peak_rss_bytes'],flush=True)
    report=dict(reference_commit=REVISION,reference_source_sha256=sha(upstream/'MinIONQC.R'),
                binary_sha256=sha(args.binary),note='Single executions, uncontrolled filesystem cache; CLI RSS includes exact QC. Probe timings use release-test code. These are measurements, not throughput guarantees.',results=results)
    (output/'measurements.json').write_text(json.dumps(report,indent=2)+'\n')


if __name__=='__main__':main()
