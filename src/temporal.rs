//! Optional summary-population time plots. Numeric external sorting is private
//! to this path; no TSV extraction, complete read objects, or new QC engine.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeMap, BinaryHeap},
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

const CHUNK: usize = 65_536;
const FAN_IN: usize = 32;
const EXACT_POINTS: u64 = 8192;
const YIELD_DIVISIONS: u64 = 4096;
const MAX_HOUR: u64 = 100_000;

#[derive(Clone, Debug, Serialize)]
pub struct Temporal {
    pub method: &'static str,
    pub tie_policy: &'static str,
    pub all: Population,
    pub q_selected: Population,
}
#[derive(Clone, Debug, Serialize)]
pub struct Population {
    pub observations: u64,
    pub total_yield_bp: u64,
    pub hourly: Vec<Hour>,
    pub cumulative: Curve,
}
#[derive(Clone, Debug, Serialize)]
pub struct Hour {
    pub hour: u64,
    pub read_count: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Point {
    pub start_seconds: f64,
    pub cumulative_yield_bp: u64,
    pub observation_index: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Curve {
    pub representation: &'static str,
    pub points: Vec<Point>,
    pub omitted_points: u64,
    pub yield_error_bound_bp: u64,
    pub maximum_linear_interpolation_error_bp: f64,
    pub tied_timestamp_groups: u64,
    pub tied_observations: u64,
    pub maximum_tied_yield_bp: u64,
}

#[derive(Clone, Copy, Debug)]
struct Observation {
    time: f64,
    length: u64,
    order: u64,
    selected: bool,
}
impl PartialEq for Observation {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Observation {}
impl PartialOrd for Observation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Observation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time
            .total_cmp(&other.time)
            .then(self.order.cmp(&other.order))
    }
}
impl Observation {
    fn write(self, out: &mut impl Write) -> Result<()> {
        out.write_all(&self.time.to_le_bytes())?;
        out.write_all(&self.length.to_le_bytes())?;
        out.write_all(&self.order.to_le_bytes())?;
        out.write_all(&[u8::from(self.selected)])?;
        Ok(())
    }
    fn read(input: &mut impl Read) -> Result<Option<Self>> {
        let mut bytes = [0u8; 25];
        if input.read(&mut bytes[..1])? == 0 {
            return Ok(None);
        }
        input
            .read_exact(&mut bytes[1..])
            .context("truncated private temporal sort file")?;
        Ok(Some(Self {
            time: f64::from_le_bytes(bytes[..8].try_into().unwrap()),
            length: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            order: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            selected: bytes[24] != 0,
        }))
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "ontrunqc-time-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            let mut directory = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                directory.mode(0o700);
            }
            match directory.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(e).context(
                        "cannot create temporal sort scratch directory; check TMPDIR/free space",
                    );
                }
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct Builder {
    scratch: Scratch,
    buffer: Vec<Observation>,
    levels: Vec<Vec<PathBuf>>,
    next_file: u64,
    counts: [u64; 2],
    yields: [u64; 2],
    hours: BTreeMap<u64, [u64; 2]>,
    chunk_size: usize,
    sort_time: std::time::Duration,
    merge_time: std::time::Duration,
}
impl Builder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            scratch: Scratch::new()?,
            buffer: Vec::with_capacity(CHUNK),
            levels: Vec::new(),
            next_file: 0,
            counts: [0; 2],
            yields: [0; 2],
            hours: BTreeMap::new(),
            chunk_size: CHUNK,
            sort_time: std::time::Duration::ZERO,
            merge_time: std::time::Duration::ZERO,
        })
    }
    pub fn add(&mut self, record: &crate::record::Record<'_>, cutoff: f64) -> Result<()> {
        if record.is_control {
            return Ok(());
        }
        ensure!(
            record.start_time.is_finite() && record.start_time >= 0.0,
            "invalid temporal start time"
        );
        let hour = (record.start_time / 3600.0).floor();
        ensure!(
            hour <= MAX_HOUR as f64,
            "reference hourly plot exceeds {MAX_HOUR} hours; use native diagnostics without --minionqc-time-series"
        );
        let selected = record.qscore >= cutoff;
        let order = self.counts[0];
        let hour_counts = self.hours.entry(hour as u64).or_default();
        for (i, hour_count) in hour_counts
            .iter_mut()
            .enumerate()
            .take(usize::from(selected) + 1)
        {
            self.counts[i] = self.counts[i]
                .checked_add(1)
                .context("temporal count overflow")?;
            self.yields[i] = self.yields[i]
                .checked_add(record.length)
                .context("temporal yield overflow")?;
            *hour_count += 1;
        }
        self.buffer.push(Observation {
            time: if record.start_time == 0.0 {
                0.0
            } else {
                record.start_time
            },
            length: record.length,
            order,
            selected,
        });
        if self.buffer.len() == self.chunk_size {
            self.spill()?;
        }
        Ok(())
    }
    fn path(&mut self) -> PathBuf {
        self.next_file += 1;
        self.scratch.0.join(self.next_file.to_string())
    }
    fn spill(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let started = std::time::Instant::now();
        self.buffer.sort_unstable();
        self.sort_time += started.elapsed();
        let path = self.path();
        let mut out = BufWriter::new(File::create(&path)?);
        for value in &self.buffer {
            value.write(&mut out)?;
        }
        out.flush()?;
        self.buffer.clear();
        let mut carry = path;
        let mut level = 0;
        loop {
            if self.levels.len() == level {
                self.levels.push(Vec::new());
            }
            self.levels[level].push(carry);
            if self.levels[level].len() < FAN_IN {
                break;
            }
            let paths = std::mem::take(&mut self.levels[level]);
            carry = self.merge(paths)?;
            level += 1;
        }
        Ok(())
    }
    fn merge(&mut self, paths: Vec<PathBuf>) -> Result<PathBuf> {
        let started = std::time::Instant::now();
        let path = self.path();
        let mut out = BufWriter::new(File::create(&path)?);
        let mut readers = paths
            .iter()
            .map(|p| File::open(p).map(BufReader::new))
            .collect::<std::io::Result<Vec<_>>>()?;
        let mut heap = BinaryHeap::new();
        for (i, input) in readers.iter_mut().enumerate() {
            if let Some(value) = Observation::read(input)? {
                heap.push(Reverse((value, i)));
            }
        }
        while let Some(Reverse((value, index))) = heap.pop() {
            value.write(&mut out)?;
            if let Some(value) = Observation::read(&mut readers[index])? {
                heap.push(Reverse((value, index)));
            }
        }
        out.flush()?;
        drop(readers);
        for old in paths {
            fs::remove_file(old)?;
        }
        self.merge_time += started.elapsed();
        Ok(path)
    }
    pub fn finish(mut self) -> Result<Temporal> {
        self.spill()
            .context("cannot sort temporal observations; check scratch storage")?;
        let mut paths: Vec<_> = self.levels.iter_mut().flat_map(std::mem::take).collect();
        while paths.len() > 1 {
            let split = paths.len().saturating_sub(FAN_IN);
            let batch = paths.split_off(split);
            paths.push(self.merge(batch)?);
        }
        let curve_started = std::time::Instant::now();
        let mut curves = [
            CurveBuilder::new(self.counts[0], self.yields[0]),
            CurveBuilder::new(self.counts[1], self.yields[1]),
        ];
        if let Some(path) = paths.first() {
            let mut input = BufReader::new(File::open(path)?);
            while let Some(value) = Observation::read(&mut input)? {
                curves[0].add(value);
                if value.selected {
                    curves[1].add(value);
                }
            }
        }
        let mut curves = curves.map(CurveBuilder::finish);
        // Measure actual simplification error against EVERY sorted numeric
        // observation, not merely the retained display coordinates.
        if let Some(path) = paths.first() {
            let mut input = BufReader::new(File::open(path)?);
            let mut positions = [0usize; 2];
            let mut counts = [0u64; 2];
            let mut yields = [0u64; 2];
            while let Some(value) = Observation::read(&mut input)? {
                for i in 0..=usize::from(value.selected) {
                    counts[i] += 1;
                    yields[i] += value.length;
                    let curve = &mut curves[i];
                    let j = &mut positions[i];
                    while *j + 1 < curve.points.len()
                        && curve.points[*j + 1].observation_index < counts[i]
                    {
                        *j += 1;
                    }
                    let a = &curve.points[*j];
                    let b = &curve.points[(*j + 1).min(curve.points.len() - 1)];
                    let error = if a.start_seconds == b.start_seconds {
                        0.0
                    } else {
                        let weight = ((value.time - a.start_seconds)
                            / (b.start_seconds - a.start_seconds))
                            .clamp(0.0, 1.0);
                        ((b.cumulative_yield_bp - a.cumulative_yield_bp) as f64 * weight
                            - (yields[i] - a.cumulative_yield_bp) as f64)
                            .abs()
                    };
                    curve.maximum_linear_interpolation_error_bp =
                        curve.maximum_linear_interpolation_error_bp.max(error);
                }
            }
        }
        eprintln!(
            "Reference time-series numeric work: {} observations; chunk comparisons {:.6}s, merge I/O {:.6}s, curve/measurement scans {:.6}s",
            self.counts[0],
            self.sort_time.as_secs_f64(),
            self.merge_time.as_secs_f64(),
            curve_started.elapsed().as_secs_f64()
        );
        let max_hour = self.hours.keys().next_back().copied();
        let [all_curve, selected_curve] = curves;
        let population = |i, curve| Population {
            observations: self.counts[i],
            total_yield_bp: self.yields[i],
            hourly: max_hour
                .map(|max| {
                    (0..=max)
                        .map(|hour| Hour {
                            hour,
                            read_count: self.hours.get(&hour).map_or(0, |n| n[i]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            cumulative: curve,
        };
        Ok(Temporal {
            method: "summary_hours_and_sorted_read_start_cumulative",
            tie_policy: "stable_source_file_then_record_order; R channel-merge tie ordering may differ; compare tied-time envelopes",
            all: population(0, all_curve),
            q_selected: population(1, selected_curve),
        })
    }
}

struct CurveBuilder {
    curve: Curve,
    count: u64,
    total: u64,
    previous: Option<Point>,
    exact: bool,
    tie_count: u64,
    tie_yield: u64,
}
impl CurveBuilder {
    fn new(count: u64, total: u64) -> Self {
        let exact = count <= EXACT_POINTS;
        Self {
            curve: Curve {
                representation: if exact {
                    "exact_observations"
                } else {
                    "bounded_monotone_polyline"
                },
                points: Vec::new(),
                omitted_points: 0,
                yield_error_bound_bp: if exact {
                    0
                } else {
                    total.div_ceil(YIELD_DIVISIONS)
                },
                maximum_linear_interpolation_error_bp: 0.0,
                tied_timestamp_groups: 0,
                tied_observations: 0,
                maximum_tied_yield_bp: 0,
            },
            count: 0,
            total: 0,
            previous: None,
            exact,
            tie_count: 0,
            tie_yield: 0,
        }
    }
    fn push(&mut self, point: Point) {
        if self
            .curve
            .points
            .last()
            .is_none_or(|p| p.observation_index != point.observation_index)
        {
            self.curve.points.push(point);
        }
    }
    fn end_tie(&mut self) {
        if self.tie_count > 1 {
            self.curve.tied_timestamp_groups += 1;
            self.curve.tied_observations += self.tie_count;
            self.curve.maximum_tied_yield_bp = self.curve.maximum_tied_yield_bp.max(self.tie_yield);
        }
    }
    fn add(&mut self, value: Observation) {
        if self
            .previous
            .as_ref()
            .is_none_or(|p| p.start_seconds != value.time)
        {
            self.end_tie();
            self.tie_count = 0;
            self.tie_yield = 0;
        }
        self.tie_count += 1;
        self.tie_yield += value.length;
        self.count += 1;
        self.total += value.length;
        let point = Point {
            start_seconds: value.time,
            cumulative_yield_bp: self.total,
            observation_index: self.count,
        };
        if self.exact || self.curve.points.is_empty() {
            self.push(point.clone());
        } else if self.total - self.curve.points.last().unwrap().cumulative_yield_bp
            > self.curve.yield_error_bound_bp
        {
            if let Some(previous) = self.previous.clone() {
                self.push(previous);
            }
            self.push(point.clone());
        }
        self.previous = Some(point);
    }
    fn finish(mut self) -> Curve {
        self.end_tie();
        if let Some(point) = self.previous.take() {
            self.push(point);
        }
        self.curve.omitted_points = self.count - self.curve.points.len() as u64;
        self.curve
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(time: f64, length: u64, qscore: f64) -> crate::record::Record<'static> {
        crate::record::Record {
            line: 2,
            is_control: false,
            num_events_template: None,
            flow_cell_id: None,
            run_id: None,
            channel: 1,
            mux: None,
            start_time: time,
            template_duration: None,
            passed: None,
            length,
            qscore,
            end_reason: None,
        }
    }
    fn values(v: &serde_json::Value) -> Vec<f64> {
        if let Some(array) = v.as_array() {
            array.iter().map(|n| n.as_f64().unwrap()).collect()
        } else {
            vec![v.as_f64().unwrap()]
        }
    }
    #[test]
    fn actual_pinned_r_temporal_goldens_match_counts_and_tied_envelopes() {
        let gold: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/reference/temporal-layers-golden.json"
        ))
        .unwrap();
        for (name, case) in gold["cases"].as_object().unwrap() {
            let times = values(&case["time"]);
            let lengths = values(&case["length"]);
            let qs = values(&case["q"]);
            let mut builder = Builder::new().unwrap();
            for i in 0..times.len() {
                builder
                    .add(
                        &record(times[i], lengths[i] as u64, qs[i]),
                        case["cutoff"].as_f64().unwrap(),
                    )
                    .unwrap();
            }
            let data = builder.finish().unwrap();
            for (group, population) in [(1, &data.all), (2, &data.q_selected)] {
                let hours: Vec<_> = case["hourly_layer"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["group"] == group)
                    .collect();
                assert_eq!(hours.len(), population.hourly.len(), "{name}");
                for (r, h) in hours.iter().zip(&population.hourly) {
                    assert_eq!(r["x"].as_f64().unwrap(), h.hour as f64, "{name}");
                    assert_eq!(r["y"].as_f64().unwrap(), h.read_count as f64, "{name}");
                }
                let mut expected = BTreeMap::<u64, u64>::new();
                for r in case["cumulative_layer"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["group"] == group)
                {
                    let time = (r["x"].as_f64().unwrap() * 3600.0).round() as u64;
                    let y = (r["y"].as_f64().unwrap() * 1e9).round() as u64;
                    expected
                        .entry(time)
                        .and_modify(|v| *v = (*v).max(y))
                        .or_insert(y);
                }
                let mut actual = BTreeMap::<u64, u64>::new();
                for p in &population.cumulative.points {
                    actual.insert(p.start_seconds as u64, p.cumulative_yield_bp);
                }
                assert_eq!(actual, expected, "{name}");
                assert_eq!(
                    population.cumulative.maximum_linear_interpolation_error_bp,
                    0.0
                );
            }
        }
    }
    #[test]
    fn external_multilevel_merge_preserves_stable_numeric_order_and_cleans_up() {
        let mut b = Builder::new().unwrap();
        b.chunk_size = 4;
        let path = b.scratch.0.clone();
        let mut expected = Vec::new();
        for i in 0..1000 {
            let t = ((i * 137) % 317) as f64;
            let len = (i + 1) as u64;
            b.add(&record(t, len, 7.0), 7.0).unwrap();
            expected.push((t, i, len));
        }
        expected.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let data = b.finish().unwrap();
        let mut sum = 0;
        for (p, (time, _, length)) in data.all.cumulative.points.iter().zip(expected) {
            sum += length;
            assert_eq!(p.start_seconds, time);
            assert_eq!(p.cumulative_yield_bp, sum);
        }
        assert_eq!(data.all.total_yield_bp, sum);
        assert!(!path.exists());
    }
    #[test]
    fn dense_monotone_curve_preserves_jumps_and_measures_every_observation() {
        let mut b = Builder::new().unwrap();
        let n = 100_000;
        let length = |i| {
            if i == 50000 {
                1_000_000
            } else {
                (i % 127 + 1) as u64
            }
        };
        for i in (0..n).rev() {
            b.add(
                &record(i as f64, length(i), if i % 3 == 0 { 0.0 } else { 10.0 }),
                7.0,
            )
            .unwrap();
        }
        let data = b.finish().unwrap();
        for (selected, p) in [(false, &data.all), (true, &data.q_selected)] {
            let c = &p.cumulative;
            assert!(c.points.len() <= 8194);
            assert!(c.omitted_points > 0);
            assert!(c.maximum_linear_interpolation_error_bp <= c.yield_error_bound_bp as f64);
            assert_eq!(
                c.points.last().unwrap().cumulative_yield_bp,
                p.total_yield_bp
            );
            assert!(c.points.iter().any(|p| p.start_seconds == 50000.0));
            assert!(c.points.iter().any(|p| p.start_seconds == 49999.0));
            let mut sum = 0;
            let mut j = 0;
            let mut error: f64 = 0.0;
            for i in 0..n {
                if selected && i % 3 == 0 {
                    continue;
                }
                sum += length(i);
                while j + 1 < c.points.len() && c.points[j + 1].start_seconds < (i as f64) {
                    j += 1;
                }
                let a = &c.points[j];
                let z = &c.points[(j + 1).min(c.points.len() - 1)];
                if z.start_seconds > a.start_seconds {
                    let w = (i as f64 - a.start_seconds) / (z.start_seconds - a.start_seconds);
                    error = error.max(
                        ((z.cumulative_yield_bp - a.cumulative_yield_bp) as f64 * w
                            - (sum - a.cumulative_yield_bp) as f64)
                            .abs(),
                    );
                }
            }
            assert_eq!(error, c.maximum_linear_interpolation_error_bp);
        }
    }
    #[test]
    fn empty_controls_zero_yield_and_unselected_populations_are_explicit() {
        let empty = Builder::new().unwrap().finish().unwrap();
        assert!(empty.all.hourly.is_empty());
        let mut b = Builder::new().unwrap();
        let mut r = record(0.0, 100, 20.0);
        r.is_control = true;
        b.add(&r, 7.0).unwrap();
        for _ in 0..10_000 {
            b.add(&record(7200.0, 0, -3.0), 7.0).unwrap();
        }
        let d = b.finish().unwrap();
        assert_eq!(d.all.observations, 10_000);
        assert_eq!(d.q_selected.observations, 0);
        assert_eq!(d.all.cumulative.points.len(), 2);
        assert_eq!(d.all.cumulative.tied_observations, 10_000);
        assert_eq!(d.q_selected.hourly.len(), 3);
        assert_eq!(d.all.hourly[2].read_count, 10_000);
    }
    #[test]
    fn failed_sort_and_excessive_time_are_actionable_and_scratch_is_removed() {
        let mut b = Builder::new().unwrap();
        b.chunk_size = 1;
        let path = b.scratch.0.clone();
        b.add(&record(0.0, 10, 7.0), 7.0).unwrap();
        fs::remove_file(&b.levels[0][0]).unwrap();
        assert!(b.finish().is_err());
        assert!(!path.exists());
        let mut b = Builder::new().unwrap();
        assert!(
            b.add(&record((MAX_HOUR + 1) as f64 * 3600.0, 1, 7.0), 7.0)
                .unwrap_err()
                .to_string()
                .contains("use native diagnostics")
        );
    }
}
