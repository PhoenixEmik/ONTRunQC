# MinIONQC parity

Reference: [MinIONQC v1.4.1 source](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R),
README, options, `tests.R`, public YAML and example figures at commit `54485d5`.
Statuses describe scientific functionality, not identical ggplot pixels.
**Full format/rendering parity is not claimed.** JSON remains canonical.

Validation keys below:

- **U**: `cargo test` (unit and CLI integration tests, synthetic inputs).
- **P**: `python3 tests/minionqc_reference.py /path/to/pinned/minion_qc`
  after `cargo build --release --locked`; independent standard-library calculations
  plus upstream YAML. Downloads stay outside this repository. See [metrics](metrics.md#public-reference-validation-and-memory).
- **V**: generate `--format svg --yaml --add-stat --muxes 8`, inspect HTML and
  independent figures offline. Routine CI needs neither R nor a browser.

## Inputs and metrics

| MinIONQC functionality | Status | PoreGauge behavior / reproducible validation |
| --- | --- | --- |
| Single summary | Complete | Existing positional input or `--input`; U `complete_synthetic_metrics`, P short/MinION/PromethION. |
| Recursive directory / multiple summaries | Complete | Deterministic discovery, path/inode deduplication, no directory symlinks; U `multiple_runs_chunks_and_duplicate_paths_have_exact_pooled_statistics`. |
| Individual and combined QC | Complete | Exact pooled observations, not averages of run summaries; U pooled/chunk and `collection_exports` tests, P multiple MinION inputs. |
| Combined-only | Complete | No individual outputs or MinION samples; U `combined_only_skips_individual_outputs_and_explicit_grouping_rejects_conflicts`. |
| Run/chunk/flowcell identity | Complete | Run-ID grouping; explicit unknown sources, optional real flowcell ID; U ambiguity/conflicting metadata tests. Multiple runs on one physical flowcell remain separately labeled. |
| Albacore/Guppy minimal template headers | Complete | Optional absent metadata stays unavailable; U `minimal_legacy_inputs_keep_missing_metadata_unavailable`, P original files. |
| 2D/1D2 field substitution | Complete | Complete 2D pair selected, no row-level template mixing; U `legacy_2d_substitution_is_consistent_and_never_falls_back_per_row`, P 1D2. |
| Direct-RNA YHR174W exclusion | Complete | Exact name, explicit excluded count; U `direct_rna_controls_are_excluded_and_counted_separately`, P RNA (37 controls). |
| Inclusive configurable Q cutoff | Complete | Separate from native pass; U `q_cutoff_is_inclusive_exact_and_independent_of_native_filtering`, P same cutoff as upstream calls. |
| Read count / total bases | Complete | Exact integers; zero-length summary vs basecalled counts explicit; U count/yield conservation, P. |
| Mean/median/max length; mean/median Q | Complete | Exact retained observations; per-read arithmetic, unrounded; U partition/odd/even/legacy tests, P. |
| N50 | Complete | Exact cumulative >=half; documented strict-half tie difference from R; U `q_cutoff_even_and_odd_exact_statistics_preserve_n50_half_convention`, P. |
| Seven length thresholds | Complete | Inclusive >=10/20/50/100/200/500 kb/1 Mb, despite original “>” labels; U `exact_yield_thresholds_curve_and_ultralong_are_consistent`, P. |
| Ultralong longest-first prefix | Complete | Maximal prefix with strict-half N50 >=100 kb; corrected upstream final-element loss; U singleton/tie/prefix tests, P. |
| Exact per-channel medians | Complete | Summary and basecalled denominators separate; U `exact_channel_medians_include_explicit_summary_and_basecalled_denominators`, P. |
| Four channel summary families | Complete | Bases, true read count, mean/median length; All-summary/Q cutoff, explicit bins; U channel distribution conservation. Does not reproduce R's `sum(which(...))` count bug. |
| Explicit physical layouts | Complete | PromethION 25×120 reference and exact MinION R9.5 32×16; U all 3000/512 unique coordinates, edges, invalid IDs and yield placement. No device inference. |
| MinKNOW telemetry / occupancy | Not applicable | Neither summary-derived analysis establishes instrument availability or pore health. |

## Plot families

All plots are offline Rust-generated SVG. All/Q comparisons use native Q selection,
not native passing. Fixed histogram bins, unsmoothed 15-minute means and bounded
KDE differences are intentional and documented in [metrics](metrics.md).

| Upstream report family | Status | Implementation / validation |
| --- | --- | --- |
| `length_histogram`, `q_histogram` | Complete | All/Q count and yield histograms; fixed documented bins rather than 300 automatic bins; U histogram conservation / offline tests, V. |
| `yield_over_time`, `reads_per_hour` | Complete | Absolute cumulative yield and counts in 15-minute bins, All/Q; U interval/cumulative conservation and gap tests, V. Counts are per 15 min, explicitly labeled. |
| `yield_by_length` | Complete | Exact retained bp at bounded display queries, exact threshold counts; All/Q, full yield range rather than R's 99% crop; U `every_display_query_uses_exact_lengths_including_ties`, P, V. |
| `length_by_hour`, `q_by_hour` | Partial | Unchanged 15-minute means plus opt-in interpolated LOESS/cubic-shrinkage REML fits; U temporal/fitting tests. Singular LOESS, large-group exact knots and ribbons remain incomplete. |
| `channel_summary` | Complete | Four channel distributions and exact values, All-summary/Q; U channel summary tests, V. |
| `gb_per_channel_overview` | Complete | Explicit-layout yield heatmaps including All/Q (native maps additionally available); U layout/yield conservation, V. |
| MinION `flowcell_overview` | Partial | Exact physical panels, actual start/length/Q values, deterministic <=16 reads/channel; U sample bound/mapping tests, V. Full-read panels not reproduced. |
| MinION `length_vs_q` | Partial | Actual bounded points, optional events/base color, missing gray; U MinION samples, V. Uses log10(1+events/base); original uses log(events/base). No claim of point-for-point parity. |
| PromethION physical read-level overview | Not applicable | Upstream omits the per-read channel panels for PromethION. |
| PromethION `length_vs_q` | Complete | Bounded logarithmic-length/Q count heatmaps, matching the upstream bin2d analysis family; fixed documented bins, no individual read points; U joint count conservation, V. |
| `combined_length_histogram`, `combined_q_histogram`, `combined_yield_by_length` | Complete | Pooled All/Q observations and exact yield queries; U pooled conservation/exact median/N50, P multiple inputs, V. |
| Per-source `length_distributions`, `q_distributions` | Partial | Legacy KDE retained; opt-in shared-domain R-grid estimates, including summary Q observations. U kernel/bandwidth/mass tests; actual R differential accuracy pending. |
| Per-source `yield_over_time`, `yield_by_length` | Complete | Absolute yields, common units/colors; U combined report/curve tests, P pooled totals, V. Run/source labels never invent flowcell identity. |
| Per-source `length_by_hour`, `q_by_hour` | Partial | All/Q unsmoothed means plus opt-in raw-read fits with global method selection; U collection/time and fit tests. Exact R fits/ribbons remain Partial. |

## CLI and outputs

| Original option / output | Status | Equivalent / difference / validation |
| --- | --- | --- |
| `--input` / `-i` | Complete | Repeated input option plus positional files/directories; U alias and discovery tests. `analyze` subcommand retained. |
| `--outputdirectory` / `-o` | Complete | Alias of required `--output`; no implicit writes beside inputs; U alias/output-failure tests. |
| `--qscore_cutoff` / `-q` | Complete | Alias of `--qscore-cutoff`, default 7 inclusive; U cutoff boundary tests. Finite nonnegative cutoffs only. |
| `--processors` / `-p` | Partial | Accepts 1; larger values fail explicitly. Processing remains sequential, no parallel speed parity. |
| `--combined-only` / `-c` | Complete | Flag, skips individual outputs; U collection exports. |
| `--smallfigures` / `-s` | Complete | 120 mm SVG width vs normal 180 mm; wide physical maps stay 320 mm; U export dimensions, V. Boolean flag instead of R logical value. |
| `--muxes` / `-m` | Complete | Configured hour references, not events; default 0 disables, maximum 512 markers; U relative-coordinate/disable/error tests. R's inferred default and fixed combined 8h default are not assumed. |
| `--add-stat` / `-a` | Complete | Exact-model mean length/N50 and mean Q on corresponding histograms; U SVG annotation/JSON consistency, V. Flag instead of R logical value. |
| `--format` / `-f` | Partial | `html` default, `svg` adds independent figures. Unsupported values fail, never silently converted; U export/CLI tests. |
| `summary.yaml` | Complete | Optional `--yaml`, MinIONQC-shaped All/Q sections, thresholds/ultralong; U YAML scalar/mapping parity/escaping/nulls, P canonical values. Intentional denominator/rounding differences below. |
| Independent SVG figures | Complete | Current manifest, embedded CSS/title, mm dimensions; U export/offline/staging tests, V. No external fonts/scripts required. |
| Independent PNG / PDF / PS / JPEG / TIFF / BMP | Missing | No bundled raster/document renderer. SVG is a lightweight intermediate; conversion is user's external publishing step. Browser printing is not figure-format parity. |
| HTML / JSON | Complete | PoreGauge additions; unchanged validated metrics, schema 1 additive fields; U deterministic/offline/escaping tests. No-JS mode retains all scientific plots/table data. |

## Scientific differences and evidence

All summary records, positive-length basecalled reads, native passed reads and
Q-selected reads are separate populations. YAML “All reads” uses basecalled reads;
channel comparison explicitly also exposes All-summary denominators. Core N50
retains >=half; ultralong uses upstream's strict-half definition with its endpoint
bug corrected. Required malformed rows fail rather than silently disappearing.
Legacy finite signed Q values remain unmodified; modern validation stays strict.
YAML retains full available precision, null unavailable values and integer bases;
upstream rounds several fields to one decimal. No private input identifiers enter
output paths. Detailed numeric MinION samples are bounded and explicitly labeled.

Earlier public comparisons: short 99 /1,089,633 bp; PromethION 75,160 /623,196,506;
1D2 159,799 /930,249,488; RNA 963 retained /197,413 (37 excluded); pooled MinION
608,208 /7,203,301,321 bp (N50 33,161; median 2,616). Threshold/ultralong values
agree with upstream YAML and independent calculations. Short/1D2 YAML labels are
stale (Q>=7 labels, Q8 invocation/results). This fast check did not execute R. The private
PBM95914 baseline was not rerun; its historical totals remain in metrics.md.

Issues should remain open wherever their required acceptance criteria exceed
these implemented, tested behaviors. In particular, sampling, smoothing/KDE and
six static export formats prevent an unqualified full-parity claim.

Earlier baseline checks (before #11–#13): rustfmt, Clippy with warnings denied, **17 unit +76
integration tests**, and the locked release build. Existing JSON subtrees match
the pre-multi-run binary on synthetic, advanced-quality and Deep-QC fixtures.
Chrome inspection covered combined HTML at 1440/390 px, a no-script MinION report
copy, and an independent SVG with its title/legend. The final two-run synthetic
export's 89 SVG files passed XML parsing; Ruby Psych independently parsed a YAML
summary and confirmed its integer yield against JSON. These are validation
checks, not full-run performance benchmarks. No Rust dependencies were added.

## Executable reference validation (#11)

The optional [paired R harness](../tests/reference/README.md) runs the exact pinned
script and actual `ggplot_build()` layers, with R 4.4.1 / ggplot2 3.5.1 / mgcv
1.9-1 and a dated CRAN snapshot. It records source/input hashes, environment,
actual YAML, plot metadata and machine-readable differences. Statuses distinguish
exact, rounding-tolerance, intentional semantic, suspected PoreGauge defect,
proved upstream defect and unverified results. The validation-only Rust probe
preserves upstream file-scoped populations without changing CLI run grouping.

Actual execution (2026-10-18): all **17 cases executed the unmodified R script to
completion**, including MinION, PromethION, short, 1D2, RNA, two-flowcell pooled
QC and the synthetic boundary/fit/density cases. Native macOS arm64 versions were
checked: R 4.4.1, ggplot2 3.5.1, mgcv 1.9-1. Complete transitive package versions
are captured; these are not claimed identical to the optional CRAN snapshot.
The first full `--capture-only` run saved actual YAML and numerical ggplot layers
in `benchmark-results/minionqc-closure/`. It returned **1**, with 50,254 exact,
38,165 tolerance, 34 intentional differences, 8,946 upstream defects, **10,425
suspected defects** and 592 unverified checks. The suspected differences were
GAM/singular LOESS, constant-domain densities, and six ultralong endpoint checks
whose proof was too narrow (now extended to every full-prefix strict-half N50
>=100 kb, with the reference count/yield checked independently). They are not
accepted as equivalent. Subsequent corrections/results are recorded below.
A separate preflight-enabled short run also generated **11 actual PDF figures**;
its four confidence-band comparisons remain unverified (exit 2).

The harness now checks installed versions before analysis, checkpoints every
case, and records execution/completion/exit status and capture hashes. Six Python
harness tests pass. Use an existing R installation as documented; automatic R CI
builds remain removed. #11 execution evidence does not establish #12/#13 parity.

## Fitted time trends (#12): Partial

`--minionqc-trends` now renders separate individual-read fits in single and
comparative reports without replacing interval means. The implementation follows
[ggplot2 3.5.1 method selection](https://github.com/tidyverse/ggplot2/blob/v3.5.1/R/stat-smooth.R):
largest group across panels, LOESS for <1,000, otherwise mgcv-style `cs`/REML.
Bounded unique-time knots and a second streaming pass avoid unbounded paired-read
retention. LOESS now uses padded boundaries, median-split cells and Hermite
interpolation of fitted vertex values/derivatives (cell .2). R's pseudoinverse
for some singular local fits, exact large-group GAM knots/null-space orientation
and confidence bands are **not** claimed equivalent. Algorithm and population details
are in metrics.md. Polynomial/spline/constant-model unit tests and CLI regressions
passed.

Limited actual R comparison: the unchanged public short fixture at Q8, pinned
upstream `54485d5`, ran to completion under native macOS arm64 R 4.4.1, ggplot2
3.5.1, mgcv 1.9-1, data.table 1.15.4, readr 2.1.5 and scales 1.3.0. All four
80-point fits (length/Q, All/Q8) met the stated tolerances: 320 fitted values,
maximum absolute error **1.274e-10**, maximum relative error **1.855e-14**; maximum
X error **1.388e-17 h**. The harness recorded 888 exact, 402 within tolerance,
four intentional population differences, 20 proved upstream channel-count bugs
and four unverified confidence-band comparisons. This is one fixture, not proof
of GAM, density or complete-suite parity. Native transitive packages differ from
the dated container snapshot; full package versions/sessionInfo are in the local
capture. No private input or derived report was committed.
The executable harness compares methods, all 80 X/Y points and explicitly records
unverified confidence intervals. Issue #12 remains open.

Actual full-suite follow-up: the GAM cases with >4,096 unique start times fail
the fitted-value target with approximate knots; these are reported as suspected
defects, not rounded away. The 1,500-read synthetic GAM meets the target. A focused
LOESS correction now uses equilibrated QR/SVD local solves and singleton cell
splits, recovering the zero/cutoff subset. `tests/reference/trends-golden.json`
contains only synthetic inputs and actual pinned-R layer values, with capture
hashes. Its small/large/zero-cutoff test compares **958 uncensored fitted values**:
maximum absolute error **6.698e-6**, within absolute 1e-6 + relative 1e-6.
Two reference-negative length predictions are scale-censored, not invented zeros.
Four-read N50/ultralong fixtures remain roundoff-sensitive in rank-deficient
neighborhoods; confidence intervals remain unavailable. These limitations keep
#12 **Partial/open**. No raw time-bin or core QC metric changed.

## R-grid densities (#13): Partial

`--minionqc-density` adds a separate R 4.4-style Gaussian estimator and uses it in
combined distribution charts. Legacy density values remain available unchanged.
The estimator uses shared scale domains, transformed log lengths, `bw.nrd0`,
512 mass/evaluation bins, four-bandwidth extension and direct discrete convolution
of the same padded-grid kernel. Summary-Q density explicitly includes zero-length
rows; ordinary basecalled quality metrics still exclude them. Details and extra
optional zero-Q storage are documented in metrics.md.

Tests passed for bandwidth, area/symmetry/peaks, near-identical/constant values,
long tails, unequal sizes and shared axes. The expanded optional harness covers
17 public/synthetic cases and compares 512 X/density values per series, domain
integrals and peak locations. It does not confuse an observed-domain integral
with full Gaussian mass or auto-accept mismatches. **Actual R verification remains
unavailable**; constant-domain expansion, FFT rounding and fitted confidence bands
are not verified. Neither #12 nor #13 is promoted to Complete or closed.

Current closure checks: rustfmt, warning-free Clippy, **25 unit +81 integration
tests**, four Python harness tests and the locked release build passed. The
optional Rust file-population probe also executed on the public short fixture
and produced 80-point fits and 512-point densities; this is not an R comparison.
Chrome inspection covered the new combined views at 1440/390 px; all 87 exported
synthetic SVG files passed XML parsing. No private-data rerun or performance
benchmark was performed. No Rust dependencies were added. Fits require an extra
streaming pass; optional summary-Q densities retain eight bytes per zero-length
Q observation in addition to existing buffers.
