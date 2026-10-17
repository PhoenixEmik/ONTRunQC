use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    layout::{FlowcellLayout, PhysicalLayout},
    record::Record,
    report::{Report, Run, Tool},
};

const TIME_BIN_SECONDS: u64 = 900;
const LENGTH_BOUNDS: [u64; 9] = [
    1, 1_000, 5_000, 10_000, 25_000, 50_000, 100_000, 250_000, 1_000_000,
];
const QUALITY_BOUNDS: [f64; 9] = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0];
const SPEED_BOUNDS: [f64; 9] = [0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 800.0, 1000.0];
const JOINT_LENGTH_BOUNDS: [u64; 21] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072,
    262144, 524288, 1048576,
];
const JOINT_QUALITY_BOUNDS: [f64; 26] = [
    0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 30.0,
    32.0, 34.0, 36.0, 38.0, 40.0, 42.0, 44.0, 46.0, 48.0, 50.0,
];
// Bound spare capacity; each observation belongs to exactly one pass/fail partition.
const RESERVE_ELEMENTS: usize = 65_536;

#[derive(Default, Debug, Serialize)]
pub struct OverallMetrics {
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub zero_length_records: u64,
    pub total_yield_bp: u64,
    pub passed_reads: u64,
    pub failed_reads: u64,
    pub passed_yield_bp: u64,
    pub failed_yield_bp: u64,
    pub mean_read_length_bp: Option<f64>,
    pub median_read_length_bp: Option<f64>,
    pub n50_bp: Option<u64>,
    pub min_read_length_bp: Option<u64>,
    pub max_read_length_bp: Option<u64>,
}

impl OverallMetrics {
    /// Percentage of basecalled reads passing the input's native filtering flag.
    /// Kept here so renderers do not introduce a separate QC definition.
    pub fn pass_rate_percent(&self) -> Option<f64> {
        (self.basecalled_reads != 0)
            .then(|| 100.0 * self.passed_reads as f64 / self.basecalled_reads as f64)
    }
}

#[derive(Debug, Serialize)]
pub struct QualityMetrics {
    pub mean_qscore: Option<f64>,
    pub median_qscore: Option<f64>,
    pub min_qscore: Option<f64>,
    pub max_qscore: Option<f64>,
    pub histogram: Vec<QualityBin>,
}

#[derive(Debug, Serialize)]
pub struct QualityBin {
    pub lower_inclusive: f64,
    pub upper_exclusive: Option<f64>,
    pub read_count: u64,
}

#[derive(Debug, Serialize)]
pub struct LengthBin {
    pub lower_inclusive_bp: u64,
    pub upper_exclusive_bp: Option<u64>,
    pub read_count: u64,
    pub yield_bp: u64,
}

#[derive(Default, Debug, Serialize)]
pub struct ChannelMetrics {
    pub channel: u32,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub total_yield_bp: u64,
    pub passed_reads: u64,
    pub passed_yield_bp: u64,
    pub mean_qscore: Option<f64>,
    pub mean_read_length_bp: Option<f64>,
    pub mean_passed_read_length_bp: Option<f64>,
}

#[derive(Default, Debug, Serialize)]
pub struct MuxMetrics {
    pub mux: u32,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub total_yield_bp: u64,
    pub passed_yield_bp: u64,
}

#[derive(Default, Debug, Serialize)]
pub struct TimeBin {
    pub start_seconds: u64,
    pub end_seconds: u64,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub yield_bp: u64,
    pub cumulative_yield_bp: u64,
    pub passed_reads: u64,
    pub passed_yield_bp: u64,
    pub mean_read_length_bp: Option<f64>,
    pub mean_passed_read_length_bp: Option<f64>,
    pub qscore_observations: u64,
    pub mean_qscore: Option<f64>,
    pub mean_passed_qscore: Option<f64>,
}

#[derive(Default, Debug, Serialize)]
pub struct EndReasonMetrics {
    pub end_reason: String,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub total_yield_bp: u64,
}

