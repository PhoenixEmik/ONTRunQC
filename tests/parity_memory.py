#!/usr/bin/env python3
"""Optional focused memory check; never part of normal CI or a real-run benchmark."""
import argparse
import json
from pathlib import Path
import resource
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--reads", type=int, default=1_000_000)
parser.add_argument("--output", type=Path, default=Path("benchmark-results/parity-memory"))
parser.add_argument("--binary", type=Path, default=Path("target/release/poregauge"))
args = parser.parse_args()
if args.reads < 1:
    parser.error("--reads must be positive")
args.output.mkdir(parents=True, exist_ok=True)
path = args.output / "synthetic.tsv"
with path.open("w") as stream:
    stream.write("run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\n")
    for i in range(args.reads):
        channel = i % 3000 + 1
        group = (i // 3000) % 4
        native = "TRUE" if group & 2 else "FALSE"
        qscore = 12 if group & 1 else 5
        stream.write(f"fictional-memory\t{channel}\t1\t0\t1\t{native}\t{i % 100000 + 1}\t{qscore}\tnormal\n")
subprocess.run([str(args.binary.resolve()), "analyze", str(path), "--output", str(args.output / "report")], check=True)
# This fresh process has exactly one child: the CLI. macOS reports bytes,
# Linux reports KiB. Parent fixture generation is not included in this value.
usage = resource.getrusage(resource.RUSAGE_CHILDREN)
rss_bytes = usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024)
report = json.loads((args.output / "report/report.json").read_text())
assert report["metrics"]["total_records"] == args.reads
print(json.dumps({"records": args.reads, "channels": len(report["channels"]),
                  "input_bytes": path.stat().st_size, "peak_rss_bytes": rss_bytes,
                  "peak_rss_mib": rss_bytes / 1048576}, indent=2))
