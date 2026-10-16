use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    record::Record,
    report::{Report, Run, Tool},
};

const TIME_BIN_SECONDS: u64 = 900;
const LENGTH_BOUNDS: [u64; 9] = [
    1, 1_000, 5_000, 10_000, 25_000, 50_000, 100_000, 250_000, 1_000_000,
];
const QUALITY_BOUNDS: [f64; 9] = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0];
// Limit spare capacity instead of geometrically doubling two large vectors.
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
}

#[derive(Default, Debug, Serialize)]
pub struct EndReasonMetrics {
    pub end_reason: String,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub total_yield_bp: u64,
}

#[derive(Default)]
pub struct Aggregator {
    overall: OverallMetrics,
    lengths: Vec<u64>,
    qscores: Vec<f64>,
    mean_qscore: f64,
    quality_counts: [u64; QUALITY_BOUNDS.len()],
    length_counts: [u64; LENGTH_BOUNDS.len()],
    length_yields: [u64; LENGTH_BOUNDS.len()],
    channels: BTreeMap<u32, ChannelMetrics>,
    muxes: BTreeMap<u32, MuxMetrics>,
    time_bins: BTreeMap<u64, TimeBin>,
    end_reasons: BTreeMap<String, EndReasonMetrics>,
}

impl Aggregator {
    pub fn add(&mut self, record: &Record<'_>) -> Result<()> {
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
            .or_insert_with(|| TimeBin {
                start_seconds,
                end_seconds,
                ..TimeBin::default()
            });
        // Borrowed lookup avoids a fresh String for every record.
        if !self.end_reasons.contains_key(record.end_reason) {
            self.end_reasons
                .insert(record.end_reason.to_owned(), EndReasonMetrics::default());
        }
        let reason = self.end_reasons.get_mut(record.end_reason).unwrap();
        channel.total_records += 1;
        mux.total_records += 1;
        time.total_records += 1;
        reason.total_records += 1;
        if record.length == 0 {
            self.overall.zero_length_records += 1;
            return Ok(());
        }

        push_compact(&mut self.lengths, record.length)?;
        push_compact(&mut self.qscores, record.qscore)?;
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
        time.basecalled_reads += 1;
        time.yield_bp += record.length;
        reason.basecalled_reads += 1;
        reason.total_yield_bp += record.length;
        if record.passed {
            self.overall.passed_reads += 1;
            self.overall.passed_yield_bp += record.length;
            channel.passed_reads += 1;
            channel.passed_yield_bp += record.length;
            mux.passed_yield_bp += record.length;
        } else {
            self.overall.failed_reads += 1;
            self.overall.failed_yield_bp += record.length;
        }
        let length_bin = LENGTH_BOUNDS.partition_point(|bound| *bound <= record.length) - 1;
        self.length_counts[length_bin] += 1;
        self.length_yields[length_bin] += record.length;
        let quality_bin = QUALITY_BOUNDS.partition_point(|bound| *bound <= record.qscore) - 1;
        self.quality_counts[quality_bin] += 1;
        Ok(())
    }

    pub fn finish(mut self, run_id: Option<String>) -> Report {
        self.lengths.sort_unstable();
        self.qscores.sort_unstable_by(f64::total_cmp);
        if !self.lengths.is_empty() {
            self.overall.mean_read_length_bp =
                Some(self.overall.total_yield_bp as f64 / self.overall.basecalled_reads as f64);
            self.overall.median_read_length_bp = median_lengths(&self.lengths);
            self.overall.min_read_length_bp = self.lengths.first().copied();
            self.overall.max_read_length_bp = self.lengths.last().copied();
            self.overall.n50_bp = n50(&self.lengths, self.overall.total_yield_bp);
        }
        let quality = QualityMetrics {
            mean_qscore: (!self.qscores.is_empty()).then_some(self.mean_qscore),
            median_qscore: median_quality(&self.qscores),
            min_qscore: self.qscores.first().copied(),
            max_qscore: self.qscores.last().copied(),
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
            cumulative += time.yield_bp;
            time.cumulative_yield_bp = cumulative;
        }
        let end_reasons = self
            .end_reasons
            .into_iter()
            .map(|(name, mut reason)| {
                reason.end_reason = name;
                reason
            })
            .collect();
        Report {
            schema_version: 1,
            tool: Tool::current(),
            run: Run { run_id },
            metrics: self.overall,
            quality,
            read_length_distribution: distribution,
            channels: self.channels.into_values().collect(),
            muxes: self.muxes.into_values().collect(),
            time_bins: self.time_bins.into_values().collect(),
            end_reasons,
        }
    }
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
    let middle = sorted.len() / 2;
    let &upper = sorted.get(middle)?;
    if sorted.len() % 2 == 0 {
        // Widen before adding so even u64::MAX lengths cannot overflow.
        Some((u128::from(sorted[middle - 1]) + u128::from(upper)) as f64 / 2.0)
    } else {
        Some(upper as f64)
    }
}

fn median_quality(sorted: &[f64]) -> Option<f64> {
    let middle = sorted.len() / 2;
    let &upper = sorted.get(middle)?;
    if sorted.len() % 2 == 0 {
        Some(sorted[middle - 1] + (upper - sorted[middle - 1]) / 2.0)
    } else {
        Some(upper)
    }
}

fn n50(sorted_lengths: &[u64], total_yield: u64) -> Option<u64> {
    let target = total_yield.div_ceil(2);
    let mut cumulative = 0_u64;
    for &length in sorted_lengths.iter().rev() {
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
}
