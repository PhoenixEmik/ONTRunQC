#!/usr/bin/env python3
"""Optional static-export decoder/measurement suite (no R or private data).

Requires ImageMagick 7, Poppler (pdfinfo/pdftoppm/pdftotext), Ghostscript and
the existing pinned public MinIONQC checkout. Writes ignored artifacts only.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

FORMATS = ["svg", "png", "pdf", "ps", "jpeg", "tiff", "bmp"]
REVISION = "54485d586038d8ee36f168dd94f31c423f52f581"


def run(command):
    result = subprocess.run(list(map(str, command)), capture_output=True, text=True)
    assert result.returncode == 0, (command, result.stdout, result.stderr)
    return result.stdout.strip()


def rgb_hash(magick, path):
    digest = hashlib.sha256()
    process = subprocess.Popen([magick, str(path), "-depth", "8", "rgb:-"], stdout=subprocess.PIPE)
    while chunk := process.stdout.read(1024 * 1024):
        digest.update(chunk)
    assert process.wait() == 0
    return digest.hexdigest()


def measure(binary, inputs, output, options, format):
    output.mkdir(parents=True, exist_ok=True)
    command = [str(binary), "analyze", *map(str, inputs), *options, "--add-stat", "--format", format, "--output", str(output)]
    begin = time.perf_counter()
    with (output / "execution.log").open("wb") as log:
        child = subprocess.Popen(command, stdout=log, stderr=log)
        _, status, usage = os.wait4(child.pid, 0)
        child.returncode = os.waitstatus_to_exitcode(status)
    assert child.returncode == 0, (output / "execution.log").read_text()
    return dict(wall_seconds=time.perf_counter() - begin, user_seconds=usage.ru_utime,
                system_seconds=usage.ru_stime,
                peak_rss_bytes=usage.ru_maxrss * (1 if os.uname().sysname == "Darwin" else 1024),
                figure_bytes=sum(f.stat().st_size for f in output.rglob(f"*.{format}") if f.name != "report.html"))


def inspect(path, svg, args, decoded):
    root = ET.parse(svg).getroot()
    mm = [float(root.attrib[k].removesuffix("mm")) for k in ["width", "height"]]
    expected = [round(v / 25.4 * 300) for v in mm]
    format = path.suffix[1:]
    if format == "pdf":
        info = run(["pdfinfo", "-box", path])
        page = re.search(r"(?:Page\s+)?MediaBox:\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)", info)
        assert page, info
        values = list(map(float, page.groups()))
        assert all(abs(a-b*72/25.4) < .02 for a,b in zip(values[2:], mm)), (path, values, mm)
        assert "Pages:           1" in info
        text = run(["pdftotext", path, "-"])
        assert len(text.strip()) > 15, (path, "missing selectable text")
        run(["pdftoppm", "-r", "300", "-png", "-singlefile", path, decoded.with_suffix("")])
    elif format == "ps":
        content = path.read_text()
        page = re.search(r"%%HiResBoundingBox: 0 0 ([\d.]+) ([\d.]+)", content)
        assert page and all(abs(float(a)-b*72/25.4) < .002 for a,b in zip(page.groups(), mm))
        run([args.gs, "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=png16m", "-r300", f"-sOutputFile={decoded}", path])
    else:
        # Mean forces an actual pixel decode, rather than header-only identify.
        run([args.magick, path, "-format", "%[fx:mean]", "info:"])
    image = decoded if format in ["pdf", "ps"] else path
    info = run([args.magick, image, "-format", "%w %h %x %y %[units]", "info:"]).split()
    dimensions = list(map(int, info[:2]))
    assert all(abs(a-b) <= 1 for a,b in zip(dimensions, expected)), (path, dimensions, expected)
    density = None
    if format in ["png", "jpeg", "tiff", "bmp"]:
        density = float(info[2]) * (2.54 if info[4] == "PixelsPerCentimeter" else 1)
        assert abs(density - 300) < .01, (path, info)
    return dict(file=str(path), dimensions=dimensions, svg_mm=mm, dpi=density,
                rgb_sha256=rgb_hash(args.magick, image), rendered_file=str(image))


def fixture(path, run_id, count=120):
    header = "run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\n"
    rows = [f"{run_id}\t{i%16+1}\t{i%5+1}\t{i*73}\t1\t{'TRUE' if i%3 else 'FALSE'}\t{100+i*37%10000}\t{6+i%12}\tfictional\n" for i in range(count)]
    path.write_text(header + "".join(rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--binary", type=Path, default=Path("target/release/poregauge"))
    parser.add_argument("--output", type=Path, default=Path("benchmark-results/static-exports"))
    parser.add_argument("--gs", default="gs")
    parser.add_argument("--magick", default="magick")
    parser.add_argument("--case", choices=["synthetic", "combined", "small-minion", "dense-minion"], action="append")
    args = parser.parse_args()
    for tool in [args.gs, args.magick, "pdfinfo", "pdftoppm", "pdftotext"]:
        assert shutil.which(tool), f"Decoder unavailable: {tool}; comparisons are not verified"
    upstream = args.upstream.resolve()
    assert run(["git", "-C", upstream, "rev-parse", "HEAD"]) == REVISION
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    a, b = output / "synthetic-a.tsv", output / "synthetic-b.tsv"
    fixture(a, "fictional-export-a")
    fixture(b, "fictional-export-b", 150)
    cases = [
        ("synthetic", [a], ["--flowcell-layout", "promethion-3000", "--smallfigures", "--minionqc-trends", "--muxes", "0.25"], ["length-counts", "spatial-total-yield", "fitted-false-false"]),
        ("combined", [a, b], ["--combined-only", "--minionqc-trends", "--minionqc-density", "--smallfigures"], ["length-counts", "combined-yield-time-false", "combined-distribution-false-false", "fitted-false-false"]),
        ("small-minion", [upstream / "testfiles/short_run/sequencing_summary.txt"], ["--split-runs", "--flowcell-layout", "minion-r9.5"], ["minion-length-quality", "minion-flowcell-overview"]),
        ("dense-minion", [upstream / "example_input_minion/RB7_A2/sequencing_summary.txt"], ["--split-runs", "--flowcell-layout", "minion-r9.5"], ["minion-length-quality", "minion-flowcell-overview"]),
    ]
    results = []
    environment = dict(platform=sys.platform, python=sys.version.split()[0],
                       rust=run(["rustc", "--version"]),
                       ghostscript=run([args.gs, "--version"]),
                       imagemagick=run([args.magick, "--version"]).splitlines()[0],
                       binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest())
    for name, inputs, options, representatives in cases:
        if args.case and name not in args.case:
            continue
        destination = output / name
        baseline = None
        for format in FORMATS:
            directory = destination / format
            timing = measure(args.binary.resolve(), inputs, directory, options, format)
            reports = sorted(directory.rglob("report.json"))
            assert reports
            references = {}
            for report in reports:
                data = json.loads(report.read_text())
                data.pop("presentation") # Only rendering controls differ.
                relative = report.relative_to(directory)
                references[str(relative)] = data
                if baseline is not None:
                    assert baseline[str(relative)] == data, "Scientific JSON changed"
            if format == "svg":
                baseline = references
                continue
            checks = []
            seen = set()
            for manifest in sorted(directory.rglob("manifest.json")):
                relative = manifest.parent.relative_to(directory)
                names = json.loads(manifest.read_text())
                expected = [n.removesuffix(".svg") + "." + format for n in json.loads((destination / "svg" / relative / "manifest.json").read_text())]
                assert names == expected and len(set(names)) == len(names)
                for item in names:
                    path = manifest.parent / item
                    assert path.is_file() and path.stat().st_size > 20
                    if path.stem not in representatives:
                        continue
                    seen.add(path.stem)
                    decoded = path.with_name(path.stem + "-decoded.png")
                    check = inspect(path, destination / "svg" / relative / (path.stem + ".svg"), args, decoded)
                    # All lossless RGB formats must agree pixel for pixel.
                    if format in ["ps", "tiff", "bmp"]:
                        png = destination / "png" / relative / (path.stem + ".png")
                        assert check["rgb_sha256"] == rgb_hash(args.magick, png)
                    checks.append(check)
            assert checks, f"No representative figures checked: {name}/{format}"
            assert seen == set(representatives), (name, format, "Missing required plot families", seen)
            results.append(dict(case=name, format=format, measurements=timing, decoded=checks))
            (output / "measurements.json").write_text(json.dumps(dict(reference_commit=REVISION, environment=environment, results=results), indent=2) + "\n")
            print(name, format, timing, "decoded", len(checks), flush=True)
    print("All selected format/population/dimension/decoder checks passed. No R fitting targets were changed.")


if __name__ == "__main__":
    main()
