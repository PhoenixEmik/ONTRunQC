# ONTRunQC

**Oxford Nanopore sequencing-run quality control.**

ONTRunQC is a standalone Rust CLI for post-sequencing analysis of Oxford Nanopore
sequencing summaries. It streams TSV input, calculates exact core statistics,
and produces structured JSON and a self-contained HTML report. It supports
modern MinKNOW/Dorado summaries, legacy Albacore/Guppy and 2D/1D2 summaries,
PromethION and MinION layouts, and individual or combined run reports.

Current development package: **0.4.1**. CLI executable: `ontrunqc`.

## Features

- Exact read-length median/N50 and per-read Q-score statistics, with separate
  all-basecalled, native-passed and configurable Q-cutoff populations.
- Read-length and quality distributions, exact yield by minimum length,
  ultralong subsets, and length × Q-score heatmaps.
- Fifteen-minute yield, read-count, channel-activity, mean-length and mean-Q plots.
- Exact per-channel medians, channel × mux summaries, termination diagnostics,
  and an optional template-duration speed proxy.
- Explicit PromethION 3000-channel and MinION R9.5 512-channel physical maps.
- Complete-data MinION read-level plots: exact points for small inputs, bounded
  full-count pixel aggregation with extrema for larger inputs.
- Recursive input discovery, run-aware chunk grouping, exact pooled statistics,
  and bounded parallel aggregation of independent runs.
- Offline HTML/JSON, optional YAML and independent SVG, PNG, PDF, PS, JPEG, TIFF
  or BMP figures. Normal analysis and export require neither R nor a browser.

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
Original v0.3.0 `*-linux-gnu` archives required newer glibc; use the static Linux
archives for older HPC systems. Container checks do not verify every host kernel,
CPU or site policy. See the [release workflow](.github/workflows/release.yml).

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

Enable physical maps explicitly; the device is never inferred from channel IDs:

```sh
ontrunqc analyze sequencing_summary.txt \
  --flowcell-layout promethion-3000 --qscore-cutoff 9 --output qc_results/

ontrunqc analyze sequencing_summary.txt \
  --flowcell-layout minion-r9.5 --output minion_qc/
```

