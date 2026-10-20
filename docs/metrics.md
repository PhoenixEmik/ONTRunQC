<a id="metrics-and-json-schema-version-1"></a>

# Metric definitions

Scientific definitions and exact JSON fields for ONTRunQC. Counts, filtering populations and unavailable values are defined here; commands, input handling and output formats have separate references.
[Documentation index](README.md) · [Input](input.md) · [Output/schema](output.md) · [MinIONQC comparison](minionqc-comparison.md)

## Contents

- [Conventions and schema](#conventions-and-schema)
- [Populations and cutoff](#populations-and-cutoff)
- [Overall sequencing metrics](#overall-sequencing-metrics)
- [Quality](#quality)
- [Histograms and joint distributions](#histograms-and-joint-distributions)
- [Optional MinIONQC histogram layers (#19)](#optional-minionqc-histogram-layers-19)
- [Optional MinIONQC temporal plots (#20)](#optional-minionqc-temporal-plots-20)
- [Yield over time](#yield-over-time)
- [Channels, muxes and termination](#channels-muxes-and-termination)
- [Sequencing speed proxy](#sequencing-speed-proxy)
- [Exact retained yield and ultralong](#exact-retained-yield-and-ultralong)
- [Multi-run statistics](#multi-run-statistics)
- [Complete-data MinION read-level plots (#14)](#complete-data-minion-read-level-plots-14)
- [Provisional individual-read fits (#12)](#provisional-individual-read-fits-12)
- [Optional R-grid density estimates (#13)](#optional-r-grid-density-estimates-13)
- [Public reference validation and memory](#public-reference-validation-and-memory)
- [Related input and output references](#related-input-and-output-references)

## Conventions and schema

Core QC metrics are calculated centrally in `src/metrics.rs`; optional raw-read
fits and reference-grid densities use `src/trends.rs` and `src/density.rs`. Input rows must pass
validation before aggregation. Counts and yields are unsigned 64-bit integers;
overflow is an error, never wrapping arithmetic. Statistics use IEEE-754 `f64`;
required floating input must be finite; modern Q/time values are nonnegative,
while legacy signed Q values are explicitly retained. There are no NaN/Infinity values
in a valid report. Unavailable statistics are JSON `null`.

See [report envelope and ordering](output.md#report-envelope-and-ordering) for the
complete output structure. The `diagnostics` and `comparison` objects extend
schema version 1 additively; unknown fields must be tolerated. See [input validation](input.md)
for schema/representation selection and control exclusion.

<a id="minionqc-parity-comparisons"></a>

## Populations and cutoff

- Summary records: all retained valid input rows, including 0 bp, after control-read exclusion.
- Basecalled reads: length >0, unchanged throughout existing fields.
- Native passed: basecalled reads with `passes_filtering=TRUE`; FALSE is native
  failed. No Q threshold is inferred from these flags.
- Q-score selected: basecalled reads whose selected template/2D per-read Q-score
  is `>= qscore_cutoff`. CLI `-q/--qscore-cutoff` defaults to **7**, accepts finite
  signed values and includes exact boundary matches. Negative cutoffs do not
  relax modern input validation. Zero lengths are excluded at every cutoff.

`comparison.qscore_selected` has the same exact length/quality statistic fields
as `diagnostics.passed_read_statistics`: `read_count`, `total_yield_bp`, mean/median
length, N50, minimum/maximum length, mean/median Q-score. These use the established
ONTRunQC N50 (cumulative bases **>= half**) and arithmetic per-read Q-score.
Missing statistics are null; empty counts/yields are real zeros.
`length_histogram` and `quality_histogram` reuse the existing documented boundaries.
`time_bins` has one entry per occupied original 900-second interval, with
`start_seconds`, selected `basecalled_reads`, `yield_bp`, `cumulative_yield_bp`,
`mean_read_length_bp`, `mean_qscore`. Means use selected read counts and are null
in zero-observation bins; omitted intervals contain no records. No smoothing or
basecalling-completion time is inferred.

`passes_filtering` is optional; required columns depend on the
[selected input schema](input.md#tsv-format-and-required-columns). If absent, `native_filtering_available=false` and
`native_unclassified_reads` counts basecalled reads without a flag. Native count
and yield fields count only observed TRUE/FALSE values (both zero in this case),
not inferred outcomes. Native means/medians and termination pass fractions are
null; HTML labels native outcomes unavailable. The general count invariant is
`basecalled_reads = passed_reads + failed_reads + native_unclassified_reads`.
See [legacy input](input.md#legacy-representation-and-rna-controls) for missing metadata
and 2D/1D2 behavior.

## Overall sequencing metrics

| Field under `metrics` | Definition |
| --- | --- |
| `total_records` | All validated non-control data rows, excluding the header; RNA control exclusions are counted separately. |
| `basecalled_reads` | Retained rows with selected representation length >0 (template or explicitly selected 2D pair). |
| `zero_length_records` | Rows with length zero, even if filtering is TRUE. |
| `total_yield_bp` | Sum of basecalled read lengths, in base pairs. |
| `passed_reads`, `failed_reads` | Basecalled reads with TRUE or FALSE filtering, respectively. |
| `passed_yield_bp`, `failed_yield_bp` | Sum of lengths in the respective filtering group. |
| `mean_read_length_bp` | Total yield divided by basecalled reads. |
| `median_read_length_bp` | Middle sorted length; average of the two middle lengths for even counts. |
| `n50_bp` | Descending length at which cumulative bases first reach at least half the total yield. |
| `min_read_length_bp`, `max_read_length_bp` | Extremes among basecalled reads. |

Lengths are retained exactly as `u64` values and sorted in place. N50 uses integer
arithmetic, including the half-yield comparison. The median uses the exact sorted
middle values, with its result represented as `f64`; exceptionally large lengths
above the exact integer range of `f64` (2^53) can round in floating outputs.
For lengths 10, 20, 30, 40: yield = 100 bp, N50 = 30 bp, median = 25 bp.

Required invariants when `passes_filtering` is present:

```text
total_records = basecalled_reads + zero_length_records
basecalled_reads = passed_reads + failed_reads
total_yield_bp = passed_yield_bp + failed_yield_bp
```

Zero-length rows contribute to record counts and grouped record counts, including
time bins. They contribute neither basecalled/pass/fail read counts nor quality
observations. A positive-length read with Q0 is a valid quality observation.
Native filtering is taken solely from `passes_filtering`, without a Q7/Q10/Q20
cutoff or an inference from `end_reason`. Independent Q-cutoff results are in
`comparison` and never change native fields.

## Quality

`quality` contains `mean_qscore`, `median_qscore`, `min_qscore`, `max_qscore`, and
`histogram`. Observations use the selected per-read Q-score field
(`mean_qscore_template` or `mean_qscore_2d`) from basecalled reads, retained as
`f64` for the median.

The mean is the **arithmetic mean of per-read Q-scores**. It is neither
length-weighted nor a transformation of aggregate base error probabilities.
An online arithmetic mean avoids overflowing a sum of large finite values.
The median is the middle sorted Q-score or the average of the two middle values.
Floating arithmetic can incur ordinary rounding error. For identical input and
the same executable, aggregation and serialization are deterministic. Different
row orders can produce small floating mean differences.

Each histogram entry has `lower_inclusive`, `upper_exclusive`, and `read_count`.
Bins start at 0, 5, 10, 15, 20, 25, 30, 40, and 50. All bins are emitted, including
empty ones; the last upper bound is `null` (unbounded). These are distribution
boundaries, not filtering criteria. Legacy negative Q-scores add a separate
`[-f64::MAX, 0)` category when observed; the finite sentinel is not an observed
minimum. HTML labels this category as below zero.

<a id="exact-passed-read-statistics-and-storage"></a>

### Native passed-read statistics

`diagnostics.passed_read_statistics` contains `read_count`, `total_yield_bp`,
`mean_read_length_bp`, `median_read_length_bp`, `n50_bp`, `min_read_length_bp`,
`max_read_length_bp`, `mean_qscore`, and `median_qscore`. Definitions are identical
to the corresponding overall statistics, restricted to positive-length native
passed reads. Empty subsets have zero counts/yield and null statistics. Medians
and N50 use exact observations, never histogram interpolation.

Each read contributes one `u64` length and one `f64` Q-score (16 bytes of
elements), with no duplicate observations for all/native/Q subsets. Existing
streaming Q-score means retain input-order arithmetic. The current partition
layout, exact channel medians, capacity bounds and measured synthetic memory
check are described under [MinIONQC comparisons](#public-reference-validation-and-memory).

<a id="advanced-read-quality-diagnostics-v040"></a>

## Histograms and joint distributions

### Read-length distribution

`read_length_distribution` entries have `lower_inclusive_bp`,
`upper_exclusive_bp`, `read_count`, and `yield_bp`. Lower bounds are 1, 1,000,
5,000, 10,000, 25,000, 50,000, 100,000, 250,000, and 1,000,000 bp. The final upper
bound is `null`. All bins are emitted; zero lengths are excluded. Histogram read
counts sum to `basecalled_reads`; histogram yields sum to `total_yield_bp`.

### Read length × Q-score histogram

`diagnostics.length_quality_histogram` contains:

- `length_lower_bounds_bp`: 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1,024, 2,048,
  4,096, 8,192, 16,384, 32,768, 65,536, 131,072, 262,144, 524,288, 1,048,576.
- `qscore_lower_bounds`: 0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28,
  30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50.
- `all_read_counts` and `passed_read_counts`: 26 rows × 21 columns of `u64` counts.
  Rows follow ascending Q-score bins; columns follow ascending length bins.

Intervals are `[lower, next_lower)`; the final interval on each axis is
`[last_lower, infinity)`. Zero lengths are excluded. All finite nonnegative
Q-scores and positive `u64` lengths, including values above the display range,
are counted. The two matrices use 8,736 bytes during aggregation and no per-read
length/Q-score pairs. When legacy negative Q-scores occur, an additional
below-zero row is prepended to each matrix and `-f64::MAX` to the Q boundaries.
This categorical underflow row covers all finite negative Q-scores; its stored
sentinel is not a proportional plotted range. Matrix sums equal
`basecalled_reads` and `passed_reads`.

HTML positions finite length bins logarithmically (base 2) and Q-score bins
linearly. Dashed terminal column/row boundaries identify open-ended overflow
cells; their widths do not represent an infinite numeric interval. Both maps
share a sequential indigo scale normalized by `log10(1 + cell_count)` against the
all-read maximum. Accessible SVG cell labels/tooltips and a nonzero-cell table
retain integer counts. Histogram boundaries impose no filtering threshold.

## Optional MinIONQC histogram layers (#19)

`--minionqc-plot-bins` adds `minionqc_plot_bins` to JSON and a separate HTML
section/standalone figure family. Native histograms and every existing canonical
statistic retain their definitions. It is **not** implicitly enabled by
`--minionqc-compat`, which retains its established mux/output behavior.

| Layer | Population and coordinates | Bin construction |
| --- | --- | --- |
| `length` | Positive-length summary rows, then `log10(length_bp)`; All/Q-selected | `StatBin(bins=300)` after transformation. |
| `quality` | All retained summary rows, including zero lengths; All/Q-selected | `StatBin(bins=300)` in signed per-read Q coordinates. |
| `channels` | Summary-read bases/count/mean/median per observed channel; All/Q-selected include eligible zero lengths | `StatBin(bins=30)` per metric, shared All/Q X limits. |
| `length_vs_q` | All retained rows with finite log-length; Q limits train on all summary rows | `StatBin2d`, 30 requested bins per axis; occupied cells only. |

For 1D range `[a,b]`, ggplot2 3.5.1 uses width `(b-a)/(bins-1)`, or **0.1**
for its `zero_range` condition. The default boundary is `width/2`; the origin is
`boundary + floor((a-boundary)/width)*width`. Breaks extend through
`b + (1-1e-8)*width`. Assignment is right-closed, lowest edge included, with
`1e-8 * median(diff(breaks))` fuzz. Fuzz affects assignment, not reported 1D
geometry. Each emitted bin records `x`, `xmin`, `xmax`, `width`, integer `count`;
empty bins within an observed population are retained. An absent finite
population has an empty array. All/Q facets share X limits, with independent Y
scales; a requested bin count need not equal the resulting number of bins.

For 2D, widths are `(b-a)/30`; origins are `floor(a/width)*width`.
`adjust_breaks` shifts the first edge down and others up by
`1e-7 * median(diff(breaks))`. These adjusted edges determine both assignment
and plotted cell geometry. JSON records axis breaks and occupied cells with
`x/xmin/xmax/y/ymin/ymax/count`. Colors represent **linear read count**,
normalized over occupied-cell minimum/maximum, with constant counts at mid-color.
Empty or constant axes yield an explicit unavailable reason, corresponding to
upstream's inability to build a positive-width grid. Native empty reports still
succeed. Unrepresentable floating ranges fail with an actionable error; they
are not silently truncated or serialized as infinite coordinates.

Q histograms count zero-length Q measurements **only as reference plot data**;
they never enter canonical basecalled Q statistics. `summary_records`,
`q_selected_summary_records` and `zero_length_records` expose plotting
populations. Channel selected-summary medians reuse existing sorted partitions
plus counted zero lengths; native selected-basecalled medians remain unchanged.
True channel counts intentionally replace upstream's `n(n+1)/2` defect, so that
one channel-distribution family is not numerically identical to original R.

Bounds are tracked during normal aggregation. One additional source-guarded
streaming pass fills bounded histograms/cells, including gzip trailer/digest and
run-membership validation. No extra per-read vector is retained. Work/storage
scales with run groups, at most roughly 300 bins/population, about 31×31 cells,
and observed channels. Combined bins are rebuilt against pooled limits, never
summed from incompatible per-run grids; combined-only skips individual bin plans.
Existing exact numeric observations still determine whole-process memory use.

Bin2D is device-neutral without a selected physical layout. Explicit MinION mode
keeps its original read-level view; explicit PromethION mode includes Bin2D.
Neither this histogram nor channel IDs infer physical device geometry.
Reference validation uses the original default `--add-stat FALSE`. Native
`--add-stat` adds canonical basecalled-statistic text without retraining bins;
R annotation layers can expand X limits, so that separate rendering setting is
not claimed numerically equivalent.
Schema version remains 1: this opt-in field is additive and omitted by default.
See [verified comparisons](minionqc-comparison.md#reference-histogram-and-bin2d-validation-19).

## Optional MinIONQC temporal plots (#20)

`--minionqc-time-series` adds `minionqc_time_series` to individual reports and
`collection.runs[]`. It is independent of `--minionqc-compat`,
`--minionqc-plot-bins`, `--minionqc-trends` and `--minionqc-density`; none enables
another. Defaults and existing 900-second fields/plots are unchanged. Schema 1
is extended additively; strict consumers must allow unknown fields.

### Populations and integer hours

`all` includes every retained summary record, including zero lengths and signed
legacy Q-scores. `q_selected` includes records with Q >= the configured cutoff,
**also including zero lengths**. RNA controls use the existing exclusion policy.
These plot populations are deliberately distinct from canonical basecalled
statistics and native passing. Each population has `observations`, exact
`total_yield_bp`, `hourly` and `cumulative`.

`hourly` stores exact `{hour, read_count}` pairs at
`floor(start_time / 3600)`. Both populations are zero-filled from hour 0 through
the All population's last observed hour; an empty source has no hours. There is
no division by exposure, interpolation or conversion of 15-minute counts to
hourly rates. To bound explicit zero-fill/output, this option rejects a last
hour above 100,000 (over eleven years), with an actionable error; native plots
remain available without it. An empty Q population has no cumulative curve; known hourly counts remain
explicit zeros. A chart with no retained observations shows an explanatory
unavailable state. Upstream MinIONQC instead exits when no selected records exist.

### Read-start cumulative curves and ties

Numeric observations are sorted by finite start seconds, then deterministic
source-file/record order. `points` contain original `start_seconds`, exact
`cumulative_yield_bp` and a one-based population `observation_index`. Zero-length
observations remain observations even when cumulative yield does not increase.
No artificial time-zero point is added. JSON stores integer bases; SVG axes
use hours and decimal Gb. All/Q comparison panels share the All time range,
with independent Y ranges for separate per-source population figures. Independent runs have independent curves; the pooled
report does **not** invent a combined instrument timeline.

R merges MinION channels before sorting time and later reorders rows by length
and hour. For tied times its individual cumulative values and GeomLine ordering
can therefore differ from input order, including backwards vertical traversals.
ONTRunQC does not emulate that implementation-dependent ordering. It reports
`tied_timestamp_groups`, `tied_observations`, and `maximum_tied_yield_bp` for each
population. Post-tie cumulative maxima and final yields are invariant; exact
point-for-point or intervening line-segment equivalence is **not** claimed at
ties. Validation measures both post-tie envelopes and actual R one-sided
polyline limits separately. The conservative additional tie-order error bound
is `maximum_tied_yield_bp`.

### Bounded display and measured error

Up to 8,192 observations are displayed as exact numeric points. Larger
populations use `representation=bounded_monotone_polyline`, with
`epsilon = ceil(total_yield_bp / 4096)`. Starting at the first observation, retain
the previous and current point whenever the current cumulative value exceeds
the retained anchor by epsilon; always retain the last point. Consecutive large
jumps thus retain both endpoints. Every read contributes; this is not sampling.
There are at most 8,194 display points per population, including first/last.
Zero-yield dense populations need only first/last coordinates.

Between nonconsecutive retained vertices the vertical span is at most epsilon;
consecutive vertices reproduce the original segment. Hence the linear-display
error is at most epsilon bp against the native stable-order curve.
`yield_error_bound_bp` records this guarantee and
`maximum_linear_interpolation_error_bp` measures the actual maximum over **all**
sorted original vertices in a second numeric-file scan. Shared-time vertical
segments cover intermediate native values. `omitted_points` reports the number
of original vertices not displayed. The measured value is not silently reused
as an original-R error: the optional R suite independently checks every original
R breakpoint/one-sided limit, retaining explicit tied-order differences.

### Passes, scratch and integrity

One guarded TSV reread per run/file membership supplies the sorter; interleaved
runs sharing a file require one such reread for each affected run. All gzip
members/CRC/trailing bytes, membership, source identity and optional raw-byte
SHA guards are validated using the existing API. One run is sorted at a time,
including in parallel/combined-only analyses. No full TSV extraction occurs.

Private `TMPDIR/ontrunqc-time-*` files contain only 25-byte numeric observations
(time, length, stable ordinal, selection bit). Unix scratch directories use mode
0700. Sorting buffers at most 65,536 observations (about 2 MiB), merges at most
32 files at once, and carries a bounded number of files per merge level.
Peak numeric scratch is at most approximately twice 25 bytes per observation
(1.5 GB for 30 million observations), excluding staging of reports. Merge depth
increases numeric I/O; curves require two scans of the final sorted file, not
extra TSV/decompression passes. Files are removed on success/error; OS/process
termination can leave scratch, as with any nonpersistent temporary workspace.
Disk-full/I/O/source-change errors prevent that report's publication. Existing
per-file publication guarantees apply; there is no collection-wide transaction.

## Yield over time

`time_bins` entries contain `start_seconds`, `end_seconds`, `total_records`,
`basecalled_reads`, `yield_bp`, and `cumulative_yield_bp`. The bin start is
`floor(start_time / 900) * 900`, using finite nonnegative `f64` input seconds.
Bins are half-open: a read starting at 900 seconds belongs to [900, 1800).
Times too large to express the bin's start/end as `u64` seconds are rejected.

Only occupied bins are emitted, including bins containing only zero-length
records. Missing bins imply zero records and zero yield; cumulative yield stays
unchanged across gaps. Bins are ordered by start time and cumulative yield is
calculated after aggregation, so input need not be time-sorted.

All bases of a read are attributed to its **start time**. Duration is validated
but not used to distribute bases across bins. This is not exact instrument
throughput, pore occupancy, or completion-time yield.

Each occupied bin also contains:

| Field | Definition |
| --- | --- |
| `passed_reads`, `passed_yield_bp` | Native passed basecalled count and yield. |
| `mean_read_length_bp` | `yield_bp / basecalled_reads`. |
| `mean_passed_read_length_bp` | `passed_yield_bp / passed_reads`. |
| `qscore_observations` | Number of positive-length Q-score observations; equals `basecalled_reads` since Q-score is required. |
| `mean_qscore` | Arithmetic per-basecalled-read Q-score mean. |
| `mean_passed_qscore` | Arithmetic Q-score mean among passed reads; denominator `passed_reads`. |

Zero-length rows contribute none of these observations; means with zero
denominators are null. HTML uses hours, kb, and per-read Q-score units, connects
only adjacent occupied 900-second bins with available means, and breaks each
series independently at unavailable values. Q0 on a positive-length read is
available data, not a gap. There is no smoothing or hourly resampling.

### Observed channel activity

`channel_activity` entries have `start_seconds` and
`basecalled_producing_channels`, ordered by start time and aligned with existing
`time_bins`. The count is distinct channel IDs with at least one positive-length
read starting in that bin; multiple reads count once. Zero-length-only bins have
0; missing bins imply 0. Counts are maintained with one set per occupied bin,
not one stored channel per read. Memory scales with distinct (bin, channel)
observations. These are observed basecalled-producing channels, not MinKNOW active
pores, occupancy, availability, or continuous read-duration overlap. Chart points
are at bin midpoints; missing intervals are not joined by lines.

<a id="additive-deep-qc-diagnostics-v030"></a>

## Channels, muxes and termination

### Grouped statistics

| Array | Entry fields |
| --- | --- |
| `channels` | `channel`, `total_records`, `basecalled_reads`, `total_yield_bp`, `passed_reads`, `passed_yield_bp`, `mean_qscore` |
| `muxes` | `mux`, `total_records`, `basecalled_reads`, `total_yield_bp`, `passed_yield_bp` |
| `end_reasons` | `end_reason`, `total_records`, `basecalled_reads`, `total_yield_bp` |

Channel Q-score means use the same arithmetic definition and zero-length exclusion
as the run mean. v0.4.0 adds `mean_read_length_bp` and
`mean_passed_read_length_bp`, calculated as channel yield / corresponding
basecalled read count. A zero denominator gives `null` for that mean.
Unknown termination strings are preserved verbatim and grouped by exact string.
Channels and muxes are identifiers. Physical coordinates exist only with an explicitly selected supported layout.

### Exact channel summaries

`comparison.channels` is sorted by channel ID. Every entry has three populations:
`all_summary_records`, `all_basecalled_reads`, `qscore_selected_reads`. Each has
`read_count`, `total_yield_bp`, `mean_read_length_bp`, `median_read_length_bp`.
Summary means/medians include zero lengths, making the upstream denominator
available without changing existing basecalled metrics. A zero-only observed
channel has summary mean/median 0 and unavailable basecalled/selected means and
medians. Unobserved channels are absent. Positive-length channel medians are exact,
including even-count half-integer results. Counts/yields conserve run totals.

`channel_distributions` has four families (`metric`): `total_yield_bp`, `read_count`,
`mean_read_length_bp`, `median_read_length_bp`, each with `axis_maximum` and 30
shared equal-width `bins` for All-summary/Q-selected channels. Lower bounds are
inclusive; upper bounds exclusive; final upper bound is null and includes the
observed maximum. Empty/all-zero families use axis maximum 1. Counts are
`all_summary_channels` and `qscore_selected_channels`; channels absent from a
population do not count. These are distributions across channels, not reads.
Counts are true row counts: upstream's `sum(which(length>=0))` incorrectly gives
n(n+1)/2. That implementation bug is deliberately not copied. Existing channel
native-passed means remain unchanged. Chart bin placement is deterministic and
explicit, rather than ggplot2's automatically placed 30 bins. Read histograms
retain ONTRunQC's existing fixed bins rather than upstream's automatic 300 bins.
Time means remain unsmoothed 15-minute observations rather than ggplot smoothing.
Unrounded values are retained in JSON, unlike upstream's one-decimal summaries.

### Distribution of channel mean lengths

`diagnostics.channel_read_length_distribution` uses the existing nine read-length
interval boundaries. Each entry has `lower_inclusive_bp`, `upper_exclusive_bp`,
`channel_count`, and `passed_channel_count`. The first count bins channel all-read
means; the second bins channel passed-read means. Each eligible channel counts
once, without read-count weighting. Channels without the corresponding reads are
excluded. These are channel-mean distributions, not per-read length distributions.
HTML shows the all-read distribution and both means in the existing channel table.

### Channel × mux

`channel_mux` entries are ordered by ascending `(channel, mux)` and contain those
identifiers plus `total_records`, `basecalled_reads`, `zero_length_records`,
`total_yield_bp`, `passed_reads`, and `passed_yield_bp`. Only observed pairs occur;
no four-mux assumption is made. Zero-length records contribute only record/zero
counts. Group yields and passed yields sum to the respective run totals; group
records sum to all records. Counts/yield do not establish pore occupancy. HTML
provides a collapsible table sharing channel search, sorting, and pagination.

### Termination diagnostics

`termination_reasons` preserves reason strings and adds `passed_reads`,
`zero_length_records`, and these centrally calculated ratios:

| Field | Numerator / denominator |
| --- | --- |
| `summary_record_percent` | 100 × reason records / run records |
| `zero_length_fraction` | reason zero-length records / reason records |
| `basecalled_read_fraction` | reason basecalled reads / reason records |
| `passed_fraction_of_basecalled` | reason passed basecalled reads / reason basecalled reads |
| `total_yield_percent` | 100 × reason yield / run yield |

Fractions range from 0–1 and percentages from 0–100. Zero denominators produce
null, never an invented zero. HTML formats fractions as percentages. This new
array and the HTML table sort by descending record count, then ascending reason
name. The original `end_reasons` array retains ascending name order. Reasons are
not assigned a good/bad classification and do not determine filtering status.

### Explicit physical layout

`diagnostics.physical_layout` is `null` unless a supported layout is explicitly
selected. PromethION has `name`, `rows: 25`, `columns: 120`, and
`channel_coordinates` (`channel`, `row`, `column`) for all 3000 channels in channel
order. Yields are not duplicated: coordinates join to the existing `channels`
array. Absent channel observations are unavailable, not observed zero yield.

The [EPI2ME Basic QC Tutorial](https://epi2me.nanoporetech.com/notebooks/Basic_QC_Tutorial.html)
implements `np.hstack([np.arange(x, x + 250).reshape(25, 10) for x in
range(1, 2752, 250)])`. With `i = channel - 1`, our zero-based coordinates are:

```text
row    = (i % 250) / 10         (integer division)
column = (i / 250) * 10 + i % 10
```

The first block occupies columns 0–9; the twelfth occupies 110–119. Channel 10 is
(0, 9), 11 is (1, 0), 250 is (24, 9), and 251 is (0, 10). All 3000 coordinates
are unique and in range. Explicitly selected layouts reject incompatible channel
IDs, including zero-length records, with the input line number. No layout is
inferred from maximum channel ID. Compatibility with other flowcell revisions,
models, or numbering conventions and instrument-facing orientation is not
established. Unobserved channels must not be described as dead or inactive pores.

SVG maps use linear independent total/passed-yield scales in explicit bp/kb/Mb/Gb
units. Each cell has a channel label and exact bp tooltip. Hatching identifies
unobserved summary channels; a pale solid cell identifies observed zero yield.

The explicit `minion-r9.5` map is the exact upstream p1..p8/q1..q8 construction,
zero-based, 32 rows ×16 columns. Left block starts: 33,481,417,353,289,225,161,97;
right block starts: 1,449,385,321,257,193,129,65. Each block contains 32 channels,
4 rows ×8 columns; right columns are reversed. All 512 coordinates and block
edges are tested. No layout is inferred from channel maxima. Total/native/Q-cutoff
spatial yields remain distinct; absent observations differ from observed zero.

## Sequencing speed proxy

`sequencing_speed` is `null` when the optional `template_duration` header is absent.
When present it contains `eligible_reads`, `mean_bp_per_second`, `histogram`, and
`time_bins`. A read is eligible only if length is positive, its template duration
is finite and positive, and length / template duration is finite. Blank,
malformed (including non-UTF-8), negative, zero, or nonfinite optional durations
exclude speed observations only. Invalid required fields still fail the record.
`duration` is never used as a fallback. Zero eligible reads yield a null mean,
zero histogram counts, and an explanatory unavailable HTML state.

Means use online arithmetic accumulation per eligible read, without length or
duration weighting. No per-read speed vector or quantile is retained. Histogram
fields are `lower_inclusive_bp_per_second`, `upper_exclusive_bp_per_second`, and
`read_count`; bounds are 0, 100, 200, 300, 400, 500, 600, 800, 1000, with a null
final upper bound. Histogram counts sum to `eligible_reads`.

Speed `time_bins` contain `start_seconds`, `eligible_reads`, and
`mean_bp_per_second`, aligned with occupied run bins; bins without eligible
speeds have a null mean. Chart points use bin midpoints and do not connect across
missing or unavailable bins. This is a basecalled-length/template-duration proxy,
not a direct motor-speed or raw-signal measurement; no speed threshold is applied.

## Exact retained yield and ultralong

`yield_by_minimum_length.thresholds` stores ascending `minimum_length_bp`,
`all_read_count`, `all_yield_bp`, `qscore_selected_read_count`, and
`qscore_selected_yield_bp` at **10,000; 20,000; 50,000; 100,000; 200,000; 500,000;
1,000,000 bp**. Selection is inclusive, matching upstream code despite its “>”
labels. Length ties are included together. Results come from exact sorted lengths.

`curve` has the same fields at at most 520 deterministic queries: deduplicated
integer positions `1 + floor((end-1)*i/512)`, i=0..512, plus requested thresholds
inside the displayed domain. `end=max_length+1`, saturating at u64::MAX; an empty
population uses 1. Every returned point is exact; connecting segments are a
bounded display summary, not values at every unique length. All thresholds remain
available even outside the observed domain. Unlike upstream's 99%-yield crop,
the graph displays the full range; no longest reads are silently dropped.

`ultralong_all` and `ultralong_qscore_selected` contain `target_n50_bp=100000`,
`read_count`, `total_yield_bp`, `minimum_read_length_bp`. Sort lengths descending;
choose the largest prefix whose N50 under upstream's **strict >half** convention
is >=100,000 bp. Equivalently, its total yield is strictly less than twice the
yield of all reads >=100,000 bp. Shorter reads can enter this subset; ties at the
prefix endpoint can be partially included (all tied lengths are identical, so
aggregate results are deterministic). No long reads gives an empty subset/null
minimum. Eligible entire sets stay entire, fixing upstream `binSearch`'s
`max-1` endpoint loss, including singleton inputs. Established global/native/Q
N50 remains >=half: e.g. 100,50,50 yields N50=100; upstream's strict-half N50=50.
Tests cover this deliberate difference and ultralong exact-half boundaries.

<a id="multi-run-reports-8"></a>

## Multi-run statistics

`collection.runs` contains compact per-run statistics/distributions/timelines and
source metadata; top-level metrics are exact pooled statistics. Per-read numeric
buffers are moved into the existing aggregator, never inferred from histograms.
Pooled channel/mux/time arrays are deliberately empty: overlapping local IDs and
relative clocks have no meaningful pooled interpretation. The HTML distinguishes
pooled statistics from per-run comparisons; `--combined-only` skips all individual
outputs. Publication remains per file, not atomic across a collection.

### Default per-run density estimates

`distributions` contains Gaussian KDEs for all/Q-selected length and Q-score
populations, each with `observations`, `bandwidth`, and 512 `(x,density)` points.
Length x is log10(bp), Q x is the reported per-read Q-score. Bandwidth uses R's
`bw.nrd0`: 0.9 × min(sample SD, type-7 IQR/1.34) × n^-0.2, with SD/absolute-first/
1 fallbacks for degenerate data. Evaluations use 2,048 linear mass bins, Gaussian
convolution, and a domain extending three bandwidths past each data endpoint.
This differs numerically from R's FFT implementation and ggplot panel cropping;
synthetic tests compare against direct Gaussian evaluation within 0.5% of peak.
Empty or nonrepresentable floating ranges return null, never an invented curve.
Singleton fallback is defined even though R's automatic density needs two points.
Exact QC statistics do not depend on density estimation. No additional per-read
vectors are retained; only bounded density grids and aggregated run summaries.

## Complete-data MinION read-level plots (#14)

Only explicit `--flowcell-layout minion-r9.5` enables these two plots. Population:
**every positive-length summary record after control exclusion**, regardless of
native passing or Q cutoff, including signed legacy Q values. Zero lengths are
counted separately, never plotted as valid Q0 reads. Original MinIONQC includes
these rows at nonfinite logarithmic coordinates; this is an intentional difference.
Combined-only analysis skips these plots; pooled reports never overlay distinct
instruments into one physical map.

- **Exact points**: at most 4,096 eligible reads per run, all retained. Canonical
  time/channel/length/Q/events ordering makes output deterministic, rather than
  reproducing R's merge-dependent overplot order. Overview alpha is 0.35, scatter
  alpha 0.05; point sizes and palette interpolation are not pixel-identical to R.
- **Full-count pixel aggregation**: larger runs retain no read-point array.
  One additional streaming pass counts every eligible read in both fixed grids.
  Overview uses 64×40 pixels/channel (1024×1280 physical atlas); scatter uses
  512×256 pixels. Bins include both domain extrema; constant domains map centrally.
  Overview X is start time (hours), Y is log10 length, shared across all panels;
  scatter X is log10 length and Y is per-read Q. No clipping of rare long/short or
  low/high-Q reads. Per-channel min/max length, Q and positive events/base
  representatives are overlaid as actual extrema, not added to raster counts.
- Overview color is the **minimum Q per occupied pixel**, preserving low-Q
  observations. Scatter color is the arithmetic mean of quantized per-read mapped
  RGB colors, including missing/zero categories; it is **not** mean events/base.
  Positive events/base = template events / selected template length, using
  **natural log**, inferno anchors; Q uses viridis anchors. Missing events are gray,
  zero events slate (log undefined). The existing optional-event parser treats
  negative/malformed/absent counts as unavailable, counted with missing events;
  these values never enter a logarithm. 2D observations never borrow template events. R colors missing/zero
  event observations gray; explicitly distinguishing these categories is intentional.
- Both raster opacities are `0.25 + 0.75*ln(1+count)/ln(1+max_count)`, quantized to
  8-bit alpha; empty pixels are transparent. Legends show the corresponding count
  levels. Overlap cannot be resolved into individual reads, and mixed pixel colors
  cannot recover the original event-color distribution. Exact count metadata
  preserves density; extrema preserve rare tails. This is full-data aggregation,
  not a claim of independent-point or ggplot pixel equivalence.

`diagnostics.minion_read_visualization` is an additive optional schema-1 field:
`method`, `population`, `eligible_reads`, `zero_length_records`, missing/zero event
counts, `additional_input_passes`, exact `bounds`, per-channel counts/`extrema`,
`points` (all exact points or empty), and nullable `overview`/`scatter`. Each raster
has dimensions, total/max read counts, `color_reduction`, an embedded PNG data URI,
and sorted sparse `[pixel_index, count]` pairs, indexed from the top left in
row-major order. Overview indices address the physical atlas, not channel order.
Both count sums equal `eligible_reads`. Empty/zero-only reports have null bounds
and rasters. Deprecated `diagnostics.minion_sample` remains **null**; it is never
silently relabeled as complete data. Consumers of its former non-null sample need
to migrate; strict unknown-field consumers also need updating. Existing scientific
fields and schema version stay unchanged.

Accumulation memory is bounded by grids, 512 channels and at most six extrema per
channel (plus the 4,096-point small-input limit). Grids use approximately 28 MiB;
PNG buffers/serialization add temporary overhead. Only one run's grids are live
at once. Each dense run rescans its contributing files; interleaved runs in one
file can require repeated scans. Existing exact-statistics arrays remain separate
and unchanged. PNG/base64 crates only encode embedded images; they add no runtime
service or frontend dependency. SVG exports embed the PNG, not an external asset.

Reproduce full-count/semantic checks against actual pinned R with:

```sh
cargo build --release --locked
python3 tests/minion_visualization.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript --render-reference
```

Requires the existing R 4.4.1 / ggplot2 3.5.1 / mgcv 1.9-1 environment; does not
install R or modify #12's reference capture. Optional `--baseline /path/to/old/binary`
compares all scientific JSON after removing only the changed visualization fields.
Independent Python and actual R plot-data calculations compare every pixel count,
channel position, population, axis/alpha and natural-log color transformation.
See [measured rendering costs](benchmark.md#complete-data-minion-rendering-14).

Optional public validation now reads original files without adapters or Q clipping:
99 short-run records / 1,089,633 bp; 75,160 PromethION / 623,196,506 bp;
159,799 1D2 / 930,249,488 bp; 963 retained direct-RNA /197,413 bp (37 controls
excluded); two MinION inputs pooled: 608,208 /7,203,301,321 bp, N50 33,161,
median 2,616. Inputs can contain several run IDs; they are split rather than
pretended to be a single physical flowcell. Threshold/ultralong counts match
available upstream YAML and independent calculations. Short/1D2 YAML labels
Q>=7 are stale: numeric values match Q8, as used by upstream tests.R. R was not
executed; the historical private PromethION dataset was not rerun.

## Provisional individual-read fits (#12)

`--minionqc-trends` adds optional `fitted_trends` (schema 1, additive). Existing
interval means and all prior metrics remain unchanged. This costs an additional
streaming input pass. Separate single-run and combined-context curves retain 80
uniform evaluation times over each population's observed start-time range (hours).
All summary records include zero lengths; the inclusive Q-selected summary subset
can also include zero lengths. These are upstream comparison populations, not
basecalled-read quality statistics or native passing. RNA controls remain excluded.
Single-run Q fits exclude negative Q values to model upstream's pre-fit y scale
limit; combined Q plots have no such limit.

The largest All group selects LOESS below 1,000 observations, otherwise a cubic
shrinkage regression spline (`cs`, k=10) with Gaussian REML. Combined
selection uses the largest run/source across panels, so a small run can use LOESS
alone and GAM in a collection. LOESS uses degree 2, span .75, tricube weights and
a median-split interpolated surface (cell .2, padded range, vertex derivatives
and cubic Hermite interpolation). Local quadratics use equilibrated Householder
QR and a three-column Jacobi SVD pseudoinverse with R's 100-epsilon cutoff;
zero-cell leaves also split at singleton observations. Two-active-row singular
neighborhoods use a direct minimum-norm solve, preserving responses with tiny
weights that QR previously lost. Actual R goldens cover the four-read N50-tie
and ultralong endpoint cases. GAM uses natural cubic basis functions, integrated
second-derivative penalty, .1/.01 null-space shrinkage, a centered smooth and an
unpenalized intercept. The two mathematical null directions use an explicit endpoint-zero linear /
orthogonal constant convention, weighted .1/.01. R can swap these directions
when sorting roundoff-size eigenvalues, so some shrinkage penalties remain
different; this is explicitly Partial. Smoothing uses Gaussian joint Newton
optimization of log(lambda) and log(variance), with analytical derivatives and
the pinned mgcv initialization, step limits and stopping rule. Original response
units enter the likelihood/convergence scale. Singular/noiseless optimization
can fall back to a bracket-checked profiled REML fit; this does not claim complete
mgcv behavior on degenerate data.

Knots are exact type-7 quantiles of each population's **unique** start times,
including endpoints; `exact_knots=true` and `knots_hours` expose the result.
R's probability/index arithmetic is retained because one-ulp changes can alter
its numerical null-space ordering; the bounded sorting algorithm is unchanged. Knot
selection uses at most 262,144 numeric (time, cohort-flags) elements (4 MiB) per
tracker, then sorted temporary runs with deduplication and at most 64 merge readers.
On disk each entry is 9 bytes before deduplication; intermediate merges require
additional scratch space/I/O proportional to observations, not additional read
objects in RAM. Run-name metadata grows logarithmically. Scratch files use the
system temporary directory (respecting `TMPDIR`) and are removed on normal success
or error; abrupt process termination may leave files. Unix directories/files use
0700/0600 permissions. Scratch failures abort with an error rather than selecting
approximate knots. Inputs below the buffer limit require no scratch files.

Every raw observation contributes to second-pass sufficient statistics: no fit
uses interval means or downsampled responses. Up to 999 observations/population
are retained for LOESS; larger groups discard that buffer. Empty, constant-time,
unsupported-rank and nonrepresentable fits have a reason and no points. Fits can
bridge unobserved intervals; observed-bin plots retain their gaps. All fit
calculations live in `trends.rs`, with numeric knot selection in `trend_knots.rs`;
HTML renders the final model only.

Fitted points add nullable `standard_error`, `lower` and `upper`. GAM uses pointwise 95%
Bayesian normal intervals conditional on the estimated smoothing parameter,
using residual variance RSS/(n−edf). `confidence_interval` names this convention;
LOESS uses `pointwise_95_percent_student_t`: the interpolated local-polynomial
influence weights, R's exact hat trace and approximate delta calibration give
residual scale sqrt(RSS/delta1), df=delta1²/delta2, and SE=scale×sqrt(sum(weights²)).
Bounds use the Student-t .975 quantile, as in ggplot2. A nonsingular local fit
does not guarantee finite residual df; nonpositive/nonfinite deltas or scales
leave uncertainty unavailable. No normal substitute or invented interval is used.
Influence vectors are retained only for the <=999-row LOESS population (O(n×vertices));
GAM memory remains independent of observation count.
HTML renders available bands
and omits segments crossing negative bounds on nonnegative axes. These are additive schema-1 fields;
consumers that reject unknown fields must be updated. Existing metrics and
unsmoothed charts are unchanged.

Unit/CLI tests exercise polynomial reproduction, natural knot interpolation,
constant intercepts, method selection, exact populations/knots, forced scratch
merges, cleanup, unchanged QC metrics and deterministic reports. Five synthetic
actual-R goldens verify 1,598 fitted values and 960 GAM uncertainty values at
absolute 1e-6 + relative 1e-6. The 18-case paired run verifies all captured knots,
and all public PromethION/1D2 fitted values and GAM bands meet the point target.
Four public PromethION aggregate-model snapshots guard the earlier 211 bp endpoint
error and its uncertainty, without embedding reads. A further eight-case synthetic
R snapshot checks 5,280 LOESS fitted/uncertainty values, trace, deltas, scale, df
and critical values, including ties, sparse/degenerate populations and 999 reads.
Some MinION selected/combined fits and intermediate penalties still differ from
unmodified R. Controlled R fits with the explicit Rust null-space convention
explain these differences; they do not replace strict reference targets. See
the parity matrix for numerical evidence and the compatibility policy. Optional
fitting remains Partial; core QC statistics and KDE are unchanged.

## Optional R-grid density estimates (#13)

`--minionqc-density` adds `minionqc_density`, leaving the original `distributions`
and all existing statistics unchanged. The new four estimates use the existing
`DistributionEstimates` shape (bandwidth, observation count, 512 x/density pairs).
Length observations are transformed with log10 **before** estimation, matching
scale transformation before ggplot statistics; density units are per log10(bp),
not per bp. No raw-length Jacobian is applied. Zero lengths cannot enter that
transformed population. Q density includes all retained summary-record Q values,
including zero-length rows; its named Q-cutoff subset is inclusive. These are
reference comparison populations, separate from basecalled Q-score statistics.

Bandwidth is Gaussian `bw.nrd0`, sample SD and type-7 IQR/1.34, adjust 1. All groups
share the plot's observed scale domain (`trim=false`); selected groups do not
silently acquire independent X grids. For constant domains, all 512 evaluations
use the single unchanged coordinate, matching actual ggplot2 `scale$dimension()`.
HTML expands only the display axis and shows evaluated points, with an explicit
constant-domain note. It never invents a statistical range. R 4.4 corrected coordinates (`old.coords=false`)
use 512 linear mass bins, an internal domain extended by four bandwidths and a
zero-padded discrete Gaussian convolution, then linear interpolation to 512 plot
points. Direct convolution replaces FFT computation, without another dependency;
its mathematical convolution is the same, but floating rounding may differ.
No boundary reflection or forced re-normalization is applied. Area on the plotted
observed range can be less than one because Gaussian tails extend beyond it.
Very wide domains/small bandwidths can underresolve peaks, as in a finite R grid.

Existing positive-length numeric buffers are reused. Only this optional mode
additionally retains one `f64` per zero-length record (8 bytes of elements plus
capacity), required for exact summary-Q bandwidth quantiles. Buffers are moved
into pooling, not duplicated by subset. Kernel/mass/evaluation arrays are fixed
size; sparse (<2 observations) and nonrepresentable estimates are null. Original
single-observation fallback behavior remains unchanged in legacy distributions.

Tests cover known bandwidths, integral mass on a sufficiently wide domain,
symmetry/peaks, close/constant observations, long tails, shared domains, unequal
source sizes, zero-Q population distinctions, determinism and unchanged old QC.
The executable harness now captures actual ggplot2 bandwidths as well as every
density point, plotted-domain integral and peak position. Pinned R 4.4.1 / ggplot2
3.5.1 comparisons passed on public combined inputs and six synthetic density
cases, including singleton groups. Checked-in synthetic-only golden data cover
11,264 points plus unavailable singletons. Tolerances and measured errors are in
`minionqc-parity.md`; direct convolution is not claimed bit-identical to FFT.
The optional estimator's descriptive `method` now omits the obsolete
`unverified` suffix; report fields, legacy distributions and core metrics retain
their definitions.

## Public reference validation and memory

Run the optional standard-library check outside normal CI:

```sh
git clone https://github.com/roblanf/minion_qc /tmp/minion-qc-reference
git -C /tmp/minion-qc-reference checkout 54485d586038d8ee36f168dd94f31c423f52f581
python3 tests/minionqc_reference.py /tmp/minion-qc-reference
python3 tests/parity_memory.py  # 1 million fictional reads, 3,000 channels
```

The optional validator reads the original pinned public examples directly, without
fabricated fields or Q-score clipping. Results, control exclusions and fixture
caveats are listed in [legacy input handling](input.md#legacy-representation-and-rna-controls).
It checks exact integers, per-channel medians and independent pooled calculations;
upstream rounded Gb values have a 50.1 bp tolerance. This fast check does not execute
R; the actual 18-case paired R suite and plot-layer audit are documented in
[reference validation](../tests/reference/README.md) and the [comparison](minionqc-comparison.md).
No upstream read records or identifying reports are committed.

Lengths and Q-scores still consume **16 bytes per basecalled read** in elements.
Lengths are stored once in four disjoint native/Q partitions per channel; Q-scores
once in four global partitions, without per-channel Q arrays or full read objects.
Channel medians are calculated before buffers are moved/reused for run statistics.
Spare channel length capacity is <4×256×8 bytes per channel (~23.4 MiB/3,000
channels), global Q spare capacity <=2 MiB; final global length spare capacity
<=2 MiB. Moving a channel partition can transiently duplicate that partition;
very skewed channels can increase the peak. Aggregated maps/output also consume
memory. No <512 MiB claim is made for 30 million records.

Before the multi-run/legacy/export additions, the macOS focused
1,000,000-record / 3,000-channel synthetic
check (48,520,325 input bytes, all four partitions exercised) measured **59,310,080
bytes / 56.56 MiB peak child RSS**. This is an actual small synthetic measurement,
not a full-run benchmark or a performance guarantee. The private PBM95914 summary
was not rerun. Its previously supplied record/yield/native-pass/N50 values remain
historical validation evidence, not newly measured results.

Supplied historical PBM95914 baseline (original summary unavailable here):
28,202,608 records; 28,201,298 basecalled reads; 170,083,038,417 bp total yield;
8,860 bp all-basecalled N50; 26,900,272 native passed reads;
163,423,869,988 bp native passed yield; 8,867 bp native passed N50. Matching
Q>=9 totals for that run do not establish universal or per-read filter equivalence.

### Validation denominators

Summary records include zero lengths; basecalled reads exclude them; passed reads
add the native TRUE flag. An externally defined Q ≥ 9 subset is distinct from
native passing. The maintainer's real-run reference matched these subset totals
for that run only. Different zero-length denominators and rounded external Gb
outputs must be reconciled before comparing means, medians, and yields. No
original records or private identifiers are used in public regression fixtures.

## Related input and output references

<a id="plain-and-gzip-input"></a>
<a id="legacy-schemas-and-minion-9"></a>
<a id="bounded-run-processing-and-cli-compatibility-16"></a>
<a id="multi-pass-input-integrity"></a>

Input transport, legacy schemas, parallel workers and consistency checks are in
[Input and run processing](input.md).

<a id="report-envelope-and-ordering"></a>
<a id="html-presentation"></a>
<a id="output-controls-and-yaml-10"></a>
<a id="native-static-figure-exports-15"></a>

Output schema, HTML, YAML, annotations, sizes and codecs are in
[Reports, schema and figure export](output.md).
