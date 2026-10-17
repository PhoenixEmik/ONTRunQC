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

Required invariants:

```text
total_records = basecalled_reads + zero_length_records
basecalled_reads = passed_reads + failed_reads
total_yield_bp = passed_yield_bp + failed_yield_bp
```

Zero-length rows contribute to record counts and grouped record counts, including
time bins. They contribute neither basecalled/pass/fail read counts nor quality
observations. A positive-length read with Q0 is a valid quality observation.
Filtering is taken solely from `passes_filtering`, without a Q7/Q10/Q20 cutoff or
an inference from `end_reason`.

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

Lengths/Q-scores are partitioned by native pass/fail status, sorted in place,
and merged with constant lookahead for global medians/N50. Each read still
contributes one `u64` length and one `f64` Q-score: 16 bytes of elements, not
multiple independent copies for all/passed. Four fixed-growth buffers reserve
at most 4 × 65,536 × 8 bytes (2 MiB) spare element capacity. This is up to 1 MiB
more than the former two-buffer bound, independent of record count. A synthetic
131,075-read test checks retained counts/capacity; this does not measure RSS.
Existing streaming global Q-score means keep their input-order arithmetic.

Per-channel exact medians are deliberately deferred: a straightforward additional
per-channel length collection costs 8 bytes/read (about 233 MiB for 30.55 million
reads), in addition to the roughly 466 MiB retained core elements. Retaining
channel identity through ordering also needs extra storage or complexity.
No approximate per-channel median is substituted.

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
