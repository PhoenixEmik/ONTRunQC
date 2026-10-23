<a id="minionqc-v141-vs-ontrunqc--detailed-compatibility-comparison"></a>

# MinIONQC scientific and visual comparison

Source-backed assessment against [MinIONQC v1.4.1](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R).
Functional coverage, populations/formulas, numerical layers, visual rendering
and CLI defaults are separate claims. Full strict numerical parity is not claimed.

| Provenance | Recorded baseline |
| --- | --- |
| Reference source | `54485d586038d8ee36f168dd94f31c423f52f581` |
| Pinned environment | R 4.4.1, ggplot2 3.5.1, mgcv 1.9-1; macOS ARM64 |
| Final acceptance audit | 2026-10-23, clean local/remote `main` at `3612a5542f83769db05c3730fd13ea0725f879fb`; package 0.4.1, Rust MSRV 1.85 |
| Historical source/figure audit | 2026-10-19, native commit `8f3d4d73a18a531ea2c3f728fb859cadd90ba801`; #19–#21 add scoped numerical evidence |
| Immutable numerical evidence | [Validation summary](../tests/reference/validation-summary.json), [revalidation signatures](../tests/reference/revalidation-summary.json) |
| Actual figure/layer evidence | [Gallery](figures/minionqc-comparison/README.md), [measurements](figures/minionqc-comparison/measurements.json), [discrepancies](figures/minionqc-comparison/discrepancies.json) |

[Documentation index](README.md) · [Metric reference](metrics.md) · [Engineering ledger](minionqc-parity.md) · [Reference commands](../tests/reference/README.md)

The historical audit is preserved. The #19–#21 sections record additional actual
R executions without changing original scientific targets or strict GAM evidence.

## Contents