#[derive(Debug, Serialize)]
pub struct Diagnostics {
    pub physical_layout: Option<PhysicalLayout>,
    pub channel_activity: Vec<ChannelActivity>,
    pub channel_mux: Vec<ChannelMuxMetrics>,
    pub sequencing_speed: Option<SequencingSpeed>,
    pub termination_reasons: Vec<TerminationDiagnostics>,
    pub passed_read_statistics: PassedReadStatistics,
    pub length_quality_histogram: LengthQualityHistogram,
    pub channel_read_length_distribution: Vec<ChannelLengthBin>,
}

#[derive(Debug, Serialize)]
pub struct PassedReadStatistics {
    pub read_count: u64,
    pub total_yield_bp: u64,
    pub mean_read_length_bp: Option<f64>,
    pub median_read_length_bp: Option<f64>,
    pub n50_bp: Option<u64>,
    pub min_read_length_bp: Option<u64>,
    pub max_read_length_bp: Option<u64>,
    pub mean_qscore: Option<f64>,
    pub median_qscore: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct LengthQualityHistogram {
    pub length_lower_bounds_bp: Vec<u64>,
    pub qscore_lower_bounds: Vec<f64>,
    // Rows are ascending Q-score bins; columns are ascending read-length bins.
    // The final interval on each axis is unbounded.
    pub all_read_counts: Vec<Vec<u64>>,
    pub passed_read_counts: Vec<Vec<u64>>,
}

#[derive(Debug, Serialize)]
pub struct ChannelLengthBin {
    pub lower_inclusive_bp: u64,
    pub upper_exclusive_bp: Option<u64>,
    pub channel_count: u64,
    pub passed_channel_count: u64,
}

#[derive(Debug, Serialize)]
pub struct ChannelActivity {
    pub start_seconds: u64,
    pub basecalled_producing_channels: u64,
}

#[derive(Default, Debug, Serialize)]
pub struct ChannelMuxMetrics {
    pub channel: u32,
    pub mux: u32,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub zero_length_records: u64,
    pub total_yield_bp: u64,
    pub passed_reads: u64,
    pub passed_yield_bp: u64,
}

#[derive(Default, Debug, Serialize)]
pub struct SpeedSummary {
    pub eligible_reads: u64,
    pub mean_bp_per_second: Option<f64>,
}

impl SpeedSummary {
    fn add(&mut self, speed: f64) {
        self.eligible_reads += 1;
        update_mean(
            self.mean_bp_per_second.get_or_insert(0.0),
            speed,
            self.eligible_reads,
        );
    }
}

#[derive(Debug, Serialize)]
pub struct SequencingSpeed {
    #[serde(flatten)]
    pub summary: SpeedSummary,
    pub histogram: Vec<SpeedBin>,
    pub time_bins: Vec<SpeedTimeBin>,
}

#[derive(Debug, Serialize)]
pub struct SpeedBin {
    pub lower_inclusive_bp_per_second: f64,
    pub upper_exclusive_bp_per_second: Option<f64>,
    pub read_count: u64,
}

#[derive(Debug, Serialize)]
pub struct SpeedTimeBin {
    pub start_seconds: u64,
    #[serde(flatten)]
    pub summary: SpeedSummary,
}

#[derive(Debug, Serialize)]
pub struct TerminationDiagnostics {
    pub end_reason: String,
    pub passed_reads: u64,
    pub zero_length_records: u64,
    pub summary_record_percent: Option<f64>,
    pub zero_length_fraction: Option<f64>,
    pub basecalled_read_fraction: Option<f64>,
    pub passed_fraction_of_basecalled: Option<f64>,
    pub total_yield_percent: Option<f64>,
}

#[derive(Default)]
struct TimeAccumulator {
    metrics: TimeBin,
    channels: BTreeSet<u32>,
    speed: SpeedSummary,
}

#[derive(Default)]
struct ReasonAccumulator {
    metrics: EndReasonMetrics,
    passed_reads: u64,
}

#[derive(Default)]
pub struct Aggregator {
    overall: OverallMetrics,
    passed_lengths: Vec<u64>,
    failed_lengths: Vec<u64>,
    passed_qscores: Vec<f64>,
    failed_qscores: Vec<f64>,
    mean_qscore: f64,
    passed_mean_qscore: f64,
    quality_counts: [u64; QUALITY_BOUNDS.len()],
    length_counts: [u64; LENGTH_BOUNDS.len()],
    length_yields: [u64; LENGTH_BOUNDS.len()],
    channels: BTreeMap<u32, ChannelMetrics>,
    muxes: BTreeMap<u32, MuxMetrics>,
    time_bins: BTreeMap<u64, TimeAccumulator>,
    end_reasons: BTreeMap<String, ReasonAccumulator>,
    channel_mux: BTreeMap<(u32, u32), ChannelMuxMetrics>,
    layout: Option<FlowcellLayout>,
    speed_available: bool,
    speed: SpeedSummary,
    speed_counts: [u64; SPEED_BOUNDS.len()],
    joint_counts: [[u64; JOINT_LENGTH_BOUNDS.len()]; JOINT_QUALITY_BOUNDS.len()],
    joint_passed_counts: [[u64; JOINT_LENGTH_BOUNDS.len()]; JOINT_QUALITY_BOUNDS.len()],
}

impl Aggregator {
    pub fn new(layout: Option<FlowcellLayout>, speed_available: bool) -> Self {
        Self {
            layout,
            speed_available,
            ..Self::default()
        }
    }

