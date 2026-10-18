# Metrics and JSON schema (version 1)

All metrics are calculated centrally in `src/metrics.rs`. Input rows must pass
validation before aggregation. Counts and yields are unsigned 64-bit integers;
overflow is an error, never wrapping arithmetic. Statistics use IEEE-754 `f64`;
required floating input must be finite; modern Q/time values are nonnegative,
while legacy signed Q values are explicitly retained. There are no NaN/Infinity values
in a valid report. Unavailable statistics are JSON `null`.

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
`histogram`. All observations are per-read `mean_qscore_template` values from
basecalled reads, retained as `f64` for the median.

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
boundaries, not filtering criteria.

## Read-length distribution

`read_length_distribution` entries have `lower_inclusive_bp`,
`upper_exclusive_bp`, `read_count`, and `yield_bp`. Lower bounds are 1, 1,000,
5,000, 10,000, 25,000, 50,000, 100,000, 250,000, and 1,000,000 bp. The final upper
bound is `null`. All bins are emitted; zero lengths are excluded. Histogram read
counts sum to `basecalled_reads`; histogram yields sum to `total_yield_bp`.

## Grouped statistics

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

v0.4.0 adds the following fields to each existing occupied bin:

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

## Report envelope and ordering

Top-level fields are `schema_version` (1), `tool` (`name: "PoreGauge"`, package
`version`), `run` (`run_id`), `metrics`, `quality`, `read_length_distribution`,
`channels`, `muxes`, `time_bins`, `end_reasons`, and (since v0.3.0) `diagnostics`.

Channel and mux arrays use ascending numeric order, time bins ascending start
seconds, termination reasons ascending Rust string order, and histogram bins
ascending lower bounds. JSON fields follow the declared schema order. There are
no timestamps, input paths, random identifiers, or individual records in reports.
Pretty-printed JSON ends with a newline.

For header-only input, `run.run_id` is `null`, counts/yields are zero, statistics
are `null`, grouped arrays are empty, and fixed histograms contain zero counts.
A run containing only zero-length records retains its run ID and grouped record
counts but has unavailable length/quality statistics.

## HTML presentation

`report.html` presents this same final report model without adding individual
reads or changing JSON schema 1. Pass rate is `100 * passed_reads /
basecalled_reads`, defined centrally in the metrics implementation; it is
unavailable when there are no basecalled reads. Other HTML conversions only
format units, round display labels, sort groups, or scale SVG coordinates.

Cumulative time plots step at each occupied bin's end using its existing
`cumulative_yield_bp`. They hold constant across omitted intervals. Interval
charts preserve actual start/end coordinates so gaps remain visible. The ranked
channel plot displays one existing channel yield per point, with descending
yield and ascending channel ID for ties. It carries no physical coordinate
information. Histograms retain the original categorical interval boundaries;
no per-read distribution is inferred. All table counts and bp yields are exact
integers; unknown floating statistics display **Not available**.


## Additive Deep QC diagnostics (v0.3.0)

Schema version remains **1**: no previous fields, definitions, or array ordering
are removed or changed. Consumers should ignore unfamiliar fields. The additive
`diagnostics` object contains the following independently ordered groups. It has
no complete read records and does not imply automatic run-level QC judgments.
The later explicit MinION extension includes a bounded numeric visualization sample.
The v0.4.0 additions follow the same contract: tolerant consumers can continue
reading old fields, but strict consumers rejecting unknown keys (including keys
inside `channels`/`time_bins` entries) must update. No incompatible field change
requires a schema version increment in this milestone.

### Explicit physical layout

`physical_layout` is `null` unless `--flowcell-layout promethion-3000` is selected.
The selected object has `name`, `rows: 25`, `columns: 120`, and
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

### Channel × mux

`channel_mux` entries are ordered by ascending `(channel, mux)` and contain those
identifiers plus `total_records`, `basecalled_reads`, `zero_length_records`,
`total_yield_bp`, `passed_reads`, and `passed_yield_bp`. Only observed pairs occur;
no four-mux assumption is made. Zero-length records contribute only record/zero
counts. Group yields and passed yields sum to the respective run totals; group
records sum to all records. Counts/yield do not establish pore occupancy. HTML
provides a collapsible table sharing channel search, sorting, and pagination.

