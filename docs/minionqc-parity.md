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
| `length_by_hour`, `q_by_hour` | Partial | Unchanged 15-minute means plus opt-in interpolated LOESS/cubic-shrinkage REML fits; U temporal/fitting tests. Exact unique-time knots, singular two-row LOESS solves and GAM ribbons are implemented; public GAM numerical parity remains incomplete; LOESS pointwise Student-t intervals are verified against actual R. |
| `channel_summary` | Complete | Four channel distributions and exact values, All-summary/Q; U channel summary tests, V. |
| `gb_per_channel_overview` | Complete | Explicit-layout yield heatmaps including All/Q (native maps additionally available); U layout/yield conservation, V. |
| MinION `flowcell_overview` | Partial | Exact physical panels, actual start/length/Q values, deterministic <=16 reads/channel; U sample bound/mapping tests, V. Full-read panels not reproduced. |
| MinION `length_vs_q` | Partial | Actual bounded points, optional events/base color, missing gray; U MinION samples, V. Uses log10(1+events/base); original uses log(events/base). No claim of point-for-point parity. |
| PromethION physical read-level overview | Not applicable | Upstream omits the per-read channel panels for PromethION. |
| PromethION `length_vs_q` | Complete | Bounded logarithmic-length/Q count heatmaps, matching the upstream bin2d analysis family; fixed documented bins, no individual read points; U joint count conservation, V. |
| `combined_length_histogram`, `combined_q_histogram`, `combined_yield_by_length` | Complete | Pooled All/Q observations and exact yield queries; U pooled conservation/exact median/N50, P multiple inputs, V. |
| Per-source `length_distributions`, `q_distributions` | Complete | Opt-in shared-domain R-grid estimator, including summary Q observations; actual pinned-R comparison of 52 curves meets documented tolerances. U synthetic R goldens; executable evidence below. Legacy KDE remains separately labeled. |
| Per-source `yield_over_time`, `yield_by_length` | Complete | Absolute yields, common units/colors; U combined report/curve tests, P pooled totals, V. Run/source labels never invent flowcell identity. |
| Per-source `length_by_hour`, `q_by_hour` | Partial | All/Q unsmoothed means plus opt-in raw-read fits with global method selection; U collection/time and fit tests. LOESS intervals verified; strict original-R GAM fits/ribbons remain Partial. |

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

## Executable reference validation (#11): Complete

Actual execution on 2026-10-18 used the unchanged pinned MinIONQC script, native
macOS arm64 **R 4.4.1 / ggplot2 3.5.1 / mgcv 1.9-1**. Installed versions are
checked before analysis, not inferred from an environment name. Full package
versions/sessionInfo are recorded; native transitive versions are not claimed
identical to the optional dated Docker snapshot. Normal PoreGauge never needs R.

All original **17 cases plus a singleton case (18 total)** completed the R side
with exit 0: MinION, PromethION, short, 1D2, RNA, two-flowcell pooled QC, zero/cutoff/
N50/ultralong boundaries, small/large/sparse fits and six density scenarios.
The final paired run used `--capture-only`: actual YAML and `ggplot_build()` layer
values, metadata, panels, axes and legends were generated without PDF rendering.
A separate short run generated **11 actual PDF figures**. The file-population
probe preserves upstream grouping without changing production run identity.

The full comparison is `benchmark-results/parity-final/comparison.json`.
The committed [machine-readable summary](../tests/reference/validation-summary.json)
records input/source/capture/binary hashes, checked versions, per-case completion,
counts and maximum errors. Run instructions are in the
[reference README](../tests/reference/README.md). Cases checkpoint independently;
wrong versions fail preflight and unavailable/partial R output never fabricates
comparisons. Eleven Python harness tests pass; Rust checks pass (37 unit +83
integration tests, one optional probe ignored), including release build. Desktop
Chrome inspection confirmed the offline GAM ribbons; mobile rendering was not
rechecked in this mathematical correction.
After the final tooltip-only change, all 18 CLI cases were rerun: 63 JSON reports
match the actual paired execution exactly. Both binary hashes and the recheck
are recorded; the unchanged R captures were not rerun for the caption change.

