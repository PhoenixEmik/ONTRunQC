# Repository Guidelines

## Project Structure & Module Organization

ONTRunQC is a single-package Rust 2024 CLI for post-sequencing QC of PromethION summaries. `src/main.rs` connects the pipeline; `cli.rs` defines arguments; `input.rs` validates streaming TSV records; `record.rs` holds borrowed fields; `batch.rs` discovers inputs and routes run chunks; `integrity.rs` guards canonical source metadata, pass membership and optional streaming digests; `metrics.rs` owns QC calculations; `trends.rs` implements optional raw-read fits with bounded exact knot selection in `trend_knots.rs`; `density.rs` implements the optional reference-grid estimator; `layout.rs` implements the explicit, reference-based PromethION and MinION coordinate maps; `minion.rs` accumulates exact points or bounded full-count display rasters. `report.rs` stages requested outputs; `export.rs` emits optional YAML and streams canonical standalone SVGs; optional `figure_render.rs` converts those figures to native raster/PDF/PS formats using bundled `assets/fonts/`; `html.rs` renders offline HTML using embedded `report.css` and `report.js`.

Unit tests live beside implementations; CLI integration tests are in `tests/integration.rs`, with fictional inputs in `tests/fixtures/`. Consult `docs/metrics.md` for definitions/schema and `docs/benchmark.md` for measurement procedures.

## Build, Test, and Development Commands

Use Rust 1.85 or newer. Run from the repository root:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
target/release/ontrunqc analyze sequencing_summary.txt --output qc_results/
```

These check formatting/lints, run tests, build the binary, and generate `report.json` plus `report.html`. Open HTML with `file://`. Try `tests/fixtures/synthetic.tsv` as input. Commit `Cargo.lock` when dependencies change.

## Coding Style & Naming Conventions

Use rustfmt defaults, four-space indentation, `snake_case` functions/modules, and `UpperCamelCase` types. Prefer straightforward functions, minimal dependencies, and contextual `anyhow` errors. Reuse parsing buffers and borrow fields; avoid per-record strings. Use bounded independent input-component workers only when requested with `--processors`; preserve within-run input order and ordered pooled reductions. Keep fitting, dense MinION passes and rendering sequential.

## Testing Guidelines

Use Rust's built-in test framework and descriptive behavior names. Preserve parser/metrics regressions; cover HTML/JSON consistency, unavailable statistics, metadata escaping, offline assets, thousands of channels, and write-failure cleanup. Review generated HTML on desktop and narrow screens. The optional pinned R differential suite is documented in `tests/reference/README.md`; missing R must produce unverified results, never invented parity evidence. No coverage percentage is mandated. Run all checks above before submitting code changes.

Zero-length records count toward summary totals but not basecalled length/quality statistics. Native filtering comes exclusively from `passes_filtering`; independent `--qscore-cutoff` comparisons must never replace it. Missing native flags mean unavailable, never inferred failure. Exact channel medians reuse disjoint length partitions. Preserve unknown termination reasons, checked integer totals, ordered output, and `null` unavailable statistics. Update metric documentation with behavior/schema changes; increment `schema_version` for incompatible report changes.

Render both formats from the final report model. Escape all input-derived HTML; keep metadata out of scripts and external resources out of reports. Prepare all requested temporary outputs and revalidate sources before replacing files; do not claim atomicity across files or runs. Keep YAML a projection of canonical metrics and static exports self-contained. Convert one canonical SVG at a time; never duplicate chart calculations. Default `static-figures` builds include all export formats; `--no-default-features` keeps JSON/HTML/SVG independent. Preserve physical dimensions, codec metadata and bundled font licensing. PS is raster-backed, never advertised as vector.

## Commit & Pull Request Guidelines

Use focused, imperative commits, e.g. `Add standalone HTML reports`. PRs should explain behavior changes, link relevant issues, report validation, and identify compatibility implications.

After completing and validating requested work, push its commits to the current branch's configured upstream automatically, unless the user requests local-only changes. A separate push request is not required.

## Research Data & Performance

Commit only synthetic fixtures. Keep research identifiers, real reads, credentials, and identifying reports out of Git. Build output, `qc_results/`, and `benchmark-results/` are ignored. Inputs are uncompressed TSV; multi-input routing groups known run IDs and labels unknown sources explicitly. Modern and legacy schemas retain separate validation rules. Keep MinIONQC parity focused; do not add unrelated formats, large benchmarks, or speculative optimization. Physical maps require explicit layout selection. Channel activity and speed are summary-derived proxies, not pore health or occupancy. Optional template durations affect template-read speed eligibility only. 2D reads never borrow template-native filtering or events. MinION read-level plots must account for every eligible observation: exact points for small inputs, bounded full-count pixels plus actual extrema for dense inputs. Dense rendering rescans one run at a time; never reintroduce sampling or modify GAM/KDE reference evidence to validate display changes.