The selected layout must match the input's channel numbering. Out-of-range
channels cause an error. Geometry assumptions and orientation limits are in
[metric definitions](docs/metrics.md#explicit-physical-layout).

## Multiple inputs and runs

```sh
ontrunqc analyze /data/flowcells/ --processors 2 --output collection_qc/
ontrunqc analyze chunk_a.tsv chunk_b.tsv --combined-only --output pooled_qc/
ontrunqc analyze mixed_runs.tsv --split-runs --output split_qc/
```

Directory discovery recursively selects files containing `sequencing_summary`
in their name and ending in `.txt` or `.tsv`, in deterministic order. Explicit
file paths can have any name. Path/inode aliases are deduplicated; directory
symlinks are not followed. Separately copied duplicate reads are not deduplicated.

Matching `run_id` values group chunks across files. Distinct runs remain separate;
run IDs are not assumed to identify distinct physical flowcells. Identity-free
files remain labeled sources; use `--group-inputs` only to explicitly group them.
Conflicting known identities are rejected. A mixed-run single file requires
`--split-runs`.

```text
collection_qc/
├── runs/
│   ├── run_001/report.{json,html}
│   └── run_002/report.{json,html}
└── combined/report.{json,html}
```

Combined median/N50 are calculated from pooled observations, never averages of
run statistics. Timelines and channel IDs remain local to each run.
`--combined-only` omits individual outputs. `--processors N` bounds independent
aggregation workers; fitting, dense MinION passes and rendering remain sequential.
Discovery adds I/O, so more workers do not guarantee faster processing.

## Supported input

Input is **uncompressed, unquoted TSV with a header**. Column order is arbitrary;
LF/CRLF and additional columns are supported. CSV-style quoting and multiline
fields are not supported. Malformed required values fail with line numbers.
An empty file is rejected; header-only files produce unavailable statistics.

Modern headers (`--input-schema modern` enforces this schema) require:

```text
run_id  channel  mux  start_time  duration
sequence_length_template  mean_qscore_template  end_reason
```

These names are separated by tabs in the actual file. `passes_filtering` is
optional and accepts case-insensitive TRUE/FALSE. Its absence leaves native
filtering unavailable. Optional `template_duration` enables the read-speed proxy;
missing, invalid or nonpositive values make that observation ineligible for speed.
`duration` is never substituted.

Auto detection also accepts minimal legacy headers: `channel`, `start_time`
and a complete template or `sequence_length_2d`/`mean_qscore_2d` pair. Auto prefers
2D when both pairs exist; `--read-representation template` overrides this choice.
There is no per-row fallback between representations. Legacy finite negative
Q-scores are retained; modern Q-scores must be nonnegative. Missing metadata is
not fabricated. Validated rows with `calibration_strand_genome_template` equal to
`YHR174W` are excluded and counted as MinIONQC-compatible RNA controls.

## Reports and figure export

| Report section | Main analyses |
| --- | --- |
| Overview | Summary/basecalled counts, yield, native pass rate, N50, mean length and Q-score. |
| Yield and temporal QC | Cumulative/interval yield, read counts, observed channel activity, all/passed mean length and Q-score. |
| Length and quality | Histograms, exact all/passed statistics, joint length × Q heatmaps. |
| Channels and mux | Four channel distributions, exact medians, searchable tables, optional physical maps, channel × mux breakdown. |
| Termination and speed | Reason counts/fractions/yield; template length/duration proxy when available. |
| Q-cutoff comparison | Separate selected population, exact thresholds, retained-yield curve and ultralong statistics. |
| Combined reports | Exact pooled summaries and per-run distributions, yields and temporal comparisons. |

```sh
ontrunqc analyze sequencing_summary.txt --output qc_results/ --yaml \
  --format pdf --smallfigures --add-stat --muxes 8
```

JSON and HTML are always written. `--yaml` adds `summary.yaml`. `--format` selects
one independent figure format per invocation and writes `figures/manifest.json`
with the current figure list. Older unlisted files are not automatically deleted;
optional outputs are refreshed only when requested.

| Figure format | Output |
| --- | --- |
| SVG | Vector paths/text; dense MinION images remain embedded rasters. |
| PDF | Vector content and embedded selectable fonts, with MinION raster images. |
| PNG, TIFF, BMP | Lossless RGB at 300 DPI. |
| JPEG | Lossy quality-95 RGB at 300 DPI. |
| PS | PostScript Level 3, **raster-backed** at 300 DPI. |

Normal widths are 180 mm, or 120 mm with `--smallfigures`; dense physical maps
retain 320 mm for readability. `--add-stat` uses existing mean/N50/Q statistics.
`--muxes HOURS` draws configured reference markers, not measured instrument events;
0 disables markers outside compatibility mode. Bundled Liberation Sans uses its
[own font license](assets/fonts/LICENSE). See [export definitions](docs/metrics.md#native-static-figure-exports-15).

## Scientific interpretation and MinIONQC compatibility

- **Summary records** include zero-length rows. **Basecalled reads** have length >0;
  zero lengths are excluded from basecalled length and Q-score statistics.
- **Native passed reads** use `passes_filtering`. **Q-selected reads** independently
  use mean read Q ≥ `--qscore-cutoff` (default 7). These populations are not interchangeable.
- Mean Q-score is the arithmetic mean of per-read Q-scores, not an aggregate error
  probability. Unavailable statistics are `null` in JSON and labeled in HTML.
- Time statistics use read start times and unsmoothed 15-minute bins. They do not
  reconstruct exact instrument throughput. Channel activity and length/duration
  speed are summary-derived proxies, not pore occupancy, availability or motor speed.
- JSON schema remains version 1 with additive fields; consumers must tolerate
  unknown fields. JSON preserves integer bp/counts; display values may be rounded.

MinIONQC-compatible metrics, legacy inputs, multi-run reports, YAML, CLI aliases
and all six static formats are implemented. Scientific differences in denominators,
N50 ties, histogram bins, temporal resolution and corrected upstream bugs are
explicitly documented; identical ggplot pixels are not claimed.

`--minionqc-trends` adds raw-read LOESS/GAM fits in an additional input pass,
alongside the unsmoothed means. LOESS fits and pointwise confidence intervals have
actual pinned-R validation. **Strict GAM parity remains Partial**: the retained
18-case suite records **1,542 original-reference fit/uncertainty discrepancies**.
`--minionqc-density` enables the shared R-grid estimator; 52 actual-R density
curves meet their documented tolerances. Neither option changes core QC totals.
See the [parity matrix and evidence](docs/minionqc-parity.md).

Historical logical syntax such as `--smallfigures TRUE --add-stat FALSE` works
alongside bare flags. `--minionqc-compat` opts into historical mux-marker defaults
and allows omitted output for exactly one input path by reserving a fresh
`ontrunqc_qc/` beside a file or inside a directory. Existing destinations are
refused; multiple paths require explicit output. Normal defaults remain
HTML/JSON with an explicit output directory. Request PNG explicitly when needed.

## Input integrity, memory and limitations

Keep inputs stable throughout analysis. Metadata, bytes consumed and per-file
run membership are checked across passes and again before publication.
`--verify-input-content` additionally compares streaming SHA-256 digests and
rereads contributing sources after rendering. This costs I/O/CPU; neither mode
is a file lock, snapshot or guarantee against NFS cache staleness or hostile writes.
See [input-integrity limits](docs/metrics.md#multi-pass-input-integrity).

Completed temporary files replace destination files individually. Publication is **not atomic
across files or runs**; a later rename failure can leave complete old/new outputs
mixed. Inspect errors before using reports after a failed command.

Parsing is streaming, but exact statistics retain numeric lengths and Q-scores:
16 bytes per basecalled read, about 466 MiB for 30.55 million reads, plus allocation
and grouped/display data. Multi-run processing retains pooled numeric observations;
more workers do not reduce this memory requirement. No below-512-MiB real-run
RSS guarantee is made. See [measurements and procedures](docs/benchmark.md).
HTML size grows with aggregated groups; all detailed table rows ship for no-JS
access, while JavaScript mounts only the current page. Printing omits large channel
tables with a notice; full values remain in HTML/JSON.

ONTRunQC is not a basecaller or a replacement for MinKNOW monitoring. It does not
parse FASTQ/BAM/POD5, estimate pore occupancy, assign automatic run pass/fail,
or provide live monitoring, barcode analysis or gzip input support.

## Development and validation

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo test --locked --no-default-features
cargo build --release --locked --no-default-features
python3 -m unittest discover -s tests/reference
```

[CI](.github/workflows/ci.yml) also tests Rust 1.85 and static Linux exports.
Optional public-input and pinned-R differential checks are documented in
[reference validation](tests/reference/README.md); R is not a runtime dependency.
Use separate Cargo target directories if running different feature builds
concurrently. Keep private sequencing data and identifying reports out of Git.

[Release automation](.github/workflows/release.yml) uses matching
`vMAJOR.MINOR.PATCH` tags. Maintainers can build an existing tag without publishing
through its manual `tag`/`publish=false` inputs.

Further documentation: [metric definitions and schema](docs/metrics.md),
[MinIONQC parity](docs/minionqc-parity.md), [benchmarks](docs/benchmark.md),
[contributor guidelines](AGENTS.md). Licensed under the [MIT License](LICENSE).