| Final check classification | Count |
| --- | ---: |
| Exact match | 65,118 |
| Within declared tolerance | 75,241 |
| Intentional semantic difference | 36 |
| Proved upstream defect | 8,954 |
| Suspected PoreGauge defect | **1,542** |
| Not yet verified | **0** |

**The suite returns 1, not success.** Remaining failures are 390 fitted values,
381 standard errors, 381 lower and 390 upper bounds: **1,542**, down from 6,302
under the same point tolerances. They occur in the MinION Q-selected population
and its duplicated combined-context plots. The former 112 LOESS availability
checks are resolved by 22,080 finite SE/bound comparisons and 4,800 explicitly
nonfinite availability comparisons, all passing. All **92,299 non-fit checks** match
the preceding paired run exactly, including core statistics, channels and KDE.
Intermediate checks record 48,648 matches/tolerant matches and **1,912 failures**
separately in `gam_diagnostics`;
they do not disappear into a changed primary pass count.

## Fitted time trends (#12): Partial

Existing unsmoothed 15-minute means and core JSON meanings are unchanged. Optional
raw-read fits retain ggplot2's method/population/grouping rules and exact bounded
unique-time knot selection. The follow-up diagnoses two independent causes:

1. **The cs null space.** Raw natural cubic bases, second derivatives, positive
   eigenvalues, null-space projectors and design crossproducts agree. After
   assigning unequal .1/.01 shrinkage weights, numerical eigenvalue ordering can
   swap the two null directions. The preceding PromethION selected penalty
   differed by 2.113e-6 after normalization, despite raw-matrix differences around
   2e-16. Rust now specifies an endpoint-zero linear direction and its orthogonal
   constant complement, with .1/.01 weights respectively. This is deterministic,
   but **is not identical to every pinned-R numerical ordering**.
2. **Finite-tolerance REML optimization.** mgcv jointly optimizes log(lambda) and
   log(variance); the old profiled bisection sought a more accurate stationary
   point, which differs from mgcv's returned finite-iteration result. The focused
   Gaussian Newton implementation uses analytical gradients/Hessians, original
   response-unit likelihoods, mgcv initialization, step limits, backtracking and
   score-scaled stopping conditions. Gaussian posterior covariance uses the
   final RSS/(n−edf), matching mgcv's postprocessing, rather than optimized REML
   variance. A bracket-checked profiled fallback retains finite noiseless views;
   full degenerate-fit parity is not claimed.

No fitted-value corrections or wider tolerances are used. The existing sorting/
scratch algorithm is unchanged. Only type-7 arithmetic order was aligned with
R's `seq()` and one-based interpolation indices; all 320 captured knot coordinates
now match exactly. Fused multiply-adds reproduce the pinned natural-cubic kernel's
single-rounding operations. Tests cover R's exact synthetic knot arithmetic,
analytical REML derivatives, existing synthetic goldens and four public PromethION
aggregate models (no sequencing records/identifiers stored).

| Public case | Before 406e354: max fitted length error (bp) | 406e354 and current (bp) |
| --- | ---: | ---: |
| MinION | 2.350897 | **1.715426** |
| PromethION | 211.017091 | **1.342e-8** |
| 1D2 | 0.197879 | **0.000622210** |
| Combined | 7.440929 | **1.715426** |

All PromethION and 1D2 predicted points, standard errors and bounds meet the
unchanged absolute 1e-6 + relative 1e-6 target. The original problematic endpoint
is R **11750.65090436984 bp**; its regression test uses the same tolerance.
Passing curves alone do not prove penalty equivalence: some 1D2 and synthetic large-fit
shrinkage matrices still differ, with sufficiently weak effects to pass the
point target. MinION selected fits and combined comparisons remain outside it.

