<a id="minionqc-v141-vs-ontrunqc--detailed-compatibility-comparison"></a>

# MinIONQC scientific and visual comparison

Source-backed assessment against [MinIONQC v1.4.1](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R).
Functional coverage, populations/formulas, numerical layers, visual rendering
and CLI defaults are separate claims. Full strict numerical parity is not claimed.

| Provenance | Recorded baseline |
| --- | --- |
| Reference source | `54485d586038d8ee36f168dd94f31c423f52f581` |
| Pinned environment | R 4.4.1, ggplot2 3.5.1, mgcv 1.9-1; macOS ARM64 |
| Source/figure audit | 2026-10-19, starting from native commit `8f3d4d73a18a531ea2c3f728fb859cadd90ba801` |
| Immutable numerical evidence | [Validation summary](../tests/reference/validation-summary.json), [revalidation signatures](../tests/reference/revalidation-summary.json) |
| Actual figure/layer evidence | [Gallery](figures/minionqc-comparison/README.md), [measurements](figures/minionqc-comparison/measurements.json), [discrepancies](figures/minionqc-comparison/discrepancies.json) |

[Documentation index](README.md) · [Metric reference](metrics.md) · [Engineering ledger](minionqc-parity.md) · [Reference commands](../tests/reference/README.md)

The historical audit is preserved. The #19–#21 sections record additional actual
R executions without changing original scientific targets or strict GAM evidence.

## Contents

