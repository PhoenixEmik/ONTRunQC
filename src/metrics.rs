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

#[derive(Clone, Default, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct QualityMetrics {
    pub mean_qscore: Option<f64>,
    pub median_qscore: Option<f64>,
    pub min_qscore: Option<f64>,
    pub max_qscore: Option<f64>,
    pub histogram: Vec<QualityBin>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QualityBin {
    pub lower_inclusive: f64,
    pub upper_exclusive: Option<f64>,
    pub read_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LengthBin {
    pub lower_inclusive_bp: u64,
    pub upper_exclusive_bp: Option<u64>,
    pub read_count: u64,
    pub yield_bp: u64,
}

#[derive(Clone, Default, Debug, Serialize)]
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

#[derive(Clone, Default, Debug, Serialize)]
pub struct MuxMetrics {
    pub mux: u32,
    pub total_records: u64,
    pub basecalled_reads: u64,
    pub total_yield_bp: u64,
    pub passed_yield_bp: u64,
}

#[derive(Clone, Default, Debug, Serialize)]
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

#[derive(Clone, Default, Debug, Serialize)]
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
    pub minion_sample: Option<MinionSample>,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct LengthQualityHistogram {
    pub length_lower_bounds_bp: Vec<u64>,
    pub qscore_lower_bounds: Vec<f64>,
    // Rows are ascending Q-score bins; columns are ascending read-length bins.
    // The final interval on each axis is unbounded.
    pub all_read_counts: Vec<Vec<u64>>,
    pub passed_read_counts: Vec<Vec<u64>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelLengthBin {
    pub lower_inclusive_bp: u64,
    pub upper_exclusive_bp: Option<u64>,
    pub channel_count: u64,
    pub passed_channel_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelActivity {
    pub start_seconds: u64,
    pub basecalled_producing_channels: u64,
}

#[derive(Clone, Default, Debug, Serialize)]
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

#[derive(Clone, Default, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct SequencingSpeed {
    #[serde(flatten)]
    pub summary: SpeedSummary,
    pub histogram: Vec<SpeedBin>,
    pub time_bins: Vec<SpeedTimeBin>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpeedBin {
    pub lower_inclusive_bp_per_second: f64,
    pub upper_exclusive_bp_per_second: Option<f64>,
    pub read_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpeedTimeBin {
    pub start_seconds: u64,
    #[serde(flatten)]
    pub summary: SpeedSummary,
}

#[derive(Clone, Debug, Serialize)]
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

/// Additive MinIONQC comparisons; existing native/basecalled metrics stay intact.
#[derive(Clone, Debug, Serialize)]
pub struct ComparisonMetrics {
    pub qscore_cutoff: f64,
    pub native_filtering_available: bool,
    pub native_unclassified_reads: u64,
    pub qscore_selected: PassedReadStatistics,
    pub length_histogram: Vec<LengthBin>,
    pub quality_histogram: Vec<QualityBin>,
    pub time_bins: Vec<SelectedTimeBin>,
    pub yield_by_minimum_length: YieldByLength,
    pub channels: Vec<ChannelComparison>,
    pub channel_distributions: Vec<ChannelDistribution>,
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct SelectedTimeBin {
    pub start_seconds: u64,
    pub basecalled_reads: u64,
    pub yield_bp: u64,
    pub cumulative_yield_bp: u64,
    pub mean_read_length_bp: Option<f64>,
    pub mean_qscore: Option<f64>,
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct ChannelPopulation {
    pub read_count: u64,
    pub total_yield_bp: u64,
    pub mean_read_length_bp: Option<f64>,
    pub median_read_length_bp: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelComparison {
    pub channel: u32,
    pub all_summary_records: ChannelPopulation,
    pub all_basecalled_reads: ChannelPopulation,
    pub qscore_selected_reads: ChannelPopulation,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelDistribution {
    pub metric: &'static str,
    pub axis_maximum: f64,
    pub bins: Vec<ChannelDistributionBin>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelDistributionBin {
    pub lower_inclusive: f64,
    pub upper_exclusive: Option<f64>,
    pub all_summary_channels: u64,
    pub qscore_selected_channels: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct YieldByLength {
    pub thresholds: Vec<LengthThreshold>,
    pub curve: Vec<LengthThreshold>,
    pub ultralong_all: Ultralong,
    pub ultralong_qscore_selected: Ultralong,
}

#[derive(Clone, Debug, Serialize)]
pub struct LengthThreshold {
    pub minimum_length_bp: u64,
    pub all_read_count: u64,
    pub all_yield_bp: u64,
    pub qscore_selected_read_count: u64,
    pub qscore_selected_yield_bp: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Ultralong {
    pub target_n50_bp: u64,
    pub read_count: u64,
    pub total_yield_bp: u64,
    pub minimum_read_length_bp: Option<u64>,
}

// Native pass (bit 1) × independent Q selection (bit 0). Only primitives are
// retained, with exactly one length and one Q-score per basecalled observation.
#[derive(Default)]
struct ChannelLengths {
    partitions: [Vec<u64>; 4],
    selected_reads: u64,
    selected_yield: u64,
}

struct ComparisonAccumulator {
    cutoff: f64,
    native_available: bool,
    unclassified: u64,
    selected_reads: u64,
    selected_yield: u64,
    mean_qscore: f64,
    length_counts: [u64; LENGTH_BOUNDS.len()],
    length_yields: [u64; LENGTH_BOUNDS.len()],
    quality_counts: [u64; QUALITY_BOUNDS.len()],
    time_bins: BTreeMap<u64, SelectedTimeBin>,
}

impl Default for ComparisonAccumulator {
    fn default() -> Self {
        Self {
            cutoff: 7.0,
            native_available: true,
            unclassified: 0,
            selected_reads: 0,
            selected_yield: 0,
            mean_qscore: 0.0,
            length_counts: [0; LENGTH_BOUNDS.len()],
            length_yields: [0; LENGTH_BOUNDS.len()],
            quality_counts: [0; QUALITY_BOUNDS.len()],
            time_bins: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct InputSummary {
    pub input_records: u64,
    pub excluded_control_records: u64,
    pub negative_qscore_records: u64,
    pub schemas: Vec<crate::input::InputSchema>,
    pub read_representation: Option<crate::input::ReadRepresentation>,
}

#[derive(Debug, Serialize)]
pub struct MinionSample {
    pub method: &'static str,
    pub limit_per_channel: usize,
    pub eligible_reads: u64,
    pub channels: Vec<SampledChannel>,
}

#[derive(Default, Debug, Serialize)]
pub struct SampledChannel {
    pub channel: u32,
    pub eligible_reads: u64,
    pub points: Vec<ReadPoint>,
}

#[derive(Debug, Serialize)]
pub struct ReadPoint {
    pub start_seconds: f64,
    pub length_bp: u64,
    pub qscore: f64,
    pub events_per_base: Option<f64>,
}

#[derive(Default)]
pub struct Aggregator {
    overall: OverallMetrics,
    input: InputSummary,
    reference_density: bool,
    reference_domain: crate::density::Bounds,
    zero_qscores: Vec<f64>,
    negative_quality_count: u64,
    negative_joint: [u64; JOINT_LENGTH_BOUNDS.len()],
    negative_passed_joint: [u64; JOINT_LENGTH_BOUNDS.len()],
    samples: BTreeMap<u32, SampledChannel>,
    sample_enabled: bool,
    channel_lengths: BTreeMap<u32, ChannelLengths>,
    qscores: [Vec<f64>; 4],
    comparison: ComparisonAccumulator,
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

#[derive(Clone, Debug, Serialize)]
pub struct DistributionEstimates {
    pub method: &'static str,
    pub length_log10: Option<DensityEstimate>,
    pub qscore: Option<DensityEstimate>,
    pub selected_length_log10: Option<DensityEstimate>,
    pub selected_qscore: Option<DensityEstimate>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DensityEstimate {
    pub observations: usize,
    pub bandwidth: f64,
    pub points: Vec<DensityPoint>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DensityPoint {
    pub x: f64,
    pub density: f64,
}

fn mixed_quality<'a>(parts: [&'a [f64]; 4], zero: &'a [f64]) -> impl Iterator<Item = f64> + 'a {
    let mut indices = [0usize; 5];
    let slices = [parts[0], parts[1], parts[2], parts[3], zero];
    std::iter::from_fn(move || {
        let next = (0..5)
            .filter(|&i| indices[i] < slices[i].len())
            .min_by(|&a, &b| slices[a][indices[a]].total_cmp(&slices[b][indices[b]]))?;
        let value = slices[next][indices[next]];
        indices[next] += 1;
        Some(value)
    })
}

// Gaussian KDE with R's bw.nrd0 rule and type-7 quartiles. Linear binning
// bounds evaluation cost without another per-read collection.
fn density_estimate<I: Iterator<Item = f64>>(values: impl Fn() -> I) -> Option<DensityEstimate> {
    let n = values().count();
    if n == 0 {
        return None;
    }
    let first = values().next()?;
    let last = values().last()?;
    let scale = first.abs().max(last.abs()).max(1.0);
    let quantile = |p: f64| {
        let rank = (n - 1) as f64 * p;
        let mut iter = values();
        let a = iter.nth(rank.floor() as usize).unwrap() / scale;
        let b = if rank.fract() == 0.0 {
            a
        } else {
            iter.next().unwrap() / scale
        };
        a + (b - a) * rank.fract()
    };
    let (mut mean, mut m2) = (0.0, 0.0);
    for (i, v) in values().enumerate() {
        let v = v / scale;
        let delta = v - mean;
        mean += delta / (i + 1) as f64;
        m2 += delta * (v - mean);
    }
    let sd = if n > 1 {
        (m2 / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    let mut spread = sd.min((quantile(0.75) - quantile(0.25)) / 1.34);
    if spread <= 0.0 {
        spread = if sd > 0.0 {
            sd
        } else if first != 0.0 {
            first.abs() / scale
        } else {
            1.0 / scale
        };
    }
    let bandwidth = 0.9 * spread * (n as f64).powf(-0.2);
    let from = first / scale - 3.0 * bandwidth;
    let to = last / scale + 3.0 * bandwidth;
    if bandwidth <= 0.0
        || !bandwidth.is_finite()
        || !(from * scale).is_finite()
        || !(to * scale).is_finite()
    {
        return None;
    }
    let step = (to - from) / 2047.0;
    if step <= 0.0 {
        return None;
    }
    let mut weights = [0.0; 2048];
    for v in values() {
        let position = ((v / scale - from) / step).clamp(0.0, 2047.0);
        let index = position.floor() as usize;
        weights[index] += (1.0 - position.fract()) / n as f64;
        if index < 2047 {
            weights[index + 1] += position.fract() / n as f64;
        }
    }
    let points: Vec<DensityPoint> = (0..512)
        .map(|i| {
            let x = from + (to - from) * i as f64 / 511.0;
            let density = weights
                .iter()
                .enumerate()
                .filter(|(_, w)| **w != 0.0)
                .map(|(j, weight)| {
                    let z = (x - (from + step * j as f64)) / bandwidth;
                    weight * (-0.5 * z * z).exp()
                        / (2.0 * std::f64::consts::PI).sqrt()
                        / bandwidth
                        / scale
                })
                .sum();
            DensityPoint {
                x: x * scale,
                density,
            }
        })
        .collect();
    if points
        .iter()
        .any(|p| !p.x.is_finite() || !p.density.is_finite())
    {
        return None;
    }
    Some(DensityEstimate {
        observations: n,
        bandwidth: bandwidth * scale,
        points,
    })
}

pub struct NumericObservations {
    zero_qscores: Vec<f64>,
    lengths: [Vec<u64>; 4],
    qscores: [Vec<f64>; 4],
}

impl Aggregator {
    pub fn new(layout: Option<FlowcellLayout>, speed_available: bool) -> Self {
        Self {
            layout,
            speed_available,
            sample_enabled: true,
            ..Self::default()
        }
    }

    /// Pool exact observations by ownership; instrument-local channel/time groups
    /// deliberately remain in the individual reports, never conflated across runs.
    pub fn absorb(&mut self, report: &Report, observations: NumericObservations) -> Result<()> {
        self.input.input_records = self
            .input
            .input_records
            .checked_add(report.input.input_records)
            .context("combined input count exceeds u64 capacity")?;
        self.input.excluded_control_records += report.input.excluded_control_records;
        self.input.negative_qscore_records += report.input.negative_qscore_records;
        for schema in &report.input.schemas {
            if !self.input.schemas.contains(schema) {
                self.input.schemas.push(*schema);
            }
        }
        // A collection may contain different representations; they remain explicit
        // in each run. No single representation is asserted for pooled reads.
        let m = &report.metrics;
        let before = self.overall.basecalled_reads;
        self.overall.total_records = self
            .overall
            .total_records
            .checked_add(m.total_records)
            .context("combined record count exceeds u64 capacity")?;
        self.overall.total_yield_bp = self
            .overall
            .total_yield_bp
            .checked_add(m.total_yield_bp)
            .context("combined yield exceeds u64 capacity")?;
        combine_mean(
            &mut self.mean_qscore,
            before,
            report.quality.mean_qscore,
            m.basecalled_reads,
        );
        combine_mean(
            &mut self.passed_mean_qscore,
            self.overall.passed_reads,
            report.diagnostics.passed_read_statistics.mean_qscore,
            m.passed_reads,
        );
        let c = &mut self.comparison;
        combine_mean(
            &mut c.mean_qscore,
            c.selected_reads,
            report.comparison.qscore_selected.mean_qscore,
            report.comparison.qscore_selected.read_count,
        );
        self.overall.basecalled_reads += m.basecalled_reads;
        self.overall.zero_length_records += m.zero_length_records;
        self.overall.passed_reads += m.passed_reads;
        self.overall.failed_reads += m.failed_reads;
        self.overall.passed_yield_bp += m.passed_yield_bp;
        self.overall.failed_yield_bp += m.failed_yield_bp;
        c.selected_reads += report.comparison.qscore_selected.read_count;
        c.selected_yield += report.comparison.qscore_selected.total_yield_bp;
        c.unclassified += report.comparison.native_unclassified_reads;
        c.native_available &= report.comparison.native_filtering_available;
        for i in 0..LENGTH_BOUNDS.len() {
            self.length_counts[i] += report.read_length_distribution[i].read_count;
            self.length_yields[i] += report.read_length_distribution[i].yield_bp;
            c.length_counts[i] += report.comparison.length_histogram[i].read_count;
            c.length_yields[i] += report.comparison.length_histogram[i].yield_bp;
        }
        for bin in &report.quality.histogram {
            if bin.lower_inclusive < 0.0 {
                self.negative_quality_count += bin.read_count;
            } else {
                let i = QUALITY_BOUNDS.partition_point(|b| *b <= bin.lower_inclusive) - 1;
                self.quality_counts[i] += bin.read_count;
            }
        }
        for i in 0..QUALITY_BOUNDS.len() {
            c.quality_counts[i] += report.comparison.quality_histogram[i].read_count;
        }
        let joint = &report.diagnostics.length_quality_histogram;
        let offset = usize::from(joint.qscore_lower_bounds.first().is_some_and(|v| *v < 0.0));
        if offset == 1 {
            for l in 0..JOINT_LENGTH_BOUNDS.len() {
                self.negative_joint[l] += joint.all_read_counts[0][l];
                self.negative_passed_joint[l] += joint.passed_read_counts[0][l];
            }
        }
        for q in 0..JOINT_QUALITY_BOUNDS.len() {
            for l in 0..JOINT_LENGTH_BOUNDS.len() {
                self.joint_counts[q][l] += joint.all_read_counts[q + offset][l];
                self.joint_passed_counts[q][l] += joint.passed_read_counts[q + offset][l];
            }
        }
        let destination = &mut self.channel_lengths.entry(0).or_default().partitions;
        for (to, from) in destination.iter_mut().zip(observations.lengths) {
            append_observations(to, from)?;
        }
        for (to, from) in self.qscores.iter_mut().zip(observations.qscores) {
            append_observations(to, from)?;
        }
        append_observations(&mut self.zero_qscores, observations.zero_qscores)?;
        Ok(())
    }

    pub fn enable_reference_density(&mut self, enabled: bool) {
        self.reference_density = enabled;
    }
    pub fn density_bounds(&self) -> crate::density::Bounds {
        crate::density::Bounds {
            length: crate::density::range(
                self.channel_lengths
                    .values()
                    .flat_map(|c| c.partitions.iter().flatten())
                    .map(|n| (*n as f64).log10()),
            ),
            qscore: crate::density::range(
                self.qscores
                    .iter()
                    .flatten()
                    .chain(&self.zero_qscores)
                    .copied(),
            ),
        }
    }
    pub fn set_density_domain(&mut self, bounds: crate::density::Bounds) {
        self.reference_domain = bounds;
    }
    pub fn set_sampling_enabled(&mut self, enabled: bool) {
        self.sample_enabled = enabled;
    }

    pub fn include_input_format(
        &mut self,
        schema: crate::input::InputSchema,
        representation: crate::input::ReadRepresentation,
    ) -> Result<()> {
        if let Some(previous) = self.input.read_representation {
            ensure!(
                previous == representation,
                "incompatible template/2D representations within one run; select --read-representation explicitly"
            );
        }
        self.input.read_representation = Some(representation);
        if !self.input.schemas.contains(&schema) {
            self.input.schemas.push(schema);
        }
        Ok(())
    }

    pub fn include_input_capabilities(&mut self, native: bool, speed: bool) {
        self.comparison.native_available &= native;
        self.speed_available |= speed;
    }

    pub fn set_qscore_cutoff(&mut self, cutoff: f64, native_available: bool) {
        self.comparison.cutoff = cutoff;
        self.comparison.native_available = native_available;
    }

    pub fn add(&mut self, record: &Record<'_>) -> Result<()> {
        self.input.input_records = self
            .input
            .input_records
            .checked_add(1)
            .context("input record count exceeds u64 capacity")?;
        if record.is_control {
            self.input.excluded_control_records += 1;
            return Ok(());
        }
        if record.qscore < 0.0 {
            self.input.negative_qscore_records += 1;
        }
        if let Some(layout) = self.layout {
            ensure!(
                layout.coordinate(record.channel).is_some(),
                "channel {} is incompatible with --flowcell-layout {} (expected 1–{}); select a matching layout or omit the option",
                record.channel,
                layout.name(),
                layout.channel_count()
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
        let mut mux = record.mux.map(|id| {
            self.muxes.entry(id).or_insert_with(|| MuxMetrics {
                mux: id,
                ..MuxMetrics::default()
            })
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
        let mut channel_mux = record.mux.map(|mux| {
            self.channel_mux
                .entry((record.channel, mux))
                .or_insert_with(|| ChannelMuxMetrics {
                    channel: record.channel,
                    mux,
                    ..ChannelMuxMetrics::default()
                })
        });
        let mut reason = if let Some(name) = record.end_reason {
            if !self.end_reasons.contains_key(name) {
                self.end_reasons
                    .insert(name.to_owned(), ReasonAccumulator::default());
            }
            self.end_reasons.get_mut(name)
        } else {
            None
        };
        channel.total_records += 1;
        if let Some(value) = &mut mux {
            value.total_records += 1;
        }
        time.metrics.total_records += 1;
        if let Some(value) = &mut reason {
            value.metrics.total_records += 1;
        }
        if let Some(value) = &mut channel_mux {
            value.total_records += 1;
        }
        if record.length == 0 {
            if self.reference_density {
                push_compact(&mut self.zero_qscores, record.qscore)?;
            }
            self.overall.zero_length_records += 1;
            if let Some(value) = &mut channel_mux {
                value.zero_length_records += 1;
            }
            return Ok(());
        }

        let selected = record.qscore >= self.comparison.cutoff;
        let partition = usize::from(record.passed == Some(true)) * 2 + usize::from(selected);
        let lengths = self.channel_lengths.entry(record.channel).or_default();
        let values = &mut lengths.partitions[partition];
        if values.len() == values.capacity() {
            values
                .try_reserve_exact(256)
                .context("cannot allocate channel length buffer")?;
        }
        values.push(record.length);
        push_compact(&mut self.qscores[partition], record.qscore)?;
        if record.passed.is_none() {
            self.comparison.unclassified += 1;
            self.comparison.native_available = false;
        }
        if selected {
            let c = &mut self.comparison;
            c.selected_reads += 1;
            c.selected_yield += record.length;
            update_mean(&mut c.mean_qscore, record.qscore, c.selected_reads);
            lengths.selected_reads += 1;
            lengths.selected_yield += record.length;
            let length_bin = LENGTH_BOUNDS.partition_point(|b| *b <= record.length) - 1;
            c.length_counts[length_bin] += 1;
            c.length_yields[length_bin] += record.length;
            c.quality_counts[QUALITY_BOUNDS.partition_point(|b| *b <= record.qscore) - 1] += 1;
            let bin = c
                .time_bins
                .entry(start_seconds)
                .or_insert_with(|| SelectedTimeBin {
                    start_seconds,
                    ..SelectedTimeBin::default()
                });
            bin.basecalled_reads += 1;
            bin.yield_bp += record.length;
            update_mean(
                bin.mean_qscore.get_or_insert(0.0),
                record.qscore,
                bin.basecalled_reads,
            );
        }
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
        if let Some(value) = &mut mux {
            value.basecalled_reads += 1;
        }
        if let Some(value) = &mut mux {
            value.total_yield_bp += record.length;
        }
        time.metrics.basecalled_reads += 1;
        time.metrics.yield_bp += record.length;
        time.metrics.qscore_observations += 1;
        update_mean(
            time.metrics.mean_qscore.get_or_insert(0.0),
            record.qscore,
            time.metrics.qscore_observations,
        );
        time.channels.insert(record.channel);
        if let Some(value) = &mut reason {
            value.metrics.basecalled_reads += 1;
        }
        if let Some(value) = &mut reason {
            value.metrics.total_yield_bp += record.length;
        }
        if let Some(value) = &mut channel_mux {
            value.basecalled_reads += 1;
        }
        if let Some(value) = &mut channel_mux {
            value.total_yield_bp += record.length;
        }
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
        if record.passed == Some(true) {
            self.overall.passed_reads += 1;
            self.overall.passed_yield_bp += record.length;
            update_mean(
                &mut self.passed_mean_qscore,
                record.qscore,
                self.overall.passed_reads,
            );
            channel.passed_reads += 1;
            channel.passed_yield_bp += record.length;
            if let Some(value) = &mut mux {
                value.passed_yield_bp += record.length;
            }
            if let Some(value) = &mut channel_mux {
                value.passed_reads += 1;
            }
            if let Some(value) = &mut channel_mux {
                value.passed_yield_bp += record.length;
            }
            if let Some(value) = &mut reason {
                value.passed_reads += 1;
            }
            time.metrics.passed_reads += 1;
            time.metrics.passed_yield_bp += record.length;
            update_mean(
                time.metrics.mean_passed_qscore.get_or_insert(0.0),
                record.qscore,
                time.metrics.passed_reads,
            );
        } else if record.passed == Some(false) {
            self.overall.failed_reads += 1;
            self.overall.failed_yield_bp += record.length;
        }
        let length_bin = LENGTH_BOUNDS.partition_point(|bound| *bound <= record.length) - 1;
        self.length_counts[length_bin] += 1;
        self.length_yields[length_bin] += record.length;
        let joint_length = JOINT_LENGTH_BOUNDS.partition_point(|bound| *bound <= record.length) - 1;
        if record.qscore < 0.0 {
            self.negative_quality_count += 1;
            self.negative_joint[joint_length] += 1;
            if record.passed == Some(true) {
                self.negative_passed_joint[joint_length] += 1;
            }
        } else {
            let quality_bin = QUALITY_BOUNDS.partition_point(|bound| *bound <= record.qscore) - 1;
            self.quality_counts[quality_bin] += 1;
            let joint_quality =
                JOINT_QUALITY_BOUNDS.partition_point(|bound| *bound <= record.qscore) - 1;
            self.joint_counts[joint_quality][joint_length] += 1;
            if record.passed == Some(true) {
                self.joint_passed_counts[joint_quality][joint_length] += 1;
            }
        }
        if self.sample_enabled && matches!(self.layout, Some(FlowcellLayout::MinionR95)) {
            const LIMIT: usize = 16;
            let sample = self
                .samples
                .entry(record.channel)
                .or_insert_with(|| SampledChannel {
                    channel: record.channel,
                    ..SampledChannel::default()
                });
            sample.eligible_reads += 1;
            let point = ReadPoint {
                start_seconds: record.start_time,
                length_bp: record.length,
                qscore: record.qscore,
                events_per_base: record
                    .num_events_template
                    .map(|events| events as f64 / record.length as f64),
            };
            if sample.points.len() < LIMIT {
                sample.points.push(point);
            } else {
                let mut hash = sample.eligible_reads ^ (u64::from(record.channel) << 32);
                hash = hash.wrapping_add(0x9e3779b97f4a7c15);
                hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
                let index = (hash ^ (hash >> 31)) % sample.eligible_reads;
                if index < LIMIT as u64 {
                    sample.points[index as usize] = point;
                }
            }
        }
        Ok(())
    }

    pub fn finish(self, run_id: Option<String>) -> Result<Report> {
        self.finish_retaining(run_id).map(|(report, _)| report)
    }

    pub fn finish_retaining(
        mut self,
        run_id: Option<String>,
    ) -> Result<(Report, NumericObservations)> {
        // Sort channel partitions first, then move their buffers into global
        // partitions. Channel medians never require a second retained length copy.
        let mut channel_comparisons = Vec::with_capacity(self.channels.len());
        for channel in self.channels.values() {
            let lengths = self.channel_lengths.entry(channel.channel).or_default();
            for part in &mut lengths.partitions {
                part.sort_unstable();
            }
            let all = length_slices(&lengths.partitions, Population::All);
            let selected = length_slices(&lengths.partitions, Population::Selected);
            let zero = channel.total_records - channel.basecalled_reads;
            channel_comparisons.push(ChannelComparison {
                channel: channel.channel,
                all_summary_records: ChannelPopulation {
                    read_count: channel.total_records,
                    total_yield_bp: channel.total_yield_bp,
                    mean_read_length_bp: fraction(channel.total_yield_bp, channel.total_records),
                    median_read_length_bp: partition_median_length(all, zero),
                },
                all_basecalled_reads: ChannelPopulation {
                    read_count: channel.basecalled_reads,
                    total_yield_bp: channel.total_yield_bp,
                    mean_read_length_bp: fraction(channel.total_yield_bp, channel.basecalled_reads),
                    median_read_length_bp: partition_median_length(all, 0),
                },
                qscore_selected_reads: ChannelPopulation {
                    read_count: lengths.selected_reads,
                    total_yield_bp: lengths.selected_yield,
                    mean_read_length_bp: fraction(lengths.selected_yield, lengths.selected_reads),
                    median_read_length_bp: partition_median_length(selected, 0),
                },
            });
        }
        let mut lengths: [Vec<u64>; 4] = std::array::from_fn(|_| Vec::new());
        // Reuse the largest channel buffer in each partition. Remaining source
        // buffers are released as they are moved, rather than retained as copies.
        for (index, destination) in lengths.iter_mut().enumerate() {
            if let Some((_, largest)) = self
                .channel_lengths
                .iter_mut()
                .max_by_key(|(_, c)| c.partitions[index].len())
            {
                *destination = std::mem::take(&mut largest.partitions[index]);
            }
        }
        for channel in self.channel_lengths.into_values() {
            for (destination, source) in lengths.iter_mut().zip(channel.partitions) {
                for length in source {
                    push_compact(destination, length)?;
                }
            }
        }
        for part in &mut lengths {
            part.sort_unstable();
        }
        for part in &mut self.qscores {
            part.sort_unstable_by(f64::total_cmp);
        }
        self.zero_qscores.sort_unstable_by(f64::total_cmp);
        let all_lengths = length_slices(&lengths, Population::All);
        let native_lengths = length_slices(&lengths, Population::Native);
        let selected_lengths = length_slices(&lengths, Population::Selected);
        let all_q = quality_slices(&self.qscores, Population::All);
        let native_q = quality_slices(&self.qscores, Population::Native);
        let selected_q = quality_slices(&self.qscores, Population::Selected);
        if self.overall.basecalled_reads != 0 {
            self.overall.mean_read_length_bp =
                fraction(self.overall.total_yield_bp, self.overall.basecalled_reads);
            self.overall.median_read_length_bp = partition_median_length(all_lengths, 0);
            self.overall.min_read_length_bp =
                all_lengths.iter().filter_map(|p| p.first()).copied().min();
            self.overall.max_read_length_bp =
                all_lengths.iter().filter_map(|p| p.last()).copied().max();
            self.overall.n50_bp =
                n50_descending(descending_lengths(all_lengths), self.overall.total_yield_bp);
        }
        let distributions = DistributionEstimates {
            method: "gaussian_bw_nrd0_linear_binning_2048_grid_512",
            length_log10: density_estimate(|| {
                ascending_lengths(all_lengths).map(|n| (n as f64).log10())
            }),
            qscore: density_estimate(|| ascending_quality(all_q)),
            selected_length_log10: density_estimate(|| {
                ascending_lengths(selected_lengths).map(|n| (n as f64).log10())
            }),
            selected_qscore: density_estimate(|| ascending_quality(selected_q)),
        };
        let minionqc_density = if self.reference_density {
            let selected_zero = &self.zero_qscores[self
                .zero_qscores
                .partition_point(|q| *q < self.comparison.cutoff)..];
            Some(DistributionEstimates {
                method: "R4.4.1_gaussian_nrd0_grid512_ext4_discrete_convolution",
                length_log10: crate::density::estimate(
                    || ascending_lengths(all_lengths).map(|n| (n as f64).log10()),
                    self.reference_domain.length,
                ),
                selected_length_log10: crate::density::estimate(
                    || ascending_lengths(selected_lengths).map(|n| (n as f64).log10()),
                    self.reference_domain.length,
                ),
                qscore: crate::density::estimate(
                    || mixed_quality(all_q, &self.zero_qscores),
                    self.reference_domain.qscore,
                ),
                selected_qscore: crate::density::estimate(
                    || mixed_quality(selected_q, selected_zero),
                    self.reference_domain.qscore,
                ),
            })
        } else {
            None
        };
        let passed_read_statistics = subset_statistics(
            native_lengths,
            native_q,
            self.overall.passed_reads,
            self.overall.passed_yield_bp,
            self.passed_mean_qscore,
        );
        let selected_statistics = subset_statistics(
            selected_lengths,
            selected_q,
            self.comparison.selected_reads,
            self.comparison.selected_yield,
            self.comparison.mean_qscore,
        );
        let mut quality = QualityMetrics {
            mean_qscore: (self.overall.basecalled_reads != 0).then_some(self.mean_qscore),
            median_qscore: partition_median_quality(all_q),
            min_qscore: all_q
                .iter()
                .filter_map(|p| p.first())
                .copied()
                .min_by(f64::total_cmp),
            max_qscore: all_q
                .iter()
                .filter_map(|p| p.last())
                .copied()
                .max_by(f64::total_cmp),
            histogram: QUALITY_BOUNDS
                .iter()
                .enumerate()
                .map(|(i, &lower)| QualityBin {
                    lower_inclusive: lower,
                    upper_exclusive: QUALITY_BOUNDS.get(i + 1).copied(),
                    read_count: self.quality_counts[i],
                })
                .collect(),
        };
        if self.negative_quality_count > 0 {
            quality.histogram.insert(
                0,
                QualityBin {
                    lower_inclusive: -f64::MAX,
                    upper_exclusive: Some(0.0),
                    read_count: self.negative_quality_count,
                },
            );
        }
        let yield_by_minimum_length = yield_by_length(all_lengths, selected_lengths);
        let channel_distributions = channel_distributions(&channel_comparisons);
        let mut selected_cumulative = 0;
        let selected_time_bins = self
            .time_bins
            .keys()
            .map(|&start| {
                let mut bin = self
                    .comparison
                    .time_bins
                    .remove(&start)
                    .unwrap_or(SelectedTimeBin {
                        start_seconds: start,
                        ..SelectedTimeBin::default()
                    });
                selected_cumulative += bin.yield_bp;
                bin.cumulative_yield_bp = selected_cumulative;
                bin.mean_read_length_bp = fraction(bin.yield_bp, bin.basecalled_reads);
                bin
            })
            .collect();
        let comparison = ComparisonMetrics {
            qscore_cutoff: self.comparison.cutoff,
            native_filtering_available: self.comparison.native_available,
            native_unclassified_reads: self.comparison.unclassified,
            qscore_selected: selected_statistics,
            length_histogram: LENGTH_BOUNDS
                .iter()
                .enumerate()
                .map(|(i, &lower)| LengthBin {
                    lower_inclusive_bp: lower,
                    upper_exclusive_bp: LENGTH_BOUNDS.get(i + 1).copied(),
                    read_count: self.comparison.length_counts[i],
                    yield_bp: self.comparison.length_yields[i],
                })
                .collect(),
            quality_histogram: QUALITY_BOUNDS
                .iter()
                .enumerate()
                .map(|(i, &lower)| QualityBin {
                    lower_inclusive: lower,
                    upper_exclusive: QUALITY_BOUNDS.get(i + 1).copied(),
                    read_count: self.comparison.quality_counts[i],
                })
                .collect(),
            time_bins: selected_time_bins,
            yield_by_minimum_length,
            channels: channel_comparisons,
            channel_distributions,
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
                    passed_fraction_of_basecalled: self
                        .comparison
                        .native_available
                        .then(|| fraction(reason.passed_reads, m.basecalled_reads))
                        .flatten(),
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
        let mut joint_q_bounds = JOINT_QUALITY_BOUNDS.to_vec();
        let mut joint_all: Vec<_> = self.joint_counts.iter().map(|row| row.to_vec()).collect();
        let mut joint_passed: Vec<_> = self
            .joint_passed_counts
            .iter()
            .map(|row| row.to_vec())
            .collect();
        if self.negative_quality_count > 0 {
            joint_q_bounds.insert(0, -f64::MAX);
            joint_all.insert(0, self.negative_joint.to_vec());
            joint_passed.insert(0, self.negative_passed_joint.to_vec());
        }
        let minion_sample = (self.sample_enabled
            && matches!(self.layout, Some(FlowcellLayout::MinionR95)))
        .then(|| MinionSample {
            method: "deterministic_splitmix64_reservoir_per_channel",
            limit_per_channel: 16,
            eligible_reads: self.overall.basecalled_reads,
            channels: self.samples.into_values().collect(),
        });
        let diagnostics = Diagnostics {
            minion_sample,
            physical_layout: self.layout.map(FlowcellLayout::description),
            channel_activity,
            channel_mux: self.channel_mux.into_values().collect(),
            sequencing_speed,
            termination_reasons,
            passed_read_statistics,
            length_quality_histogram: LengthQualityHistogram {
                length_lower_bounds_bp: JOINT_LENGTH_BOUNDS.to_vec(),
                qscore_lower_bounds: joint_q_bounds,
                all_read_counts: joint_all,
                passed_read_counts: joint_passed,
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
        Ok((
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
                comparison,
                distributions,
                minionqc_density,
                input: self.input,
                presentation: crate::report::PresentationOptions::default(),
                fitted_trends: None,
                source: None,
                collection: None,
            },
            NumericObservations {
                zero_qscores: self.zero_qscores,
                lengths,
                qscores: self.qscores,
            },
        ))
    }
}

#[derive(Clone, Copy)]
enum Population {
    All,
    Native,
    Selected,
}

fn population_includes(population: Population, partition: usize) -> bool {
    match population {
        Population::All => true,
        Population::Native => partition & 2 != 0,
        Population::Selected => partition & 1 != 0,
    }
}

fn length_slices(parts: &[Vec<u64>; 4], population: Population) -> [&[u64]; 4] {
    std::array::from_fn(|i| {
        if population_includes(population, i) {
            parts[i].as_slice()
        } else {
            &[]
        }
    })
}

fn quality_slices(parts: &[Vec<f64>; 4], population: Population) -> [&[f64]; 4] {
    std::array::from_fn(|i| {
        if population_includes(population, i) {
            parts[i].as_slice()
        } else {
            &[]
        }
    })
}

fn ascending_lengths(parts: [&[u64]; 4]) -> impl Iterator<Item = u64> + '_ {
    merge_sorted(
        merge_sorted(parts[0].iter().copied(), parts[1].iter().copied(), u64::cmp),
        merge_sorted(parts[2].iter().copied(), parts[3].iter().copied(), u64::cmp),
        u64::cmp,
    )
}

fn descending_lengths(parts: [&[u64]; 4]) -> impl Iterator<Item = u64> + '_ {
    let compare = |a: &u64, b: &u64| b.cmp(a);
    merge_sorted(
        merge_sorted(
            parts[0].iter().rev().copied(),
            parts[1].iter().rev().copied(),
            compare,
        ),
        merge_sorted(
            parts[2].iter().rev().copied(),
            parts[3].iter().rev().copied(),
            compare,
        ),
        compare,
    )
}

fn partition_median_length(parts: [&[u64]; 4], zeros: u64) -> Option<f64> {
    let len = zeros as usize + parts.iter().map(|p| p.len()).sum::<usize>();
    let (a, b) = middle_pair(
        std::iter::repeat_n(0, zeros as usize).chain(ascending_lengths(parts)),
        len,
    )?;
    Some((u128::from(a) + u128::from(b)) as f64 / 2.0)
}

fn ascending_quality(parts: [&[f64]; 4]) -> impl Iterator<Item = f64> + '_ {
    merge_sorted(
        merge_sorted(
            parts[0].iter().copied(),
            parts[1].iter().copied(),
            f64::total_cmp,
        ),
        merge_sorted(
            parts[2].iter().copied(),
            parts[3].iter().copied(),
            f64::total_cmp,
        ),
        f64::total_cmp,
    )
}

fn partition_median_quality(parts: [&[f64]; 4]) -> Option<f64> {
    let values = ascending_quality(parts);
    let len = parts.iter().map(|p| p.len()).sum();
    let (a, b) = middle_pair(values, len)?;
    Some(if len % 2 != 0 {
        a
    } else if (b - a).is_finite() {
        a + (b - a) / 2.0
    } else {
        a / 2.0 + b / 2.0
    })
}

fn subset_statistics(
    lengths: [&[u64]; 4],
    qscores: [&[f64]; 4],
    reads: u64,
    yield_bp: u64,
    mean_q: f64,
) -> PassedReadStatistics {
    PassedReadStatistics {
        read_count: reads,
        total_yield_bp: yield_bp,
        mean_read_length_bp: fraction(yield_bp, reads),
        median_read_length_bp: partition_median_length(lengths, 0),
        n50_bp: n50_descending(descending_lengths(lengths), yield_bp),
        min_read_length_bp: lengths.iter().filter_map(|p| p.first()).copied().min(),
        max_read_length_bp: lengths.iter().filter_map(|p| p.last()).copied().max(),
        mean_qscore: (reads != 0).then_some(mean_q),
        median_qscore: partition_median_quality(qscores),
    }
}

const PARITY_LENGTH_THRESHOLDS: [u64; 7] =
    [10_000, 20_000, 50_000, 100_000, 200_000, 500_000, 1_000_000];

fn yield_by_length(all: [&[u64]; 4], selected: [&[u64]; 4]) -> YieldByLength {
    let maximum = all
        .iter()
        .filter_map(|p| p.last())
        .copied()
        .max()
        .unwrap_or(0);
    let end = maximum.saturating_add(1).max(1);
    let mut display: BTreeSet<_> = (0..=512_u128)
        .map(|i| 1 + ((u128::from(end - 1) * i) / 512) as u64)
        .collect();
    display.extend(PARITY_LENGTH_THRESHOLDS.into_iter().filter(|t| *t <= end));
    let queries: BTreeSet<_> = display
        .iter()
        .copied()
        .chain(PARITY_LENGTH_THRESHOLDS)
        .collect();
    let mut all_values = descending_lengths(all).peekable();
    let mut selected_values = descending_lengths(selected).peekable();
    let (mut all_reads, mut all_yield, mut selected_reads, mut selected_yield) = (0, 0, 0, 0);
    let mut curve = Vec::new();
    let mut thresholds = Vec::new();
    for minimum in queries.into_iter().rev() {
        while all_values.peek().is_some_and(|v| *v >= minimum) {
            all_reads += 1;
            all_yield += all_values.next().unwrap();
        }
        while selected_values.peek().is_some_and(|v| *v >= minimum) {
            selected_reads += 1;
            selected_yield += selected_values.next().unwrap();
        }
        let point = || LengthThreshold {
            minimum_length_bp: minimum,
            all_read_count: all_reads,
            all_yield_bp: all_yield,
            qscore_selected_read_count: selected_reads,
            qscore_selected_yield_bp: selected_yield,
        };
        if display.contains(&minimum) {
            curve.push(point());
        }
        if PARITY_LENGTH_THRESHOLDS.contains(&minimum) {
            thresholds.push(point());
        }
    }
    curve.reverse();
    thresholds.reverse();
    YieldByLength {
        thresholds,
        curve,
        ultralong_all: ultralong(all),
        ultralong_qscore_selected: ultralong(selected),
    }
}

fn ultralong(parts: [&[u64]; 4]) -> Ultralong {
    let mut result = Ultralong {
        target_n50_bp: 100_000,
        read_count: 0,
        total_yield_bp: 0,
        minimum_read_length_bp: None,
    };
    let mut long_yield = 0_u64;
    for length in descending_lengths(parts) {
        if length >= result.target_n50_bp {
            long_yield += length;
        }
        // Match upstream's strict-half N50 for this dedicated diagnostic.
        // Fix its binary-search endpoint bug: an eligible full set stays full.
        if u128::from(result.total_yield_bp) + u128::from(length) >= 2 * u128::from(long_yield) {
            break;
        }
        result.read_count += 1;
        result.total_yield_bp += length;
        result.minimum_read_length_bp = Some(length);
    }
    result
}

fn channel_distributions(channels: &[ChannelComparison]) -> Vec<ChannelDistribution> {
    let metrics = [
        "total_yield_bp",
        "read_count",
        "mean_read_length_bp",
        "median_read_length_bp",
    ];
    metrics
        .into_iter()
        .map(|metric| {
            let value = |p: &ChannelPopulation| match metric {
                "total_yield_bp" => (p.read_count > 0).then_some(p.total_yield_bp as f64),
                "read_count" => (p.read_count > 0).then_some(p.read_count as f64),
                "mean_read_length_bp" => p.mean_read_length_bp,
                _ => p.median_read_length_bp,
            };
            let maximum = channels
                .iter()
                .flat_map(|c| {
                    [
                        value(&c.all_summary_records),
                        value(&c.qscore_selected_reads),
                    ]
                })
                .flatten()
                .reduce(f64::max)
                .unwrap_or(0.0);
            // Shared equal-width bins for both populations; 0 is a real value.
            let upper = if maximum == 0.0 { 1.0 } else { maximum };
            let mut bins: Vec<_> = (0..30)
                .map(|i| ChannelDistributionBin {
                    lower_inclusive: upper * (i as f64 / 30.0),
                    upper_exclusive: (i < 29).then(|| upper * ((i + 1) as f64 / 30.0)),
                    all_summary_channels: 0,
                    qscore_selected_channels: 0,
                })
                .collect();
            for channel in channels {
                for (selected, population) in [
                    (false, &channel.all_summary_records),
                    (true, &channel.qscore_selected_reads),
                ] {
                    if let Some(v) = value(population) {
                        let index = bins
                            .partition_point(|b| b.lower_inclusive <= v)
                            .saturating_sub(1);
                        if selected {
                            bins[index].qscore_selected_channels += 1;
                        } else {
                            bins[index].all_summary_channels += 1;
                        }
                    }
                }
            }
            ChannelDistribution {
                metric,
                axis_maximum: upper,
                bins,
            }
        })
        .collect()
}

fn append_observations<T>(destination: &mut Vec<T>, source: Vec<T>) -> Result<()> {
    if destination.is_empty() {
        *destination = source;
    } else {
        for value in source {
            push_compact(destination, value)?;
        }
    }
    Ok(())
}

fn combine_mean(mean: &mut f64, previous_count: u64, next_mean: Option<f64>, next_count: u64) {
    if let Some(next) = next_mean.filter(|_| next_count > 0) {
        let weight = next_count as f64 / (previous_count + next_count) as f64;
        let delta = next - *mean;
        if delta.is_finite() {
            *mean += delta * weight;
        } else {
            *mean = *mean * (1.0 - weight) + next * weight;
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
    let delta = value - *mean;
    if delta.is_finite() {
        *mean += delta / count as f64;
    } else {
        *mean = *mean * ((count - 1) as f64 / count as f64) + value / count as f64;
    }
}

#[cfg(test)]
fn median_lengths(sorted: &[u64]) -> Option<f64> {
    merged_median_lengths(sorted, &[])
}

#[cfg(test)]
fn median_quality(sorted: &[f64]) -> Option<f64> {
    merged_median_quality(sorted, &[])
}

#[cfg(test)]
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

#[cfg(test)]
fn merged_median_lengths(a: &[u64], b: &[u64]) -> Option<f64> {
    let values = merge_sorted(a.iter().copied(), b.iter().copied(), u64::cmp);
    let (lower, upper) = middle_pair(values, a.len() + b.len())?;
    // Widen before adding so even u64::MAX lengths cannot overflow.
    Some((u128::from(lower) + u128::from(upper)) as f64 / 2.0)
}

#[cfg(test)]
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

/// Reference positions are configuration, never inferred telemetry events.
pub fn configure_mux_markers(report: &mut Report) -> Result<()> {
    report.presentation.mux_markers_seconds.clear();
    let Some(hours) = report.presentation.mux_interval_hours else {
        return Ok(());
    };
    let end = report
        .collection
        .as_ref()
        .map(|c| {
            c.runs
                .iter()
                .filter_map(|r| r.time_bins.last().map(|b| b.end_seconds))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or_else(|| report.time_bins.last().map_or(0, |b| b.end_seconds));
    if end == 0 {
        return Ok(());
    }
    let interval = hours * 3600.0;
    let count = (end as f64 / interval).floor() + 1.0;
    ensure!(
        count <= 512.0,
        "configured mux interval generates more than 512 markers; increase --muxes or use 0 to disable"
    );
    report.presentation.mux_markers_seconds =
        (0..count as usize).map(|i| i as f64 * interval).collect();
    Ok(())
}

/// MinIONQC-shaped projection of the same canonical metrics; no new QC engine.
pub fn yaml_summary(report: &Report) -> serde_json::Value {
    use serde_json::json;
    let mut summary = serde_json::Map::new();
    summary.insert(
        "input file".into(),
        json!(
            report
                .source
                .as_ref()
                .map(|s| s.source_id.as_str())
                .unwrap_or(if report.collection.is_some() {
                    "pooled run/source collection"
                } else {
                    "single input source"
                })
        ),
    );
    summary.insert("run_id".into(), json!(report.run.run_id));
    summary.insert(
        "summary.records".into(),
        json!(report.metrics.total_records),
    );
    summary.insert(
        "excluded.control.records".into(),
        json!(report.input.excluded_control_records),
    );
    for selected in [false, true] {
        let m = &report.metrics;
        let s = &report.comparison.qscore_selected;
        let (reads, yield_bp, n50, mean, median, maximum, mean_q, median_q) = if selected {
            (
                s.read_count,
                s.total_yield_bp,
                s.n50_bp,
                s.mean_read_length_bp,
                s.median_read_length_bp,
                s.max_read_length_bp,
                s.mean_qscore,
                s.median_qscore,
            )
        } else {
            (
                m.basecalled_reads,
                m.total_yield_bp,
                m.n50_bp,
                m.mean_read_length_bp,
                m.median_read_length_bp,
                m.max_read_length_bp,
                report.quality.mean_qscore,
                report.quality.median_qscore,
            )
        };
        let analysis = &report.comparison.yield_by_minimum_length;
        let (mut counts, mut yields) = (serde_json::Map::new(), serde_json::Map::new());
        for t in &analysis.thresholds {
            let key = if t.minimum_length_bp == 1_000_000 {
                ">1m".to_owned()
            } else {
                format!(">{}kb", t.minimum_length_bp / 1000)
            };
            counts.insert(
                key.clone(),
                json!(if selected {
                    t.qscore_selected_read_count
                } else {
                    t.all_read_count
                }),
            );
            yields.insert(
                key,
                json!(
                    (if selected {
                        t.qscore_selected_yield_bp
                    } else {
                        t.all_yield_bp
                    }) as f64
                        / 1e9
                ),
            );
        }
        let ultra = if selected {
            &analysis.ultralong_qscore_selected
        } else {
            &analysis.ultralong_all
        };
        counts.insert("ultralong".into(), json!(ultra.read_count));
        yields.insert("ultralong".into(), json!(ultra.total_yield_bp as f64 / 1e9));
        summary.insert(
            if selected {
                format!("Q>={}", report.comparison.qscore_cutoff)
            } else {
                "All reads".into()
            },
            json!({
                "total.reads":reads,"total.bases":yield_bp,"total.gigabases":yield_bp as f64/1e9,
                "N50.length":n50,"mean.length":mean,"median.length":median,"max.length":maximum,
                "mean.q":mean_q,"median.q":median_q,"reads":counts,"gigabases":yields
            }),
        );
    }
    summary.insert("notes".into(),json!("All reads means positive-length basecalled reads; zero lengths/control exclusions are separate. Native passes_filtering is independent of Q cutoff. Length labels use inclusive >= boundaries. Unrounded per-read means/medians; core N50 reaches >=half; ultralong is the maximal longest-first prefix with strict-half N50 >=100kb. Input identification uses source labels/run_id, not original file paths."));
    serde_json::Value::Object(summary)
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
                        run_id: Some("fictional"),
                        flow_cell_id: None,
                        channel: 1,
                        mux: Some(1),
                        start_time: 0.0,
                        template_duration: None,
                        passed: Some(passed_flag),
                        length,
                        qscore,
                        end_reason: Some("normal"),
                        is_control: false,
                        num_events_template: None,
                    })
                    .unwrap();
                if passed_flag {
                    passed.push(length);
                    passed_q.push(qscore);
                }
                all_q.push(qscore);
            }
            let report = aggregator.finish(None).unwrap();
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
                    run_id: Some("fictional"),
                    flow_cell_id: None,
                    channel: 1,
                    mux: Some(1),
                    start_time: 0.0,
                    template_duration: None,
                    passed: Some(i % 2 == 0),
                    length: 100,
                    qscore: 12.0,
                    end_reason: Some("normal"),
                    is_control: false,
                    num_events_template: None,
                })
                .unwrap();
        }
        let lengths = &aggregator.channel_lengths[&1].partitions;
        assert_eq!(lengths.iter().map(Vec::len).sum::<usize>(), observations);
        assert_eq!(
            aggregator.qscores.iter().map(Vec::len).sum::<usize>(),
            observations
        );
        let capacity_bytes = 8
            * (lengths.iter().map(Vec::capacity).sum::<usize>()
                + aggregator.qscores.iter().map(Vec::capacity).sum::<usize>());
        assert!(capacity_bytes <= 16 * observations + 4 * (RESERVE_ELEMENTS + 256) * 8);
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