    pub fn add(&mut self, record: &Record<'_>) -> Result<()> {
        if let Some(layout) = self.layout {
            ensure!(
                layout.coordinate(record.channel).is_some(),
                "channel {} is incompatible with --flowcell-layout promethion-3000 (expected 1–3000); select a matching layout or omit the option",
                record.channel
            );
        }
        // Validate cumulative counters first; every subgroup is bounded by these totals.
        self.overall.total_records = self
            .overall
            .total_records
            .checked_add(1)
            .context("total record count exceeds u64 capacity")?;
        self.overall.total_yield_bp = self
            .overall
            .total_yield_bp
            .checked_add(record.length)
            .context("total yield exceeds u64 capacity")?;
        let index = (record.start_time / TIME_BIN_SECONDS as f64).floor();
        ensure!(
            index < u64::MAX as f64,
            "start_time is too large for a time bin"
        );
        let start_seconds = (index as u64)
            .checked_mul(TIME_BIN_SECONDS)
            .context("start_time is too large for a time bin")?;
        let end_seconds = start_seconds
            .checked_add(TIME_BIN_SECONDS)
            .context("start_time is too large for a time bin")?;

        let channel = self
            .channels
            .entry(record.channel)
            .or_insert_with(|| ChannelMetrics {
                channel: record.channel,
                ..ChannelMetrics::default()
            });
        let mux = self.muxes.entry(record.mux).or_insert_with(|| MuxMetrics {
            mux: record.mux,
            ..MuxMetrics::default()
        });
        let time = self
            .time_bins
            .entry(start_seconds)
            .or_insert_with(|| TimeAccumulator {
                metrics: TimeBin {
                    start_seconds,
                    end_seconds,
                    ..TimeBin::default()
                },
                ..TimeAccumulator::default()
            });
        let channel_mux = self
            .channel_mux
            .entry((record.channel, record.mux))
            .or_insert_with(|| ChannelMuxMetrics {
                channel: record.channel,
                mux: record.mux,
                ..ChannelMuxMetrics::default()
            });
        // Borrowed lookup avoids a fresh String for every record.
        if !self.end_reasons.contains_key(record.end_reason) {
            self.end_reasons
                .insert(record.end_reason.to_owned(), ReasonAccumulator::default());
        }
        let reason = self.end_reasons.get_mut(record.end_reason).unwrap();
        channel.total_records += 1;
        mux.total_records += 1;
        time.metrics.total_records += 1;
        reason.metrics.total_records += 1;
        channel_mux.total_records += 1;
        if record.length == 0 {
            self.overall.zero_length_records += 1;
            channel_mux.zero_length_records += 1;
            return Ok(());
        }

        let (lengths, qscores) = if record.passed {
            (&mut self.passed_lengths, &mut self.passed_qscores)
        } else {
            (&mut self.failed_lengths, &mut self.failed_qscores)
        };
        push_compact(lengths, record.length)?;
        push_compact(qscores, record.qscore)?;
        self.overall.basecalled_reads += 1;
        update_mean(
            &mut self.mean_qscore,
            record.qscore,
            self.overall.basecalled_reads,
        );
        channel.basecalled_reads += 1;
        channel.total_yield_bp += record.length;
        let channel_mean = channel.mean_qscore.get_or_insert(0.0);
        update_mean(channel_mean, record.qscore, channel.basecalled_reads);
        mux.basecalled_reads += 1;
        mux.total_yield_bp += record.length;
        time.metrics.basecalled_reads += 1;
        time.metrics.yield_bp += record.length;
        time.metrics.qscore_observations += 1;
        update_mean(
            time.metrics.mean_qscore.get_or_insert(0.0),
            record.qscore,
            time.metrics.qscore_observations,
        );
        time.channels.insert(record.channel);
        reason.metrics.basecalled_reads += 1;
        reason.metrics.total_yield_bp += record.length;
        channel_mux.basecalled_reads += 1;
        channel_mux.total_yield_bp += record.length;
        if let Some(speed) = record
            .template_duration
            .map(|duration| record.length as f64 / duration)
            .filter(|speed| speed.is_finite())
        {
            self.speed.add(speed);
            time.speed.add(speed);
            let bin = SPEED_BOUNDS.partition_point(|bound| *bound <= speed) - 1;
            self.speed_counts[bin] += 1;
        }
        if record.passed {
            self.overall.passed_reads += 1;
            self.overall.passed_yield_bp += record.length;
            update_mean(
                &mut self.passed_mean_qscore,
                record.qscore,
                self.overall.passed_reads,
            );
            channel.passed_reads += 1;
            channel.passed_yield_bp += record.length;
            mux.passed_yield_bp += record.length;
            channel_mux.passed_reads += 1;
            channel_mux.passed_yield_bp += record.length;
            reason.passed_reads += 1;
            time.metrics.passed_reads += 1;
            time.metrics.passed_yield_bp += record.length;
            update_mean(
                time.metrics.mean_passed_qscore.get_or_insert(0.0),
                record.qscore,
                time.metrics.passed_reads,
            );
        } else {
            self.overall.failed_reads += 1;
            self.overall.failed_yield_bp += record.length;
        }
        let length_bin = LENGTH_BOUNDS.partition_point(|bound| *bound <= record.length) - 1;
        self.length_counts[length_bin] += 1;
        self.length_yields[length_bin] += record.length;
        let quality_bin = QUALITY_BOUNDS.partition_point(|bound| *bound <= record.qscore) - 1;
        self.quality_counts[quality_bin] += 1;
        let joint_length = JOINT_LENGTH_BOUNDS.partition_point(|bound| *bound <= record.length) - 1;
        let joint_quality =
            JOINT_QUALITY_BOUNDS.partition_point(|bound| *bound <= record.qscore) - 1;
        self.joint_counts[joint_quality][joint_length] += 1;
        if record.passed {
            self.joint_passed_counts[joint_quality][joint_length] += 1;
        }
        Ok(())
    }