- [Compatibility status and evidence vocabulary](#compatibility-status-and-evidence-vocabulary)
- [Five-dimensional compatibility contract](#five-dimensional-compatibility-contract)
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

All 20 original report/context families have an implementation and a source-backed mapping. **This is scoped functional coverage, not universal original-R numerical, pixel-rendering or output-default identity.** In particular:

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

## Five-dimensional compatibility contract

The authoritative machine matrix is `compatibility.features` in the existing
[discrepancy register](figures/minionqc-comparison/discrepancies.json). It extends
that register rather than introducing a duplicate matrix store. Each stable
feature ID names original formula/population/source lines, Rust symbol/line,
mode, tested context, evidence, intentional difference and remaining action.
The table below is generated from those 58 records.

**F** functional operation; **S** population/formula; **N** numerical outputs;
**V** visual meaning/rendering; **C** CLI/output contract. Each axis is independent.
**E** verified exact in the named scope; **T** within declared tolerance;
**B** verified bounded representation/conservation, not original point identity;
**D** functional with an explicit difference; **I** intentional correction;
**X** original strict numerical failure; **U** not verified; **—** not applicable.
An E under F says the operation exists and is verified; it does not certify its
other axes. No row is an unqualified “Complete”.

Evidence keys resolve through `compatibility.evidence_registry`: `strict` is
actual original-R comparison plus frozen signatures; `bins`, `temporal`, `yield`
are actual pinned plot-layer executions; `gam` is a **controlled diagnostic**;
`gallery` combines retained original-R rendering with visual inspection;
`public` is independent mathematics/retained upstream YAML; `commands` is the
fresh CLI-pair record; `rust` is synthetic Rust regression; `source` is source
inspection. Original-R numeric claims require original-R evidence, not merely
CI, a controlled model or image similarity.

Plot rows describe the explicit compatible flags recorded in JSON, with matching
source populations and original default annotation settings. Native fixed bins,
15-minute diagnostics and legacy KDE remain available. `--minionqc-compat`
only changes historical mux/output-placement settings; it does **not** enable
bins, time-series, trends or density flags automatically.

<!-- compatibility-matrix:start -->
| Feature / explicit comparison scope | F | S | N | V | C | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| `input.template` · Template / modern summaries | D | D | E | — | D | strict, rust |
| `input.2d` · Whole-file 2D / 1D2 substitution | E | E | E | D | D | strict, public, rust |
| `input.rna` · RNA named-control exclusion | D | D | D | — | D | strict, gallery, rust |
| `input.validation` · Malformed / missing / empty inputs | D | I | D | — | D | rust, source |
| `input.populations` · Summary / basecalled / native populations | D | D | D | D | D | strict, rust |
| `input.signed_q` · Signed legacy Q and cutoff | D | D | E | D | D | rust, bins, temporal |
| `input.discovery` · Recursive discovery and deduplication | D | D | E | — | D | public, rust, source |
| `input.gzip` · Explicit gzip and gzip discovery | D | E | E | — | D | commands, rust |
| `input.metadata` · Missing metadata and explicit device selection | D | D | D | D | D | rust, source |
| `input.identity` · Run IDs, flowcells and chunks | D | D | D | D | D | public, rust |
| `stats.yield` · Read counts and total bases | E | D | E | D | D | strict, public, rust |
| `stats.length` · Mean / median / maximum length | E | D | D | D | D | strict, public, rust |
| `stats.quality` · Arithmetic mean / median Q | E | D | D | D | D | strict, public, rust |
| `stats.n50` · N50 half boundary | E | I | I | D | D | strict, public, rust |
| `stats.cutoff` · Independent inclusive Q cutoff | E | D | E | D | D | strict, rust |
| `stats.thresholds` · Seven inclusive length thresholds | E | E | E | D | D | strict, public, yield, rust |
| `stats.ultralong` · Maximal ultralong prefix | E | I | I | D | D | strict, public, yield, rust |
| `stats.channels` · Channel count / yield / mean / median | E | I | I | D | D | strict, bins, rust |
| `runs.pooled` · True pooled summaries | E | D | D | D | D | strict, public, commands, rust |
| `runs.combined_only` · Combined-only outputs | E | E | E | D | D | commands, rust |
| `runs.workers` · Bounded independent processing | D | D | E | — | D | rust, commands |
| `plot.single.length_histogram` · single: length_histogram | E | E | E | D | D | bins |
| `plot.single.q_histogram` · single: q_histogram | E | E | E | D | D | bins |
| `plot.single.flowcell_overview` · single: flowcell_overview | E | D | B | B | D | gallery, rust |
| `plot.single.yield_over_time` · single: yield_over_time | E | D | B | B | D | temporal |
| `plot.single.yield_by_length` · single: yield_by_length | E | D | B | B | D | yield |
| `plot.single.length_by_hour` · single: length_by_hour | E | D | X | D | D | strict, gam, gallery |
| `plot.single.q_by_hour` · single: q_by_hour | E | D | X | D | D | strict, gam, gallery |
| `plot.single.reads_per_hour` · single: reads_per_hour | E | E | E | D | D | temporal |
| `plot.single.length_vs_q` · single: MinION length_vs_q | E | D | B | B | D | gallery, rust |
| `plot.single.channel_summary` · single: channel_summary | E | I | I | D | D | bins |
| `plot.single.gb_per_channel_overview` · single: gb_per_channel_overview | E | E | E | D | D | gallery, rust |
| `plot.pooled.combined_length_histogram` · pooled: combined_length_histogram | E | E | E | D | D | bins |
| `plot.pooled.combined_q_histogram` · pooled: combined_q_histogram | E | E | E | D | D | bins |
| `plot.pooled.combined_yield_by_length` · pooled: combined_yield_by_length | E | D | B | B | D | yield |
| `plot.multi.length_distributions` · multi: length_distributions | E | E | T | D | D | strict, gallery |
| `plot.multi.q_distributions` · multi: q_distributions | E | E | T | D | D | strict, gallery |
| `plot.multi.yield_over_time` · multi: yield_over_time | E | D | B | B | D | temporal |
| `plot.multi.yield_by_length` · multi: yield_by_length | E | D | B | B | D | yield |
| `plot.multi.length_by_hour` · multi: length_by_hour | E | D | X | D | D | strict, gam, gallery |
| `plot.multi.q_by_hour` · multi: q_by_hour | E | D | X | D | D | strict, gam, gallery |
| `plot.single.promethion_bin2d` · single: PromethION length_vs_q | E | E | E | D | D | bins, gallery |
| `cli.input` · CLI --input | D | — | — | — | D | source, commands, rust |
| `cli.output` · CLI --output | D | — | — | — | D | source, commands, rust |
| `cli.qscore_cutoff` · CLI --qscore_cutoff | E | E | E | — | D | source, commands, rust |
| `cli.processors` · CLI --processors | D | — | E | — | D | source, commands, rust |
| `cli.smallfigures` · CLI --smallfigures | E | — | — | D | D | source, commands, rust |
| `cli.combined-only` · CLI --combined-only | E | E | E | D | D | source, commands, rust |
| `cli.format` · CLI --format | E | — | — | D | D | source, commands, rust |
| `cli.muxes` · CLI --muxes | D | — | D | D | D | source, commands, rust |
| `cli.add-stat` · CLI --add-stat | E | D | D | D | D | source, commands, rust |
| `output.yaml` · YAML summaries | E | D | D | D | D | strict, public, commands, rust |
| `output.static` · Six real static formats / SVG | E | — | — | D | D | rust, source, commands |
| `output.publication` · Publication, names and safe defaults | D | — | — | D | D | rust, source |
| `native.extensions` · Native pass / mux / speed / termination / activity / HTML+JSON | — | — | — | — | — | rust |
| `limit.cross_arch` · Native cross-architecture bitwise GAM identity | — | — | U | U | — | source, gam, rust |
| `limit.nfs` · Remote NFS writer/cache guarantees | — | — | U | U | — | source, gam, rust |
| `limit.degenerate` · Arbitrary degenerate GAM parity | — | — | U | U | — | source, gam, rust |
<!-- compatibility-matrix:end -->

### Source scope is part of the contract

The gallery B panels use the production parser, metrics and renderer through
`tests/reference/probe.rs`, deliberately matching R's **file-scoped** population.
They do not exercise production run routing. Normal CLI routing retains known
run IDs: the public two-MinION files produce **eight native run groups**, compared
with R's two parent-directory/file labels; public PromethION produces eleven
native groups, and 1D2 four. Pooled totals agree. Per-source fits/densities have
matching numerical targets only after explicitly matching those populations.
No public CLI option is claimed to force arbitrary file grouping across
conflicting known run IDs; `--group-inputs` rejects that conflict. A source label
is never asserted to identify a distinct physical flowcell.

### Completion boundary

The audit accepts mapped functionality, verified scoped formulas/layers and
documented limits. It does not waive original strict targets. Known N50-half,
ultralong-endpoint and channel-count defects remain corrections; RNA unknown
labels, safer validation, run grouping and CLI defaults remain explicit semantic
differences. Bounded temporal, length-yield and MinION raster views retain their
defined error/resolution limits. Arbitrary degenerate GAMs, native cross-platform
bitwise fitting and actual NFS/cache behavior remain unverified.

No additional compatibility implementation sprint is justified by the completed
evidence. A separate product-hardening phase can address concrete operational
requirements after maintainer review; it must not imply #8/#12 strict acceptance.

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
| Input | readr TSV loader, single file or recursive `sequencing_summary.txt` regex discovery; pinned readr also decodes gzip | Plain or streaming gzip, unquoted TSV; explicit files or recursive matching sequencing_summary*.txt/.tsv and .gz variants | Fresh original-R gzip/public-short execution verified decoding; native CRC/trailer/source-integrity guarantees and suffix/deduplication policies remain separately tested |
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

This audit reads the **entire 952-line pinned script**, not only example images. Earlier prose reported 956 lines; the pinned source hash is unchanged, and current source spans are checked against the actual file.
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
| Read selected columns, convert numeric; drop `readr` problem rows | `load_summary`, 205–225 | `input.rs::Columns::{new,parse}`, `SummaryReader::next_record` | Parser malformed/missing-column integration tests. Rust rejects malformed records; no silent dropping. |
| Exclude calibration label exactly `YHR174W`; R `subset(label != control)` also drops missing labels | 233–236 | `input.rs::Columns::parse`, `metrics.rs::Aggregator::add` | `direct_rna_controls_are_excluded_and_counted_separately`. Actual synthetic R capture: missing/NA/ordinary/control lengths 10/20/30/40 retain 1 read/30 bp in R, 3 reads/60 bp here. `unknown_rna_calibration_labels_are_retained_instead_of_assumed_controls` protects this intentional distinction. |
| If 2D length column exists, substitute 2D length/Q for the **whole file**, events unavailable | 238–250 | `input.rs::Columns::new/parse` | `legacy_2d_substitution_is_consistent_and_never_falls_back_per_row`; incomplete pairs fail rather than silently mix representations. |
| All = `q >= -Inf`; selected = `q >= cutoff`; zero lengths retained by R | `add_cols`, 159–195; 254–276 | `metrics.rs::Aggregator::{set_qscore_cutoff,add}` | `q_cutoff_is_inclusive_exact_and_independent_of_native_filtering`, `signed_cutoffs_select_legacy_reads_without_weakening_modern_validation`. Rust core/subset length/Q stats use B / B∩Q. |
| Device = max(channel) ≤512; merge with physical map | 138–181 | `layout.rs::FlowcellLayout::coordinate`, explicit CLI layout | All512/3000 coordinate tests; `layout_is_opt_in_and_rejects_every_incompatible_record`. Explicit choice avoids false instrument inference. |
| Reads = nrow(S); bases = Σl; Gb = Σl/10⁹ | `summary.stats`, 347–405 | `metrics.rs::Aggregator::add/finish_retaining`, `yaml_summary` | `complete_synthetic_metrics`, public paired YAML; integer bp canonical. Distinct summary/B counts. |
| Mean length = Σl/|S|, mean Q = Σq/|S|; median = middle value or mean of two middle values | 357–362 | `subset_statistics`, `partition_median_length/quality` | `exact_partitions_match_an_independently_sorted_reference_for_every_split`. Rust B denominator and unrounded JSON; R rounds YAML to one decimal. |
| N50 = first descending length with cumulative bases **>** total/2 | 356 | `n50_descending` | `q_cutoff_even_and_odd_exact_statistics_preserve_n50_half_convention`. Rust **≥** half is intentional; e.g. [100,50,25,25]: R50, Rust100. |
| Threshold count = ΣI(l≥T), bases = Σl I(l≥T), T=10/20/50/100/200/500kb/1Mb | `reads.gt/bases.gt`, 279–291, 371–390 | `yield_by_length` | `exact_yield_thresholds_curve_and_ultralong_are_consistent`; R labels “>” are inaccurate at exact boundaries. |
| Ultralong = longest descending prefix whose strict-half N50 ≥100kb | `binSearch`, 323–344; 364–369 | `ultralong` | Same prefix question; R final-index `max-1` bug drops an eligible endpoint. Synthetic singleton/tie/end tests prove correction. |
| Channel bases = Σl; mean/median per channel; **R count = Σwhich(l≥0) = n(n+1)/2** | `channel.summary`, 406–420 | `finish_retaining`, `channel_distributions` | `exact_channel_medians_include_explicit_summary_and_basecalled_denominators`. Correct true count, All-summary means/medians available separately from B. |
| Hour = floor(t/3600); counts by hour; explicitly zero-fill 0…max(hour) | 188–194; 609–638 | `temporal.rs::Builder`, `html.rs::reference_temporal`; native 900s diagnostics separate | `reference_temporal_preserves_canonical_statistics_and_r_populations`; actual R hourly pairs match exactly in explicit time mode. Fifteen-minute counts remain a distinct view. |
| Chronological cumulative yield = cumsum(l sorted by t), including each read's start | 183–184; 564–572 | `temporal.rs::Builder::finish`, `html.rs::reference_temporal`; native bin-end charts retained | Explicit time mode preserves start coordinates and terminal yield, with measured simplification/tie differences; see #20. Native 15-minute paths remain distinct. |
| Length yield curve = descending cumulative bases at **each read**, then ggplot sorts X; ties yield vertical segments | 186–187; 575–584 | `yield_by_length`, `html.rs::yield_length_plot/comparison_yield_length` | `every_display_query_uses_exact_lengths_including_ties`: legacy exact queries unchanged. New adaptive step envelope has a measured one-sided error contract; actual R tied min/max checked independently. R crop/per-read ordering remain distinct; see [#21 evidence](#yield-by-length-display-21). |
| Length histogram: log10(l) **before** StatBin, bins=300; Q histogram: raw q, bins=300 | 472–545; 770–790 | `plot_bins.rs::Grid/Plan`, `html.rs::reference_histogram`; native fixed bins separate | `bin-layers-golden.json` and #19 actual-R coordinate/count comparisons; optional compatible layers match, native fixed intervals intentionally differ. |
| Channel histograms: StatBin(bins=30), automatic centred breaks per variable shared across All/Q | 670–686 | `plot_bins.rs::channels`; native `channel_distributions` separate | Reference-mode bases/mean/median histograms match. True counts and their derived boundaries intentionally differ from the R bug; native categorical geometry is retained. |
| Smooth raw reads, not bin means; ggplot chooses LOESS below 1,000 in largest group, otherwise GAM `cs`, REML | 587–606; 871–891 | `trends.rs::Tracker/Plan`; `html.rs::fitted_trend_plot` | Actual 18-case differential, LOESS/golden tests. Single Q limits remove negative Q before fitting; combined Q does not. Default fit opt-in; #12 remains Partial. |
| KDE = stat density after scale transform; Gaussian bw.nrd0, shared panel domain, n512 | 828–845 | `density.rs::estimate`, `comparison_distribution` | `density-golden.json`, 52 actual R curves. `--minionqc-density` required; default legacy KDE differs in normalization/grid. |
| MinION points: All, x=t/3600/y=log10(l)/colour=q, α.35; length-vs-Q x=log10(l)/y=q/colour=log(events/l), α.05 | 548–561; 640–668 | `minion.rs::Tracker/Renderer`; `html.rs::minion_read_plots` | `tests/minion_visualization.py`, complete finite-row audit grids. Rust exact≤4096; otherwise full-count rasters plus extrema. Mapped color averaging/opacity/point order differ explicitly. |
| PromethION length-vs-Q = All, StatBin2d default 30 bins after log10 length | 650–665 | `plot_bins.rs::Plan`, `html.rs::reference_bin2d`; native joint histogram separate | All 516 occupied cells and 75,160 public records match original geometry/counts in reference mode. Native passed heatmap is an addition, not a Q-cutoff panel. |
| Spatial tiles = channel bases/10⁹, mapped row/col, reversed Y, All/Q facets | 689–720 | `physical_heatmaps`, `layout.rs` | Actual pre-colour tile values, coordinates and yield comparisons; missing channels explicit, palette/panel styling different. |
| Combined summary = pooled observations, recompute descending cumulative sum and exact summaries | 725–805 | `batch.rs::analyze`, `Aggregator::absorb/finish_retaining` | `multiple_runs_chunks_and_duplicate_paths_have_exact_pooled_statistics`; no averaging N50s/medians. File-scoped R vs reliable run-ID grouping remain distinct. |
| Per-flowcell comparisons keep absolute yield and per-source time origin, All/Q facets | `multi.plots`, 817–891 | `html.rs::combined_comparisons`, per-source `RunComparison` | Public combined capture plus exact pooling tests; same colours per source, eight-colour palette repeats. Numeric fit differences independently retained. |
| File/directory dispatch; per-parent file identity; mclapply workers; summary/plot writes | 898–951; options 31–100 | `batch.rs::{discover,scan_inputs,independent_components,scan_components}`, `cli.rs`, `report.rs::write` | `parallel_groups_preserve_interleaved_chunks_legacy_empty_and_exact_outputs`, worker overlap/failure tests. Bounded safe components and staged files differ intentionally from forked per-file processing. |

Source links: [entire pinned R](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R),
[Rust parser](../src/input.rs), [metrics](../src/metrics.rs), [HTML](../src/html.rs),
[batch](../src/batch.rs), [integration tests](../tests/integration.rs).
Read these with the CLI/default matrix above; plot exports, physical dimensions,
PNG/PDF/PS/JPEG/TIFF/BMP and YAML are projections of the same canonical model.
No original MinIONQC equivalent exists for native pass/mux/speed/end-reason/observed
channel-activity metrics: these are **not applicable** to its parity contract.

<a id="3-scientific-statistics-values-definitions-edge-cases"></a>

Original metrics source: [summary.stats / channel.summary](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L347-L420). Detailed equations, zero-length and N50 examples: [ONTRunQC metrics](metrics.md).

**Public pooled multi-run reference:** 608,208 summary records; 606,143 positive-length basecalled reads; 7,203,301,321 bases; N50 = 33,161 bp; median = 2,616 bp. Fresh production CLI/public-reference checks and original R execution reconfirmed these pooled values during the sign-off. Production routing yields eight run groups; the file-scoped probe matches two original R source populations. The private PBM95914 sequencing summary was not rerun.

<a id="4-one-to-one-scientific-plot-inventory"></a>

## Plot-family inventory

**Important:** a corresponding plot/report family is not a claim of identical pixel layouts or ggplot-generated bin boundaries. All/selected Q populations, native filtering and zero-length eligibility may require different axes/labels.

| Original MinIONQC output (single-run) | Original layer/semantics | ONTRunQC equivalent | Difference / verification |
| --- | --- | --- | --- |
| length_histogram | Two All/Q panels, 300 ggplot bins after length transform | Native fixed bins plus explicit reference layers | Reference-mode boundaries/counts verified; palettes/layout differ |
| q_histogram | Mean-read-Q distribution by All/Q | Native basecalled bins plus reference summary-population bins | Reference mode includes zero lengths; native core remains basecalled-only |
| flowcell_overview (MinION only) | Faceted read start-time vs log length, colored by Q | Full-data MinION R9.5 physical overview | Exact points for small inputs; all-count 64×40 pixels/channel for dense inputs + extrema; explicitly not point-for-point ggplot |
| yield_over_time | Per-read cumulative gigabases vs time | Native end-bin chart plus explicit read-start series | Exact totals/coordinates and measured bounded/tied-order differences in reference-time mode |
| yield_by_length | Yield retained above minimum read length, cropped display X | Exact retained-yield queries and graph | ONTRunQC shows broader range, exact integer underlying queries |
| length_by_hour | ggplot geom_smooth of per-read length vs start-time | Optional fitted length trends and unsmoothed observed means | LOESS verified; GAM original-R null-space difference persists (#12) |
| q_by_hour | ggplot geom_smooth of per-read Q vs start-time | Optional Q-score fitted trends and observed means | Same remaining GAM issue; do not substitute binned means for R raw fit |
| reads_per_hour | Counts in integer-hour bins; missing hours zero-filled | Native interval chart plus explicit integer-hour series | Exact summary-hour All/Q counts in reference-time mode; native 15-minute view remains separate |
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
| yield_over_time (multi) | Per-flowcell cumulative yield | Native per-run timeline and optional read-start comparison | Exact terminals and measured bounded/tie differences; same source scope required |
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

Do not call these original-R comparisons passing, globally relax tolerances, or relabel a controlled R fit as an unmodified MinIONQC reference run. Original run/source hashes remain in the [reference harness documentation](../tests/reference/README.md) and [unchanged baseline](../tests/reference/validation-summary.json). The independent-backend investigation below adds evidence without replacing that baseline.

### Final numerical assessment — 2026-10-23

**Strict parity failed; #12 remains Partial/open. No production solver correction
is justified by the completed experiments.** Starting at `49fcc45906f3`, the
OpenBLAS 18-case rerun reproduces every primary, GAM-intermediate and LOESS
comparison signature. Counts remain **65,118 exact, 75,241 within tolerance,
36 intentional differences, 8,954 upstream-defect checks, 1,542 primary GAM
failures and zero unverified checks**. The failed fields remain 390 predictions,
381 SEs, 381 lower and 390 upper bounds; 1,912 intermediate failures are separate.
Both actual differential runs returned **exit 1**, not a parity pass.

The [bounded machine-readable assessment](../tests/reference/gam-final-assessment.json)
records source/capture/binary hashes, sessions, strict per-case maxima, controlled
models and perturbations. Full matrices and original artifacts remain separately
under ignored `benchmark-results/gam-final-20261023/`.

| Mathematical stage | Evidence / first divergence |
| --- | --- |
| Knots / natural-cubic design | Unique-time type-7 knots and second-derivative construction retained. `trend_knots.rs` and production `trends.rs` are unchanged. Comparisons use identical observations and explicit knots. |
| Raw penalty / eigenspace | Nine OpenBLAS/Netlib models have bit-identical raw penalties and numerical null rank 2. Compare the two-dimensional **projector**, not eigenvector signs. In the repeated-time example, projector difference is 2.182e-14. |
| Shrinkage penalty | mgcv's `cr` constructor with `shrink=.1` assigns the last two numerically ordered directions .1/.01 times the smallest positive eigenvalue. Unequal weights are not invariant to a null-space rotation or swap. This is the first substantive divergence. |
| Centering / normalization | Compare effective penalties as `T⁻ᵀ S T⁻¹` in common nodal coordinates, accounting for mgcv `S.scale` and the largest raw eigenvalue. Canonical R/native differences are at most 1.055e-15 across the six public selected models. Arbitrary centered matrix cells are not comparison targets. |
| REML / coefficients | Twenty controlled models per backend use original, swapped, canonical, identical-native-penalty and fixed-lambda variants. Aligning penalties makes all curve, nodal-coefficient and nodal-covariance checks pass the original tolerance; fixing lambda reveals no additional over-tolerance discrepancy. |
| Prediction / uncertainty | Compare uncensored predictions and Bayesian conditional covariance before plot censoring. SE and pointwise normal 95% bounds agree after alignment. This does not assert identical regularization or uncertainty for the unmodified R model. |

The one-ULP public MinION experiment reproduces a **1.7763568394e-15 h** knot
change, **1.5543122345e-15** raw-penalty change and **1.7154258345 bp** endpoint
change. The null subspace is stable; assigning different nonzero weights within
it changes the model. The 1,542 failures are two selected MinION curves repeated
in three contexts, rather than 1,542 independent defects. Swapping only the
affected controlled R null weights explains their fit/SE/bound discrepancies;
globally swapping Rust directions would damage already agreeing cases.

#### Independent numerical backends

R 4.4.1, ggplot2 3.5.1 and mgcv 1.9-1 were unchanged on macOS 27.0.1 arm64.
Actual loaded images **and** `dgemm_`/`dsyevr_` symbol providers verify:

| Backend | Verified binary / result |
| --- | --- |
| OpenBLAS 0.3.34, VORTEX/OpenMP; LAPACK 3.12.0 | Library SHA-256 begins `7a89772da7fb`; original 18-case baseline reproduced exactly. ARMV8/VORTEX aliases are not separate backends. |
| Netlib BLAS/LAPACK 3.12.0, conda-forge `2_h41cbde3_netlib` | Separate BLAS/LAPACK hashes begin `9ec863adc1c9` / `2aa386c1e320`. All 18 cases executed; same 1,542 primary failure locations, 64,894 exact and 75,465 tolerant checks. Its 1,642 intermediate failures are experimental results, never replacements for the original 1,912. |

With **identical raw matrices, knots and observations**, the repeated-timestamp
synthetic model switches geometric null weights from (.01,.1) to (.1,.01).
Original R cross-backend maximum differences are **0.308657025 bp fitted**,
**0.017085655 bp SE**, and **0.275438176 / 0.341875873 bp lower/upper bounds**.
Normalized shrinkage penalties differ by 3.7574e-5. Supplying identical original
penalty bytes reduces fitted differences to **5.0023e-12 bp**; explicit canonical
directions reduce them to **6.8213e-12 bp**. The public selected models do not show
this large cross-backend change (maximum fitted difference 1.427e-9 bp).
Thus backend dependence is measured on an independent input, not assumed for
every public dataset.

Each backend repeats nine models and 228 perturbations identically in fresh
processes. The sweep covers ±1/4/1,024 ULP knot steps, irregular/repeated times,
matrix rescaling and symmetric roundoff perturbations. Rust regression tests
cover null projectors, canonical adjacent-knot continuity, reversed accumulation,
and REML derivatives/descent. Central finite differences (step 1e-4) have maximum
errors divided by `1+|analytic|` of **1.766e-8 gradient / 1.667e-9 Hessian**.
These derivative checks do not change the strict reference tolerance.

**Compatibility contract:** retain exact QC semantics and the original strict
targets; label native GAMs by their deterministic geometric null convention.
They are reproducible scientific displays, not the same weighted penalty as
every original mgcv execution. A single portable convention cannot exactly
reproduce the two measured original penalties simultaneously. This does not
prove all future discrepancies explained, or Rust bitwise identity across
architectures: only one host/architecture was executed; CPU model inspection
was denied by the sandbox, and complete expanded backend compiler flags were
unavailable. Package recipes, compiler versions and full sessions are retained.
#8 acceptance remains separately blocked. The final #18 audit gate below may close the documentation/evidence task without changing #8/#12 contracts.

<a id="12-verification-performed-for-this-audit"></a>

## Executed validation and limits

The dated [sign-off record](../tests/reference/signoff-summary.json) separates
fresh executions, retained evidence checks and unavailable properties. The audited
production baseline is `3612a5542f83`; this sign-off changes no Rust source,
lockfile, original target or numerical tolerance.

| Executed check (2026-10-23) | Observed result |
| --- | --- |
| Formatting, locked all-target Clippy, default/minimal release builds | Passed; no dependency/version change. |
| Full Rust suite | 79 unit + 116 integration tests passed; 3 optional tests ignored. |
| Minimal Rust suite | 73 unit + 114 integration tests passed; 2 optional tests ignored. |
| Python evidence/harness guards | 39 passed, including matrix/source coverage and diagnostic ordinate regressions. |
| Actual R bounded-layer tests | Histogram/Bin2D/count conservation, nonfinite handling, bounded retention and tied cumulative queries passed. |
| Retained six-case gallery | All 85 original/native figures and reports resolve; 44 representative A/B/C cards cover all 20 original context-qualified report families. All cards visually inspected. |
| Retained plot checks | 441 checks: classifications and source hashes retained in gallery measurements; no new original captures substituted. |
| **Fresh original strict differential** | 18 actual R cases completed; **exit 1, strict parity failed**: 65,118 exact, 75,241 within original tolerance, 36 intentional semantic, 8,954 proven upstream-defect checks, **1,542 GAM failures**, zero unverified. |
| Strict regression guard | Every primary/GAM/LOESS check signature reproduced; 1,912 separate GAM intermediate differences remain. Guard pass is not strict parity pass. |
| Fresh public production CLI checks | Short, PromethION, 1D2, RNA and pooled two-MinION inputs passed independent exact-statistic/YAML checks. Pooled: 608,208 records, 7,203,301,321 bp, N50 33,161 bp, median 2,616 bp. |
| Fresh original-R/native command pairs | Single PNG, gzip input, and two-worker combined-only PDF/SVG pairs all exit 0; 20 original figures rendered. Native short plain/gzip JSON is byte-identical. Raw integer yields match; R YAML rounding differs by 33/21 bp and is reported separately. |
| Independent-backend provenance | Retained OpenBLAS/Netlib matrices, actual binary hashes and protected targets rechecked; backend experiments were **not** rerun for this sign-off. |
| Rust 1.85 and static Linux exports | Baseline [CI 39885674043](https://github.com/PhoenixEmik/ONTRunQC/actions/runs/39885674043) passed MSRV, default/minimal tests and static-musl export checks on unchanged production sources/lockfile; not a fresh local MSRV run. |

The six gallery cases are public MinION RB7_A2, PromethION, short, 1D2, direct
RNA and two MinION sources/combined. Cutoffs are 7 except short/1D2 at 8.
Gallery B uses the explicitly file-scoped validation probe; the separate fresh
production CLI check uses actual run-ID routing. Source/input/capture hashes and
R sessions remain linked from the measurements.

**Diagnostic corrections, not QC changes:** two density C panels previously
compared R's `count = density × population` with native density. They now compare
actual plotted `y` density values: maximum residuals **2.6534e-14 per log10(bp)**
and **2.1649e-15 per Q unit**. Two singleton hourly C panels now draw a point
instead of an invisible one-point line. Original A/B image panels and R captures
are unchanged; old/new card hashes and the invalid prior diagnostic values are
retained in `diagnostic_corrections` and finding A17. No KDE implementation or
strict target changed.

No private PBM95914 input was reprocessed, browser layout test performed, or new
external PS/PDF decoder execution claimed. Local checks used Rust 1.98.1 on
arm64 macOS; native cross-architecture bitwise GAM identity, arbitrary degenerate
fits and live NFS cache/writer races remain unverified. Public audit helpers are
validation tools, not a 30-million-read performance benchmark.

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

**Limits not established by the cited checks:** exact ggplot pixels/palette/alpha, universal input/schema coverage, native cross-platform bitwise GAM identity, live source mutation immunity on NFS, or full-run speed guarantees. Strict numeric status remains Partial. This audit adds live R plot-layer/rendering checks; it is not an HPC performance benchmark.

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
  --rscript /path/to/Rscript --reference-bins --reference-time
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
images establish coverage and expose differences; exact bin/cell agreement comes from numeric layer checks, not pixels, fonts, palettes or alpha.

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

<a id="remaining-acceptance-work"></a>

## Final acceptance review

The audit's completion boundary is an evidence-backed compatibility assessment,
not universal original-R numerical or pixel identity. The five-dimensional
register is authoritative; historical `Complete` labels in the engineering ledger
refer only to their named scope.

| #18 original audit criterion | Evidence / decision |
| --- | --- |
| Complete source and formula mapping | Full unchanged 952-line script reviewed; 58 matrix rows map source operations, populations, Rust symbols and tests. Optional guard parses all original report outputs and all nine CLI options. Satisfied. |
| All relevant report families | 20 original context-qualified families, including device branches, pooled and per-source contexts; legacy/short/RNA cases and explicitly limited raster/curve representations. Satisfied. |
| Actual original-R numeric evidence | Fresh 18-case execution plus retained bin, temporal, length-yield and KDE captures, immutable hashes and strict failure signatures. Satisfied; strict GAM parity itself **failed**. |
| Actual reproducible comparative figures | 85 original/native figures retained, 44 representative cards reviewed; four diagnostic panels corrected with original A/B and capture provenance preserved. All families covered. Satisfied. |
| CLI/output matrix and discrepancy register | Independent classifications, nine original options, six static formats, safe defaults, source routing and intentional corrections documented. Satisfied. |
| Scripts, provenance and honest limitations | Existing reproducers plus focused `signoff.py` guards, dated machine-readable execution record, tests and links. No unsupported property relabeled as verified. Satisfied. |
| Consistent researcher-facing claims | README links to this current contract; engineering history remains dated; original strict failures are explicit. Satisfied. |
| Closure decision | **#18's audit criteria are satisfied; recommend closing #18 with this evidence.** This does not close #8/#12, approve different requirements, or claim full numerical parity. |

| Issue | Verified scope | Remaining acceptance / recommendation |
| --- | --- | --- |
| [#8 multi-flowcell](https://github.com/PhoenixEmik/ONTRunQC/issues/8) | Run/chunk grouping, deduplication, exact pooled populations/statistics, combined-only, all pooled/per-source families, deterministic workers and offline outputs. File-scope versus run-scope comparisons are explicitly distinguished. | **Option A (current): keep open** because original fitted-curve numerical expectations still depend on #12. **Option B (proposal only):** accept independently verified functional scope and retain strict fitted-curve requirements under #12. Requires explicit maintainer authorization; none is inferred. |
| [#12 GAM](https://github.com/PhoenixEmik/ONTRunQC/issues/12) | Investigation complete within the measured scope; two independent backends, aligned-penalty controls and deterministic native convention explain the observed differences. | **Partial/open:** original strict parity failed; no portable production solver change justified. Future acceptance-contract change requires explicit maintainer decision. Unsupported degeneracies and native cross-platform bitwise identity remain unverified. |

No further speculative compatibility solver sprint is justified by this evidence.
Transition to a **separate product-hardening phase** is recommended, with its
scope agreed independently. Keep #8/#12 and all historical strict targets visible;
this audit grants no release or acceptance-contract change.
