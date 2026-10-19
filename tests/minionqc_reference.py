#!/usr/bin/env python3
"""Optional upstream public-input validation; standard library only, no R runtime.

Usage: python3 tests/minionqc_reference.py UPSTREAM_CHECKOUT [ONTRUNQC_BINARY]
No input adapters or private datasets. Reports live in a temporary directory.
"""
import bisect
import csv
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile

REVISION = "54485d586038d8ee36f168dd94f31c423f52f581"
THRESHOLDS = [10000, 20000, 50000, 100000, 200000, 500000, 1000000]


def expected_summary(path):
    """Read the numeric mappings of the known upstream YAML, not general YAML."""
    result = {}
    group = section = None
    for line in path.read_text().splitlines():
        if line in ("All reads:", "Q>=7:"):
            group = line[:-1]
            result[group] = {}
        elif group and line.startswith("  "):
            key, value = line.strip().split(":", 1)
            key = key.strip("'")
            if not value.strip():
                section = key
                result[group][key] = {}
            elif line.startswith("    "):
                result[group][section][key] = float(value)
            else:
                result[group][key] = float(value)
        elif line and not line.startswith(" "):
            group = None
    return result


def projected_yaml(path):
    """Parse the writer's quoted mapping/scalar subset (YAML 1.2)."""
    root, stack = {}, []
    stack.append((-1, root))
    for line in path.read_text().splitlines():
        if line == "---":
            continue
        indent = len(line) - len(line.lstrip())
        key, value = line.lstrip().split(":", 1)
        while stack[-1][0] >= indent:
            stack.pop()
        key = json.loads(key)
        current = {} if not value else json.loads(value)
        stack[-1][1][key] = current
        if not value:
            stack.append((indent, current))
    return root


def check_projected_yaml(summary, report, cutoff):
    for selected in [False, True]:
        section = summary[f"Q>={cutoff}" if selected else "All reads"]
        m = report["comparison"]["qscore_selected"] if selected else report["metrics"]
        q = m if selected else report["quality"]
        assert section["total.reads"] == m["read_count" if selected else "basecalled_reads"]
        assert section["total.bases"] == m["total_yield_bp"]
        for external, internal in [("N50.length", "n50_bp"), ("mean.length", "mean_read_length_bp"), ("median.length", "median_read_length_bp"), ("max.length", "max_read_length_bp")]:
            assert section[external] == m[internal]
        assert section["mean.q"] == q["mean_qscore"]
        assert section["median.q"] == q["median_qscore"]
        for threshold in report["comparison"]["yield_by_minimum_length"]["thresholds"]:
            n = threshold["minimum_length_bp"]
            key = ">1m" if n == 1000000 else f">{n//1000}kb"
            prefix = "qscore_selected" if selected else "all"
            assert section["reads"][key] == threshold[f"{prefix}_read_count"]
            assert abs(section["gigabases"][key]*1e9 - threshold[f"{prefix}_yield_bp"]) < 0.001
        ultra = report["comparison"]["yield_by_minimum_length"]["ultralong_qscore_selected" if selected else "ultralong_all"]
        assert section["reads"]["ultralong"] == ultra["read_count"]


def n50(lengths):
    total = sum(lengths)
    cumulative = 0
    for length in sorted(lengths, reverse=True):
        cumulative += length
        if cumulative >= (total + 1) // 2:
            return length
    return None


def observations(paths):
    result, excluded = [], 0
    for path in paths:
        with path.open() as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            two_d = "sequence_length_2d" in reader.fieldnames
            length_key = "sequence_length_2d" if two_d else "sequence_length_template"
            q_key = "mean_qscore_2d" if two_d else "mean_qscore_template"
            for row in reader:
                if row.get("calibration_strand_genome_template") == "YHR174W":
                    excluded += 1
                else:
                    result.append((int(row[length_key]), float(row[q_key]), int(row["channel"]), row.get("run_id")))
    return result, excluded