    pub fn finish(mut self, run_id: Option<String>) -> Report {
        self.passed_lengths.sort_unstable();
        self.failed_lengths.sort_unstable();
        self.passed_qscores.sort_unstable_by(f64::total_cmp);
        self.failed_qscores.sort_unstable_by(f64::total_cmp);
        if self.overall.basecalled_reads != 0 {
            self.overall.mean_read_length_bp =
                Some(self.overall.total_yield_bp as f64 / self.overall.basecalled_reads as f64);
            self.overall.median_read_length_bp =
                merged_median_lengths(&self.passed_lengths, &self.failed_lengths);
            self.overall.min_read_length_bp = self
                .passed_lengths
                .first()
                .into_iter()
                .chain(self.failed_lengths.first())
                .min()
                .copied();
            self.overall.max_read_length_bp = self
                .passed_lengths
                .last()
                .into_iter()
                .chain(self.failed_lengths.last())
                .max()
                .copied();
            self.overall.n50_bp = n50_descending(
                merge_sorted(
                    self.passed_lengths.iter().rev().copied(),
                    self.failed_lengths.iter().rev().copied(),
                    |a, b| b.cmp(a),
                ),
                self.overall.total_yield_bp,
            );
        }
        let passed_read_statistics = PassedReadStatistics {
            read_count: self.overall.passed_reads,
            total_yield_bp: self.overall.passed_yield_bp,
            mean_read_length_bp: fraction(self.overall.passed_yield_bp, self.overall.passed_reads),
            median_read_length_bp: median_lengths(&self.passed_lengths),
            n50_bp: n50(&self.passed_lengths, self.overall.passed_yield_bp),
            min_read_length_bp: self.passed_lengths.first().copied(),
            max_read_length_bp: self.passed_lengths.last().copied(),
            mean_qscore: (self.overall.passed_reads != 0).then_some(self.passed_mean_qscore),
            median_qscore: median_quality(&self.passed_qscores),
        };
        let quality = QualityMetrics {
            mean_qscore: (self.overall.basecalled_reads != 0).then_some(self.mean_qscore),
            median_qscore: merged_median_quality(&self.passed_qscores, &self.failed_qscores),
            min_qscore: self
                .passed_qscores
                .first()
                .into_iter()
                .chain(self.failed_qscores.first())
                .copied()
                .min_by(f64::total_cmp),
            max_qscore: self
                .passed_qscores
                .last()
                .into_iter()
                .chain(self.failed_qscores.last())
                .copied()
                .max_by(f64::total_cmp),
            histogram: QUALITY_BOUNDS
                .iter()
                .enumerate()
                .map(|(index, &lower)| QualityBin {
                    lower_inclusive: lower,
                    upper_exclusive: QUALITY_BOUNDS.get(index + 1).copied(),
                    read_count: self.quality_counts[index],
                })
                .collect(),
        };
        let distribution = LENGTH_BOUNDS
            .iter()
            .enumerate()
            .map(|(index, &lower)| LengthBin {
                lower_inclusive_bp: lower,
                upper_exclusive_bp: LENGTH_BOUNDS.get(index + 1).copied(),
                read_count: self.length_counts[index],
                yield_bp: self.length_yields[index],
            })
            .collect();
        let mut cumulative = 0_u64;
        for time in self.time_bins.values_mut() {
            cumulative += time.metrics.yield_bp;
            time.metrics.cumulative_yield_bp = cumulative;
            time.metrics.mean_read_length_bp =
                fraction(time.metrics.yield_bp, time.metrics.basecalled_reads);
            time.metrics.mean_passed_read_length_bp =
                fraction(time.metrics.passed_yield_bp, time.metrics.passed_reads);
        }
        let mut channel_length_counts = [0_u64; LENGTH_BOUNDS.len()];
        let mut passed_channel_length_counts = [0_u64; LENGTH_BOUNDS.len()];
        for channel in self.channels.values_mut() {
            channel.mean_read_length_bp =
                fraction(channel.total_yield_bp, channel.basecalled_reads);
            channel.mean_passed_read_length_bp =
                fraction(channel.passed_yield_bp, channel.passed_reads);
            for (mean, counts) in [
                (channel.mean_read_length_bp, &mut channel_length_counts),
                (
                    channel.mean_passed_read_length_bp,
                    &mut passed_channel_length_counts,
                ),
            ] {
                if let Some(mean) = mean {
                    let index = LENGTH_BOUNDS.partition_point(|bound| *bound as f64 <= mean) - 1;
                    counts[index] += 1;
                }
            }
        }
        let channel_activity = self
            .time_bins
            .values()
            .map(|time| ChannelActivity {
                start_seconds: time.metrics.start_seconds,
                basecalled_producing_channels: time.channels.len() as u64,
            })
            .collect();
        let sequencing_speed = self.speed_available.then(|| SequencingSpeed {
            summary: self.speed,
            histogram: SPEED_BOUNDS
                .iter()
                .enumerate()
                .map(|(i, &lower)| SpeedBin {
                    lower_inclusive_bp_per_second: lower,
                    upper_exclusive_bp_per_second: SPEED_BOUNDS.get(i + 1).copied(),
                    read_count: self.speed_counts[i],
                })
                .collect(),
            time_bins: self
                .time_bins
                .values()
                .map(|time| SpeedTimeBin {
                    start_seconds: time.metrics.start_seconds,
                    summary: SpeedSummary {
                        eligible_reads: time.speed.eligible_reads,
                        mean_bp_per_second: time.speed.mean_bp_per_second,
                    },
                })
                .collect(),
        });
        let mut termination_reasons: Vec<_> = self
            .end_reasons
            .iter()
            .map(|(name, reason)| {
                let m = &reason.metrics;
                let zero = m.total_records - m.basecalled_reads;
                TerminationDiagnostics {
                    end_reason: name.clone(),
                    passed_reads: reason.passed_reads,
                    zero_length_records: zero,
                    summary_record_percent: fraction(m.total_records, self.overall.total_records)
                        .map(|v| v * 100.0),
                    zero_length_fraction: fraction(zero, m.total_records),
                    basecalled_read_fraction: fraction(m.basecalled_reads, m.total_records),
                    passed_fraction_of_basecalled: fraction(
                        reason.passed_reads,
                        m.basecalled_reads,
                    ),
                    total_yield_percent: fraction(m.total_yield_bp, self.overall.total_yield_bp)
                        .map(|v| v * 100.0),
                }
            })
            .collect();
        termination_reasons.sort_by(|a, b| {
            self.end_reasons[&b.end_reason]
                .metrics
                .total_records
                .cmp(&self.end_reasons[&a.end_reason].metrics.total_records)
                .then_with(|| a.end_reason.cmp(&b.end_reason))
        });
        let end_reasons = self
            .end_reasons
            .into_iter()
            .map(|(name, reason)| EndReasonMetrics {
                end_reason: name,
                ..reason.metrics
            })
            .collect();
        let diagnostics = Diagnostics {
            physical_layout: self.layout.map(FlowcellLayout::description),
            channel_activity,
            channel_mux: self.channel_mux.into_values().collect(),
            sequencing_speed,
            termination_reasons,
            passed_read_statistics,
            length_quality_histogram: LengthQualityHistogram {
                length_lower_bounds_bp: JOINT_LENGTH_BOUNDS.to_vec(),
                qscore_lower_bounds: JOINT_QUALITY_BOUNDS.to_vec(),
                all_read_counts: self.joint_counts.iter().map(|row| row.to_vec()).collect(),
                passed_read_counts: self
                    .joint_passed_counts
                    .iter()
                    .map(|row| row.to_vec())
                    .collect(),
            },
            channel_read_length_distribution: LENGTH_BOUNDS
                .iter()
                .enumerate()
                .map(|(i, &lower)| ChannelLengthBin {
                    lower_inclusive_bp: lower,
                    upper_exclusive_bp: LENGTH_BOUNDS.get(i + 1).copied(),
                    channel_count: channel_length_counts[i],
                    passed_channel_count: passed_channel_length_counts[i],
                })
                .collect(),
        };
        Report {
            schema_version: 1,
            tool: Tool::current(),
            run: Run { run_id },
            metrics: self.overall,
            quality,
            read_length_distribution: distribution,
            channels: self.channels.into_values().collect(),
            muxes: self.muxes.into_values().collect(),
            time_bins: self
                .time_bins
                .into_values()
                .map(|time| time.metrics)
                .collect(),
            end_reasons,
            diagnostics,
        }
    }
}

fn fraction(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

fn push_compact<T>(values: &mut Vec<T>, value: T) -> Result<()> {
    if values.len() == values.capacity() {
        values
            .try_reserve_exact(RESERVE_ELEMENTS)
            .context("cannot allocate exact-statistics buffer")?;
    }
    values.push(value);
    Ok(())
}

// An online arithmetic mean avoids overflowing a sum of finite Q-scores.
fn update_mean(mean: &mut f64, value: f64, count: u64) {
    *mean += (value - *mean) / count as f64;
}

fn median_lengths(sorted: &[u64]) -> Option<f64> {
    merged_median_lengths(sorted, &[])
}

fn median_quality(sorted: &[f64]) -> Option<f64> {
    merged_median_quality(sorted, &[])
}

fn n50(sorted_lengths: &[u64], total_yield: u64) -> Option<u64> {
    n50_descending(sorted_lengths.iter().rev().copied(), total_yield)
}

// Merge already sorted partitions using only two lookahead values, never a
// combined per-read buffer. Used for exact global middle values and N50.
fn merge_sorted<T>(
    a: impl Iterator<Item = T>,
    b: impl Iterator<Item = T>,
    compare: impl Fn(&T, &T) -> Ordering,
) -> impl Iterator<Item = T> {
    let (mut a, mut b) = (a.peekable(), b.peekable());
    std::iter::from_fn(move || match (a.peek(), b.peek()) {
        (Some(left), Some(right)) if compare(left, right) != Ordering::Greater => a.next(),
        (Some(_), Some(_)) => b.next(),
        (Some(_), None) => a.next(),
        (None, Some(_)) => b.next(),
        (None, None) => None,
    })
}

fn middle_pair<T: Copy>(mut sorted: impl Iterator<Item = T>, len: usize) -> Option<(T, T)> {
    if len == 0 {
        return None;
    }
    let lower = sorted.nth((len - 1) / 2)?;
    let upper = if len % 2 == 0 { sorted.next()? } else { lower };
    Some((lower, upper))
}

fn merged_median_lengths(a: &[u64], b: &[u64]) -> Option<f64> {
    let values = merge_sorted(a.iter().copied(), b.iter().copied(), u64::cmp);
    let (lower, upper) = middle_pair(values, a.len() + b.len())?;
    // Widen before adding so even u64::MAX lengths cannot overflow.
    Some((u128::from(lower) + u128::from(upper)) as f64 / 2.0)
}

fn merged_median_quality(a: &[f64], b: &[f64]) -> Option<f64> {
    let values = merge_sorted(a.iter().copied(), b.iter().copied(), f64::total_cmp);
    let (lower, upper) = middle_pair(values, a.len() + b.len())?;
    Some(if (a.len() + b.len()) % 2 == 0 {
        lower + (upper - lower) / 2.0
    } else {
        lower
    })
}

fn n50_descending(lengths: impl Iterator<Item = u64>, total_yield: u64) -> Option<u64> {
    let target = total_yield.div_ceil(2);
    let mut cumulative = 0_u64;
    for length in lengths {
        cumulative += length;
        if cumulative >= target {
            return Some(length);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_filter_pass_rate_uses_basecalled_reads() {
        assert_eq!(OverallMetrics::default().pass_rate_percent(), None);
        let metrics = OverallMetrics {
            total_records: 6,
            basecalled_reads: 4,
            passed_reads: 2,
            ..OverallMetrics::default()
        };
        assert_eq!(metrics.pass_rate_percent(), Some(50.0));
    }

    #[test]
    fn exact_length_statistics() {
        assert_eq!(median_lengths(&[10, 20, 30, 40]), Some(25.0));
        assert_eq!(n50(&[10, 20, 30, 40], 100), Some(30));
        assert_eq!(median_lengths(&[10, 20, 90]), Some(20.0));
        assert_eq!(n50(&[1, 2, 4], 7), Some(4));
        assert_eq!(n50(&[1, 1, 2], 4), Some(2));
        assert_eq!(median_lengths(&[]), None);
        assert_eq!(n50(&[], 0), None);
    }

    #[test]
    fn medians_and_means_do_not_overflow() {
        assert_eq!(median_lengths(&[u64::MAX, u64::MAX]), Some(u64::MAX as f64));
        assert_eq!(median_quality(&[f64::MAX, f64::MAX]), Some(f64::MAX));
        let mut mean = 0.0;
        update_mean(&mut mean, f64::MAX, 1);
        update_mean(&mut mean, f64::MAX, 2);
        assert_eq!(mean, f64::MAX);
        assert_eq!(median_quality(&[]), None);
    }

    #[test]
    fn exact_partitions_match_an_independently_sorted_reference_for_every_split() {
        let lengths = [9, 1, 4, 1, 100, 7, 2];
        for mask in 0..1_u32 << lengths.len() {
            let mut aggregator = Aggregator::default();
            let mut passed = Vec::new();
            let mut passed_q = Vec::new();
            let mut all_q = Vec::new();
            for (i, length) in lengths.into_iter().enumerate() {
                let qscore = i as f64 * 2.5;
                let passed_flag = mask & (1 << i) != 0;
                aggregator
                    .add(&Record {
                        line: i as u64 + 2,
                        run_id: "fictional",
                        channel: 1,
                        mux: 1,
                        start_time: 0.0,
                        template_duration: None,
                        passed: passed_flag,
                        length,
                        qscore,
                        end_reason: "normal",
                    })
                    .unwrap();
                if passed_flag {
                    passed.push(length);
                    passed_q.push(qscore);
                }
                all_q.push(qscore);
            }
            let report = aggregator.finish(None);
            let mut all = lengths.to_vec();
            all.sort_unstable();
            passed.sort_unstable();
            let reference_median = |values: &[u64]| {
                (!values.is_empty()).then(|| {
                    let n = values.len();
                    (values[(n - 1) / 2] + values[n / 2]) as f64 / 2.0
                })
            };
            let reference_n50 = |values: &[u64]| {
                let total: u64 = values.iter().sum();
                let mut bases = 0;
                values.iter().rev().copied().find(|length| {
                    bases += length;
                    bases * 2 >= total
                })
            };
            let reference_quality = |values: &[f64]| {
                (!values.is_empty()).then(|| {
                    let n = values.len();
                    (values[(n - 1) / 2] + values[n / 2]) / 2.0
                })
            };
            assert_eq!(report.metrics.median_read_length_bp, reference_median(&all));
            assert_eq!(report.metrics.n50_bp, reference_n50(&all));
            assert_eq!(report.quality.median_qscore, reference_quality(&all_q));
            let subset = &report.diagnostics.passed_read_statistics;
            assert_eq!(subset.read_count, passed.len() as u64);
            assert_eq!(subset.total_yield_bp, passed.iter().sum::<u64>());
            assert_eq!(subset.median_read_length_bp, reference_median(&passed));
            assert_eq!(subset.n50_bp, reference_n50(&passed));
            assert_eq!(subset.median_qscore, reference_quality(&passed_q));
        }
    }

    #[test]
    fn partition_buffers_store_each_observation_once_with_bounded_spare_capacity() {
        let mut aggregator = Aggregator::default();
        let observations = 2 * RESERVE_ELEMENTS + 3;
        for i in 0..observations {
            aggregator
                .add(&Record {
                    line: i as u64 + 2,
                    run_id: "fictional",
                    channel: 1,
                    mux: 1,
                    start_time: 0.0,
                    template_duration: None,
                    passed: i % 2 == 0,
                    length: 100,
                    qscore: 12.0,
                    end_reason: "normal",
                })
                .unwrap();
        }
        assert_eq!(
            aggregator.passed_lengths.len() + aggregator.failed_lengths.len(),
            observations
        );
        assert_eq!(
            aggregator.passed_qscores.len() + aggregator.failed_qscores.len(),
            observations
        );
        let capacity_bytes = 8
            * (aggregator.passed_lengths.capacity()
                + aggregator.failed_lengths.capacity()
                + aggregator.passed_qscores.capacity()
                + aggregator.failed_qscores.capacity());
        assert!(capacity_bytes <= 16 * observations + 4 * RESERVE_ELEMENTS * 8);
        assert_eq!(
            std::mem::size_of_val(&aggregator.joint_counts)
                + std::mem::size_of_val(&aggregator.joint_passed_counts),
            8736
        );
        assert_eq!(
            aggregator.joint_counts.iter().flatten().sum::<u64>(),
            observations as u64
        );
    }
}
