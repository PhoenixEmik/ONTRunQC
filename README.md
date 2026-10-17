# PoreGauge

PoreGauge is a small, single-threaded Rust CLI for post-sequencing quality control
of Oxford Nanopore sequencing summaries, initially designed for PromethION and
modern MinKNOW/Dorado output. It streams TSV records, validates them, calculates
run metrics, and writes deterministic JSON and standalone HTML reports.

Current release: **[v0.4.0 — Advanced Read Quality Diagnostics](https://github.com/PhoenixEmik/PoreGauge/tree/v0.4.0)**.
Development version: **0.4.1 — HTML Report Performance Optimization**.

PoreGauge is not a basecaller or a replacement for MinKNOW instrument monitoring.
It does not analyze FASTQ, BAM, or POD5 files.

## Features

- Separate counts for all summary records, basecalled reads, and zero-length records.
- Pass/fail counts and yields using the input's `passes_filtering` flag.
- Exact read-length median and N50; arithmetic per-read Q-score mean and median.
- Length and Q-score histograms, including yield by length range.
- Channel, mux, termination-reason, and 15-minute start-time yield statistics.
- Header-based parsing, numeric validation, single-run checks, and errors with line numbers.
- Offline scientific reports with inline SVG charts and searchable channel tables.
- Explicit PromethION 3000-channel total/passed-yield heatmaps.
- Distinct basecalled-producing channels over time and channel × mux statistics.
- Optional template-duration speed proxy and termination-reason percentages.
- Exact passed-read length median/N50 and Q-score median, with all/passed comparison.
- All/passed mean read length and arithmetic Q-score in 15-minute read-start bins.
- Bounded length × Q-score heatmaps, including open-ended length/quality tails.
- Per-channel all/passed mean length and a compact distribution of channel means.

## Installation

[Release automation](.github/workflows/release.yml) produces `.tar.gz` archives for
Linux (x86-64 and ARM64) and macOS (Intel and Apple Silicon), plus `SHA256SUMS`.
When available, download the matching archive from
[GitHub Releases](https://github.com/PhoenixEmik/PoreGauge/releases), verify its
checksum, extract it, and run the included `poregauge` binary. The Linux release
workflow builds static musl binaries with no system glibc dependency. Download
`poregauge-v0.4.0-linux-x86_64.tar.gz` for Intel/AMD systems or
`poregauge-v0.4.0-linux-arm64.tar.gz` for ARM64 systems. The extracted directory
uses the same name without `.tar.gz`. Rocky Linux 8 is the
minimum Linux compatibility test environment: each Linux archive must pass a
version check and synthetic JSON/HTML analysis in its native Rocky Linux 8
container before upload. Containers share the CI host kernel; this does not
verify every HPC kernel, CPU, or site policy. macOS builds are tested on macOS 15
(Intel) and macOS 14 (Apple Silicon).
The archives include the license, README, and metric definitions.

The original v0.3.0 `*-linux-gnu` downloads were built against newer glibc and do
not run on Rocky Linux 8. A `GLIBC_2.xx not found` error occurs before PoreGauge
starts. Use the `linux-x86_64` or `linux-arm64` build for these systems. The manual
build-by-tag procedure below can rebuild `v0.3.0` with this workflow; maintainers
can explicitly replace the old release assets after the compatibility checks pass.

Install Rust 1.85 or newer, including Cargo. Build this checkout from the repository root:

```sh
cargo build --release --locked
```

The executable is `target/release/poregauge`. Optionally install it on your Cargo
executable path with `cargo install --path . --locked` to use `poregauge` directly.

## Usage

```sh
target/release/poregauge analyze sequencing_summary.txt --output qc_results/
target/release/poregauge analyze --help
```

The output directory is created as needed. Successful analysis produces:

```text
qc_results/
├── report.json    # Structured metrics for downstream tools
└── report.html    # Self-contained visual QC report
```

Existing reports in that directory are replaced.
Open `report.html` directly in a modern browser using `file://`, or double-click it.
The HTML file can be moved or shared alone: CSS, SVG, and the optional channel-table
script are embedded, with no CDN, network requests, or web server required.
Invalid input exits nonzero with an error on stderr and preserves previous reports.
The input must fit the single-run model: different `run_id` values are rejected.

Both outputs are fully rendered, flushed, and synced to temporary files in the
destination directory before publication. Each completed file is replaced by a
rename; **the two replacements are not an atomic pair**. A failure during the
second replacement can leave new JSON alongside older HTML. Ordinary failures
clean up temporary files; abrupt process termination can leave temporary files.
Inspect errors and rerun analysis before using outputs after a failed command.

To try the report using fictional input:

```sh
target/release/poregauge analyze tests/fixtures/synthetic.tsv --output qc_results/example/
```

Open `qc_results/example/report.html`. This small fixture has 6 summary records,
4 basecalled reads, 2 zero-length records, 100 bp total yield, a 30 bp N50,
and a 25 bp median read length. Its intentionally small values exercise report
behavior; they are not representative of a production sequencing run.

For the advanced diagnostics, use `tests/fixtures/advanced_quality.tsv` with the
same command. This fictional unsorted input includes missing/zero-length-only
intervals, native passed Q0 reads, and an exact passed median of 25 bp/N50 of 30 bp.

For a compatible 3000-channel PromethION summary, explicitly enable spatial maps:

```sh
target/release/poregauge analyze sequencing_summary.txt \
  --flowcell-layout promethion-3000 --output qc_results/
# Complete Deep QC example using fictional data:
target/release/poregauge analyze tests/fixtures/deep_qc.tsv \
  --flowcell-layout promethion-3000 --output qc_results/deep-example/
```

## Supported input

One uncompressed, unquoted TSV file with a header. LF and CRLF line endings,
additional columns, and arbitrary column order are supported. Quotes are literal
characters; CSV-style quoting and multiline fields are not supported.

Required columns:

```text
run_id  channel  mux  start_time  duration  passes_filtering
sequence_length_template  mean_qscore_template  end_reason
```

Columns are separated by tabs, not spaces. Numeric fields must be valid and
nonnegative; floating values must be finite. Channel and mux identifiers must be
positive `u32` integers, without a device-specific upper limit. Filtering accepts
`TRUE`/`FALSE`, case-insensitively. Run IDs and termination reasons must be
nonempty UTF-8 strings. Blank lines, duplicate/empty headers, mismatched field
counts, and malformed required fields are errors, including on zero-length rows.
The optional `template_duration` column enables sequencing speed analysis.
Blank, malformed, nonfinite, or nonpositive optional durations make the row
ineligible for speed only; required fields still undergo strict validation.
`duration` is never substituted. Other unused columns are ignored. A header-only
file produces zero counts and `null` statistics; an empty file is rejected.

## HTML report sections

| Section | Contents |
| --- | --- |
| Run overview | Run ID, summary/basecalled counts, yield, native pass rate, N50, mean length, mean Q-score. |
| Sequencing yield | Cumulative/interval yield, basecalled reads, distinct basecalled-producing channels, exact interval values. |
| Read length | All/passed comparison table, exact subset median/N50, length/yield histograms, mean length over time. |
| Read quality | Q-score histogram, mean Q-score over time, all/passed length × Q-score heatmaps, native pass/fail counts and yields. |
| Channel performance | Ranked yield, distribution of channel mean lengths, all/passed mean lengths in the existing sortable table, optional spatial maps and channel × mux table. |
| Mux performance | Basecalled read counts, total/passed yield by observed mux, exact totals. |
| Termination reasons | Counts, yield, record/yield shares, zero-length/basecalled fractions, and native pass fraction, sorted by record count. |
| Sequencing speed | Fixed-bin speed distribution and mean per-read speed over time, when eligible template durations exist. |

Tables use right-aligned numerical headers and values with tabular numerals;
descriptive labels remain left-aligned. Termination-reason bars sit beneath their
record counts in the same cell, with aligned tracks and visible numerical values.
Long reason names wrap without compressing numerical columns. Decorative table
bars use CSS; scientific charts and physical heatmaps remain SVG.

Charts and wide tables scroll within their own containers on narrow screens to
retain readable labels and exact values. All charts and all channel rows remain
available without JavaScript; the script only adds channel search, sorting, and
pagination. With JavaScript, only the current 25/50/100 rows of each channel
table are mounted; other rows stay in memory. Printing omits the detailed channel
and channel × mux tables in both modes and displays an omission notice. Complete
values remain available in JSON and the offline HTML. Other collapsed detail
sections remain omitted from print until opened.

Unavailable statistics display **Not available**. Plot labels may be
rounded for readability; tables and SVG tooltips provide counts in exact integers.
Units use decimal prefixes:
1 kb = 1,000 bp, 1 Mb = 1,000,000 bp, 1 Gb = 1,000,000,000 bp.

## JSON and metric definitions

The JSON schema has `schema_version: 1`, tool version, run ID, overall metrics,
quality metrics, and ordered arrays of distributions and grouped statistics.
Unavailable values are `null`; integer yields are in base pairs. No individual
read records, sample IDs, filenames, or experiment IDs appear in reports.
The run ID and unknown termination-reason strings are preserved. Schema version 1
is retained: v0.4.1 changes no QC fields, definitions, or array ordering.
The v0.4.0 additions remain available.
New fields extend time bins/channels and `diagnostics`; consumers must tolerate
unknown fields. Strict consumers that reject additional fields need updating.
Both renderers use the same final metrics model.
The HTML pass rate is passed basecalled reads divided by all basecalled reads,
or unavailable when that denominator is zero. Input-derived HTML text is escaped;
no input data is interpolated into scripts.

See [metric definitions and schema](docs/metrics.md). Zero-length records do not
contribute to read-length or quality statistics. No implicit Q-score threshold
is applied. Time-bin yield is attributed to read start time, rather than an exact
reconstruction of instrument throughput.
Missing time intervals contribute no new yield; cumulative steps occur at bin
ends. Histogram bins are categorical and do not reconstruct individual reads.
Channel rank indicates yield ordering. No chemistry, model, flowcell identity,
pore occupancy, or run-level pass/fail judgment is inferred from missing metadata.

## Deep QC definitions and limits

- **Physical maps:** `--flowcell-layout promethion-3000` selects the
  [EPI2ME Basic QC Tutorial mapping](https://epi2me.nanoporetech.com/notebooks/Basic_QC_Tutorial.html).
  Twelve consecutive 250-channel blocks are each reshaped row-major to 25 × 10,
  then joined horizontally into 25 × 120. Coordinates are zero-based; channel 1
  is (0, 0), 251 is (0, 10), and 3000 is (24, 119). Channel IDs outside 1–3000
  cause an error with the selected layout; no device is inferred from channel IDs.
  Verify numbering compatibility for the actual flowcell/model. Orientation follows
  the reference rather than a verified instrument-facing orientation. Hatched cells
  mean no summary records were observed; observed zero yield has a separate style.
- **Observed channel activity:** distinct channels with at least one positive-length
  read starting in a 15-minute bin. Repeated reads count once; zero-length rows do
  not count. This is not MinKNOW active pore count, occupancy, or pore availability.
- **Channel × mux:** counts and total/passed yields for observed pairs, including
  zero-length counts. All positive mux identifiers are supported; four values are
  not assumed. These statistics do not measure pore occupancy.
- **Read speed proxy:** length / `template_duration` in bp/s for positive-length
  reads with a finite positive duration and finite ratio. Means are arithmetic per
  eligible read. Fixed bins start at 0, 100, 200, 300, 400, 500, 600, 800, and
  1000 bp/s; the final bin is unbounded. There are no speed quantiles or thresholds.
  This proxy is not direct motor speed or raw-signal analysis.
- **Termination percentages:** shares of all records/yield use run totals;
  zero-length/basecalled fractions use records within each reason; passed fraction
  uses basecalled reads within that reason. Zero denominators are unavailable.
  Unknown reasons remain unchanged and are not classified as good or bad.

Summary-derived observations cannot establish pore health, availability, or the
cause of spatial or temporal variation. Instrument telemetry is not analyzed.

## Advanced quality definitions and limits

- All-read length/quality means refer to **basecalled reads**, excluding zero lengths.
  Passed statistics additionally require native `passes_filtering=TRUE`.
- Time plots retain unsmoothed 15-minute bins and break at missing/unavailable values.
  Q-score means are arithmetic per read; neither length weighting nor an aggregate
  error-probability transformation is applied. Values are attributed to read start.
- Joint histograms use powers-of-two length boundaries from 1 to 1,048,576 bp and
  Q-score boundaries 0, 2, …, 50. Final bins are unbounded; no reads are discarded
  above the displayed range. Both heatmaps share a log-count color scale.
- Channel means are channel yield / channel basecalled count (likewise for passed
  reads). Exact per-channel medians are deferred to avoid additional per-read storage.

In the maintainer-supplied v0.3.0 validation baseline, 28,202,608 summary records
include 1,310 zero lengths, leaving 28,201,298 basecalled reads. MinIONQC's
all-record denominator can therefore produce different means/medians. The matching
native passed and Q ≥ 9 totals in that run do **not** make these filters universally
equivalent. v0.4.0 regression tests use fictional inputs; that real summary has
not been reprocessed for this milestone.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
```

[GitHub Actions CI](.github/workflows/ci.yml) runs these checks on stable Rust for
pushes and pull requests, and can also be started manually. A separate job runs
tests on the minimum supported Rust version, 1.85. Both jobs use synthetic fixtures.

To publish a release, update `Cargo.toml` and `Cargo.lock`, commit the changes,
then push a matching stable `vMAJOR.MINOR.PATCH` tag. The release workflow checks
the version, formatting, and Clippy, then tests, builds, and smoke-tests each
platform before publishing assets and generated release notes.

To build an existing tag without publishing, open **Actions → Release → Run workflow**,
select the workflow on `main`, enter a tag such as `v0.3.0`, and leave **Publish a
GitHub Release after building** unchecked. Download `poregauge-v0.3.0` from the
completed run's artifacts for all four platform archives and `SHA256SUMS` (retained
for seven days). The CLI equivalent is:

```sh
gh workflow run release.yml --ref main -f tag=v0.3.0 -F publish=false
```

Enable the publish option to release an existing tag; this also works for tags
that predate automation. Pushing a new matching version tag publishes automatically.
A failed upload leaves a draft that can be retried. Published release assets are
protected by default. To republish a mutable release, enable both **Publish a
GitHub Release after building** and **Replace assets of an existing published
release**. After all builds and compatibility tests pass, the workflow temporarily
returns the release to draft, replaces archives/checksums, removes old Linux
archive names, updates the release notes, and publishes again. The original
source tag is preserved. Immutable releases cannot be updated this way.
Prereleases and Windows binaries are not currently automated. Adding the workflow
does not publish existing tags automatically.

`src/input.rs` validates borrowed records; `src/metrics.rs` owns all statistical
calculations; `src/report.rs` serializes JSON and stages both outputs.
`src/layout.rs` defines the supported coordinate map. `src/html.rs` renders HTML
with embedded `src/report.css` and `src/report.js`;
there is no frontend build pipeline. The CLI is in `src/cli.rs` and
`src/main.rs`. Tests use fictional data in `tests/fixtures/`. No coverage percentage
is mandated; changes should test metric definitions and meaningful failure cases.

Tests cover existing metrics and output reliability plus mapping boundaries and
uniqueness, layout validation, yield conservation, distinct-channel counting,
unusual mux values, speed eligibility/boundaries, termination denominators,
escaping, deterministic output, and offline resources. Review generated reports
at desktop and mobile widths after presentation changes.

A small synthetic 3,000-channel/10,000-pair structural regression runs in
`cargo test`. If Node.js is available, tests also run the dependency-free
`tests/report_tables.cjs` behavior checks; Node is optional and is never needed
to build or use PoreGauge. Reproduce HTML size/DOM comparisons and optional
browser checks using the procedure in [benchmarking](docs/benchmark.md#html-report-maintenance-v041).

## Status and limitations

Version 0.4.1 reduces report markup and table interaction overhead, preserving
v0.4.0 scientific results. There is no
automatic QC judgment, gzip support, barcode analysis, multi-run analysis, live
monitoring, or instrument API integration. Physical maps support only the explicit
EPI2ME 3000-channel layout; other models/geometries are not verified.

Parsing is streaming, but exact medians require retained lengths (`u64`) and
Q-scores (`f64`): 16 bytes per basecalled read, approximately 466 MiB of elements
for 30,549,981 reads, plus allocator overhead and grouped metrics. Pass/fail
partitions each retain an observation once; global exact statistics merge sorted
partitions without a combined copy. Four vectors have at most about 2 MiB spare
element capacity (about 1 MiB more than v0.3.0). Two fixed joint-count matrices
use 8,736 bytes during aggregation. A synthetic storage regression checks this
bound; it is not an RSS benchmark. New diagnostics retain channel/mux
pairs and distinct channel IDs per occupied time bin, not individual reads.
Memory also grows with reasons and occupied time bins. Sorting is in place. The
below-512-MiB RSS goal needs validation on the full real run; the supplied fixture
and historical v0.1.0 engine measurements are documented in [benchmarking](docs/benchmark.md).
No real-run benchmark was performed for v0.4.1. HTML size depends on grouped
observations (channels, muxes, time bins, reasons), not individual reads. All
table rows still ship in HTML for no-JS access and must initially be parsed;
pagination reduces mounted DOM and subsequent updates, not retained row memory.

Licensed under the [MIT License](LICENSE).