def check_metrics(report, rows, cutoff):
    positives = [r for r in rows if r[0] > 0]
    lengths = [r[0] for r in positives]
    assert report["metrics"]["total_records"] == len(rows)
    assert report["metrics"]["basecalled_reads"] == len(positives)
    for selected in [False, True]:
        cohort = [r for r in positives if not selected or r[1] >= cutoff]
        lengths = sorted(r[0] for r in cohort)
        qscores = [r[1] for r in cohort]
        m = report["comparison"]["qscore_selected"] if selected else report["metrics"]
        quality = m if selected else report["quality"]
        count = m["read_count"] if selected else m["basecalled_reads"]
        assert count == len(lengths)
        assert m["total_yield_bp"] == sum(lengths)
        assert m["n50_bp"] == n50(lengths)
        assert m["median_read_length_bp"] == (statistics.median(lengths) if lengths else None)
        if lengths:
            assert abs(m["mean_read_length_bp"] - statistics.mean(lengths)) < 1e-8
            assert abs(quality["mean_qscore"] - statistics.mean(qscores)) < 1e-10
            assert abs(quality["median_qscore"] - statistics.median(qscores)) < 1e-12
        prefix = "qscore_selected" if selected else "all"
        for t, length in zip(report["comparison"]["yield_by_minimum_length"]["thresholds"], THRESHOLDS):
            above = [n for n in lengths if n >= length]
            assert t[f"{prefix}_read_count"] == len(above)
            assert t[f"{prefix}_yield_bp"] == sum(above)
        descending = list(reversed(lengths))
        cumulative, total = [], 0
        for length in descending:
            total += length
            cumulative.append(total)
        lo, hi = 0, len(descending)
        while lo < hi:
            mid = (lo + hi + 1) // 2
            index = bisect.bisect_right(cumulative, cumulative[mid-1] // 2)
            if descending[index] >= 100000:
                lo = mid
            else:
                hi = mid-1
        ultra = report["comparison"]["yield_by_minimum_length"]["ultralong_qscore_selected" if selected else "ultralong_all"]
        assert ultra["read_count"] == lo
        assert ultra["total_yield_bp"] == (cumulative[lo-1] if lo else 0)
    by_channel = {}
    for row in rows:
        by_channel.setdefault(row[2], []).append(row)
    for channel in report["comparison"]["channels"]:
        records = by_channel[channel["channel"]]
        for key, cohort in [("all_summary_records", records), ("all_basecalled_reads", [r for r in records if r[0] > 0]), ("qscore_selected_reads", [r for r in records if r[0] > 0 and r[1] >= cutoff])]:
            lengths = [r[0] for r in cohort]
            assert channel[key]["read_count"] == len(lengths)
            assert channel[key]["median_read_length_bp"] == (statistics.median(lengths) if lengths else None)


def check_yaml(report, expected):
    for name, selected in [("All reads", False), ("Q>=7", True)]:
        e = expected[name]
        m = report["comparison"]["qscore_selected"] if selected else report["metrics"]
        assert e["total.reads"] == (m["read_count"] if selected else m["total_records"])
        assert abs(e["total.gigabases"]*1e9 - m["total_yield_bp"]) <= 50.1
        assert e["N50.length"] == m["n50_bp"]
        for threshold in report["comparison"]["yield_by_minimum_length"]["thresholds"]:
            length = threshold["minimum_length_bp"]
            key = ">1m" if length == 1000000 else f">{length//1000}kb"
            prefix = "qscore_selected" if selected else "all"
            assert threshold[f"{prefix}_read_count"] == e["reads"][key]
            assert abs(threshold[f"{prefix}_yield_bp"] - e["gigabases"][key]*1e9) <= 50.1
        ultra = report["comparison"]["yield_by_minimum_length"]["ultralong_qscore_selected" if selected else "ultralong_all"]
        assert ultra["read_count"] == e["reads"]["ultralong"]
        assert abs(ultra["total_yield_bp"] - e["gigabases"]["ultralong"]*1e9) <= 50.1


def main():
    upstream = Path(sys.argv[1]).resolve()
    binary = str(Path(sys.argv[2] if len(sys.argv)>2 else "target/release/ontrunqc").resolve())
    assert subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip() == REVISION
    cases = [
        ("short", ["testfiles/short_run/sequencing_summary.txt"], 8, "testfiles/short_run/summary.yaml"),
        ("promethion", ["example_input_promethion/sequencing_summary.txt"], 7, "example_output_promethion/summary.yaml"),
        ("1d2", ["testfiles/1D2/sequencing_summary.txt"], 8, "testfiles/1D2/summary.yaml"),
        ("rna", ["testfiles/directRNA/Direct_RNA_sequencing_summary.txt"], 7, "testfiles/directRNA/summary.yaml"),
        ("multi-minion", ["example_input_minion/RB7_A2/sequencing_summary.txt", "example_input_minion/RB7_D3/sequencing_summary.txt"], 7, None),
    ]
    for name, names, cutoff, summary in cases:
        paths = [upstream/n for n in names]
        rows, excluded = observations(paths)
        with tempfile.TemporaryDirectory(prefix="ontrunqc-upstream-") as directory:
            result = subprocess.run([binary, "analyze", *map(str,paths), "--split-runs", "--yaml", "-q", str(cutoff), "-o", directory], capture_output=True, text=True)
            assert result.returncode == 0, result.stderr
            report = json.loads((Path(directory)/"combined/report.json").read_text())
            check_projected_yaml(projected_yaml(Path(directory)/"combined/summary.yaml"), report, cutoff)
            for individual in sorted((Path(directory)/"runs").glob("*/report.json")):
                single = json.loads(individual.read_text())
                check_projected_yaml(projected_yaml(individual.with_name("summary.yaml")), single, cutoff)
                check_metrics(single, [r for r in rows if r[3] == single["run"]["run_id"]], cutoff)
        check_metrics(report, rows, cutoff)
        assert report["input"]["excluded_control_records"] == excluded
        if summary:
            check_yaml(report, expected_summary(upstream/summary))
        print(f"PASS {name}: {len(rows):,} records, {sum(r[0] for r in rows):,} bp, {excluded} controls excluded, Q>={cutoff}; {len(report['collection']['runs'])} run/source groups")
    print("Original public inputs passed independently calculated pooled statistics and available upstream YAML counts/yields/N50/thresholds/ultralong. R was not executed. Short/1D2 stored Q>=7 labels correspond numerically to Q8; no labels or scores were rewritten.")


if __name__ == "__main__":
    if "--execute-r" in sys.argv:
        from minionqc_differential import main as differential_main
        sys.exit(differential_main())
    main()