**Independent R sensitivity evidence:** using the public MinION Q≥7 subset,
changing one knot by **1.776e-15 hours** changes the raw penalty by **1.554e-15**,
swaps the numerical null directions and changes the R endpoint by **1.715426 bp**
(23021.752153 →23023.467579). This reproduces the residual discrepancy within R
itself. Run `tests/reference/cs_sensitivity.R` as documented in the reference
README. This is evidence of numerical instability, **not acceptance of an
unmatched original-reference prediction**. Emulating a particular BLAS/eigensolver's
roundoff is not a portable mathematical null-space definition. #12 stays open.

`capture.R` records raw/shrunk penalties, eigenstructure, second derivatives,
80-point design values, model transforms/crossproducts, coefficients, covariance,
REML score/optimizer diagnostics, edf and uncensored predictions/SEs. Comparisons
transform model crossproducts/covariance to common nodal coordinates; they never
compare unlike spline parameterizations or sign-dependent null eigenvectors.
GAM bands are pointwise 95% normal intervals as verified against
`ggplot2:::predictdf.glm`. LOESS now uses the actual interpolated influence weights,
exact hat trace, approximate residual-delta calibration, residual scale, effective
df and Student-t critical value. Eight synthetic actual-R cases verify 5,280
fitted/uncertainty values and nonfinite states, including ties, irregular times,
sparse groups and 999 reads. The paired suite adds 664 separate LOESS diagnostic
checks (248 exact, 416 tolerant), all passing. No private PBM95914 run was
reprocessed; no runtime dependency or benchmark claim was introduced.


### Final controlled assessment

| Strict check | 406e354 baseline | Current |
| --- | ---: | ---: |
| GAM fitted-value failures | 390 | 390 |
| GAM SE failures | 381 | 381 |
| GAM lower / upper failures | 381 / 390 | 381 / 390 |
| GAM intermediate failures | 1,912 | 1,912 |
| Unverified LOESS band checks | 112 | 0 |

GAM maxima are unchanged from 406e354 (table above); the machine summary records
before/after length and Q-score errors, SEs and both bounds for every case.

The unchanged strict comparison still fails; the following are **diagnostic
experiments, not substituted reference targets**. `gam_assessment.R` fits 20
unique public/synthetic models with identical observations and knots, changing
only the null penalty convention, then optionally imposing Rust's penalty and
lambda. For MinION Q≥7, swapping the original two null weights accounts for all
remaining curve/uncertainty discrepancies:

| MinION selected metric | Original R vs Rust: max error | R swapped weights vs Rust: max error |
| --- | ---: | ---: |
| Length fit (bp) | 1.715425619 | 2.155e-7 |
| Length SE (bp) | 0.466714392 | 3.877e-10 |
| Length lower / upper (bp) | 2.630169018 / 0.800682220 | 2.163e-7 / 2.148e-7 |
| Q-score fit | 8.974e-4 | 5.607e-12 |
| Q-score SE | 2.880e-5 | 2.010e-14 |
| Q-score lower / upper | 8.812e-4 / 9.135e-4 | 5.647e-12 / 5.567e-12 |

The 1,542 primary failures are the two selected MinION curves repeated in three
contexts: standalone MinION, its individual report inside the combined case,
and the combined comparison. Each context has 130 fit, 127 SE, 127 lower and
130 upper failures. No additional unexplained over-tolerance effect remains in
these controlled experiments.

The 1,912 original intermediate failures split into **1,400 normalized penalty
cells, 442 covariance cells, 45 coefficients, 14 lambdas, 7 edf and 4 REML scores**.
Reconstructing the canonical penalty from actual R eigenpairs matches all 3,200
captured cells (maximum difference 8.882e-16). Canonical R fits pass all curve,
coefficient and covariance targets, including the weaker 1D2/synthetic effects.
Their maximum log(lambda) difference is 1.254e-10. Imposing Rust's exact penalty
and lambda also passes; no separate optimizer or covariance defect was found
above the existing tolerance. Small remaining accumulation differences are
recorded, not asserted zero. Swapping weights on every dataset is **not** a fix:
it would disturb models whose original ordering already agrees.

