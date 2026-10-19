#!/usr/bin/env python3
"""Optional paired execution of pinned MinIONQC and ONTRunQC (not routine CI)."""
import argparse
from collections import Counter
import hashlib
import json
import math
import os
import statistics
from pathlib import Path
import shutil
import subprocess
import sys

from minionqc_reference import REVISION, n50, observations

# Keep historical comparison keys, statuses and explanations unchanged so the
# pinned numerical evidence remains verifiable after the ONTRunQC rename.
STATUSES = ("exact_match", "rounding_tolerance", "intentional_semantic_difference",
            "suspected_poregauge_defect", "upstream_minionqc_defect", "not_yet_verified")


def comparison(metric, reference, actual, atol=0.0, rtol=0.0, intentional=None, upstream=None):
    finite = lambda v: isinstance(v, (float, int)) and math.isfinite(v)
    absolute = abs(actual-reference) if finite(reference) and finite(actual) else None
    relative = absolute/abs(reference) if absolute is not None and reference else None
    if reference == actual:
        status, explanation = STATUSES[0], ""
    elif absolute is not None and absolute <= atol + rtol*abs(reference):
        status, explanation = STATUSES[1], "Within the stated rounding/numerical tolerance."
    elif intentional:
        status, explanation = STATUSES[2], intentional
    elif upstream:
        status, explanation = STATUSES[4], upstream
    else:
        status, explanation = STATUSES[3], "Unexplained mismatch; requires investigation."
    return dict(metric=metric,reference=reference,poregauge=actual,absolute_difference=absolute,
                relative_difference=relative,tolerance=dict(absolute=atol,relative=rtol),
                status=status,explanation=explanation)


def unverified(metric, reason):
    row = comparison(metric, None, None)
    row.update(status=STATUSES[5], explanation=reason)
    return row


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024*1024), b""):
            h.update(block)
    return h.hexdigest()


def reference_command(args, cli, env, mounts):
    if not args.docker:
        return [args.rscript, *cli]
    return ["docker", "run", "--rm",
            *[v for path, writable in sorted(mounts.items()) for v in
              ["-v", f"{path}:{path}" + ("" if writable else ":ro")]],
            *[v for k in ("PG_REFERENCE_SOURCE", "PG_REFERENCE_CAPTURE",
                          "PG_REFERENCE_RENDER", "PG_REFERENCE_PREFLIGHT")
              if k in env for v in ["-e", f"{k}={env[k]}"]], args.docker, *cli]


def save_comparison(output, report):
    statuses = Counter(c["status"] for case in report["cases"] for c in case["checks"])
    report["status_counts"] = dict(statuses)
    temporary = output / "comparison.json.tmp"
    temporary.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    temporary.replace(output / "comparison.json")
    return statuses


