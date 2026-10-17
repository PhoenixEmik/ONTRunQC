#!/usr/bin/env python3
"""Optional paired execution of pinned MinIONQC and PoreGauge (not routine CI)."""
import argparse
from collections import Counter
import hashlib
import json
import math
import statistics
from pathlib import Path
import shutil
import subprocess
import sys

from minionqc_reference import REVISION, THRESHOLDS, n50, observations, projected_yaml

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
        ("n50-tie",[100,50,50,10,10,0],[9]*6),
        ("ultralong-endpoint",[200000,150000,100000,100000],[8]*4),
    ]:
        result.append((name,[synthetic(edge/name/"sequencing_summary.txt", lengths,qs)],7))
    return result


def metrics(reference, pg, rows, cutoff):
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
        raw = reference.get("populations",{}).get(name, {})
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
        known_endpoint = bool(lengths) and min(lengths)>=100000 and ultra["read_count"]==len(lengths) and r["reads"]["ultralong"]==len(lengths)-1
        bug="Upstream binSearch drops the final eligible read (max-1 endpoint)." if known_endpoint else None
        results.append(comparison(f"{name}.ultralong.reads",r["reads"]["ultralong"],ultra["read_count"],upstream=bug))
        results.append(comparison(f"{name}.ultralong.bases",r["gigabases"]["ultralong"]*1e9,ultra["total_yield_bp"],50.1,upstream=bug))
    # Individual source's exact channel medians; combined channels intentionally remain run-local.
    if pg["comparison"]["channels"]:
        for row in reference.get("channels",{}).get("All reads",[]):
            if row.get("variable") != "median.read.length": continue
            channel = next((c for c in pg["comparison"]["channels"] if c["channel"]==row["channel"]),None)
            if channel:
                results.append(comparison(f"channel.{row['channel']}.median",row["value"],channel["all_summary_records"]["median_read_length_bp"]))
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
        panels={str(p["PANEL"]):p for p in array(plot["panels"])}
        legends=dict(zip(array(plot.get("colours")),array(plot.get("colour_labels"))))
        for layer in plot["layers"]:
            if layer["stat"]!="StatSmooth": continue
            by_group={}
            for row in array(layer.get("data")):
                by_group.setdefault((str(row["PANEL"]),row["group"]),[]).append(row)
            for (panel,group),rows in by_group.items():
                label=legends.get(rows[0].get("colour"))
                source=label if combined else plot["directory"]
                population=panels[panel].get("Q_cutoff") if combined else label
                selected=population!="All reads"
                fitting=sources.get(source,{}).get("fitted_trends",{}).get("combined" if combined else "single",[])
                curve=next((c for c in fitting if (c["metric"]=="qscore")==quality and c["population"].startswith("qscore")==selected),None)
                key=f"fit.{plot['directory']}.{family}.{population}"
                if curve is None:
                    checks.append(unverified(key,"No matching file-scoped Rust fit; inspect panel/legend metadata."));continue
                expected="loess" if curve["method"].startswith("loess") else "gam"
                checks.append(comparison(key+".method",layer.get("method"),expected))
                checks.append(comparison(key+".points",len(rows),len(curve["points"])))
                for i,(r,p) in enumerate(zip(rows,curve["points"])):
                    checks.append(comparison(f"{key}.{i}.hours",r.get("x"),p["hours"],1e-10))
                    checks.append(comparison(f"{key}.{i}.fitted",r.get("y"),p["value"],1e-6,1e-6))
                checks.append(unverified(key+".confidence_interval","Rust confidence intervals are deliberately unavailable until R verification."))
    return checks


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream",type=Path)
    parser.add_argument("binary",nargs="?",default="target/release/poregauge",type=Path)
    parser.add_argument("--execute-r",action="store_true")
    parser.add_argument("--output",type=Path,default=Path("benchmark-results/minionqc-reference"))
    parser.add_argument("--rscript",default="Rscript")
    parser.add_argument("--docker",metavar="IMAGE")
    parser.add_argument("--case",action="append",default=[])
    parser.add_argument("--capture-only",action="store_true",help="Skip figure files; still execute upstream functions and ggplot_build")
    args=parser.parse_args(argv)
    upstream=args.upstream.resolve();binary=args.binary.resolve();output=args.output.resolve()
    assert subprocess.check_output(["git","-C",str(upstream),"rev-parse","HEAD"],text=True).strip()==REVISION
    assert subprocess.check_output(["git","-C",str(upstream),"status","--porcelain"],text=True)=="", "Reference source/fixtures must be clean"
    output.mkdir(parents=True,exist_ok=True)
    harness=Path(__file__).resolve().parent/"reference/capture.R"
    available=shutil.which("docker" if args.docker else args.rscript)
    report=dict(reference_commit=REVISION,source_sha256=digest(upstream/"MinIONQC.R"),cases=[])
    for name, paths, cutoff in cases(upstream, output):
        if args.case and name not in args.case: continue
        directory=output/name; directory.mkdir(exist_ok=True)
        pgdir=directory/"poregauge"
        result=subprocess.run([str(binary),"analyze",*map(str,paths),"--split-runs","--yaml","-q",str(cutoff),"-o",str(pgdir)],capture_output=True,text=True)
        (directory/"poregauge.log").write_text(result.stdout+result.stderr)
        if result.returncode: raise RuntimeError(result.stderr)
        pg=json.loads((pgdir/"combined/report.json").read_text())
        probe=directory/"source-population.json"
        probe_env=dict(__import__('os').environ,PG_PROBE_INPUTS=json.dumps(list(map(str,paths))),PG_PROBE_CUTOFF=str(cutoff),PG_PROBE_OUTPUT=str(probe),PG_PROBE_CURVES="1" if available else "0")
        checked=subprocess.run(["cargo","test","reference_probe::export_report","--","--ignored","--exact"],env=probe_env,capture_output=True,text=True,cwd=Path(__file__).resolve().parents[1])
        if checked.returncode: raise RuntimeError(checked.stderr)
        source_population=json.loads(probe.read_text())
        # Keep original source bytes, including legacy columns and RNA controls.
        inputdir=directory/"inputs"; inputdir.mkdir(exist_ok=True)
        copied=[]
        for i,path in enumerate(paths):
            dest=inputdir/f"source_{i+1:03}"/"sequencing_summary.txt"
            dest.parent.mkdir(exist_ok=True);shutil.copyfile(path,dest);copied.append(dest)
        rdir=directory/"minionqc";rdir.mkdir(exist_ok=True)
        capture=directory/"capture.json"
        capture.unlink(missing_ok=True) # Never reuse a stale successful R capture.
        env=dict(__import__('os').environ,PG_REFERENCE_SOURCE=str(upstream/"MinIONQC.R"),PG_REFERENCE_CAPTURE=str(capture),PG_REFERENCE_RENDER=str(not args.capture_only).lower())
        cli=[str(harness),"-i",str(inputdir if len(paths)>1 else copied[0]),"-o",str(rdir),"-q",str(cutoff),"-p","1","-f","pdf","-s","TRUE"]
        if args.docker:
            # Same absolute paths inside the container; no shell interpolation.
            mounts=set([upstream,output,harness.parent])
            command=["docker","run","--rm",*[v for path in sorted(mounts) for v in ["-v",f"{path}:{path}"]],*[v for k in ["PG_REFERENCE_SOURCE","PG_REFERENCE_CAPTURE","PG_REFERENCE_RENDER"] for v in ["-e",f"{k}={env[k]}"]],args.docker,*cli]
        else: command=[args.rscript,*cli]
        if available:
            result=subprocess.run(command,env=env,capture_output=True,text=True)
            (directory/"minionqc.log").write_text(result.stdout+result.stderr)
        reference=json.loads(capture.read_text()) if capture.exists() else None
        rows,excluded=observations(paths)
        checks=metrics(reference,pg,rows,cutoff) if reference else [unverified("reference_execution","R/container unavailable or failed; inspect minionqc.log. No R comparisons fabricated.")]
        if reference:
            channel_checks=metrics(reference,source_population,rows,cutoff)
            checks.extend(c for c in channel_checks if c["metric"].startswith("channel."))
            checks.extend(fitted_comparisons(reference,source_population))
        if reference and reference.get("error"):
            checks.append(unverified("reference_completion",reference["error"]))
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
        report["cases"].append(dict(name=name,qscore_cutoff=cutoff,inputs=[dict(sha256=digest(p),bytes=p.stat().st_size) for p in paths],excluded_controls=excluded,reference_executed=reference is not None,checks=checks))
        print(name,dict(Counter(c["status"] for c in checks)))
    statuses=Counter(c["status"] for case in report["cases"] for c in case["checks"])
    report["status_counts"]=dict(statuses)
    (output/"comparison.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")
    return 1 if statuses[STATUSES[3]] else 2 if statuses[STATUSES[5]] else 0


if __name__=="__main__": sys.exit(main())
