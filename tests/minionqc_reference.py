#!/usr/bin/env python3
"""Optional public-example regression; standard library only, no R installation.

Usage: python3 tests/minionqc_reference.py UPSTREAM_CHECKOUT [POREGAUGE_BINARY]
Clone https://github.com/roblanf/minion_qc and check out REVISION below first.
Inputs/normalized files/reports remain temporary; no research rows are committed.
"""
import bisect
import csv
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile

REVISION = "54485d586038d8ee36f168dd94f31c423f52f581"
THRESHOLDS = [10000, 20000, 50000, 100000, 200000, 500000, 1000000]
INPUT_SHA256 = {
    "testfiles/short_run/sequencing_summary.txt": "60a070e325b0c2e3e102b9e7d27c10e1abbe6c5f4e3a18fadba304dabb1cc571",
    "example_input_promethion/sequencing_summary.txt": "dd508d9dc645519e1c677fad2e29cc08be372c809bf56d26857e15f8ca20f4ef",
}


def expected_summary(path):
    # Read only the numeric mappings in upstream's known summary.yaml fixture.
    # This is not a general YAML parser or a PoreGauge export implementation.
    result = {}
    group = section = None
    for line in path.read_text().splitlines():
        if line in ("All reads:", "Q>=7:"):
            group = line[:-1]
            result[group] = {}
            section = None
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


def n50(lengths):
    total = sum(lengths)
    cumulative = 0
    for length in sorted(lengths, reverse=True):
        cumulative += length
        if cumulative >= (total + 1) // 2:
            return length
    return None


