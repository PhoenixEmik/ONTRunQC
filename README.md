# PoreGauge

PoreGauge is a small, single-threaded Rust CLI for post-sequencing quality control
of Oxford Nanopore sequencing summaries, initially designed for PromethION and
modern MinKNOW/Dorado output. It streams TSV records, validates them, calculates
run metrics, and writes deterministic JSON and standalone HTML reports.

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

## Installation

Install Rust 1.85 or newer, including Cargo, then build from this directory:

```sh
cargo build --release --locked
# Optional: install the binary on your Cargo executable path.
cargo install --path . --locked
```

The local executable is `target/release/poregauge`.

## Usage

```sh
target/release/poregauge analyze sequencing_summary.txt --output qc_results/
target/release/poregauge analyze --help
```

The output directory is created as needed. Successful analysis writes
`qc_results/report.json` and `qc_results/report.html`, replacing existing reports.
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
Unused columns are ignored. A header-only file produces zero counts and `null`
statistics; an empty file is rejected.

## HTML report sections

| Section | Contents |
| --- | --- |
| Run overview | Run ID, summary/basecalled counts, yield, native pass rate, N50, mean length, mean Q-score. |
| Sequencing yield | Cumulative yield, yield per 15-minute interval, basecalled reads over time, exact interval table. |
| Read length | Read-count and yield histograms, exact N50/median, original interval boundaries. |
| Read quality | Per-read Q-score histogram, mean/median, pass/fail read counts and yields. |
| Channel performance | Yield ranked by channel, searchable/sortable/paginated table of exact counts, yield, and mean Q-score. |
| Mux performance | Basecalled read counts and yields by mux, with exact totals. |
| Termination reasons | Record-count distribution, basecalled counts, and yield for all observed reason strings. |

Charts and tables scroll horizontally on narrow screens to retain readable
labels. All charts and all channel rows remain available without JavaScript;
the script only adds channel search, sorting, and pagination. Unknown statistics
display **Not available**. Plot labels may be rounded for readability; tables and
SVG tooltips provide counts in exact integers. Units use decimal prefixes:
1 kb = 1,000 bp, 1 Mb = 1,000,000 bp, 1 Gb = 1,000,000,000 bp.

## JSON and metric definitions

The JSON schema has `schema_version: 1`, tool version, run ID, overall metrics,
quality metrics, and ordered arrays of distributions and grouped statistics.
Unavailable values are `null`; integer yields are in base pairs. No individual
read records, sample IDs, filenames, or experiment IDs appear in reports.
The run ID and unknown termination-reason strings are preserved. JSON schema 1
is unchanged in v0.2.0; both renderers use the same final metrics model. The HTML
pass rate is passed basecalled reads divided by all basecalled reads, or unavailable
when that denominator is zero. Input-derived HTML text is escaped; no input data
is interpolated into scripts.

See [metric definitions and schema](docs/metrics.md). Zero-length records do not
contribute to read-length or quality statistics. No implicit Q-score threshold
is applied. Time-bin yield is attributed to read start time, rather than an exact
reconstruction of instrument throughput.
Missing time intervals contribute no new yield; cumulative steps occur at bin
ends. Histogram bins are categorical and do not reconstruct individual reads.
Channel rank indicates yield ordering, not physical flowcell geometry. No
chemistry, model, flowcell identity, pore occupancy, or run-level pass/fail judgment
is inferred from missing metadata.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

`src/input.rs` validates borrowed records; `src/metrics.rs` owns all statistical
calculations; `src/report.rs` serializes JSON and stages both outputs.
`src/html.rs` renders HTML with embedded `src/report.css` and `src/report.js`;
there is no frontend build pipeline. The CLI is in `src/cli.rs` and
`src/main.rs`. Tests use fictional data in `tests/fixtures/`. No coverage percentage
is mandated; changes should test metric definitions and meaningful failure cases.

## Status and limitations

Version 0.2.0 adds standalone HTML reporting. There is no
automatic QC judgment, gzip support, barcode analysis, physical flowcell map,
multi-run analysis, or instrument API integration.

Parsing is streaming, but exact medians require retained lengths (`u64`) and
Q-scores (`f64`): approximately 466 MiB of elements for 30,549,981 basecalled reads,
plus allocator overhead and grouped metrics. Memory also grows with distinct
channels, muxes, reasons, and occupied time bins. Sorting is in place. The
below-512-MiB RSS goal needs validation on the full real run; the supplied fixture
and historical v0.1.0 engine measurements are documented in [benchmarking](docs/benchmark.md).
No real-run benchmark acceptance is claimed for this HTML milestone. HTML size
depends on grouped observations (channels, time bins, reasons), not individual reads.

Licensed under the [MIT License](LICENSE).