### Sequencing speed proxy

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

## Advanced read-quality diagnostics (v0.4.0)

### Exact passed-read statistics and storage

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
length/Q-score pairs. Their sums equal `basecalled_reads` and `passed_reads`.

HTML positions finite length bins logarithmically (base 2) and Q-score bins
linearly. Dashed terminal column/row boundaries identify open-ended overflow
cells; their widths do not represent an infinite numeric interval. Both maps
share a sequential indigo scale normalized by `log10(1 + cell_count)` against the
all-read maximum. Accessible SVG cell labels/tooltips and a nonzero-cell table
retain integer counts. Histogram boundaries impose no filtering threshold.

### Distribution of channel mean lengths

`diagnostics.channel_read_length_distribution` uses the existing nine read-length
interval boundaries. Each entry has `lower_inclusive_bp`, `upper_exclusive_bp`,
`channel_count`, and `passed_channel_count`. The first count bins channel all-read
means; the second bins channel passed-read means. Each eligible channel counts
once, without read-count weighting. Channels without the corresponding reads are
excluded. These are channel-mean distributions, not per-read length distributions.
HTML shows the all-read distribution and both means in the existing channel table.

### Validation denominators

Summary records include zero lengths; basecalled reads exclude them; passed reads
add the native TRUE flag. An externally defined Q ≥ 9 subset is distinct from
native passing. The maintainer's real-run reference matched these subset totals
for that run only. Different zero-length denominators and rounded external Gb
outputs must be reconciled before comparing means, medians, and yields. No
original records or private identifiers are used in public regression fixtures.

## MinIONQC parity comparisons

The additive top-level `comparison` object retains schema version **1**. Existing
fields are unchanged for previously accepted inputs. Consumers must tolerate new
fields; strict consumers that reject unknown properties must update. No complete per-read
records are serialized; explicit MinION reports may include the documented bounded
numeric visualization sample. Source behavior was reviewed in [MinIONQC.R at
54485d5](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R)
(`add_cols`, `summary.stats`, `binSearch`, `channel.summary`, plotting functions),
its README/options, public YAML summaries and example plots.

### Populations and cutoff

- Summary records: all valid input rows, including 0 bp.
- Basecalled reads: length >0, unchanged throughout existing fields.
- Native passed: basecalled reads with `passes_filtering=TRUE`; FALSE is native
  failed. No Q threshold is inferred from these flags.
- Q-score selected: basecalled reads with `mean_qscore_template >= qscore_cutoff`.
  CLI `-q/--qscore-cutoff` defaults to **7**, must be finite and nonnegative, and
  includes exact boundary matches. Zero lengths are excluded even at cutoff 0.

`comparison.qscore_selected` has the same exact length/quality statistic fields
as `diagnostics.passed_read_statistics`: `read_count`, `total_yield_bp`, mean/median
length, N50, minimum/maximum length, mean/median Q-score. These use the established
PoreGauge N50 (cumulative bases **>= half**) and arithmetic per-read Q-score.
Missing statistics are null; empty counts/yields are real zeros.
`length_histogram` and `quality_histogram` reuse the existing documented boundaries.
`time_bins` has one entry per occupied original 900-second interval, with
`start_seconds`, selected `basecalled_reads`, `yield_bp`, `cumulative_yield_bp`,
`mean_read_length_bp`, `mean_qscore`. Means use selected read counts and are null
in zero-observation bins; omitted intervals contain no records. No smoothing or
basecalling-completion time is inferred.

