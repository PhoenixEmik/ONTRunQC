# Metrics and JSON schema (version 1)

All metrics are calculated centrally in `src/metrics.rs`. Input rows must pass
validation before aggregation. Counts and yields are unsigned 64-bit integers;
overflow is an error, never wrapping arithmetic. Statistics use IEEE-754 `f64`;
floating input must be finite and nonnegative. There are no NaN/Infinity values
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

Channel means use the same arithmetic definition and zero-length exclusion as
the run mean. A channel without basecalled reads has `mean_qscore: null`.
Unknown termination strings are preserved verbatim and grouped by exact string.
Channels and muxes are identifiers, with no physical coordinate assumptions.

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

## Report envelope and ordering

Top-level fields are `schema_version` (1), `tool` (`name: "PoreGauge"`, package
`version`), `run` (`run_id`), `metrics`, `quality`, `read_length_distribution`,
`channels`, `muxes`, `time_bins`, and `end_reasons`.

Channel and mux arrays use ascending numeric order, time bins ascending start
seconds, termination reasons ascending Rust string order, and histogram bins
ascending lower bounds. JSON fields follow the declared schema order. There are
no timestamps, input paths, random identifiers, or individual records in reports.
Pretty-printed JSON ends with a newline.

For header-only input, `run.run_id` is `null`, counts/yields are zero, statistics
are `null`, grouped arrays are empty, and fixed histograms contain zero counts.
A run containing only zero-length records retains its run ID and grouped record
counts but has unavailable length/quality statistics.

## HTML presentation (v0.2.0)

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
