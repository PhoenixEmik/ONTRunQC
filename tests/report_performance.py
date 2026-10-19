#!/usr/bin/env python3
"""Optional synthetic report comparison; Python/Rust stdlib only. See docs/benchmark.md."""
import argparse
from collections import Counter
from html.parser import HTMLParser
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


class Counts(HTMLParser):
    def __init__(self):
        super().__init__()
        self.tags = Counter()
        self.microbars = 0

    def handle_starttag(self, tag, attrs):
        self.tags[tag] += 1
        if tag == "svg" and "inline-bar" in dict(attrs).get("class", "").split():
            self.microbars += 1

    handle_startendtag = handle_starttag


def measure(path):
    html = (path / "report.html").read_text()
    parser = Counts()
    parser.feed(html)
    return {"html_bytes": (path / "report.html").stat().st_size,
            "json_bytes": (path / "report.json").stat().st_size,
            "source_elements": sum(parser.tags.values()),
            "svg": parser.tags["svg"], "rect": parser.tags["rect"],
            "tr": parser.tags["tr"], "svg_microbars": parser.microbars}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/ontrunqc")
    parser.add_argument("--baseline-binary", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "benchmark-results/html-performance")
    parser.add_argument("--browser", action="store_true", help="also create an offline browser measurement page")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    generator = output / "large-tables-generator"
    subprocess.run(["rustc", "--edition=2024", "-O", str(ROOT / "tests/fixtures/large_tables.rs"), "-o", str(generator)], check=True)
    fixture = output / "large-tables.tsv"
    with fixture.open("wb") as data:
        subprocess.run([str(generator)], stdout=data, check=True)
    measurements = {"fixture_bytes": fixture.stat().st_size,
                    "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest()}
    reports = {}
    for label, binary in [("before", args.baseline_binary), ("after", args.binary)]:
        if binary is None:
            continue
        path = output / label
        subprocess.run([str(binary.resolve()), "analyze", str(fixture), "--flowcell-layout", "promethion-3000", "--output", str(path)], check=True)
        report = json.loads((path / "report.json").read_text())
        assert report["metrics"]["total_records"] == 20000
        assert len(report["channels"]) == 3000
        assert len(report["diagnostics"]["channel_mux"]) == 10000
        reports[label] = report
        measurements[label] = measure(path)
    if "before" in reports:
        before, after = (json.loads(json.dumps(reports[label])) for label in ("before", "after"))
        before["tool"].pop("version")
        after["tool"].pop("version")
        assert before == after, "JSON changed beyond tool.version"
        measurements["identical_json_except_tool_version"] = True
    assert measurements["after"]["svg_microbars"] == 0
    (output / "measurements.json").write_text(json.dumps(measurements, indent=2) + "\n")
    print(json.dumps(measurements, indent=2))
    if args.browser:
        report = reports["after"]
        expected = {}
        for prefix, source in [("channel", report["channels"]), ("channel-mux", report["diagnostics"]["channel_mux"])]:
            expected[prefix] = [{"channel": str(row["channel"]), "mux": str(row.get("mux", 0)),
                                 "yield": str(row["total_yield_bp"]), "reads": str(row["basecalled_reads"]),
                                 "qscore": row.get("mean_qscore"), "meanlength": row.get("mean_read_length_bp"),
                                 "passedlength": row.get("mean_passed_read_length_bp")} for row in source]
        cases = [{"label": label, "html": (output / label / "report.html").read_text()} for label in reports]
        data = json.dumps({"cases": cases, "expected": expected}, separators=(",", ":")).replace("<", "\\u003c")
        template = (ROOT / "tests/report_browser.html").read_text()
        (output / "browser.html").write_text(template.replace("/* REPORT_DATA */", data))
        print(f"Open {output / 'browser.html'} in a browser, then press Run checks. Results are shown on the page; timings are optional, not CI gates.")


if __name__ == "__main__":
    main()