def check_example(upstream, binary, source, summary, cutoff=7):
    input_path = upstream / source
    assert hashlib.sha256(input_path.read_bytes()).hexdigest() == INPUT_SHA256[source], "Public input differs from the pinned fixture"
    with input_path.open() as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    expected = expected_summary(upstream / summary)
    with tempfile.TemporaryDirectory(prefix="poregauge-public-") as directory:
        directory = Path(directory)
        normalized = directory / "input.tsv"
        # Only a validation adapter: supply absent mux/reason with neutral tags,
        # replace public run IDs, and retain the actual native flag if available.
        # This does not claim general legacy input support (#9).
        fields = ["run_id", "channel", "mux", "start_time", "duration",
                  "sequence_length_template", "mean_qscore_template", "end_reason"]
        native = "passes_filtering" in rows[0]
        if native:
            fields.append("passes_filtering")
        with normalized.open("w") as stream:
            writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
            writer.writeheader()
            for row in rows:
                output = {k: row[k] for k in fields if k in row}
                output.update(run_id="public-example-validation", mux=row.get("mux", "1"), end_reason=row.get("end_reason", "unspecified"))
                # Three older PromethION rows have invalid negative Q values.
                # Normalize these to zero ONLY in this temporary adapter; this
                # preserves length statistics and Q>=7 selection, not all-read Q.
                output["mean_qscore_template"] = str(max(0.0, float(row["mean_qscore_template"])))
                writer.writerow(output)
        process = subprocess.run([binary, "analyze", str(normalized), "--qscore-cutoff", str(cutoff), "--output", str(directory / "report")], capture_output=True, text=True)
        assert process.returncode == 0, process.stderr
        report = json.loads((directory / "report/report.json").read_text())
    assert report["metrics"]["total_records"] == len(rows)
    comparison = report["comparison"]
    assert comparison["native_filtering_available"] == native
    positives = [r for r in rows if int(r["sequence_length_template"]) > 0]
    all_lengths = [int(r["sequence_length_template"]) for r in positives]
    assert report["metrics"]["basecalled_reads"] == len(positives)
    assert report["metrics"]["n50_bp"] == n50(all_lengths)
    assert report["metrics"]["median_read_length_bp"] == statistics.median(all_lengths)
    for group, selected in [("All reads", False), ("Q>=7", True)]:
        source_rows = [r for r in rows if not selected or float(r["mean_qscore_template"]) >= cutoff]
        basecalled = [r for r in source_rows if int(r["sequence_length_template"]) > 0]
        lengths = [int(r["sequence_length_template"]) for r in basecalled]
        q = [float(r["mean_qscore_template"]) for r in basecalled]
        m = comparison["qscore_selected"] if selected else report["metrics"]
        count = m["read_count"] if selected else m["basecalled_reads"]
        assert count == len(basecalled)
        assert m["total_yield_bp"] == sum(lengths)
        assert m["median_read_length_bp"] == statistics.median(lengths)
        assert m["n50_bp"] == n50(lengths)
        if selected:
            assert abs(m["mean_qscore"] - statistics.mean(q)) < 1e-10
            assert m["median_qscore"] == statistics.median(q)
        e = expected[group]
        assert e["total.reads"] == len(source_rows)
        assert abs(e["total.gigabases"] * 1e9 - sum(lengths)) <= 50.1
        assert e["N50.length"] == m["n50_bp"]
        # Upstream rounds means/medians to one decimal and includes zero lengths.
        source_lengths = [int(r["sequence_length_template"]) for r in source_rows]
        source_q = [float(r["mean_qscore_template"]) for r in source_rows]
        for field, actual in [("mean.length", statistics.mean(source_lengths)), ("median.length", statistics.median(source_lengths)), ("mean.q", statistics.mean(source_q)), ("median.q", statistics.median(source_q))]:
            assert abs(e[field] - actual) <= 0.0500001, (group, field, e[field], actual)
        prefix = "qscore_selected" if selected else "all"
        for threshold, t in zip(THRESHOLDS, comparison["yield_by_minimum_length"]["thresholds"]):
            above = [length for length in lengths if length >= threshold]
            key = ">1m" if threshold == 1000000 else f">{threshold // 1000}kb"
            assert t[f"{prefix}_read_count"] == len(above) == e["reads"][key]
            assert t[f"{prefix}_yield_bp"] == sum(above)
            assert abs(e["gigabases"][key] * 1e9 - sum(above)) <= 50.1
        ultra = comparison["yield_by_minimum_length"]["ultralong_qscore_selected" if selected else "ultralong_all"]
        assert ultra["read_count"] == e["reads"]["ultralong"], (group, ultra, e)
        assert abs(ultra["total_yield_bp"] - e["gigabases"]["ultralong"] * 1e9) <= 50.1
        # Independently locate the maximal prefix using prefix sums and strict N50.
        descending = sorted(lengths, reverse=True)
        cumulative = []
        total = 0
        for length in descending:
            total += length
            cumulative.append(total)
        lo, hi = 0, len(descending)
        while lo < hi:
            mid = (lo + hi + 1) // 2
            index = bisect.bisect_right(cumulative, cumulative[mid - 1] // 2)
            if descending[index] >= 100000:
                lo = mid
            else:
                hi = mid - 1
        assert ultra["read_count"] == lo
        assert ultra["total_yield_bp"] == (cumulative[lo - 1] if lo else 0)
    by_channel = {}
    for row in rows:
        by_channel.setdefault(int(row["channel"]), []).append(row)
    for channel in comparison["channels"]:
        records = by_channel[channel["channel"]]
        for key, group in [("all_summary_records", records), ("all_basecalled_reads", [r for r in records if int(r["sequence_length_template"]) > 0]), ("qscore_selected_reads", [r for r in records if int(r["sequence_length_template"]) > 0 and float(r["mean_qscore_template"]) >= cutoff])]:
            lengths = [int(r["sequence_length_template"]) for r in group]
            actual = channel[key]
            assert actual["read_count"] == len(lengths)
            assert actual["total_yield_bp"] == sum(lengths)
            assert actual["median_read_length_bp"] == (statistics.median(lengths) if lengths else None)
    digest = hashlib.sha256(input_path.read_bytes()).hexdigest()
    print(f"Adapter negative-Q rows: {sum(float(r['mean_qscore_template']) < 0 for r in rows)}; all-read Q statistics of normalized input are not compared with upstream.")
    print(f"PASS {source} (Q>={cutoff}): {len(rows):,} records, {sum(all_lengths):,} bp; SHA256 {digest}")


def main():
    upstream = Path(sys.argv[1]).resolve()
    binary = str(Path(sys.argv[2] if len(sys.argv) > 2 else "target/release/poregauge").resolve())
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    assert revision == REVISION, f"Expected upstream {REVISION}, found {revision}"
    # Upstream tests.R invokes short_run with q=8 while the global q_title
    # remains Q>=7; its checked-in YAML is numerically Q>=8, mislabeled Q>=7.
    check_example(upstream, binary, "testfiles/short_run/sequencing_summary.txt", "testfiles/short_run/summary.yaml", cutoff=8)
    check_example(upstream, binary, "example_input_promethion/sequencing_summary.txt", "example_output_promethion/summary.yaml")
    print("Verified public summaries, thresholds, ultralong and independent exact channel medians; R was not executed.")


if __name__ == "__main__":
    main()
