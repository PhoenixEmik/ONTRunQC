# ONTRunQC

**Oxford Nanopore sequencing-run quality control.**

A standalone Rust CLI that streams sequencing summaries, calculates exact core QC
statistics and produces self-contained HTML plus structured JSON. Supports
PromethION and MinION data, modern and legacy summaries, and single or combined
run reports.

Development package: **0.4.1** · Executable: `ontrunqc` · Rust **1.85+**

## Contents

- [Capabilities](#capabilities)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Common workflows](#common-workflows)
- [Input and scientific interpretation](#input-and-scientific-interpretation)
- [MinIONQC compatibility status](#minionqc-compatibility-status)
- [Operational limits](#operational-limits)
- [Development and validation](#development-and-validation)
- [Documentation](#documentation)

<a id="features"></a>

## Capabilities

| Area | Implemented analyses |
| --- | --- |
| Core QC | Exact length median/N50, yield and arithmetic per-read Q statistics; native pass and independent Q-cutoff subsets. |
| Distributions and time | Length/Q histograms, joint heatmaps, retained yield by length, ultralong subsets and 15-minute temporal metrics. |
| Channels and mux | Exact channel medians, channel × mux tables, explicit physical layouts and full-data MinION display plots. |
| Multiple inputs | Streaming plain/gzip TSV, run-aware chunk grouping, exact pooled statistics and bounded independent-run workers. |
| Reports | Offline HTML/JSON, optional YAML and independent SVG/PNG/PDF/PS/JPEG/TIFF/BMP figures. |

Normal analysis and export require neither R nor a browser. ONTRunQC is a
post-sequencing summary tool; it is not a basecaller, live MinKNOW monitor or
FASTQ/BAM/POD5 reader.

## Installation

### Build from source

Requires Rust **1.85 or newer**, including Cargo.

```sh
git clone https://github.com/PhoenixEmik/ONTRunQC.git
cd ONTRunQC
cargo build --release --locked
./target/release/ontrunqc --version
```

Run `./target/release/ontrunqc`, or install it on your Cargo executable path:

```sh
cargo install --path . --locked
```

For JSON/HTML/SVG without the six native conversion formats, build with
`cargo build --release --locked --no-default-features`.

### Prebuilt binaries

Download a matching archive and `SHA256SUMS` from
[GitHub Releases](https://github.com/PhoenixEmik/ONTRunQC/releases), verify the
checksum, and extract the archive.

| Platform | Archive platform suffix |
| --- | --- |
| Linux, Intel/AMD 64-bit | `linux-x86_64` |
| Linux, ARM64 | `linux-arm64` |
| macOS, Intel | `x86_64-apple-darwin` |
| macOS, Apple Silicon | `aarch64-apple-darwin` |

The current release workflow builds static Linux musl executables without a
system glibc dependency and smoke-tests them in Rocky Linux 8 containers.
Static Linux archives are intended for older HPC systems as well. Container
checks do not verify every host kernel, CPU or site policy. See the [release workflow](.github/workflows/release.yml).

## Quick start

After installation:

```sh
ontrunqc analyze sequencing_summary.txt --output qc_results/
ontrunqc analyze --help
```

```text
qc_results/
├── report.json
└── report.html
```

Open `report.html` directly using `file://` or double-click it. CSS, SVG, optional
JavaScript and any MinION raster images are embedded; the report can be shared
as one file without a server or network access. All scientific plots and table
data remain available without JavaScript.

To try fictional data directly from a source checkout:

```sh
./target/release/ontrunqc analyze tests/fixtures/synthetic.tsv --output qc_results/example/
```

This fixture has 6 summary records, 4 basecalled reads, 100 bp yield, a 30 bp N50
and a 25 bp median length. It demonstrates report behavior, not run performance.

## Common workflows

### Physical channel maps

The device is never inferred from channel IDs. Choose a compatible layout:

```sh
ontrunqc analyze sequencing_summary.txt.gz --output qc_results/ \
  --flowcell-layout promethion-3000 --qscore-cutoff 9
ontrunqc analyze sequencing_summary.txt --output minion_qc/ \
  --flowcell-layout minion-r9.5
```

Supported maps are PromethION 3000 (25×120) and MinION R9.5 (32×16). Incompatible
channel IDs fail with source context. [Definitions and model limitations](docs/metrics.md#explicit-physical-layout).

<a id="multiple-inputs-and-runs"></a>

### Multiple runs and chunks

```sh
ontrunqc analyze /data/flowcells/ --processors 2 --output collection_qc/
ontrunqc analyze chunk_a.tsv chunk_b.tsv.gz --combined-only --output pooled_qc/
ontrunqc analyze mixed_runs.tsv --split-runs --output split_qc/
```

Matching run IDs join file chunks; different runs stay separate. Identity-free
files are explicitly labelled sources. Pooled median/N50 use combined observations,
never averages of run statistics. `--combined-only` omits individual output;
workers process independent input components, so more workers need not be faster.
See [discovery, grouping and workers](docs/input.md).

<a id="reports-and-figure-export"></a>

### Figure export and optional estimators

```sh
ontrunqc analyze sequencing_summary.txt --output qc_results/ --yaml \
  --format pdf --smallfigures --add-stat --muxes 8
ontrunqc analyze /data/flowcells/ --output comparison_qc/ \
  --minionqc-trends --minionqc-density --minionqc-plot-bins
```

`--format` exports one independent format and a figure manifest. PNG/TIFF/BMP
are lossless 300-DPI RGB; JPEG is lossy, PDF preserves vector content where
possible, and **PS is raster-backed**. Optional raw-read fits cost an additional
input pass; the R-grid density estimator has its own documented definition.
`--minionqc-plot-bins` adds separately labeled ggplot2-compatible histograms and
Bin2D layers using one extra streaming pass; native statistics/bins are preserved.
See [plot population and bin definitions](docs/metrics.md#optional-minionqc-histogram-layers-19)
and [output files, dimensions and controls](docs/output.md).

<a id="supported-input"></a>
<a id="scientific-interpretation-and-minionqc-compatibility"></a>

## Input and scientific interpretation

- Input is **plain or gzip-compressed, unquoted TSV with a header**. Modern
  MinKNOW/Dorado and legacy Albacore/Guppy/2D/1D2 schemas are supported.
- Gzip is decoded as a stream. Concatenated members, CRC and end-of-stream are
  validated; no full temporary TSV is extracted. Each extra pass decompresses again.
- Summary records include zero lengths; basecalled length/Q statistics use
  positive-length reads. Native passing uses only `passes_filtering`;
  `--qscore-cutoff` independently selects mean Q≥cutoff (default 7).
- Mean Q is the arithmetic mean of per-read Q-scores. Missing statistics remain
  `null`, and missing native filtering is never inferred from Q.
- Time/yield/activity/speed diagnostics are summary-derived proxies attributed
  to read start, not exact throughput, pore availability or motor-speed measurements.

See [input schemas and RNA exclusions](docs/input.md) and
[metric definitions](docs/metrics.md) before comparing denominators or populations.

## MinIONQC compatibility status

ONTRunQC covers the major MinIONQC v1.4.1 analysis families. This does not imply
identical populations, bins, curves, rendering or CLI defaults.

| Evidence | Current interpretation |
| --- | --- |
| Exact core metrics and pooled summaries | Validated under the documented populations and edge-case definitions. |
| LOESS and optional R-grid KDE | Actual pinned-R validation within declared tolerances. |
| Original-R GAM | **Partial:** 1,542 fit/uncertainty discrepancies remain in the 18-case strict suite. |
| Histogram/Bin2D layers | Exact pinned-R comparisons with `--minionqc-plot-bins`; native fixed bins remain default. |
| Time/yield display layers | Native resolutions and representations still differ; tracked in #20–#21. |

Start with the [source-backed comparison](docs/minionqc-comparison.md),
[actual A/B/C figure gallery](docs/figures/minionqc-comparison/README.md) and
[discrepancy register](docs/figures/minionqc-comparison/discrepancies.json).
The [engineering ledger](docs/minionqc-parity.md) preserves historical acceptance
evidence; [reference validation](tests/reference/README.md) explains how to rerun it.

<a id="input-integrity-memory-and-limitations"></a>

## Operational limits

Keep inputs stable across all passes. Metadata/membership checks are always used;
`--verify-input-content` adds raw-source SHA-256 and publication rereads. Neither
mode is a snapshot or protection against all NFS/cache/writer races.
[Source-integrity guarantees](docs/input.md#multi-pass-input-integrity).

Completed files replace outputs individually; publication is **not atomic across
files or runs**. Exact statistics retain 16 bytes per basecalled read for numeric
length/Q elements, plus allocations/groups/display data. No below-512-MiB
real-run RSS guarantee is made. See [output publication](docs/output.md#publication-and-existing-files)
and [measurements/procedures](docs/benchmark.md).

No automatic run acceptance, pore occupancy estimation, live monitoring,
barcode analysis or multi-format read parsing is implemented.

## Development and validation

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo test --locked --no-default-features
cargo build --release --locked --no-default-features
python3 -m unittest discover -s tests/reference
python3 tests/check_docs.py
```

[CI](.github/workflows/ci.yml) also tests Rust 1.85 and static Linux exports.
Optional public-input and pinned-R differential checks are documented in
[reference validation](tests/reference/README.md); R is not a runtime dependency.
Use separate Cargo target directories if running different feature builds
concurrently. Keep private sequencing data and identifying reports out of Git.

[Release automation](.github/workflows/release.yml) uses matching
`vMAJOR.MINOR.PATCH` tags. Maintainers can build an existing tag without publishing
through its manual `tag`/`publish=false` inputs.

## Documentation

[Documentation index](docs/README.md) maps each guide to its purpose.

| Read next | Purpose |
| --- | --- |
| [Input and processing](docs/input.md) | Schemas, gzip, grouping, workers and integrity. |
| [Metrics](docs/metrics.md) | Populations, formulas, distributions and diagnostic definitions. |
| [Reports and exports](docs/output.md) | JSON schema, HTML/YAML, figures and publication behavior. |
| [MinIONQC comparison](docs/minionqc-comparison.md) | Source/plot mappings, current evidence and open differences. |
| [Validation harness](tests/reference/README.md) | Optional public/pinned-R checks and numerical tolerances. |
| [Benchmarks](docs/benchmark.md) | Reproduction procedures, historical measurements and [comparison figures](docs/benchmark.md#benchmark-figures). |
| [Contributing](AGENTS.md) | Code organization, invariants and contribution checks. |

Licensed under the [MIT License](LICENSE). Bundled fonts retain their
[separate license](assets/fonts/LICENSE).
