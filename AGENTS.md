# Repository Guidelines

## Project Structure & Module Organization

PoreGauge is a single-package Rust 2024 CLI for post-sequencing QC of PromethION summaries. `src/main.rs` connects the pipeline; `cli.rs` defines arguments; `input.rs` validates streaming TSV records; `record.rs` holds borrowed fields; `metrics.rs` owns QC calculations; `layout.rs` implements the explicit, reference-based PromethION coordinate map. `report.rs` serializes JSON and stages both outputs; `html.rs` renders offline HTML using embedded `report.css` and `report.js`.

Unit tests live beside implementations; CLI integration tests are in `tests/integration.rs`, with fictional inputs in `tests/fixtures/`. Consult `docs/metrics.md` for definitions/schema and `docs/benchmark.md` for measurement procedures.

## Build, Test, and Development Commands

Use Rust 1.85 or newer. Run from the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
target/release/poregauge analyze sequencing_summary.txt --output qc_results/
```

These check formatting/lints, run tests, build the binary, and generate `report.json` plus `report.html`. Open HTML with `file://`. Try `tests/fixtures/synthetic.tsv` as input. Commit `Cargo.lock` when dependencies change.

## Coding Style & Naming Conventions

Use rustfmt defaults, four-space indentation, `snake_case` functions/modules, and `UpperCamelCase` types. Prefer straightforward functions, minimal dependencies, and contextual `anyhow` errors. Reuse parsing buffers and borrow fields; avoid per-record strings. Keep processing single-threaded unless measurements justify changing it.

## Testing Guidelines

Use Rust's built-in test framework and descriptive behavior names. Preserve parser/metrics regressions; cover HTML/JSON consistency, unavailable statistics, metadata escaping, offline assets, thousands of channels, and write-failure cleanup. Review generated HTML on desktop and narrow screens. No coverage percentage is mandated. Run all checks above before submitting code changes.

Zero-length records count toward summary totals but not basecalled length/quality statistics. Native filtering comes exclusively from `passes_filtering`; independent `--qscore-cutoff` comparisons must never replace it. Missing native flags mean unavailable, never inferred failure. Exact channel medians reuse disjoint length partitions. Preserve unknown termination reasons, checked integer totals, ordered output, and `null` unavailable statistics. Update metric documentation with behavior/schema changes; increment `schema_version` for incompatible report changes.

Render both formats from the final report model. Escape all input-derived HTML; keep metadata out of scripts and external resources out of reports. Prepare both temporary outputs before replacing files; do not claim atomicity across the pair.

## Commit & Pull Request Guidelines

Use focused, imperative commits, e.g. `Add standalone HTML reports`. PRs should explain behavior changes, link relevant issues, report validation, and identify compatibility implications.

## Research Data & Performance

Commit only synthetic fixtures. Keep research identifiers, real reads, credentials, and identifying reports out of Git. Build output, `qc_results/`, and `benchmark-results/` are ignored. Current scope is one run, uncompressed TSV, and JSON/HTML reporting. Keep Deep QC focused; do not add input formats, large benchmarks, or speculative optimization. Physical maps require explicit layout selection. Channel activity and speed are summary-derived proxies, not pore health or occupancy. Optional template durations affect speed eligibility only.