**Stability:** 32 original GAM models from independent complete reference runs
are identical in knots, sp, variance, uncensored predictions and audit data.
All 20 controlled models also repeat identically within R. Thus the pinned local
reference is reproducible; knot-perturbation sensitivity does not establish
run-to-run instability. The installed OpenBLAS 0.3.34 reports VORTEX; its ARMV8/
VORTEX library aliases resolve to the same binary, and requesting ARMV8 still
reports VORTEX. No independent second BLAS was available or measured; cross-BLAS
variation remains unverified. Library/source/capture hashes accompany the results.
Bitwise identity is a useful same-binary regression check, not a portable
scientific requirement. The original pinned-environment numerical target remains
strict and still fails; the instability evidence does not waive it.

### Parity contract

| Category | Contract / current status |
| --- | --- |
| Exact scientific metrics | Established read/yield/filter/N50 semantics and intentional upstream differences remain unchanged; all 92,299 non-fit checks preserved. |
| Pinned-environment numerical parity | Original R points/SE/bounds retain absolute 1e-6 + relative 1e-6 targets. LOESS and public PromethION/1D2 curves pass; MinION selected contexts still fail. |
| Implementation-dependent fits | Optional GAM display explicitly names the deterministic null-space convention. Same model family does **not** mean the unequal penalties are mathematically identical. Controlled evidence explains the difference but never changes strict status. |
| Unsupported/unverified | Independent-BLAS parity and general degenerate GAM fallback equivalence are unverified. Nonfinite LOESS bands remain unavailable, not zero. |

**Policy proposal:** retain the named native convention for reproducible display,
keep original-reference failures visible alongside the controlled explanation,
and keep #12 **Partial/open**. Do not emulate eigensolver roundoff or widen a
global tolerance. Full artifacts and repeatable commands are in the reference
README; the committed summary records before/after counts and controlled errors.

## R-grid densities (#13): Complete within declared tolerances

The opt-in estimator now matches actual constant-domain behavior: all 512
coordinates remain at the single observed value, rather than inventing a width.
HTML expands only its display axis and shows evaluated points with an explanatory
note. Other domains, bin definitions and legacy distributions are unchanged.

Actual `compute_density()` instrumentation captures bandwidth, n, domain, kernel
and grid size without modifying returned estimates. **52 curves / 26,624 point
pairs**, including public combined inputs, All/Q subsets, unequal sizes,
multimodal/skewed/constant/near-constant/outlier data and singleton unavailability,
passed all 53,720 density checks. Measured maxima:

- Bandwidth absolute error **6.821e-13**; X error **1.819e-12**.
- Curve error divided by its reference peak **9.212e-8**.
- Plotted-domain integral error **6.688e-11**; peaks within one grid interval.

Bandwidth tolerance: absolute 1e-12 + relative 1e-10. X: absolute 1e-10 + relative
1e-12. Density: absolute max(reference peak × 1e-7, 1e-12) + relative 1e-6.
Integral: absolute 1e-7 + relative 1e-6. Tiny tail-relative errors are not useful;
the stated absolute/peak criterion applies. This is not bit-identical FFT output.
The observed-domain integral is not forced to one; constant-domain area is zero.
Lengths are transformed with log10 before estimation (density per log10(bp)),
with no raw-length Jacobian or boundary reflection. Summary Q includes zero lengths.

[Synthetic-only density goldens](../tests/reference/density-golden.json) test all
11,264 points in 22 distinct curves plus two unavailable singleton states, exact
bandwidths/domains, integrals and peaks. Duplicate identical All/Q snapshots are
stored once; the paired suite still checks both. Scientific details and optional
zero-Q storage costs are in [metrics](metrics.md#optional-r-grid-density-estimates-13).

Final checks: rustfmt, warning-free Clippy, **28 unit +82 integration tests**, seven
Python harness tests and the locked release build passed; one optional probe is
ignored by routine tests and executed by the paired suite. Chrome `file://`
inspection verified the constant-domain points/axes/legend; no new mobile rendering
check was performed. No private PBM95914 raw summary or large benchmark was run.
No runtime dependencies were added; fits and KDE remain bounded as documented.