def synthetic(path, lengths, qualities, times=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w") as stream:
        stream.write("channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\n")
        for i, (length, q) in enumerate(zip(lengths, qualities)):
            stream.write(f"{i%4+1}\t{times[i] if times else i*71}\t{length}\t{q}\t{length*2}\n")
    return path


def cases(upstream, output):
    specs = [
        ("minion", ["example_input_minion/RB7_A2/sequencing_summary.txt"],7),
        ("promethion", ["example_input_promethion/sequencing_summary.txt"],7),
        ("short", ["testfiles/short_run/sequencing_summary.txt"],8),
        ("1d2", ["testfiles/1D2/sequencing_summary.txt"],8),
        ("rna", ["testfiles/directRNA/Direct_RNA_sequencing_summary.txt"],7),
        ("combined", ["example_input_minion/RB7_A2/sequencing_summary.txt", "example_input_minion/RB7_D3/sequencing_summary.txt"],7),
    ]
    result = [(name,[upstream/p for p in paths],q) for name,paths,q in specs]
    edge = output/"synthetic"
    for name, lengths, qs in [
        ("zero-cutoff",[0,100,50,50,10,10],[7,7,7,6.999,8,9]),
        ("n50-tie",[100,50,25,25],[9]*4),
        ("ultralong-endpoint",[200000,150000,100000,100000],[8]*4),
    ]:
        result.append((name,[synthetic(edge/name/"sequencing_summary.txt", lengths,qs)],7))
    for name,n in [("fit-small",99),("fit-large",1500),("fit-sparse",1200)]:
        times=[(i*71)%100000 for i in range(n)] if name!="fit-sparse" else [i%2*3600 for i in range(n)]
        lengths=[1000+int(200*math.sin(t/10000))+i%13 for i,t in enumerate(times)]
        qs=[8+2*math.cos(t/15000)+i%7/10 for i,t in enumerate(times)]
        result.append((name,[synthetic(edge/name/"sequencing_summary.txt",lengths,qs,times)],7))
    for name,lengths,qs in [
        ("density-multimodal",[100]*30+[10000]*30,[7.0]*30+[20.0]*30),
        ("density-skewed",[2**i for i in range(25)],[7+i/2 for i in range(25)]),
        ("density-constant",[100]*40,[20.0]*40),
        ("density-near",[100+i for i in range(40)],[20.0+i*1e-8 for i in range(40)]),
        ("density-outliers",[100]*40+[10000000],[9.0]*40+[10000.0]),
        ("density-singleton",[100,200,400],[7.0,8.0,9.0]),
    ]:
        # Two unequal source sizes are needed to exercise real upstream multi.plots.
        a=synthetic(edge/name/"a/sequencing_summary.txt",lengths,qs)
        size=1 if name=="density-singleton" else 12
        b=synthetic(edge/name/"b/sequencing_summary.txt",lengths[:size],qs[:size])
        result.append((name,[a,b],7))
    return result


def metrics(reference, pg, rows, cutoff, channel_scope=False):
    results = []
    summaries = reference.get("summaries", [])
    if not summaries:
        return [unverified("summary", reference.get("error") or "No MinIONQC YAML produced.")]
    summary = next((s["data"] for s in summaries if s["directory"]=="combinedQC"), summaries[0]["data"])
    for selected in [False, True]:
        name = f"Q>={cutoff}" if selected else "All reads"
        cohort = [r for r in rows if not selected or r[1]>=cutoff]
        basecalled = [r for r in cohort if r[0]>0]
        p = pg["comparison"]["qscore_selected"] if selected else pg["metrics"]
        quality = p if selected else pg["quality"]
        raw = (reference.get("populations") or {}).get(name, {})
        r = summary.get(name)
        if not r:
            results.append(unverified(name, "Reference subset/YAML unavailable.")); continue
        zeros = len(cohort)-len(basecalled)
        semantics = "MinIONQC includes zero-length records; PoreGauge uses positive-length basecalled reads." if zeros else None
        for metric, rv, pv, tolerance, difference in [
            ("read_count",raw.get("reads",r["total.reads"]),p["read_count" if selected else "basecalled_reads"],0,semantics),
            ("total_bases",raw.get("bases"),p["total_yield_bp"],0,None),
            ("mean_length",r["mean.length"],p["mean_read_length_bp"],0.050000001,semantics),
            ("median_length",r["median.length"],p["median_read_length_bp"],0.050000001,semantics),
            ("mean_q",r["mean.q"],quality["mean_qscore"],0.050000001,semantics),
            ("median_q",r["median.q"],quality["median_qscore"],0.050000001,semantics),
        ]:
            expected = {
                "read_count":len(basecalled), "total_bases":sum(r[0] for r in basecalled),
                "mean_length":statistics.mean([r[0] for r in basecalled]) if basecalled else None,
                "median_length":statistics.median([r[0] for r in basecalled]) if basecalled else None,
                "mean_q":statistics.mean([r[1] for r in basecalled]) if basecalled else None,
                "median_q":statistics.median([r[1] for r in basecalled]) if basecalled else None,
            }[metric]
            proof = pv==expected or isinstance(pv,(int,float)) and isinstance(expected,(int,float)) and abs(pv-expected)<1e-8
            reference_expected={"read_count":len(cohort),"total_bases":sum(r[0] for r in cohort),"mean_length":statistics.mean([r[0] for r in cohort]) if cohort else None,"median_length":statistics.median([r[0] for r in cohort]) if cohort else None,"mean_q":statistics.mean([r[1] for r in cohort]) if cohort else None,"median_q":statistics.median([r[1] for r in cohort]) if cohort else None}[metric]
            proof = proof and (rv==reference_expected or isinstance(rv,(int,float)) and isinstance(reference_expected,(int,float)) and abs(rv-reference_expected)<=tolerance)
            if rv is None and pv is not None:
                results.append(unverified(f"{name}.{metric}","Reference value unavailable; partial R output is not evidence of a PoreGauge defect."))
            else:
                results.append(comparison(f"{name}.{metric}",rv,pv,tolerance,intentional=difference if proof else None))
        lengths = sorted((r[0] for r in basecalled),reverse=True)
        total = sum(lengths); accumulated = 0; strict = None
        for length in lengths:
            accumulated += length
            if accumulated > total/2: strict=length;break
        tie = "Core N50 retains cumulative >=half; MinIONQC uses strict >half." if r["N50.length"]==strict and p["n50_bp"]==n50(lengths) else None
        results.append(comparison(f"{name}.N50",r["N50.length"],p["n50_bp"],intentional=tie))
        prefix = "qscore_selected" if selected else "all"
        for threshold in pg["comparison"]["yield_by_minimum_length"]["thresholds"]:
            length = threshold["minimum_length_bp"]
            key = ">1m" if length==1000000 else f">{length//1000}kb"
            results.append(comparison(f"{name}.reads.{key}",r["reads"][key],threshold[f"{prefix}_read_count"]))
            results.append(comparison(f"{name}.bases.{key}",r["gigabases"][key]*1e9,threshold[f"{prefix}_yield_bp"],50.1))
        ultra=pg["comparison"]["yield_by_minimum_length"]["ultralong_qscore_selected" if selected else "ultralong_all"]
        # Independent exhaustive longest-first prefix check for proof of the known endpoint bug.
        # The optional public validator already checks the optimized large-population definition.
        known_endpoint = bool(lengths) and strict is not None and strict>=100000 and ultra["read_count"]==len(lengths) and r["reads"]["ultralong"]==len(lengths)-1
        bug="Upstream binSearch drops the final eligible read (max-1 endpoint)." if known_endpoint else None
        results.append(comparison(f"{name}.ultralong.reads",r["reads"]["ultralong"],ultra["read_count"],upstream=bug))
        reference_ultra_bases = r["gigabases"]["ultralong"]*1e9
        yield_bug = bug if bug and ultra["total_yield_bp"]==sum(lengths) and abs(reference_ultra_bases-sum(lengths[:-1]))<=50.1 else None
        results.append(comparison(f"{name}.ultralong.bases",reference_ultra_bases,ultra["total_yield_bp"],50.1,upstream=yield_bug))
    if not channel_scope: return results
    # Same source-file population as upstream; production run identity is untouched.
    channels={c["channel"]:c for c in pg["comparison"]["channels"]}
    by_channel={}
    for row in rows: by_channel.setdefault(row[2],[]).append(row)
    for population,data in (reference.get("channels") or {}).items():
        selected=population!="All reads"
        for row in data:
            key=f"channel.{row['channel']}.{population}.{row['variable']}"
            channel=channels.get(row["channel"])
            if channel is None:
                results.append(unverified(key,"No matching PoreGauge channel in file-scoped probe."));continue
            p=channel["qscore_selected_reads" if selected else "all_summary_records"]
            cohort=[r[0] for r in by_channel.get(row["channel"],[]) if not selected or r[1]>=cutoff]
            positives=[n for n in cohort if n>0] if selected else cohort
            field={"total.reads":"read_count","total.bases":"total_yield_bp","mean.read.length":"mean_read_length_bp","median.read.length":"median_read_length_bp"}.get(row["variable"])
            if not field: continue
            expected={"read_count":len(positives),"total_yield_bp":sum(positives),"mean_read_length_bp":statistics.mean(positives) if positives else None,"median_read_length_bp":statistics.median(positives) if positives else None}[field]
            correct=p[field]==expected or isinstance(p[field],(int,float)) and isinstance(expected,(int,float)) and abs(p[field]-expected)<1e-9
            bug="Upstream channel.summary uses sum(which(...)), n(n+1)/2 instead of a true read count." if field=="read_count" and row["value"]==len(cohort)*(len(cohort)+1)//2 and correct else None
            semantic=None
            if selected and len(cohort)!=len(positives) and correct:
                if field=="median_read_length_bp" and row["value"]==statistics.median(cohort) or field=="mean_read_length_bp" and abs(row["value"]-statistics.mean(cohort))<1e-9:
                    semantic="Reference includes selected zero-length summary records; PoreGauge channel cutoff statistics use basecalled reads."
            results.append(comparison(key,row["value"],p[field],1e-9 if field=="mean_read_length_bp" else 0,intentional=semantic,upstream=bug))
    return results


def fitted_comparisons(reference, probe):
    checks=[]
    sources={s["source_id"]:s for s in probe.get("reference_sources",[])}
    def array(x): return x if isinstance(x,list) else [x] if x is not None else []
    for plot in reference.get("plots",[]):
        family=plot["file"].rsplit(".",1)[0]
        if family not in ("length_by_hour","q_by_hour"): continue
        quality=family=="q_by_hour"
        combined=plot["directory"]=="combinedQC"
        predictions=iter(array(plot.get("smooth_predictions")))
        panels={str(p["PANEL"]):p for p in array(plot["panels"])}
        legends=dict(zip(array(plot.get("colours")),array(plot.get("colour_labels"))))
        for layer in plot["layers"]:
            if layer["stat"]!="StatSmooth": continue
            by_group={}
            for row in array(layer.get("data")):
                by_group.setdefault((str(row["PANEL"]),row["group"]),[]).append(row)
            available_fits=sum(bool(c["points"]) for source_id,source in sources.items() if combined or source_id==plot["directory"] for c in source.get("fitted_trends",{}).get("combined" if combined else "single",[]) if (c["metric"]=="qscore")==quality)
            checks.append(comparison(f"fit.{plot['directory']}.{family}.available_groups",len(by_group),available_fits))
            for (panel,group),rows in by_group.items():
                raw=next(predictions,None)
                label=legends.get(rows[0].get("colour"))
                source=label if combined else plot["directory"]
                population=panels[panel].get("Q_cutoff") if combined else label
                selected=population!="All reads"
                fitting=sources.get(source,{}).get("fitted_trends",{}).get("combined" if combined else "single",[])
                curve=next((c for c in fitting if (c["metric"]=="qscore")==quality and c["population"].startswith("qscore")==selected),None)
                key=f"fit.{plot['directory']}.{family}."+(f"{source}." if combined else "")+str(population)
                if curve is None:
                    checks.append(unverified(key,"No matching file-scoped Rust fit; inspect panel/legend metadata."));continue
                expected="loess" if curve["method"].startswith("loess") else "gam"
                checks.append(comparison(key+".method",layer.get("method"),expected))
                checks.append(comparison(key+".points",len(rows),len(curve["points"])))
                reference_points=array(raw.get("data")) if raw else rows
                if raw:
                    checks.append(comparison(key+".raw_points",len(rows),len(reference_points)))
                    checks.append(comparison(key+".observations",raw["observations"],curve["observations"]))
                    if raw.get("knots"):
                        knots=array(curve.get("knots_hours"))
                        checks.append(comparison(key+".knots.count",len(raw["knots"]),len(knots)))
                        for i,(r,p) in enumerate(zip(raw["knots"],knots)):
                            checks.append(comparison(f"{key}.knots.{i}",r,p,1e-10,1e-12))
                for i,(r,p) in enumerate(zip(reference_points,curve["points"])):
                    checks.append(comparison(f"{key}.{i}.hours",r.get("x"),p["hours"],1e-10))
                    if r.get("y") is None:
                        checks.append(unverified(f"{key}.{i}.fitted","R layer prediction is censored or nonfinite; uncensored reference prediction is unavailable. A null cannot establish a Rust numerical defect or a match."))
                    else:
                        checks.append(comparison(f"{key}.{i}.fitted",r.get("y"),p["value"],1e-6,1e-6))
                    if raw:
                        for r_field,p_field in [("se","standard_error"),("ymin","lower"),("ymax","upper")]:
                            if r.get(r_field) is None:
                                check=comparison(f"{key}.{i}.{p_field}.finite",False,p.get(p_field) is not None)
                                if check["status"]=="exact_match":
                                    check["explanation"]="Actual uncensored R uncertainty is nonfinite; Rust also withholds it. This verifies availability, not a numerical interval."
                                checks.append(check)
                            else:
                                checks.append(comparison(f"{key}.{i}.{p_field}",r[r_field],p.get(p_field),1e-6,1e-6))
                if not raw:
                    checks.append(unverified(key+".confidence_interval","Uncensored reference or Rust confidence intervals unavailable; no uncertainty match claimed."))
    return checks


def gam_intermediate_comparisons(reference, probe):
    """Validation-only fit diagnostics; GAM matrices use common nodal coordinates."""
    def array(x): return x if isinstance(x,list) else [x] if x is not None else []
    def transpose(a): return list(map(list,zip(*a)))
    def multiply(a,b): return [[sum(x*y for x,y in zip(row,column)) for column in zip(*b)] for row in a]
    def inverse(a):
        n=len(a);a=[list(row)+[float(i==j) for j in range(n)] for i,row in enumerate(a)]
        for i in range(n):
            pivot=max(range(i,n),key=lambda j:abs(a[j][i]));a[i],a[pivot]=a[pivot],a[i]
            scale=a[i][i]
            if not scale: raise ValueError("singular reference nodal transform")
            a[i]=[v/scale for v in a[i]]
            for j in range(n):
                if i!=j:
                    f=a[j][i];a[j]=[x-f*y for x,y in zip(a[j],a[i])]
        return [row[n:] for row in a]
    sources={s["source_id"]:s for s in probe.get("reference_sources",[])}
    checks=[]
    def matrix(key, a,b,atol,rtol):
        checks.append(comparison(key+".rows",len(a),len(b)))
        for i,(r,p) in enumerate(zip(a,b)):
            checks.append(comparison(f"{key}.{i}.columns",len(r),len(p)))
            for j,(x,y) in enumerate(zip(r,p)):
                checks.append(comparison(f"{key}.{i}.{j}",x,y,atol,rtol))
    for plot in reference.get("plots",[]):
        family=plot["file"].rsplit(".",1)[0]
        if family not in ("length_by_hour","q_by_hour"):continue
        combined=plot['directory']=='combinedQC'
        predictions=iter(array(plot.get("smooth_predictions")))
        panels={str(p['PANEL']):p for p in array(plot['panels'])}
        legends=dict(zip(array(plot.get('colours')),array(plot.get('colour_labels'))))
        for layer in plot['layers']:
            if layer['stat']!='StatSmooth':continue
            groups={}
            for row in array(layer.get('data')):groups.setdefault((str(row['PANEL']),row['group']),[]).append(row)
            for (panel,group),rows in groups.items():
                r=next(predictions,None)
                if not r:continue
                label=legends.get(rows[0].get('colour'));source=label if combined else plot['directory']
                selected=(panels[panel].get('Q_cutoff') if combined else label)!='All reads'
                quality=family=='q_by_hour'
                candidates=sources.get(source,{}).get('gam_audit') or []
                d=next((d for d in candidates if d['quality']==quality and d['population'].startswith('qscore')==selected and (not d['single'] if combined or selected or not quality else d['single'])),None)
                if r.get('method')=='loess':
                    key=f"loess.{plot['directory']}.{source}.{family}.{'selected' if selected else 'all'}"
                    ra=r.get('audit') or {};pa=(d or {}).get('loess')
                    finite=all(isinstance(ra.get(f),(float,int)) and math.isfinite(ra[f])
                               for f in ('residual_scale','df','critical_value'))
                    checks.append(comparison(key+'.finite_uncertainty',finite,pa is not None))
                    if finite and pa is not None:
                        for field in ('trace_hat','one_delta','two_delta','residual_scale','df','critical_value'):
                            checks.append(comparison(key+'.'+field,ra[field],pa[field],1e-6,1e-6))
                    continue
                if r.get('method')!='gam':continue
                key=f"gam.{plot['directory']}.{source}.{family}.{'selected' if selected else 'all'}"
                if not d or not d.get('fit') or not r.get('audit'):
                    checks.append(unverified(key,'Intermediate audit missing; enable PG_GAM_AUDIT.'));continue
                ra=r['audit'];b=d['basis'];f=d['fit'];maximum=b['raw_eigenvalues'][-1]
                raw=b['raw_penalty'];raw=[[(raw[i][j]+raw[j][i])/2 for j in range(10)] for i in range(10)]
                matrix(key+'.raw_penalty',ra['raw_penalty'],raw,1e-12,1e-10)
                matrix(key+'.second_derivatives',ra['second_derivatives'],b['second'],1e-12,1e-10)
                if 'basis_at_predictions' in d:
                    matrix(key+'.basis_at_predictions',ra['basis_at_predictions'],d['basis_at_predictions'],1e-12,1e-10)
                for i,(rv,pv) in enumerate(zip(reversed(ra['raw_eigenvalues'][:8]),b['raw_eigenvalues'][2:])):
                    checks.append(comparison(f'{key}.positive_eigenvalues.{i}',rv,pv,1e-12,1e-10))
                ru=ra['raw_eigenvectors'];pu=b['shrink_eigenvectors']
                rp=[[sum(ru[i][k]*ru[j][k] for k in [8,9]) for j in range(10)] for i in range(10)]
                pp=[[sum(pu[i][k]*pu[j][k] for k in [0,1]) for j in range(10)] for i in range(10)]
                matrix(key+'.null_projector',rp,pp,1e-11,1e-10)
                matrix(key+'.normalized_shrink_penalty',[[v/maximum for v in row] for row in ra['shrink_penalty']],b['normalized_shrink_penalty'],1e-12,1e-10)
                transform=inverse(ra['model_transform'])
                xx=multiply(multiply(transpose(transform),ra['model_crossproduct']),transform)
                matrix(key+'.nodal_crossproduct',xx,f['nodal_crossproduct'],1e-7,1e-10)
                matrix(key+'.nodal_covariance',ra['nodal_covariance'],f['nodal_covariance'],1e-6,1e-6)
                matrix(key+'.nodal_coefficients',ra['nodal_coefficients'],[[x] for x in f['nodal_coefficients']],1e-6,1e-6)
                for name,rv,pv in [('normalized_lambda',r['smoothing_parameter']*maximum/ra['penalty_scale'],math.exp(f['log_lambda'])),('reml',ra['reml'],f['reml']),('edf',ra['edf'],f['edf']),('residual_variance',r['residual_variance'],f['residual_variance'])]:
                    checks.append(comparison(key+'.'+name,rv,pv,1e-6,1e-6))
    return checks


def density_comparisons(reference,probe):
    checks=[];sources={s["source_id"]:s for s in probe.get("reference_sources",[])}
    def array(x): return x if isinstance(x,list) else [x] if x is not None else []
    def integral(points): return sum((b[0]-a[0])*(a[1]+b[1])/2 for a,b in zip(points,points[1:]))
    for plot in reference.get("plots",[]):
        family=plot["file"].rsplit(".",1)[0]
        if family not in ("length_distributions","q_distributions"): continue
        parameters=iter(array(plot.get("density_parameters")))
        panels={str(p["PANEL"]):p for p in array(plot["panels"])}
        legends=dict(zip(array(plot.get("colours")),array(plot.get("colour_labels"))))
        for layer in plot["layers"]:
            if layer["stat"]!="StatDensity": continue
            groups={}
            for row in array(layer.get("data")):
                groups.setdefault((str(row["PANEL"]),row["group"]),[]).append(row)
            for (panel,group),rows in groups.items():
                parameter=next(parameters,None)
                source=legends.get(rows[0].get("colour"));population=panels[panel].get("Q_cutoff")
                field=("selected_" if population!="All reads" else "")+("length_log10" if family=="length_distributions" else "qscore")
                actual=(sources.get(source,{}).get("density") or {}).get(field)
                key=f"density.{source}.{population}.{field}"
                finite=[(r["x"],r["density"]) for r in rows if isinstance(r.get("x"),(float,int)) and isinstance(r.get("density"),(float,int))]
                if actual is None:
                    checks.append(comparison(key+".unavailable",not bool(finite),True));continue
                points=[(p["x"],p["density"]) for p in actual["points"]]
                if parameter is None:
                    checks.append(unverified(key+".bandwidth", "Capture predates actual ggplot2 bandwidth instrumentation."))
                else:
                    checks.extend([
                        comparison(key+".bandwidth",parameter["bandwidth"],actual["bandwidth"],1e-12,1e-10),
                        comparison(key+".observations",parameter["observations"],actual["observations"]),
                        comparison(key+".from",parameter["from"],points[0][0],1e-10,1e-12),
                        comparison(key+".to",parameter["to"],points[-1][0],1e-10,1e-12),
                        comparison(key+".kernel",parameter["kernel"],"gaussian"),
                        comparison(key+".grid_points",parameter["grid_points"],len(points)),
                    ])
                checks.append(comparison(key+".points",len(finite),len(points)))
                if not finite: continue
                peak=max(y for x,y in finite)
                for i,(r,p) in enumerate(zip(finite,points)):
                    checks.append(comparison(f"{key}.{i}.x",r[0],p[0],1e-10,1e-12))
                    checks.append(comparison(f"{key}.{i}.density",r[1],p[1],max(peak*1e-7,1e-12),1e-6))
                checks.append(comparison(key+".integral_on_plotted_domain",integral(finite),integral(points),1e-7,1e-6))
                step=(finite[-1][0]-finite[0][0])/511
                checks.append(comparison(key+".peak_position",max(finite,key=lambda p:p[1])[0],max(points,key=lambda p:p[1])[0],abs(step)+1e-10))
    return checks


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream",type=Path)
    parser.add_argument("binary",nargs="?",default="target/release/ontrunqc",type=Path)
    parser.add_argument("--execute-r",action="store_true")
    parser.add_argument("--output",type=Path,default=Path("benchmark-results/minionqc-reference"))
    parser.add_argument("--rscript",default="Rscript")
    parser.add_argument("--docker",metavar="IMAGE")
    parser.add_argument("--case",action="append",default=[],choices=["minion","promethion","short","1d2","rna","combined","zero-cutoff","n50-tie","ultralong-endpoint","fit-small","fit-large","fit-sparse","density-multimodal","density-skewed","density-constant","density-near","density-outliers","density-singleton"])
    parser.add_argument("--capture-only",action="store_true",help="Skip figure files; still execute upstream functions and ggplot_build")
    args=parser.parse_args(argv)
    upstream=args.upstream.resolve();binary=args.binary.resolve();output=args.output.resolve()
    assert subprocess.check_output(["git","-C",str(upstream),"rev-parse","HEAD"],text=True).strip()==REVISION
    assert subprocess.check_output(["git","-C",str(upstream),"status","--porcelain"],text=True)=="", "Reference source/fixtures must be clean"
    output.mkdir(parents=True,exist_ok=True)
    harness=Path(__file__).resolve().parent/"reference/capture.R"
    available=shutil.which("docker" if args.docker else args.rscript)
    image_id=None
    if args.docker and available:
        inspected=subprocess.run(["docker","image","inspect",args.docker,"--format","{{.Id}}"],capture_output=True,text=True)
        if inspected.returncode==0: image_id=inspected.stdout.strip()
    report=dict(reference_commit=REVISION,source_sha256=digest(upstream/"MinIONQC.R"),poregauge_binary_sha256=digest(binary),reference_image=dict(name=args.docker,id=image_id),cases=[])
    mounts={upstream:False,output:True,harness.parent:False}
    if available:
        environment_file=output/"reference-environment.json"
        environment_file.unlink(missing_ok=True)
        env=dict(os.environ,TZ="UTC",LC_ALL="C",PG_REFERENCE_PREFLIGHT="true",
                 PG_REFERENCE_CAPTURE=str(environment_file))
        checked=subprocess.run(reference_command(args,[str(harness)],env,mounts),
                               env=env,capture_output=True,text=True)
        (output/"preflight.log").write_text(checked.stdout+checked.stderr)
        report["reference_environment"]=json.loads(environment_file.read_text()) if environment_file.exists() else None
        if checked.returncode:
            report["cases"].append(dict(name="environment",checks=[unverified(
                "reference_versions", "Pinned R 4.4.1 / ggplot2 3.5.1 / mgcv 1.9-1 preflight failed; inspect preflight.log.")]))
            save_comparison(output,report)
            return 2
    for name, paths, cutoff in cases(upstream, output):
        if args.case and name not in args.case: continue
        directory=output/name; directory.mkdir(exist_ok=True)
        # Keep original source bytes, including legacy columns and RNA controls.
        inputdir=directory/"inputs"; inputdir.mkdir(exist_ok=True)
        copied=[]
        for i,path in enumerate(paths):
            dest=inputdir/f"source_{i+1:03}"/"sequencing_summary.txt"
            dest.parent.mkdir(exist_ok=True);shutil.copyfile(path,dest);copied.append(dest)
        pgdir=directory/"poregauge"
        options=["--minionqc-trends","--minionqc-density"] if available else []
        result=subprocess.run([str(binary),"analyze",*map(str,copied),"--split-runs","--yaml","--muxes","8",*options,"-q",str(cutoff),"-o",str(pgdir)],capture_output=True,text=True)
        (directory/"poregauge.log").write_text(result.stdout+result.stderr)
        if result.returncode: raise RuntimeError(result.stderr)
        pg=json.loads((pgdir/"combined/report.json").read_text())
        probe=directory/"source-population.json"
        probe_env=dict(os.environ,PG_PROBE_INPUTS=json.dumps(list(map(str,copied))),PG_PROBE_CUTOFF=str(cutoff),PG_PROBE_OUTPUT=str(probe),PG_PROBE_CURVES="1" if available else "0",PG_GAM_AUDIT="1")
        checked=subprocess.run(["cargo","test","--locked","reference_probe::export_report","--","--ignored","--exact"],env=probe_env,capture_output=True,text=True,cwd=Path(__file__).resolve().parents[1])
        if checked.returncode: raise RuntimeError(checked.stderr)
        source_population=json.loads(probe.read_text())
        rdir=directory/"minionqc";rdir.mkdir(exist_ok=True)
        capture=directory/"capture.json"
        capture.unlink(missing_ok=True) # Never reuse a stale successful R capture.
        env=dict(os.environ,TZ="UTC",LC_ALL="C",PG_REFERENCE_SOURCE=str(upstream/"MinIONQC.R"),PG_REFERENCE_CAPTURE=str(capture),PG_REFERENCE_RENDER=str(not args.capture_only).lower())
        cli=[str(harness),"-i",str(inputdir if len(paths)>1 else copied[0]),"-o",str(rdir),"-q",str(cutoff),"-p","1","-m","8","-f","pdf","-s","TRUE"]
        command=reference_command(args,cli,env,mounts)
        returncode=None
        if available:
            result=subprocess.run(command,env=env,capture_output=True,text=True)
            returncode=result.returncode
            (directory/"minionqc.log").write_text(result.stdout+result.stderr)
        else:
            (directory/"minionqc.log").write_text("Reference executable unavailable; original R script was not executed.\n")
        reference=json.loads(capture.read_text()) if capture.exists() else None
        rows,excluded=observations(paths)
        checks=metrics(reference,pg,rows,cutoff) if reference else [unverified("reference_execution","R/container unavailable or failed; inspect minionqc.log. No R comparisons fabricated.")]
        if reference:
            channel_checks=metrics(reference,source_population,rows,cutoff,channel_scope=True)
            checks.extend(c for c in channel_checks if c["metric"].startswith("channel."))
            checks.extend(fitted_comparisons(reference,source_population))
            checks.extend(density_comparisons(reference,source_population))
        if reference and reference.get("error"):
            checks.append(unverified("reference_completion",reference["error"]))
        elif available and returncode:
            checks.append(unverified("reference_completion",f"R exited {returncode}; inspect minionqc.log."))
        if reference:
            inventory=[dict(file=p["file"],directory=p["directory"],labels=p["labels"],panels=p["panels"],stats=[l["stat"] for l in p["layers"]]) for p in reference["plots"]]
            (directory/"plot-inventory.json").write_text(json.dumps(inventory,indent=2))
            expected={"length_histogram","q_histogram","yield_by_length","yield_over_time","length_by_hour","q_by_hour","reads_per_hour","length_vs_q","channel_summary","gb_per_channel_overview"}
            if len(paths)>1:
                expected|={"combined_length_histogram","combined_q_histogram","combined_yield_by_length","length_distributions","q_distributions"}
            families={p["file"].rsplit(".",1)[0] for p in reference["plots"]}
            for family in sorted(expected):
                checks.append(comparison(f"plot.{family}.generated",True,family in families))
            for item in inventory:
                if not item["labels"].get("x") or not item["labels"].get("y"):
                    checks.append(unverified(f"plot.{item['file']}.axes","Reference axis metadata missing; inspect inventory."))
        diagnostics=gam_intermediate_comparisons(reference,source_population) if reference else []
        report["cases"].append(dict(name=name,qscore_cutoff=cutoff,inputs=[dict(sha256=digest(p),bytes=p.stat().st_size) for p in paths],excluded_controls=excluded,reference_executed=available is not None,reference_completed=bool(reference is not None and returncode==0 and not reference.get("error")),reference_returncode=returncode,reference_capture_sha256=digest(capture) if capture.exists() else None,reference_environment=reference.get("session") if reference else None,checks=checks,gam_diagnostics=[d for d in diagnostics if d['metric'].startswith('gam.')],loess_diagnostics=[d for d in diagnostics if d['metric'].startswith('loess.')]))
        save_comparison(output,report)
        print(name,dict(Counter(c["status"] for c in checks)),flush=True)
    statuses=save_comparison(output,report)
    return 1 if statuses[STATUSES[3]] else 2 if statuses[STATUSES[5]] else 0


if __name__=="__main__": sys.exit(main())