- [Compatibility status and evidence vocabulary](#compatibility-status-and-evidence-vocabulary)
- [Discrepancies and priorities](#discrepancies-and-priorities)
- [Input compatibility and populations](#input-compatibility-and-populations)
- [Scientific formulas and source mapping](#scientific-formulas-and-source-mapping)
- [Plot-family inventory](#plot-family-inventory)
- [CLI defaults and output behavior](#cli-defaults-and-output-behavior)
- [Reference histogram and Bin2D validation (#19)](#reference-histogram-and-bin2d-validation-19)
- [Reference temporal validation (#20)](#reference-temporal-validation-20)
- [Yield-by-length display (#21)](#yield-by-length-display-21)
- [Strict numerical GAM limitation](#strict-numerical-gam-limitation)
- [Executed validation and limits](#executed-validation-and-limits)
- [Reproduce the evidence](#reproduce-the-evidence)
- [Remaining acceptance work](#remaining-acceptance-work)

<a id="1-executive-summary-and-meaning-of-parity"></a>

## Compatibility status and evidence vocabulary

MinIONQC is an R tool for assessing Oxford Nanopore sequencing-summary files and generating static per-flowcell and combined plots. ONTRunQC is an independent Rust CLI analyzing sequencing-summary TSV with exact core statistics, self-contained offline HTML/JSON, optional YAML and independent figures, with run-aware multiple-input analysis. **Neither is a general FASTQ/BAM/POD5 QC engine as implemented here.**

Functional coverage of the pinned MinIONQC report families is broad, but **strict numerical, plot-bin, pixel-rendering and exact output-default parity are not claimed**. In particular:

- All 18 reference cases completed their original pinned R analysis. The strict paired suite **exits 1**: 1,542 original-R GAM fitted/uncertainty checks remain outside unchanged tolerance, and 1,912 separate GAM intermediate comparisons differ. No R check has been silently reclassified.
- Measured primary statuses in the retained run: **65,118 exact**, **75,241 within tolerance**, **36 intentional semantic differences**, **8,954 checks classified as proven upstream defects**, **1,542 suspected ONTRunQC GAM differences**, and **0 unverified in that particular paired suite**. These are *check counts*, not distinct bugs or proof that every possible input was tested.
- The 1,542 failing checks are 390 fitted values, 381 standard errors, 381 lower and 390 upper confidence bounds, for the same MinION Q-selected models repeated in single-run and combined plot contexts.
- Independent R-grid KDE checks passed for 52 curves / 26,624 point pairs; LOESS pointwise values and supported intervals passed their recorded reference checks. Core population/threshold/pooled analyses have their own independent test evidence.
- The compiled/documented Rust GAM uses an explicit, stable cubic-shrinkage null-space convention. It is *not* the same penalty orientation that pinned mgcv selects for every input; an R-only controlled experiment reproduces the mismatch after changing only those shrinkage directions. A documented explanation is **not** strict numerical equivalence.

### Evidence/status vocabulary

| Status | Meaning | How this document uses it |
| --- | --- | --- |
| Exact / verified | Matched actual pinned R values or an independent exact calculation under stated definitions | Cite fixture/test and comparison scope |
| Functional with differences | Provides the same scientific question/output family but with different binning, sampling/raster, formatting, population selection or default | Identify the precise difference |
| Intentional semantic difference | Native definition differs intentionally or corrects a separately proved reference defect | Do not present as identical R output |
| Strict numerical failure | Values differ beyond existing pinned R numerical tolerance | Keep visible and refer to #12 |
| Unverified | Evidence has not been executed or property not established | Do not infer success from routine Rust CI |

**Primary unresolved acceptance:** [#8 multi-run combined parity](https://github.com/PhoenixEmik/ONTRunQC/issues/8) retains its original strict fitted-curve requirement; [#12 GAM smoothing](https://github.com/PhoenixEmik/ONTRunQC/issues/12) retains the actual failing R comparison. A proposed split between functional and strict gates is not an approved closure or waiver.

<a id="11-prioritized-discrepancy-register-and-measured-impact"></a>

## Discrepancies and priorities

The JSON register gives exact source/function/test references, previous claims,
root causes, numeric/visual/scientific impact and acceptance criteria. The negative-Q display defect below was corrected. No native
core-statistics defect was demonstrated in the tested populations; no core QC,
GAM/LOESS, KDE or reference target was changed to make plots agree.

| Finding | Classification / impact | Measured evidence | Action |
| --- | --- | --- | --- |
| Negative-Q underflow labels | Confirmed native presentation defect, fixed | Sentinel `-1.798e308` and shifted8/18/28 ticks replaced by explicit `<0` category and10/20/30 ticks; JSON unchanged | Regression plus actual public before/after figure |
| Automatic length/Q and channel histogram edges | Verified optional reference mode; native defaults retain different bins | 75 comparable panel/layer groups match original R exactly; true channel count correction remains separate | [#19 validation](#reference-histogram-and-bin2d-validation-19); native definitions preserved |
| PromethION bin2d | Verified optional reference mode | All 516 cells / 75,160 reads match actual R coordinates and counts exactly | Explicit `--minionqc-plot-bins`; fixed native heatmap retained |
| Hourly reads | Verified optional reference mode | Exact integer-hour All/Q summary counts and zero-filled gaps; native 900-second diagnostics unchanged | [#20 validation](#reference-temporal-validation-20) |
| Read-start cumulative path | Verified bounded representation; explicit tied-order differences | Original coordinates, exact terminals, full-reference error measurements; not point-for-point parity | [#20 validation](#reference-temporal-validation-20); native bin-end path retained |
| Minimum-length retained-yield curve | Verified bounded inclusive display; distinct R per-read semantics | Historical query-line errors 1,555,278 /5,464,673 /4,431,919 bp fall to 392,578 /128,977 /750,632 bp at observed inclusive lengths | [#21 measured contract](#yield-by-length-display-21); exact queries unchanged |
| R length-yield X crop | Presentation/population visibility difference | R MinION X ≤ 93,318bp, PromethION ≤ 81,890bp; 677/116 built XY rows nonfinite or censored across All/Q | Full native tail retained; R crop/clipped counts independently verified under [#21](#yield-by-length-display-21), not emulated |
| Channel count | Intentional scientific correction of proven R defect | n=2 gives R3 vs true2; original n(n+1)/2 expression | Preserve true counts; do not emulate bug |
| RNA missing calibration annotation | Intentional population difference, newly verified | Missing/NA/ordinary/control lengths 10/20/30/40: R 1 read/30 bp vs native 3 reads/60 bp | Existing exact-name policy retained; new regression and runnable counterexample |
| MinION points/raster and spatial palettes | Presentation differences with full-data conservation | Exact small points / bounded all-count pixels; actual R finite counts, marginal counts and coordinate/yield checks | Keep resolution/color-reduction labels; no independent-point/pixel identity claim |
| Fitted trends / density defaults | Numerically different default estimators | Compatible estimators require explicit flags here | Command pairs disclose settings; existence of an estimator is not default equality |
| Original GAM uncertainty/fits | Strict numerical limitation | 1,542 primary and 1,912 intermediate differences; every original strict check reproduced | [#12](https://github.com/PhoenixEmik/ONTRunQC/issues/12) stays open; no changed targets/tolerances |
| Validation lockfile mutation | Confirmed tooling defect, fixed | Unlocked probe could reorder Cargo.lock; now uses `cargo test --locked` | Repeated suite leaves lockfile unchanged |

**Completion boundary:** this audit delivers evidence and runnable comparisons;
reference-compatible histogram/Bin2D layers in #19 are now implemented and
verified below; #20 now supplies exact hours and bounded read-start curves, while
#21 now supplies measured adaptive step envelopes with explicitly different R per-read/crop semantics. #8 and #12 remain open. No release is published.

<a id="2-input-compatibility-and-population-definitions"></a>

## Input compatibility and populations

| Topic | Pinned MinIONQC | ONTRunQC | Consequence |
| --- | --- | --- | --- |
| Input | Plain sequencing_summary.txt, single file or recursive directory | Plain or streaming gzip, unquoted TSV; explicit files or recursive matching sequencing_summary*.txt/.tsv and .gz variants | Native gzip [#17](https://github.com/PhoenixEmik/ONTRunQC/issues/17) is an input-layer enhancement outside the pinned R parity contract; the same parser and metrics are used after decoding |
| Modern headers | Reads compatible template-read fields | Validates required modern fields; accepts auto/legacy modes | Invalid required data cause errors, not silent drops |
| Legacy / 2D / 1D2 | Historical Albacore/Guppy handling, complete 2D field substitutions | Explicit template/2D choice, no per-row template fallback; absent modern fields stay unavailable | Public legacy and 1D2 comparisons exist |
| Direct RNA | Removes rows identified by YHR174W calibration strand | Exact named control exclusion with excluded-record count | Exclusions must not be mistaken for a general RNA QC filter |
| Original grouping | File and parent-directory-derived flowcell labels | Actual run-ID grouping across chunks; unknown identities kept explicitly separate | Run is not assumed to equal a distinct physical flowcell |
| Native pass/fail | Core upstream plot split is based on mean Q threshold | Native passing uses only passes_filtering when available | Native pass and Q-selected subsets are *not interchangeable* |
| Q-selected | Reads with per-read mean Q ≥ configured cutoff | Separate inclusive Q-selected population, default Q 7 | Selected stats preserve independent Q threshold |
| Zero-length records | "All reads" plots/statistics can include summary rows with length zero | Total summary rows are kept separately; basecalled length/Q metrics use positive-length reads | Denominator differences affect means/medians and some plot populations |
| Malformed values | readr/reference behavior may drop problem rows | Invalid required rows fail with source/line context | Intentional correctness/diagnostic policy |
| Device geometry | Reference uses observed max channel to choose MinION vs PromethION | Explicit MinION R9.5 512 or PromethION 3000 layout | No potentially wrong inferred instrument shape |
| Processing | R mclapply across summary files | Bounded parallel *independent run components* with ordered outputs and exact pooled reduction | Shared run-ID files never silently process as distinct runs |

See [the original loader and transformations](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L159-L277) and [ONTRunQC input parser](../src/input.rs), [batch grouping](../src/batch.rs) and [integrity checks](../src/integrity.rs).

<a id="9-source-level-audit-formulas-and-population-contract"></a>

## Scientific formulas and source mapping

This audit reads the **entire 956-line pinned script**, not only example images.
`Complete` in the historical ledger meant an analysis family exists; it is **not**
a certificate of equality on all five axes below. A corrected engineering status
must distinguish functionality (**F**), population/formula (**M**), numeric plot
layer (**L**), visual rendering (**V**) and CLI defaults (**D**). Different valid
histogram/time resolutions are L differences, not defects in total yield.

In the formulas below, `S` is the reference's retained summary population, `B` is
its positive-length subset, and `Q` means inclusive mean-Q ≥ cutoff. Native
`passes_filtering` is an independent ONTRunQC population. `l`, `q`, `t` denote
reported length, mean read Q-score and read-start seconds. No table treats a
summary-derived observation as instrument telemetry.

| Operation / exact reference formula | Pinned R source | Rust implementation | Reproducible test / difference |
| --- | --- | --- | --- |
| Read selected columns, convert numeric; drop `readr` problem rows | `load_summary`, 208–232 | `input.rs::Columns::{new,parse}`, `SummaryReader::next_record` | Parser malformed/missing-column integration tests. Rust rejects malformed records; no silent dropping. |
| Exclude calibration label exactly `YHR174W`; R `subset(label != control)` also drops missing labels | 234–237 | `input.rs::Columns::parse`, `metrics.rs::Aggregator::add` | `direct_rna_controls_are_excluded_and_counted_separately`. Actual synthetic R capture: missing/NA/ordinary/control lengths 10/20/30/40 retain 1 read/30 bp in R, 3 reads/60 bp here. `unknown_rna_calibration_labels_are_retained_instead_of_assumed_controls` protects this intentional distinction. |
| If 2D length column exists, substitute 2D length/Q for the **whole file**, events unavailable | 239–252 | `input.rs::Columns::new/parse` | `legacy_2d_substitution_is_consistent_and_never_falls_back_per_row`; incomplete pairs fail rather than silently mix representations. |
| All = `q >= -Inf`; selected = `q >= cutoff`; zero lengths retained by R | `add_cols`, 159–205; 254–276 | `metrics.rs::Aggregator::{set_qscore_cutoff,add}` | `q_cutoff_is_inclusive_exact_and_independent_of_native_filtering`, `signed_cutoffs_select_legacy_reads_without_weakening_modern_validation`. Rust core/subset length/Q stats use B / B∩Q. |
| Device = max(channel) ≤512; merge with physical map | 141–205 | `layout.rs::FlowcellLayout::coordinate`, explicit CLI layout | All512/3000 coordinate tests; `layout_is_opt_in_and_rejects_every_incompatible_record`. Explicit choice avoids false instrument inference. |
| Reads = nrow(S); bases = Σl; Gb = Σl/10⁹ | `summary.stats`, 347–405 | `metrics.rs::Aggregator::add/finish_retaining`, `yaml_summary` | `complete_synthetic_metrics`, public paired YAML; integer bp canonical. Distinct summary/B counts. |
| Mean length = Σl/|S|, mean Q = Σq/|S|; median = middle value or mean of two middle values | 357–362 | `subset_statistics`, `partition_median_length/quality` | `exact_partitions_match_an_independently_sorted_reference_for_every_split`. Rust B denominator and unrounded JSON; R rounds YAML to one decimal. |
| N50 = first descending length with cumulative bases **>** total/2 | 356 | `n50_descending`, 1776–1787 | `q_cutoff_even_and_odd_exact_statistics_preserve_n50_half_convention`. Rust **≥** half is intentional; e.g. [100,50,25,25]: R50, Rust100. |
| Threshold count = ΣI(l≥T), bases = Σl I(l≥T), T=10/20/50/100/200/500kb/1Mb | `reads.gt/bases.gt`, 279–289, 373–398 | `yield_by_length`, 1527–1580 | `exact_yield_thresholds_curve_and_ultralong_are_consistent`; R labels “>” are inaccurate at exact boundaries. |
| Ultralong = longest descending prefix whose strict-half N50 ≥100kb | `binSearch`, 317–345; 364–371 | `ultralong`, 1582–1604 | Same prefix question; R final-index `max-1` bug drops an eligible endpoint. Synthetic singleton/tie/end tests prove correction. |
| Channel bases = Σl; mean/median per channel; **R count = Σwhich(l≥0) = n(n+1)/2** | `channel.summary`, 406–420 | `finish_retaining`, `channel_distributions` | `exact_channel_medians_include_explicit_summary_and_basecalled_denominators`. Correct true count, All-summary means/medians available separately from B. |
| Hour = floor(t/3600); counts by hour; explicitly zero-fill 0…max(hour) | 189–203; 609–638 | `Aggregator::add` 900s bins; `html.rs::time_plot/qscore_comparison` | `html_time_bins_preserve_gaps_and_relative_units`; audit captures exact R hourly rows. Fifteen-minute counts are **not** the reference hourly plot. |
| Chronological cumulative yield = cumsum(l sorted by t), including each read's start | 184–188; 563–572 | `Aggregator::add/finish_retaining`; `time_plot`, `time_series_plot` | Exact totals, different paths. Single overview steps at bin ends; All/Q and combined paths join bin-end points. Missing-bin handling differs from a read-level path. |
| Length yield curve = descending cumulative bases at **each read**, then ggplot sorts X; ties yield vertical segments | 190–192; 575–584 | `yield_by_length`, `html.rs::yield_length_plot/comparison_yield_length` | `every_display_query_uses_exact_lengths_including_ties`: legacy exact queries unchanged. New adaptive step envelope has a measured one-sided error contract; actual R tied min/max checked independently. R crop/per-read ordering remain distinct; see [#21 evidence](#yield-by-length-display-21). |
| Length histogram: log10(l) **before** StatBin, bins=300; Q histogram: raw q, bins=300 | 472–545; 753–781 | `html.rs::lengths/quality/qscore_comparison`, fixed metric bins | `length_quality_bins_include_boundaries_and_unbounded_tails_without_thresholds`, new actual-R bin conservation/membership checks. Equal total counts ≠ equal bin counts or visual density. |
| Channel histograms: StatBin(bins=30), automatic centred breaks per variable shared across All/Q | 670–686 | `channel_distributions`, 30 intervals from zero to maximum | Both use 30 bins, but edges/closure differ. Read-count bug is a separate formula difference; other channel metrics can match exactly. |
| Smooth raw reads, not bin means; ggplot chooses LOESS below 1,000 in largest group, otherwise GAM `cs`, REML | 587–606; 874–890 | `trends.rs::Tracker/Plan`; `html.rs::fitted_trend_plot` | Actual 18-case differential, LOESS/golden tests. Single Q limits remove negative Q before fitting; combined Q does not. Default fit opt-in; #12 remains Partial. |
| KDE = stat density after scale transform; Gaussian bw.nrd0, shared panel domain, n512 | 830–850 | `density.rs::estimate`, `comparison_distribution` | `density-golden.json`, 52 actual R curves. `--minionqc-density` required; default legacy KDE differs in normalization/grid. |
| MinION points: All, x=t/3600/y=log10(l)/colour=q, α.35; length-vs-Q x=log10(l)/y=q/colour=log(events/l), α.05 | 548–561; 640–668 | `minion.rs::Tracker/Renderer`; `html.rs::minion_read_plots` | `tests/minion_visualization.py`, complete finite-row audit grids. Rust exact≤4096; otherwise full-count rasters plus extrema. Mapped color averaging/opacity/point order differ explicitly. |
| PromethION length-vs-Q = All, StatBin2d default 30 bins after log10 length | 649–659 | `length_quality_heatmaps`, fixed log2 length /2-Q bins | Actual R bin2d conservation versus fixed-grid counts; no numeric grid-equivalence claim. Native passed panel is an addition, not the Q-cutoff panel. |
| Spatial tiles = channel bases/10⁹, mapped row/col, reversed Y, All/Q facets | 689–720 | `physical_heatmaps`, `layout.rs` | Actual pre-colour tile values, coordinates and yield comparisons; missing channels explicit, palette/panel styling different. |
| Combined summary = pooled observations, recompute descending cumulative sum and exact summaries | 725–805 | `batch.rs::analyze`, `Aggregator::absorb/finish_retaining` | `multiple_runs_chunks_and_duplicate_paths_have_exact_pooled_statistics`; no averaging N50s/medians. File-scoped R vs reliable run-ID grouping remain distinct. |
| Per-flowcell comparisons keep absolute yield and per-source time origin, All/Q facets | `multi.plots`, 817–891 | `html.rs::combined_comparisons`, per-source `RunComparison` | Public combined capture plus exact pooling tests; same colours per source, eight-colour palette repeats. Numeric fit differences independently retained. |
| File/directory dispatch; per-parent file identity; mclapply workers; summary/plot writes | 899–956; options 25–132 | `batch.rs::{discover,scan_inputs,independent_components,scan_components}`, `cli.rs`, `report.rs::write` | `parallel_groups_preserve_interleaved_chunks_legacy_empty_and_exact_outputs`, worker overlap/failure tests. Bounded safe components and staged files differ intentionally from forked per-file processing. |

Source links: [entire pinned R](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R),
[Rust parser](../src/input.rs), [metrics](../src/metrics.rs), [HTML](../src/html.rs),
[batch](../src/batch.rs), [integration tests](../tests/integration.rs).
Read these with the CLI/default matrix above; plot exports, physical dimensions,
PNG/PDF/PS/JPEG/TIFF/BMP and YAML are projections of the same canonical model.
No original MinIONQC equivalent exists for native pass/mux/speed/end-reason/observed
channel-activity metrics: these are **not applicable** to its parity contract.

<a id="3-scientific-statistics-values-definitions-edge-cases"></a>

Original metrics source: [summary.stats / channel.summary](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L347-L420). Detailed equations, zero-length and N50 examples: [ONTRunQC metrics](metrics.md).

**Public pooled multi-run reference:** 608,208 summary records; 606,143 positive-length basecalled reads; 7,203,301,321 bases; N50 = 33,161 bp; median = 2,616 bp. The public input was rerun during this audit, including the production CLI and file-scoped reference probe. The private PBM95914 sequencing summary was not rerun.

<a id="4-one-to-one-scientific-plot-inventory"></a>

## Plot-family inventory

**Important:** a corresponding plot/report family is not a claim of identical pixel layouts or ggplot-generated bin boundaries. All/selected Q populations, native filtering and zero-length eligibility may require different axes/labels.

| Original MinIONQC output (single-run) | Original layer/semantics | ONTRunQC equivalent | Difference / verification |
| --- | --- | --- | --- |
| length_histogram | Two All/Q panels, 300 ggplot bins after length transform | Native fixed bins plus explicit reference layers | Reference-mode boundaries/counts verified; palettes/layout differ |
| q_histogram | Mean-read-Q distribution by All/Q | Native basecalled bins plus reference summary-population bins | Reference mode includes zero lengths; native core remains basecalled-only |
| flowcell_overview (MinION only) | Faceted read start-time vs log length, colored by Q | Full-data MinION R9.5 physical overview | Exact points for small inputs; all-count 64×40 pixels/channel for dense inputs + extrema; explicitly not point-for-point ggplot |
| yield_over_time | Per-read cumulative gigabases vs time | Yield-over-time cumulative series | ONTRunQC 15-minute end bins, not original read-level path |
| yield_by_length | Yield retained above minimum read length, cropped display X | Exact retained-yield queries and graph | ONTRunQC shows broader range, exact integer underlying queries |
| length_by_hour | ggplot geom_smooth of per-read length vs start-time | Optional fitted length trends and unsmoothed observed means | LOESS verified; GAM original-R null-space difference persists (#12) |
| q_by_hour | ggplot geom_smooth of per-read Q vs start-time | Optional Q-score fitted trends and observed means | Same remaining GAM issue; do not substitute binned means for R raw fit |
| reads_per_hour | Counts in integer-hour bins; missing hours zero-filled | Start-time interval read-count chart | ONTRunQC 15-minute bins, different time support |
| length_vs_q | MinION scatter (events/base color); PromethION bin2d | MinION exact/full-count scatter, PromethION count heatmap | Natural-log positive events/base meaning preserved; dense colors and alpha differ |
| channel_summary | Four per-channel distribution facets | Native charts/tables plus reference geometry | Bases/mean/median match R; true-count correction intentionally changes count distribution |
| gb_per_channel_overview | Spatial channel yield tiles by All/Q | Explicit MinION/PromethION physical yield maps | Verified layouts; legend/colors/missing-vs-zero and optional native map differ |

Original single-run figures: [R single.flowcell, lines 423–723](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L423-L723). ONTRunQC charts are embedded in offline report.html, and requested independent figures in figures/ with manifest.json. Do not assume identical file basenames when cross-comparing.

| Original combined/per-source output | Reference meaning | ONTRunQC equivalent | Difference / status |
| --- | --- | --- | --- |
| combined_length_histogram | Pooled All/Q length distributions | Native bins or opt-in reference layers | Pooled reference boundaries/counts verified exactly |
| combined_q_histogram | Pooled All/Q Q distributions | Native bins or opt-in reference layers | Pooled reference summary-population bins verified exactly |
| combined_yield_by_length | Pooled minimum-length yield | Exact pooled retained-yield chart | Underlying counts/bases exact; original hardcodes .png filename in one path |
| length_distributions | Per-flowcell log-read-length density | Optional shared R-grid KDE comparisons | 52 total measured density curves within declared tolerance |
| q_distributions | Per-flowcell Q density | Optional R-grid Q density | Same R-grid evidence, distinct summary-zero-Q population handling |
| yield_over_time (multi) | Per-flowcell cumulative yield | Per-run comparison timeline | Different plotting intervals/labels, conserved totals |
| yield_by_length (multi) | Per-flowcell retained bases above length | Per-run yield-by-length comparisons | Exact thresholds/bases; display range differs |
| length_by_hour (multi) | Per-flowcell ggplot smoothed read length | Optional per-run fitted/observed trends | GAM remains partial as above |
| q_by_hour (multi) | Per-flowcell ggplot smoothed Q | Optional per-run fitted/observed trends | GAM remains partial as above |
| combined summary.yaml | All/Q combined statistics | Optional ONTRunQC summary.yaml plus JSON | Source labels, exact integer precision, denominators and known fixes differ |

Original combined figures: [R combined.flowcell](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L725-L805), [multi.plots](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L817-L891).

<a id="5-command-line-defaults-and-output-behavior"></a>

## CLI defaults and output behavior

| Original option | Original MinIONQC | ONTRunQC | Difference |
| --- | --- | --- | --- |
| -i, --input | Mandatory file/directory | Positional file/directory or repeated --input | ONTRunQC requires analyze subcommand |
| -o, --outputdirectory | Optional, can write next to input | --output normally required; --minionqc-compat offers fresh safe ontrunqc_qc directory | Avoids silently overwriting input files |
| -q, --qscore_cutoff | Default 7, numeric | Default 7, inclusive finite signed Q cutoff | Modern schema Q itself must remain valid/nonnegative |
| -p, --processors | Default 1; R mclapply across files | Bounded independent-component workers | Not fork-equivalent; grouping integrity prioritized over naive file dispatch |
| -s, --smallfigures | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Actual physical sizing is independently defined |
| -c, --combined-only | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Combined-only skips individual run outputs |
| -f, --format | PNG default, PNG/PDF/PS/JPEG/TIFF/BMP | HTML/JSON default, SVG and all six static formats on normal build | One requested independent format per execution; no bit/pixel identity |
| -m, --muxes | Reference grid; 0 uses historical single-run interval, combined default 8h | 0 disables normally; historical grid with --minionqc-compat | Generated markers are not measured MinKNOW events |
| -a, --add-stat | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Annotation style/placement differs |

### Example: single-run comparison

    Rscript MinIONQC.R -i sequencing_summary.txt -o minionqc_output -q 9 -f png

    ontrunqc analyze sequencing_summary.txt --output ontrunqc_output --qscore-cutoff 9 --format png --yaml --minionqc-trends

If the comparison includes multi-run R-grid density layers, add --minionqc-density to ONTRunQC. Neither option makes every ONTRunQC default numerically identical to R.

### Example: multiple runs, combined only

    Rscript MinIONQC.R -i input_directory -o minionqc_output -c TRUE -p 2 -f pdf

    ontrunqc analyze input_directory --output ontrunqc_output --combined-only --processors 2 --format pdf --yaml --minionqc-density --minionqc-trends

Use --minionqc-compat only if the historical mux marker/output semantics are needed and understand that safe output placement intentionally differs.

### File formats

ONTRunQC always writes report.json and report.html; --yaml additionally writes summary.yaml. --format svg/png/pdf/ps/jpeg/tiff/bmp creates independent scientific figures. PNG/TIFF/BMP are lossless 300-DPI RGB; JPEG is lossy quality 95; PDF preserves vector text/graphics where possible; **PS is raster-backed Level 3 at 300 DPI**, not vector-equivalent to ggplot output. Minimal --no-default-features builds explicitly omit the six static conversions. See [static figure definitions](output.md#native-static-figure-exports-15).

<a id="6-strict-numeric-gam-difference-measured-rather-than-waived"></a>

## Reference histogram and Bin2D validation (#19)

`--minionqc-plot-bins` is a separate opt-in presentation. It preserves native
bins and all canonical statistics, including basecalled-only Q statistics.
`--minionqc-compat` alone does not enable it. Definition/formulas and additional
pass accounting are in [metric definitions](metrics.md#optional-minionqc-histogram-layers-19).

The original script at `54485d586038d8ee36f168dd94f31c423f52f581` was executed
again under **R 4.4.1 / ggplot2 3.5.1 / mgcv 1.9-1**, using unchanged public
MinION, PromethION, short, 1D2, RNA and two-MinION/combined inputs and the same
cutoffs and the original default `--add-stat FALSE`. The optional audit captured 85 actual R plots; numerical acceptance
compares corresponding built bins/cells, never resized image pixels.

| Scope | Comparable panel/layer groups | Maximum coordinate error | Count L1 |
| --- | ---: | ---: | ---: |
| Public MinION | 10 | 0 | 0 |
| Public PromethION | 11 | 0 | 0 |
| Public short run | 10 | 0 | 0 |
| Public 1D2 | 10 | 0 | 0 |
| Public direct RNA | 10 | 0 | 0 |
| Two MinION sources and pooled histograms | 24 | 0 | 0 |

The **75 comparable groups** match exactly in this environment, including all
516 occupied PromethION cells / 75,160 reads. Every cell's six coordinates and
count are checked. Histograms check All/Q populations, every empty/occupied bin,
midpoints and both edges. Limits are shared across All/Q facets, with pooled
limits recomputed from pooled observations. Q reference histograms include zero
lengths without changing canonical basecalled-read quality statistics.

Another **14 original channel read-count panels intentionally differ**. For each,
independent input counts prove both native true-`n` membership and original
`n(n+1)/2` membership, including counts outside captured limits. Only a zero-error
proof permits that classification; unexplained membership failures still fail.
The bases, mean and median channel families match original R. Corrected-count
30-bin geometry also has separate actual-R synthetic goldens; the defective
original read-count distribution is not advertised as numerically identical.

Coordinate checks retain absolute `1e-12` plus relative `1e-13` tolerance;
counts and row counts require exact equality. Observed zero error is stronger
than that tolerance, not a reason to broaden it. Nine synthetic ggplot cases
exercise constant/singleton/empty groups, boundaries/ties, signed Q, zero-length
Q training and wide tails; four corrected channel families have 30-bin goldens.
Original R failures for empty/nonfinite/constant-axis plots are recorded, not
fabricated as successful reference figures. Native reports show explicit
unavailable states where the original cannot produce a finite layer.

Evidence: [machine-readable bin validation](../tests/reference/bin-validation-summary.json),
[synthetic layers](../tests/reference/bin-layers-golden.json), and the
[updated A/B/C gallery](figures/minionqc-comparison/README.md).
Thirteen individual/combined JSON reports match prior audit reports after
removing only the additive plotting field. This is distinct from the unchanged
18-case strict GAM evidence: **1,542 primary / 1,912 intermediate differences**
remain. No GAM/LOESS/KDE source, target or tolerance was changed.

Reproduce with the existing environment and clean pinned public checkout:

```sh
Rscript tests/reference/bin_layers.R tests/reference/bin-layers-golden.json
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript --reference-bins \
  --output benchmark-results/bin-audit --gallery benchmark-results/bin-gallery
```

Native `--add-stat` retains canonical basecalled annotations without retraining
bins. Original R text/line annotation layers can expand scale limits; numerical
equality under R `--add-stat TRUE` is outside this verified default-layer
contract. This is an explicit presentation-policy difference, not an accepted
zero residual or a change to any reference target.

Original R/native images use different palettes, typography and physical sizing.
B shows actual canonical Rust exports in explicit reference-bin mode; C shows
matched-bin/cell count residuals and boundary-error annotations. The corrected
channel-count panel instead shows separate distributions with the proven formula
difference. Bin agreement is **not** pixel-identical rendering. Native hourly,
cumulative presentation differences now have the explicit #20 contract below;
retained-yield display now has the measured #21 contract;
#8, #12 and the parent audit remain separate acceptance concerns.

## Reference temporal validation (#20)

Explicit `--minionqc-time-series` adds hourly summary counts and read-start
cumulative curves without replacing native 15-minute diagnostics. It does not
silently enable, or get enabled by, any other compatibility flag. Canonical
basecalled/native-pass statistics retain their original populations.

| Property | Contract and evidence |
| --- | --- |
| Hours | `floor(start_time/3600)`; zero fill 0..last All hour for both populations; exact integer counts including zero-length summary rows. |
| Selection | All retained summary records and inclusive Q >= cutoff; signed legacy Q retained; same existing RNA-control policy. |
| Cumulative | Exact integer cumulative bases at original read-start coordinates; stable file/record ordering; exact first/last time and final yield. |
| Multi-run | Independent run/source curves, All/Q panels with shared X range, consistent source colors and independent Y ranges; no fabricated pooled instrument timeline. |
| Small populations | Exact observations at <=8,192 points; original-R tied row ordering can still differ. |
| Dense populations | Every read contributes; <=8,194 vertices; error bound `ceil(total bases/4096)`; large jumps retain both consecutive endpoints. |
| Verification | Actual pinned R executes the unchanged script, evaluates every original timestamp and GeomLine one-sided limit, and compares exact hours/counts/terminals plus explicit display errors. |

The six public cases (MinION, PromethION, short, 1D2, RNA and two-MinION
collection) provide **14 exact hourly All/Q series and 18 cumulative context
comparisons**, with no unexplained temporal discrepancy. Some source curves
appear in both individual and combined contexts; these are not 18 independent
runs. Counts, terminal integer yields and endpoint coordinates have zero error.
GB-to-bp arithmetic alone permits 1e-5 bp roundoff; no counting/time or strict
GAM tolerance changes.

Representative All-summary results; Q-selected results and repeated combined
contexts are recorded individually in the machine-readable evidence:

| Public source | Observations | Display vertices | Native display bound (bp) | Maximum error vs original R polyline (bp) |
| --- | ---: | ---: | ---: | ---: |
| MinION RB7_A2 | 238,421 | 8,020 | 861,327 | 828,561.322 |
| PromethION | 75,160 | 7,572 | 152,148 | 115,323.454 |
| 1D2 | 159,799 | 8,050 | 227,112 | 159,758.350 |
| Short MinION | 99 | 99 | 0 | 28,536, tied-order difference |
| RNA | 963 | 963 | 0 | 76, tied-order difference |
| Second pooled MinION source | 369,787 | 8,038 | 897,293 | 815,020.110 |

**Ties are not hidden by a tolerance:** MinIONQC merges channels before time
sorting, then later reorders rows by length/hour. Built lines can traverse tied
cumulative values in a different, even backwards, vertical order. Native curves
use deterministic source/record order. Short/RNA post-tie cumulative envelopes
match exactly despite the original-path differences above. The maximum tied
source yield is recorded as a separate conservative order bound (short All:
39,005 bp; RNA All: 76 bp). Numerical evidence reports original polyline error,
post-tie error, simplification bound and tied-order bound separately; a simplified
curve is never reclassified as exact original-R point parity.

The comparison itself has a regression against collapsing vertical ties before
interpolation: that would incorrectly substitute a diagonal and can spuriously
exceed the simplification bound. Original plot data are never modified.

[Temporal validation summary](../tests/reference/temporal-validation-summary.json)
records input/source/capture hashes, versions and per-population results.
[Eight fictional R golden cases](../tests/reference/temporal-layers-golden.json)
cover boundaries 0/3599/3600/7199/7200 seconds, gaps, ties, signed Q, zero lengths,
singletons and a 2,000-hour run. Empty source/Q populations have explicit native
unavailable states; original R exits on an empty selected population.
[A/B/C figures](figures/minionqc-comparison/README.md) use original R figures,
actual canonical native exports and sampled numeric residuals; reported maxima
always evaluate the complete original layer, not those bounded display samples.

A guarded reread and private bounded-memory numeric external sort are required.
See [metric/pass/scratch definitions](metrics.md#optional-minionqc-temporal-plots-20)
and [resource measurements](benchmark.md). JSON schema 1 is extended additively;
13 previously generated individual/pooled reports retain all existing scientific
fields after removing only the new temporal field. Pooled public MinION remains
608,208 records, 7,203,301,321 bp, N50 33,161 bp, median 2,616 bp.

Reproduction commands and benchmark generation are in the
[reference guide](../tests/reference/README.md#reference-temporal-suite-20).
Histogram/Bin2D checks remain separately verified, and original GAM evidence
still records 1,542 primary / 1,912 intermediate discrepancies. The full 18-case
strict suite is not rerun or replaced by this temporal validation.


## Yield-by-length display (#21)

The 2026-10-22 execution used the unchanged pinned source and installed R 4.4.1,
ggplot2 3.5.1, mgcv 1.9-1. All six public cases produced 85 actual original
figure families: MinION, PromethION, short run, 1D2, RNA and two-MinION combined.
The extended capture evaluates every finite R length, not only diagnostic samples.
[Machine evidence](../tests/reference/yield-validation-summary.json) records source/input
hashes, actual R axis limits, clipping, population sizes and 20 All/Q source/context
comparisons. Six fictional [R goldens](../tests/reference/yield-layers-golden.json)
cover singleton, identical lengths, irregular ties, long tails, signed cutoffs
and the original zero-yield crop limitation.

### Three distinct mathematical representations

| Representation | Definition / ordering | Verification |
| --- | --- | --- |
| Original MinIONQC | Descending per-read cumulative sums, then GeomLine sorts X. For tied length L, ordinates run from F(L)−(n−1)L to F(L); hour merge may reorder internal ties. | Every visible R tied min/max equals independent full-population sums exactly, including longer reads censored from X. Original tied first/last errors are recorded separately. |
| Exact retained-yield function | F(x)=Σ l I(l≥x), an inclusive discontinuous step function. Zero lengths add no bases. | Legacy queries and all seven threshold count/bp fields unchanged; exact query errors remain 0. |
| New adaptive display | Retained whole-length jumps at yield-level crossings, both one-sided limits, interpolated omitted spans. Small inputs retain every step. Full range, no R crop. | At most 8,198 vertices/population; guaranteed bound ceil(population yield/4096) bp and measured inclusive/one-sided maxima. See [definitions](metrics.md#error-aware-retained-yield-display-21). |

The selected plotting population remains positive-length Q≥cutoff, independent of
native pass. R may also plot zero-length summary observations; their zero yield
does not change the envelope, but the full original observation count is recorded.
Empty/zero-yield native groups render safely. Original R has no finite strict
1%-yield crop when all yield is zero; the golden records that upstream limitation.

### Reproduced baseline and measured improvement

The old query-line interpolation errors below were reproduced **before editing**,
on identical SHA-256 input bytes. The legacy query array still reproduces these
historical numbers; it is retained for consumers but no longer used for drawing.
Comparison is at every observed inclusive length (same definition before/after).

| Public All population | Historical line error (bp) | Adaptive inclusive error (bp) | Adaptive one-sided error (bp) | Declared bound (bp) | Vertices |
| --- | ---: | ---: | ---: | ---: | ---: |
| minion | 1,555,278.312 | 392,578.280 | 392,578.280 | 861,327 | 8,196 |
| promethion | 5,464,673.009 | 128,976.723 | 128,976.723 | 152,148 | 8,192 |
| combined | 4,431,919.159 | 750,631.703 | 959,038.703 | 1,758,619 | 8,196 |

All 26 native All/Q populations in the 13 individual/pooled reports were checked
independently against complete length-frequency tables; measured maxima agree
with JSON. Removing only the additive display objects leaves all 13 complete
canonical JSON reports exactly equal to the #20 baseline. Pooled MinION remains
608,208 summary records, 7,203,301,321 bp, N50 33,161 bp, median 2,616 bp.

### R path, crop and intentional differences

The **20 actual-R envelope comparisons** have zero integer envelope/lower-limit
error, zero missing reference coordinates, exact terminal yield, and native
inclusive deviations within the declared bounds. The independent original crop
formula and clipped population counts also match exactly. Overall there are
441 numerical layer checks with 0 unexplained differences; this is not a claim
that 441 strict GAM comparisons passed. The earlier 421 bin/temporal/spatial
checks remain separate from the 20 new yield checks.

MinION original X max is 93,318 bp (339 All +338 Q points censored); PromethION
81,890 bp (61 All +55 Q). The native full tails extend to 208,408/351,506 bp
respectively, including their zero-yield max+1 endpoint. These are explicit
presentation choices, not omitted observations in statistics. Per-source R
plots use the maximum individual yield for the shared 1% crop, whereas the pooled
plot recomputes cumulative yield/crop from pooled lengths. Both contexts are
verified. R tied-path errors, crop counts and the display envelope error are
different quantities; none is silently reclassified as point-for-point parity.

No new compatibility flag was added: #21’s accepted bounded-display/error-disclosure
path improves the existing native figure while retaining its full-range semantics.
`--minionqc-compat` still controls only documented CLI behavior, and the histogram,
temporal, fitted-trend and density switches are unchanged. An original-R cropped
per-read rendering mode is **not implemented or claimed**. Native statistics,
thresholds, ultralong definitions and safe defaults remain stable.

The [A/B/C gallery](figures/minionqc-comparison/README.md) includes single, pooled
and per-source figures. C shows original R tied ordinates, the exact inclusive
upper envelope and native residuals, with the R crop marked. Only bounded samples
are drawn in C; error maxima use every finite original coordinate and all native
one-sided jumps. Gallery images do not establish pixel identity with ggplot.

### Resources, reproduction and remaining work

No additional source pass, decompression, sort, scratch or runtime dependency is
needed. The existing numeric partitions supply the two merge traversals. More
bounded vertices increase JSON/path size; the [before/after benchmark](benchmark.md#yield-by-length-display-21)
records that cost rather than claiming a performance optimization. Chrome file://
desktop and 390px inspections and canonical PNG/SVG exports were checked; wide
charts/tables retain intentional horizontal scroll containers.

```sh
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/pinned/Rscript --reference-bins --reference-time \
  --output benchmark-results/yield-audit --gallery benchmark-results/yield-gallery
Rscript tests/reference/yield_layers.R /path/to/pinned/MinIONQC.R \
  tests/reference/yield-layers-golden.json
```

The audit emits `yield-validation.json`; the committed summary additionally
includes the measured benchmark and exact canonical-preservation checks. The
original 18-case fitting suite was **not rerun** for #21. Protected GAM/LOESS/KDE
implementations, targets, tolerances and strict summary hashes are unchanged:
**1,542 primary GAM /1,912 intermediate differences remain**. Neither #8, #12
nor #18 is closed by this display contract. Next priority is #18’s final evidence
review, then an explicit maintainer decision on #8’s dependency on #12; further
#12 numerical work should remain independent.

## Strict numerical GAM limitation

Original-R comparison targets are still **absolute 1e-6 plus relative 1e-6** for fitted/uncertainty predictions. Recorded MinION Q≥7 original R vs native Rust maximum deviations:

| Compared value | Original pinned R vs ONTRunQC | Controlled R with canonical/swapped null directions vs ONTRunQC |
| --- | ---: | ---: |
| Length fitted mean | 1.715425619 bp | 2.155e-7 bp |
| Length standard error | 0.466714392 bp | 3.877e-10 bp |
| Length CI lower / upper | 2.630169018 / 0.800682220 bp | 2.163e-7 / 2.148e-7 bp |
| Q fitted mean | 8.974e-4 | 5.607e-12 |
| Q standard error | 2.880e-5 | 2.010e-14 |

The R reference consistently reproduces its original output in repeated runs on the pinned local installation. Tiny perturbations to knots can change sorting of two near-null eigen-directions in mgcv's cubic shrinkage spline; unequal 0.1 and 0.01 shrinkage weights then act on different directions. ONTRunQC uses a deterministic convention. Controlled R models with matching directions reproduce Rust predictions within the existing tolerance, but **the original reference penalty is genuinely different for the affected case**.

Do not call these original-R comparisons passing, globally relax tolerances, or relabel a controlled R fit as an unmodified MinIONQC reference run. Run and source hashes are in the [reference harness documentation](../tests/reference/README.md) and the [machine-readable evidence](../tests/reference/validation-summary.json). Cross-BLAS portability remains not verified.

<a id="12-verification-performed-for-this-audit"></a>

## Executed validation and limits

| Executed check | Observed result |
| --- | --- |
| `cargo fmt --check`, locked all-target Clippy with warnings denied, release build | Passed; no dependency/version change |
| Full Rust suite | 65 unit  + 103 integration tests passed; 3 optional tests ignored |
| Minimal build/suite (`--no-default-features`) | 59 unit  + 101 integration tests passed; 2 optional tests ignored; release build passed |
| Python reference-harness tests | 20 passed |
| Actual R bounded-layer tests | Histogram/bin2d/count conservation, nonfinite-grid conservation, bounded retention and tied cumulative queries passed |
| Six public visual cases | 85 actual original-R plots, all 20 context-qualified report families; 24 representative A/B/C cards |
| Independent plot checks | 307 checks: 268 exact, 32 different-aggregation, 7 intentional population differences; 0 unexplained differences |
| Strict pinned R differential suite | 18 completed cases ; 65,118 exact, 75,241 within original tolerance, 36 intentional semantic, 8,954 proven upstream-defect checks; **1,542 original GAM failures remain** (exit 1) |
| Revalidation signatures | Every strict check and intermediate-diagnostic signature reproduced the committed baseline; no target, tolerance or classification changed |
| Public pooled MinION CLI/probe | 608,208 records; 606,143 basecalled; 7,203,301,321 bp; N50 = 33,161 bp; median = 2,616 bp |
| Display correction regression | 13 public file-scoped/pooled JSON reports remain byte-identical across the negative-Q label fix; only HTML/figure presentation changed |

The six cases are public MinION RB7_A2, PromethION, short, 1D2, direct RNA and two
MinION sources/combined. Cutoffs are 7 except short/1D2 at 8. Source/input/capture
hashes and R session information are in the gallery measurements. The private
PBM95914 source was **not** reprocessed. PNG comparison cards were visually
inspected; no browser-driven layout test or new external PS/PDF decoder run is
claimed. Cross-BLAS GAM portability remains unverified. The optional audit's
Python/R helpers target these public validation fixtures, not 30-million-read
production processing; ordinary analysis retains its existing streaming engine.

<a id="7-evidence-reproduction-and-limitations"></a>

## Reproduce the evidence

### Routine and paired checks


Routine code verification:

    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked
    cargo build --release --locked
    cargo test --locked --no-default-features

Actual pinned R reference (requires installed pinned R environment and the external reference checkout):

    python3 tests/minionqc_reference.py /path/to/pinned/minion_qc --execute-r --rscript /path/to/Rscript --capture-only --output benchmark-results/current-r

A nonzero exit can be **expected and meaningful** when the strict GAM mismatches remain. To compare with historical evidence rather than falsely claim a pass:

    python3 tests/reference/check_revalidation.py benchmark-results/current-r/comparison.json

Further specialized tests: tests/minion_visualization.py for complete-data MinION pixel coverage; tests/static_exports.py for actual format decoding; tests/parallel_runs.py for deterministic combined results; tests/reference/gam_assessment.R and cs_sensitivity.R for controlled GAM diagnostics. See [benchmark documentation](benchmark.md) for verified environments and resource measurements.

**Limits not established by the cited checks:** exact ggplot pixels/palette/alpha, universal input/schema coverage, an independent second BLAS, live source mutation immunity on NFS, or full-run speed guarantees. Strict numeric status remains Partial. This audit adds live R plot-layer/rendering checks; it is not an HPC performance benchmark.

<a id="10-actual-plot-layer-and-figure-audit"></a>

### Visual layer capture and gallery

[Open the A/B/C comparison gallery](figures/minionqc-comparison/README.md).
[Machine-readable measurements](figures/minionqc-comparison/measurements.json)
record input/source hashes, cutoffs, R session versions, layer counts, transforms,
count conservation and numeric differences. The [discrepancy register](figures/minionqc-comparison/discrepancies.json)
separates confirmed defects, different aggregation, presentation and unresolved
verification. It is not a replacement for the strict fitting suite.

Reproduce with the existing pinned installation (no automatic R environment creation):

```sh
cargo build --release --locked
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript
Rscript tests/reference/plot_layers_test.R
python3 -m unittest discover -s tests/reference
```

The optional audit wraps the existing `ggsave`: it observes the **same**
`ggplot_build` result without changing data/stat calculations. StatBin, StatBin2d,
StatCount and spatial tiles are retained completely up to 20,000 rows (StatCount support is tested; no StatCount family occurs in the pinned script). Larger
point/line layers retain exact finite/nonfinite counts and complete 64×40 XY
count grids, plus ≤256 reproducible row samples, explicitly labelled samples.
Cumulative read-start curves also retain exact values at ≤4,096 quarter-hour
queries. Numeric pre-colour spatial yield is retained separately from RGB fill.
No millions-of-points JSON capture is committed.

A is the actual reference renderer at 60 DPI/smallfigures; B is the actual native
300 DPI renderer, resized and palette-reduced to 256 colours for the gallery only; originals are retained unchanged. Histograms show both All/Q native
figures; channel summaries include all four families. C uses actual layer/native
coordinates, exact count/grid diagnostics and matched-X residuals when defined.
**Unlike bins are not subtracted**; their boundary/count arrays remain in the
ignored audit report. Labels identify independent scales/populations. These
images establish coverage and expose differences; they do not establish pixel,
font, palette, alpha or automatic bin equivalence.

File-scoped validation uses the real parser, aggregator and renderer through the
existing ignored probe because the public inputs may contain multiple run IDs
that R discards. These scopes are explicitly labelled validation-only; normal
ONTRunQC run grouping is unchanged. The paired suite separately exercises the
production CLI on identical original input bytes.

`--reuse` reuses only source/input/hash-matching reports, then regenerates numeric
comparisons and cards. Full original/native reports and capture data stay under
ignored `benchmark-results/plot-audit/`; the gallery contains only bounded images
and aggregate measurements. Ordinary ONTRunQC analysis still needs no R,
ImageMagick, Python plotting libraries, browser or network access.

<a id="8-what-must-happen-next"></a>

## Remaining acceptance work

| Track | Required next step |
| --- | --- |
| [#19 automatic histogram/bin2d layers](https://github.com/PhoenixEmik/ONTRunQC/issues/19) | Verified opt-in layers under documented R settings: 75 exact groups, 14 proven count-formula exceptions; native defaults retained. |
| [#20 hourly and cumulative paths](https://github.com/PhoenixEmik/ONTRunQC/issues/20) | Implemented and numerically verified under the explicit zero-fill/bounded-display/tie contract above; original point-for-point rendering remains different. |
| [#21 length-yield display](https://github.com/PhoenixEmik/ONTRunQC/issues/21) | Adaptive full-range inclusive envelope implemented; exact thresholds preserved. Actual-R envelope, per-read/tie and clipping comparisons reported in the [#21 section](#yield-by-length-display-21); original cropped polyline is not reproduced. |
| [#12 original GAM](https://github.com/PhoenixEmik/ONTRunQC/issues/12) | Keep original failing targets/tolerances; no closure while the strict contract remains unmet. |
| [#8 combined acceptance](https://github.com/PhoenixEmik/ONTRunQC/issues/8) | Preserve its fitted-curve requirement or obtain explicit approval for an acceptance separation. |
| [#18 audit review](https://github.com/PhoenixEmik/ONTRunQC/issues/18) | Review the published source mapping, gallery and discrepancy evidence; #19 and #20 are verified within their stated contracts; #21 has explicit bounded-display evidence; original R per-read/crop presentation differences remain documented. |

Streaming gzip (#17) is implemented independently of these scientific parity
gates. The engineering ledger preserves historical decisions; this comparison
states which scientific, numerical, presentation and default behaviors differ.
