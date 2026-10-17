# Metrics and JSON schema (version 1)

All metrics are calculated centrally in `src/metrics.rs`. Input rows must pass
validation before aggregation. Counts and yields are unsigned 64-bit integers;
overflow is an error, never wrapping arithmetic. Statistics use IEEE-754 `f64`;
required floating input must be finite and nonnegative. There are no NaN/Infinity values
in a valid report. Unavailable statistics are JSON `null`.

## Overall sequencing metrics

| Field under `metrics` | Definition |
| --- | --- |
| `total_records` | All validated data rows, excluding the header. |
| `basecalled_reads` | Rows with `sequence_length_template > 0`. |
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
no individual-read data and does not imply automatic run-level QC judgments.
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
fields; strict consumers that reject unknown properties must update. No per-read
records are serialized. Source behavior was reviewed in [MinIONQC.R at
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

Executed against the pinned public examples: short-run **99 records / 1,089,633
bp** and PromethION **75,160 / 623,196,506 bp**. Exact threshold counts and
ultralong counts agree with upstream YAML; rounded Gb values agree within 50.1 bp
(the precision of stored decimal Gb). Channel medians and subset statistics also
pass independent Python reference calculations. R itself was **not executed**.
No upstream read records or identifying reports are committed; the optional
adapter replaces run IDs and supplies neutral missing mux/reason fields.

Two upstream fixture caveats are explicit: short-run's YAML says Q>=7 but its
values match **Q>=8** (upstream `tests.R` calls `single.flowcell(...,8)` while the
global title remains Q>=7). The checker runs Q8 for that comparison. PromethION
contains three negative Q-score values (-0.100058, -0.106873, -0.122276), all on
5 bp reads. Production parsing still rejects negative Q-scores. Only the
validation adapter replaces these with Q0, preserving length/yield/channel values
and Q>=7 membership; adapted all-read Q statistics are **not** claimed to match.
These cases do not justify weakening input validation or silently copying labels.

Lengths and Q-scores still consume **16 bytes per basecalled read** in elements.
Lengths are stored once in four disjoint native/Q partitions per channel; Q-scores
once in four global partitions, without per-channel Q arrays or full read objects.
Channel medians are calculated before buffers are moved/reused for run statistics.
Spare channel length capacity is <4×256×8 bytes per channel (~23.4 MiB/3,000
channels), global Q spare capacity <=2 MiB; final global length spare capacity
<=2 MiB. Moving a channel partition can transiently duplicate that partition;
very skewed channels can increase the peak. Aggregated maps/output also consume
memory. No <512 MiB claim is made for 30 million records.

On this macOS environment the focused 1,000,000-record / 3,000-channel synthetic
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