`passes_filtering` is now optional as a small prerequisite for #5; other required
columns remain required. If absent, `native_filtering_available=false` and
`native_unclassified_reads` counts basecalled reads without a flag. Native count
and yield fields count only observed TRUE/FALSE values (both zero in this case),
not inferred outcomes. Native means/medians and termination pass fractions are
null; HTML labels native outcomes unavailable. The general count invariant is
`basecalled_reads = passed_reads + failed_reads + native_unclassified_reads`.
This is not complete legacy-format support (#9).

### Exact retained yield and ultralong

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
retain PoreGauge's existing fixed bins rather than upstream's automatic 300 bins.
Time means remain unsmoothed 15-minute observations rather than ggplot smoothing.
Unrounded values are retained in JSON, unlike upstream's one-decimal summaries.

### Public reference validation and memory

Run the optional standard-library check outside normal CI:

```sh
git clone https://github.com/roblanf/minion_qc /tmp/minion-qc-reference
git -C /tmp/minion-qc-reference checkout 54485d586038d8ee36f168dd94f31c423f52f581
python3 tests/minionqc_reference.py /tmp/minion-qc-reference
python3 tests/parity_memory.py  # 1 million fictional reads, 3,000 channels
```

The optional validator reads the original pinned public examples directly, without
fabricated fields or Q-score clipping. Results, control exclusions and fixture
caveats are listed under [Legacy schemas and MinION](#legacy-schemas-and-minion-9).
It checks exact integers, per-channel medians and independent pooled calculations;
upstream rounded Gb values have a 50.1 bp tolerance. This fast check does not execute
R; the later limited native R short-run comparison is recorded in the parity matrix.
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

## Multi-run reports (#8)

Directory/multiple-file analysis routes each record by actual `run_id` in one
streaming pass. Ordinary single-file analysis retains strict run-ID validation;
`--split-runs` explicitly enables splitting a mixed-run file. Run IDs can recur
across files (chunks); flowcell IDs must be consistent within a run. Distinct run
IDs do not necessarily identify distinct physical flowcells. Missing identity is
reported as a separate source unless explicitly grouped. `--group-inputs` cannot
merge conflicting known IDs. Output paths use deterministic ordinal identifiers,
never sample or instrument identifiers. Canonical/inode deduplication covers path
aliases/hard links, not duplicate read IDs or separately copied overlapping chunks.

`collection.runs` contains compact per-run statistics/distributions/timelines and
source metadata; top-level metrics are exact pooled statistics. Per-read numeric
buffers are moved into the existing aggregator, never inferred from histograms.
Pooled channel/mux/time arrays are deliberately empty: overlapping local IDs and
relative clocks have no meaningful pooled interpretation. The HTML distinguishes
pooled statistics from per-run comparisons; `--combined-only` skips all individual
outputs. Publication remains per file, not atomic across a collection.

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

## Legacy schemas and MinION (#9)

`--input-schema auto` uses the modern schema when all eight original modern
columns and template representation are present; otherwise it selects legacy.
`--input-schema modern` enforces the original required columns and nonnegative Q;
`legacy` permits missing run/mux/duration/native/reason metadata. Any provided
numeric metadata is still validated. Legacy blanks/NA in optional metadata are
unavailable; selected length/Q and channel/start remain required. Missing muxes
and reasons create no invented groups. A partially missing native flag makes
native completeness false while known TRUE/FALSE counts remain observed counts.

Auto representation chooses the full `sequence_length_2d`/`mean_qscore_2d` pair
when either is present (both are required). `--read-representation template|2d`
makes the choice explicit. There is no row-level fallback; incompatible
representations within a run are rejected. Each collection member records its
representation; pooled comparisons may explicitly contain different run types.
Template duration, template native flags and template event counts are unavailable
for 2D reads. Finite signed legacy Q values are retained without clipping; when
needed, an explicit [-f64::MAX,0) quality underflow interval and joint-histogram
row are prepended. Nonnegative modern histogram boundaries are unchanged. Signed
means/medians use overflow-safe arithmetic; time plots support signed axes.

`input` records raw validated row counts, exact `YHR174W` control exclusions,
negative-Q record counts, schemas and chosen representation. Control rows are
validated and excluded before QC aggregation; excluded rows do not contribute
zero-length/basecalled/filtering metrics. This reproduces MinIONQC's named-control
policy, not a universal RNA filtering recommendation. Source rows with malformed
required values still fail, unlike upstream's silent malformed-row removal.

The explicit `minion-r9.5` map is the exact upstream p1..p8/q1..q8 construction,
zero-based, 32 rows ×16 columns. Left block starts: 33,481,417,353,289,225,161,97;
right block starts: 1,449,385,321,257,193,129,65. Each block contains 32 channels,
4 rows ×8 columns; right columns are reversed. All 512 coordinates and block
edges are tested. No layout is inferred from channel maxima. Total/native/Q-cutoff
spatial yields remain distinct; absent observations differ from observed zero.

MinION-only scatter and physical time/length panels use deterministic SplitMix64
reservoir replacement, at most 16 positive-length reads per channel (8,192 total).
Sampling is channel-stratified, not proportional to run read density. Coordinates
are actual sampled values; overview x is relative start time, y is log10 length,
color is per-read Q. Scatter x is logarithmic length, y is Q; optional event color
uses log10(1+events/length), with missing events gray. This intentionally differs
from upstream's full scatter and log(events/base) color transform. JSON includes
bounded numeric sample points without read identifiers; no complete record objects
are retained. Sampling is disabled for `--combined-only`; pooled reports retain
aggregate histograms, not spatial/scatter overlays across distinct instruments.

Optional public validation now reads original files without adapters or Q clipping:
99 short-run records / 1,089,633 bp; 75,160 PromethION / 623,196,506 bp;
159,799 1D2 / 930,249,488 bp; 963 retained direct-RNA /197,413 bp (37 controls
excluded); two MinION inputs pooled: 608,208 /7,203,301,321 bp, N50 33,161,
median 2,616. Inputs can contain several run IDs; they are split rather than
pretended to be a single physical flowcell. Threshold/ultralong counts match
available upstream YAML and independent calculations. Short/1D2 YAML labels
Q>=7 are stale: numeric values match Q8, as used by upstream tests.R. R was not
executed; the historical private PromethION dataset was not rerun.

## Output controls and YAML (#10)

`presentation` is additive schema-version-1 configuration: `write_yaml`,
`export_svg`, `small_figures`, `add_stat`, optional `mux_interval_hours`, and
centrally prepared `mux_markers_seconds`. It never changes QC populations or
histogram definitions. Existing consumers must tolerate additive fields.

`--yaml` emits `summary.yaml`, using quoted YAML mappings and JSON-compatible
scalars (YAML 1.2), without a new runtime dependency. Sections `All reads` and
`Q>=<cutoff>` project canonical exact statistics. They contain `total.reads`,
`total.bases` (exact integer extension), `total.gigabases`, `N50.length`,
`mean.length`, `median.length`, `max.length`, `mean.q`, `median.q`, `reads`, and
`gigabases`. Threshold labels match upstream (`>10kb` through `>1m`) but selection
is **inclusive**, as in its code. `ultralong` is the existing maximal prefix.
Unavailable statistics are null; values are unrounded, unlike upstream's
one-decimal summaries. Input identification uses generated source labels and
run IDs, not original paths. Raw summary/control counts and notes make the
positive-length All-read denominator explicit. Native passing remains separate.

Configured mux references use relative hours, starting at zero, through the
final occupied interval's end. Zero disables references (default); positive
intervals producing more than 512 markers fail actionably. References share the
same axes in individual/comparison charts. They are not telemetry, occupancy or
inferred mux switching. Cumulative comparison points sit at bin ends; mean/count
observations sit at midpoints. Missing intervals break mean lines.

`--add-stat` places exact-model mean length/N50 and mean Q annotations in the
corresponding histogram SVGs. Display rounding/unit conversion is presentation
only. `--format svg` exports every nonempty scientific SVG, with embedded styles,
internal accessible titles and a current-file manifest. There is no external
renderer or font dependency. Width is 180 mm, or 120 mm with `--smallfigures`;
wide physical figures retain 320 mm. Height preserves aspect ratio. Empty figures
stay explanatory states in HTML, rather than meaningless SVG files. Old SVGs not
listed in a new manifest are retained, not silently removed.

JSON, HTML and optional artifacts are all staged before per-file replacement;
there is no atomic transaction across outputs or runs. Predictable failures clean
staged files and preserve prior valid reports. PNG/PDF/PS/JPEG/TIFF/BMP conversions
are not implemented. Browser printing does not establish figure-format parity.
See [the parity matrix](minionqc-parity.md) for the full audit and tests.

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
