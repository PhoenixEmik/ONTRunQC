# PoreGauge

PoreGauge is a small, single-threaded Rust CLI for post-sequencing quality control
of Oxford Nanopore sequencing summaries, initially designed for PromethION and
modern MinKNOW/Dorado output. It streams TSV records, validates them, calculates
run metrics, and writes a deterministic JSON report.

PoreGauge is not a basecaller or a replacement for MinKNOW instrument monitoring.
It does not analyze FASTQ, BAM, or POD5 files.

## Features

- Separate counts for all summary records, basecalled reads, and zero-length records.
- Pass/fail counts and yields using the input's `passes_filtering` flag.
- Exact read-length median and N50; arithmetic per-read Q-score mean and median.
- Length and Q-score histograms, including yield by length range.
- Channel, mux, termination-reason, and 15-minute start-time yield statistics.
- Header-based parsing, numeric validation, single-run checks, and errors with line numbers.

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
`qc_results/report.json`, replacing an existing report. Invalid input exits
nonzero with an error on stderr; no report is written for a failed analysis.
The input must fit the single-run model: different `run_id` values are rejected.

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

## Output and definitions

The JSON schema has `schema_version: 1`, tool version, run ID, overall metrics,
quality metrics, and ordered arrays of distributions and grouped statistics.
Unavailable values are `null`; integer yields are in base pairs. No individual
read records, sample IDs, filenames, or experiment IDs appear in reports.
The run ID and unknown termination-reason strings are preserved.

See [metric definitions and schema](docs/metrics.md). Zero-length records do not
contribute to read-length or quality statistics. No implicit Q-score threshold
is applied. Time-bin yield is attributed to read start time, rather than an exact
reconstruction of instrument throughput.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

`src/input.rs` validates borrowed records; `src/metrics.rs` owns all statistical
calculations; `src/report.rs` serializes results. The CLI is in `src/cli.rs` and
`src/main.rs`. Tests use fictional data in `tests/fixtures/`. No coverage percentage
is mandated; changes should test metric definitions and meaningful failure cases.

## Status and limitations

Version 0.1.0 implements the initial JSON QC engine. There is no HTML reporting,
automatic QC judgment, gzip support, barcode analysis, physical flowcell map,
multi-run analysis, or instrument API integration.

Parsing is streaming, but exact medians require retained lengths (`u64`) and
Q-scores (`f64`): approximately 466 MiB of elements for 30,549,981 basecalled reads,
plus allocator overhead and grouped metrics. Memory also grows with distinct
channels, muxes, reasons, and occupied time bins. Sorting is in place. The
below-512-MiB RSS goal needs validation on the full real run; the supplied fixture
and synthetic measurements are documented in [benchmarking](docs/benchmark.md).

Licensed under the [MIT License](LICENSE).
