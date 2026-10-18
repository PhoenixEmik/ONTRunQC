//! Complete-data MinION display accumulators, separate from scientific QC.
//! Small inputs retain every point; dense inputs use a bounds pass followed by
//! fixed-resolution counting. No reservoir, per-read DOM, or unbounded storage.
use crate::{layout::FlowcellLayout, record::Record};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{cmp::Ordering, collections::BTreeMap};

pub const POINT_LIMIT: usize = 4096;
pub const PANEL_WIDTH: usize = 64;
pub const PANEL_HEIGHT: usize = 40;
pub const SCATTER_WIDTH: usize = 512;
pub const SCATTER_HEIGHT: usize = 256;
const PANEL_PIXELS: usize = PANEL_WIDTH * PANEL_HEIGHT;

#[derive(Clone, Debug, Serialize)]
pub struct Point {
    pub channel: u32,
    pub start_seconds: f64,
    pub length_bp: u64,
    pub qscore: f64,
    pub events_per_base: Option<f64>,
}
impl Point {
    fn from_record(r: &Record<'_>) -> Self {
        Self {
            channel: r.channel,
            start_seconds: r.start_time,
            length_bp: r.length,
            qscore: r.qscore,
            events_per_base: r.num_events_template.map(|n| n as f64 / r.length as f64),
        }
    }
    fn compare(&self, other: &Self) -> Ordering {
        self.start_seconds
            .total_cmp(&other.start_seconds)
            .then(self.channel.cmp(&other.channel))
            .then(self.length_bp.cmp(&other.length_bp))
            .then(self.qscore.total_cmp(&other.qscore))
            .then_with(|| match (self.events_per_base, other.events_per_base) {
                (Some(a), Some(b)) => a.total_cmp(&b),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Bounds {
    pub start_seconds: [f64; 2],
    pub length_bp: [u64; 2],
    pub qscore: [f64; 2],
    pub positive_events_per_base: Option<[f64; 2]>,
}
impl Bounds {
    fn observe(&mut self, p: &Point) {
        self.start_seconds[0] = self.start_seconds[0].min(p.start_seconds);
        self.start_seconds[1] = self.start_seconds[1].max(p.start_seconds);
        self.length_bp[0] = self.length_bp[0].min(p.length_bp);
        self.length_bp[1] = self.length_bp[1].max(p.length_bp);
        self.qscore[0] = self.qscore[0].min(p.qscore);
        self.qscore[1] = self.qscore[1].max(p.qscore);
        if let Some(v) = p.events_per_base.filter(|v| *v > 0.0) {
            let range = self.positive_events_per_base.get_or_insert([v, v]);
            range[0] = range[0].min(v);
            range[1] = range[1].max(v);
        }
    }
    pub fn log_length(&self) -> [f64; 2] {
        self.length_bp.map(|v| (v as f64).log10())
    }
}

#[derive(Default, Debug, Serialize)]
pub struct Channel {
    pub channel: u32,
    pub eligible_reads: u64,
    pub zero_length_records: u64,
    pub missing_events_reads: u64,
    pub zero_events_reads: u64,
    /// Actual min/max length, Q, and positive events/base representatives.
    pub extrema: Vec<Point>,
}
impl Channel {
    fn observe(&mut self, p: &Point) {
        self.eligible_reads += 1;
        self.missing_events_reads += u64::from(p.events_per_base.is_none());
        self.zero_events_reads += u64::from(p.events_per_base == Some(0.0));
        if self.extrema.is_empty() {
            self.extrema = vec![p.clone(); 4];
        }
        for i in 0..4 {
            let old = &self.extrema[i];
            let order = if i < 2 {
                p.length_bp.cmp(&old.length_bp)
            } else {
                p.qscore.total_cmp(&old.qscore)
            };
            if (if i % 2 == 0 {
                order.is_lt()
            } else {
                order.is_gt()
            }) || (order.is_eq() && p.compare(old).is_lt())
            {
                self.extrema[i] = p.clone();
            }
        }
        if let Some(v) = p.events_per_base.filter(|v| *v > 0.0) {
            if self.extrema.len() == 4 {
                self.extrema.extend([p.clone(), p.clone()]);
            }
            for i in 4..6 {
                let old = &self.extrema[i];
                let order = v.total_cmp(&old.events_per_base.unwrap());
                if (if i == 4 { order.is_lt() } else { order.is_gt() })
                    || (order.is_eq() && p.compare(old).is_lt())
                {
                    self.extrema[i] = p.clone();
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Visualization {
    pub method: &'static str,
    pub population: &'static str,
    pub point_limit: usize,
    pub eligible_reads: u64,
    pub zero_length_records: u64,
    pub missing_events_reads: u64,
    pub zero_events_reads: u64,
    pub additional_input_passes: u32,
    pub bounds: Option<Bounds>,
    pub channels: Vec<Channel>,
    pub points: Vec<Point>,
    pub overview: Option<Raster>,
    pub scatter: Option<Raster>,
}
#[derive(Debug, Serialize)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub total_reads: u64,
    pub maximum_pixel_reads: u64,
    /// Sparse [row-major pixel index, exact read count], sorted by index.
    /// Overview indices address the full physical atlas (16x32 panels).
    pub counts: Vec<[u64; 2]>,
    pub color_reduction: &'static str,
    pub png_data_uri: String,
}

pub struct Tracker {
    bounds: Option<Bounds>,
    eligible: u64,
    channels: BTreeMap<u32, Channel>,
    points: Option<Vec<Point>>,
}
impl Default for Tracker {
    fn default() -> Self {
        Self {
            bounds: None,
            eligible: 0,
            channels: BTreeMap::new(),
            points: Some(Vec::new()),
        }
    }
}
impl Tracker {
    pub fn observe(&mut self, r: &Record<'_>) {
        if r.is_control {
            return;
        }
        let channel = self.channels.entry(r.channel).or_insert_with(|| Channel {
            channel: r.channel,
            ..Channel::default()
        });
        if r.length == 0 {
            channel.zero_length_records += 1;
            return;
        }
        let point = Point::from_record(r);
        self.eligible += 1;
        channel.observe(&point);
        self.bounds
            .get_or_insert(Bounds {
                start_seconds: [point.start_seconds; 2],
                length_bp: [point.length_bp; 2],
                qscore: [point.qscore; 2],
                positive_events_per_base: None,
            })
            .observe(&point);
        if let Some(points) = &mut self.points {
            if points.len() < POINT_LIMIT {
                points.push(point);
            } else {
                self.points = None;
            }
        }
    }
    pub fn prepare(self) -> Renderer {
        let dense = self.points.is_none();
        let mut channels: Vec<_> = self.channels.into_values().collect();
        for c in &mut channels {
            c.extrema.sort_by(Point::compare);
            c.extrema.dedup_by(|a, b| a.compare(b).is_eq());
        }
        let mut points = self.points.unwrap_or_default();
        points.sort_by(Point::compare);
        Renderer {
            seen: vec![0; 512],
            grids: dense.then(|| Grids {
                overview: vec![OverviewCell::default(); 512 * PANEL_PIXELS],
                scatter: vec![ScatterCell::default(); SCATTER_WIDTH * SCATTER_HEIGHT],
            }),
            view: Visualization {
                method: if dense {
                    "full_count_pixel_aggregation"
                } else {
                    "exact_points"
                },
                population: "all_positive_length_summary_records_after_control_exclusion",
                point_limit: POINT_LIMIT,
                eligible_reads: self.eligible,
                zero_length_records: channels.iter().map(|c| c.zero_length_records).sum(),
                missing_events_reads: channels.iter().map(|c| c.missing_events_reads).sum(),
                zero_events_reads: channels.iter().map(|c| c.zero_events_reads).sum(),
                additional_input_passes: u32::from(dense),
                bounds: self.bounds,
                channels,
                points,
                overview: None,
                scatter: None,
            },
        }
    }
}

#[derive(Clone, Default)]
struct OverviewCell {
    count: u64,
    min_q: f64,
}
#[derive(Clone, Default)]
struct ScatterCell {
    count: u64,
    rgb_sum: [u128; 3],
}
struct Grids {
    overview: Vec<OverviewCell>,
    scatter: Vec<ScatterCell>,
}
pub struct Renderer {
    view: Visualization,
    grids: Option<Grids>,
    seen: Vec<u64>,
}
impl Renderer {
    pub fn needs_pass(&self) -> bool {
        self.grids.is_some()
    }
    pub fn add(&mut self, r: &Record<'_>) -> Result<()> {
        if r.is_control || r.length == 0 {
            return Ok(());
        }
        let bounds = self.view.bounds.as_ref().unwrap();
        ensure!(
            (1..=512).contains(&r.channel),
            "MinION visualization: invalid channel {}",
            r.channel
        );
        ensure!(
            (bounds.start_seconds[0]..=bounds.start_seconds[1]).contains(&r.start_time)
                && (bounds.length_bp[0]..=bounds.length_bp[1]).contains(&r.length)
                && (bounds.qscore[0]..=bounds.qscore[1]).contains(&r.qscore),
            "input changed bounds during MinION visualization pass"
        );
        let Some(grids) = &mut self.grids else {
            return Ok(());
        };
        let events = r.num_events_template.map(|n| n as f64 / r.length as f64);
        if let Some(v) = events.filter(|v| *v > 0.0) {
            ensure!(
                bounds
                    .positive_events_per_base
                    .is_some_and(|b| (b[0]..=b[1]).contains(&v)),
                "input changed events/base bounds during MinION visualization pass"
            );
        }
        self.seen[r.channel as usize - 1] += 1;
        let x = pixel(r.start_time, bounds.start_seconds, PANEL_WIDTH);
        let y =
            PANEL_HEIGHT - 1 - pixel((r.length as f64).log10(), bounds.log_length(), PANEL_HEIGHT);
        let cell =
            &mut grids.overview[(r.channel as usize - 1) * PANEL_PIXELS + y * PANEL_WIDTH + x];
        cell.min_q = if cell.count == 0 {
            r.qscore
        } else {
            cell.min_q.min(r.qscore)
        };
        cell.count += 1;
        let x = pixel(
            (r.length as f64).log10(),
            bounds.log_length(),
            SCATTER_WIDTH,
        );
        let y = SCATTER_HEIGHT - 1 - pixel(r.qscore, bounds.qscore, SCATTER_HEIGHT);
        let cell = &mut grids.scatter[y * SCATTER_WIDTH + x];
        cell.count += 1;
        let color = events_color(events, bounds);
        for (sum, v) in cell.rgb_sum.iter_mut().zip(color) {
            *sum += u128::from(v);
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<Visualization> {
        if let Some(grids) = self.grids.take() {
            ensure!(
                self.seen.iter().sum::<u64>() == self.view.eligible_reads,
                "input changed total read count during MinION visualization pass"
            );
            for c in &self.view.channels {
                ensure!(
                    self.seen[c.channel as usize - 1] == c.eligible_reads,
                    "input changed read count during MinION visualization pass for channel {}",
                    c.channel
                );
            }
            let bounds = self.view.bounds.as_ref().unwrap();
            let (width, height) = (16 * PANEL_WIDTH, 32 * PANEL_HEIGHT);
            let maximum = grids.overview.iter().map(|c| c.count).max().unwrap_or(0);
            let mut rgba = vec![0; width * height * 4];
            let mut counts = Vec::new();
            for (index, cell) in grids
                .overview
                .iter()
                .enumerate()
                .filter(|(_, c)| c.count > 0)
            {
                let channel = index / PANEL_PIXELS + 1;
                let offset = index % PANEL_PIXELS;
                let coordinate = FlowcellLayout::MinionR95
                    .coordinate(channel as u32)
                    .unwrap();
                let atlas = (coordinate.row as usize * PANEL_HEIGHT + offset / PANEL_WIDTH) * width
                    + coordinate.column as usize * PANEL_WIDTH
                    + offset % PANEL_WIDTH;
                counts.push([atlas as u64, cell.count]);
                rgba[atlas * 4..atlas * 4 + 3].copy_from_slice(&quality_color(cell.min_q, bounds));
                rgba[atlas * 4 + 3] = density_alpha(cell.count, maximum);
            }
            counts.sort_unstable_by_key(|r| r[0]);
            self.view.overview = Some(raster(
                width,
                height,
                counts,
                maximum,
                &rgba,
                "minimum per-read Q in each pixel; opacity encodes log1p(count); actual extrema overlaid",
            )?);
            drop(rgba);
            let maximum = grids.scatter.iter().map(|c| c.count).max().unwrap_or(0);
            let mut rgba = vec![0; SCATTER_WIDTH * SCATTER_HEIGHT * 4];
            let mut counts = Vec::new();
            for (i, c) in grids
                .scatter
                .iter()
                .enumerate()
                .filter(|(_, c)| c.count > 0)
            {
                counts.push([i as u64, c.count]);
                for j in 0..3 {
                    rgba[i * 4 + j] = (c.rgb_sum[j] / u128::from(c.count)) as u8;
                }
                rgba[i * 4 + 3] = density_alpha(c.count, maximum);
            }
            self.view.scatter = Some(raster(
                SCATTER_WIDTH,
                SCATTER_HEIGHT,
                counts,
                maximum,
                &rgba,
                "arithmetic mean of quantized mapped per-read event colors; opacity encodes log1p(count); actual extrema overlaid",
            )?);
        }
        Ok(self.view)
    }
}
fn raster(
    width: usize,
    height: usize,
    counts: Vec<[u64; 2]>,
    maximum: u64,
    rgba: &[u8],
    reduction: &'static str,
) -> Result<Raster> {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
        writer.finish()?;
    }
    Ok(Raster {
        width,
        height,
        total_reads: counts.iter().map(|r| r[1]).sum(),
        maximum_pixel_reads: maximum,
        counts,
        color_reduction: reduction,
        png_data_uri: format!("data:image/png;base64,{}", STANDARD.encode(png)),
    })
}
pub fn density_alpha(n: u64, maximum: u64) -> u8 {
    (255.0 * (0.25 + 0.75 * (n as f64).ln_1p() / (maximum as f64).ln_1p())).round() as u8
}
pub fn fraction(v: f64, range: [f64; 2]) -> f64 {
    if range[0] == range[1] {
        return 0.5;
    }
    let width = range[1] - range[0];
    if width.is_finite() {
        return ((v - range[0]) / width).clamp(0.0, 1.0);
    }
    let scale = range[0].abs().max(range[1].abs()).max(1.0);
    ((v / scale - range[0] / scale) / (range[1] / scale - range[0] / scale)).clamp(0.0, 1.0)
}
fn pixel(v: f64, range: [f64; 2], size: usize) -> usize {
    ((fraction(v, range) * size as f64).floor() as usize).min(size - 1)
}
pub fn quality_color(q: f64, b: &Bounds) -> [u8; 3] {
    palette(fraction(q, b.qscore), false)
}
pub fn events_color(events: Option<f64>, b: &Bounds) -> [u8; 3] {
    match events {
        Some(v) if v > 0.0 => palette(
            fraction(v.ln(), b.positive_events_per_base.unwrap().map(f64::ln)),
            true,
        ),
        Some(_) => [51, 65, 85],
        None => [148, 163, 184],
    }
}
// Nine anchors of viridis / inferno (viridisLite 0.4.3); continuous interpolation
// retains the reference's perceptually ordered color meaning, not ggplot pixels.
pub fn palette(t: f64, inferno: bool) -> [u8; 3] {
    const VIRIDIS: [[u8; 3]; 9] = [
        [68, 1, 84],
        [71, 45, 123],
        [59, 82, 139],
        [44, 114, 142],
        [33, 144, 140],
        [39, 173, 129],
        [93, 200, 99],
        [170, 220, 50],
        [253, 231, 37],
    ];
    const INFERNO: [[u8; 3]; 9] = [
        [0, 0, 4],
        [33, 12, 74],
        [86, 16, 110],
        [137, 34, 106],
        [187, 55, 84],
        [227, 89, 50],
        [249, 140, 10],
        [249, 201, 50],
        [252, 255, 164],
    ];
    let colors = if inferno { INFERNO } else { VIRIDIS };
    let at = t.clamp(0.0, 1.0) * 8.0;
    let i = (at.floor() as usize).min(7);
    let f = at - i as f64;
    std::array::from_fn(|j| {
        (colors[i][j] as f64 * (1.0 - f) + colors[i + 1][j] as f64 * f).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(
        channel: u32,
        length: u64,
        q: f64,
        time: f64,
        events: Option<u64>,
    ) -> Record<'static> {
        Record {
            channel,
            length,
            qscore: q,
            start_time: time,
            num_events_template: events,
            line: 1,
            is_control: false,
            flow_cell_id: None,
            run_id: None,
            mux: None,
            template_duration: None,
            passed: None,
            end_reason: None,
        }
    }
    fn render(rows: &[Record<'_>]) -> Visualization {
        let mut tracker = Tracker::default();
        for r in rows {
            tracker.observe(r);
        }
        let mut renderer = tracker.prepare();
        if renderer.needs_pass() {
            for r in rows {
                renderer.add(r).unwrap();
            }
        }
        renderer.finish().unwrap()
    }
    #[test]
    fn exact_points_include_every_channel_signed_q_and_explicit_zero_length_exclusion() {
        let mut rows: Vec<_> = (1..=512)
            .map(|c| record(c, c as u64, -3.0 + c as f64, 0.0, None))
            .collect();
        rows.push(record(1, 0, 0.0, 0.0, Some(0)));
        let v = render(&rows);
        assert_eq!(v.eligible_reads, 512);
        assert_eq!(v.channels.len(), 512);
        assert_eq!(v.points.len(), 512);
        assert_eq!(v.additional_input_passes, 0);
        assert_eq!(v.zero_length_records, 1);
        assert_eq!(v.missing_events_reads, 512);
        assert_eq!(v.bounds.unwrap().qscore, [-2.0, 509.0]);
        assert!(v.overview.is_none());
    }
    #[test]
    fn dense_pixels_conserve_counts_and_preserve_rare_extremes_independent_of_order() {
        let mut rows: Vec<_> = (0..5000)
            .map(|_| record(1, 100, 10.0, 900.0, Some(200)))
            .collect();
        rows.extend([
            record(512, 1, -2.0, 0.0, None),
            record(33, 1_000_000, 100.0, 3600.0, Some(0)),
            record(512, 10_000, 2.0, 1.0, Some(1)),
            record(1, 1000, 20.0, 30.0, Some(1_000_000)),
        ]);
        let v = render(&rows);
        assert_eq!(v.eligible_reads, 5004);
        assert!(v.points.is_empty());
        assert_eq!(v.additional_input_passes, 1);
        assert_eq!(v.missing_events_reads, 1);
        assert_eq!(v.zero_events_reads, 1);
        let scatter = v.scatter.as_ref().unwrap();
        let overview = v.overview.as_ref().unwrap();
        for raster in [scatter, overview] {
            assert_eq!(raster.total_reads, 5004);
            assert_eq!(raster.counts.iter().map(|r| r[1]).sum::<u64>(), 5004);
        }
        // Independent coordinates: floor((log10(100)-0)/6*512)=170;
        // floor((10-(-2))/102*256)=30, reverse Y to top-first rows.
        assert!(scatter.counts.contains(&[(225 * 512 + 170) as u64, 5000]));
        // Channel 1 is at physical col 15,row 0. Its local main pixel is (16,26).
        assert!(
            overview
                .counts
                .contains(&[(26 * 1024 + 15 * 64 + 16) as u64, 5000])
        );
        assert_eq!(overview.maximum_pixel_reads, 5000);
        let extremes: Vec<_> = v.channels.iter().flat_map(|c| &c.extrema).collect();
        assert!(extremes.iter().any(|p| p.length_bp == 1_000_000));
        assert!(extremes.iter().any(|p| p.qscore == -2.0));
        assert!(extremes.iter().any(|p| p.events_per_base == Some(1000.0)));
        let mut channel_counts = BTreeMap::<u32, u64>::new();
        for &[index, n] in &overview.counts {
            let row = index as usize / overview.width;
            let col = index as usize % overview.width;
            let channel = (1..=512)
                .find(|&c| {
                    let p = FlowcellLayout::MinionR95.coordinate(c).unwrap();
                    p.row as usize == row / PANEL_HEIGHT && p.column as usize == col / PANEL_WIDTH
                })
                .unwrap();
            *channel_counts.entry(channel).or_default() += n;
        }
        for c in &v.channels {
            assert_eq!(
                channel_counts.get(&c.channel).copied().unwrap_or(0),
                c.eligible_reads
            );
        }
        let bytes = STANDARD
            .decode(
                overview
                    .png_data_uri
                    .strip_prefix("data:image/png;base64,")
                    .unwrap(),
            )
            .unwrap();
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        let frame = decoder.next_frame(&mut pixels).unwrap();
        assert_eq!((frame.width, frame.height), (1024, 1280));
        assert_eq!(
            pixels.chunks_exact(4).filter(|p| p[3] > 0).count(),
            overview.counts.len()
        );
        rows.reverse();
        assert_eq!(
            serde_json::to_value(&v).unwrap(),
            serde_json::to_value(render(&rows)).unwrap()
        );
    }
    #[test]
    fn empty_zero_only_and_missing_or_zero_events_never_get_invalid_logarithms() {
        for rows in [vec![], vec![record(1, 0, 0.0, 0.0, Some(0))]] {
            let v = render(&rows);
            assert_eq!(v.eligible_reads, 0);
            assert!(v.bounds.is_none());
        }
        let v = render(&[
            record(1, 10, 1.0, 0.0, None),
            record(2, 10, 1.0, 0.0, Some(0)),
        ]);
        let b = v.bounds.unwrap();
        assert!(b.positive_events_per_base.is_none());
        assert_eq!(events_color(None, &b), [148, 163, 184]);
        assert_eq!(events_color(Some(0.0), &b), [51, 65, 85]);
        assert_eq!(fraction(1.0, [1.0, 1.0]), 0.5);
        assert!(fraction(1e308, [-1e308, 1e308]).is_finite());
        let b = Bounds {
            positive_events_per_base: Some([0.1, 10.0]),
            ..b
        };
        assert_eq!(events_color(Some(1.0), &b), palette(0.5, true)); // log, never log1p
    }
    #[test]
    fn dense_second_pass_requires_every_observation() {
        let mut tracker = Tracker::default();
        for _ in 0..=POINT_LIMIT {
            tracker.observe(&record(1, 100, 10.0, 0.0, None));
        }
        let renderer = tracker.prepare();
        assert!(renderer.needs_pass());
        assert!(
            renderer
                .finish()
                .unwrap_err()
                .to_string()
                .contains("read count")
        );
    }
    #[test]
    fn dense_color_reductions_keep_minimum_q_and_every_mapped_event_color() {
        let mut rows: Vec<_> = (0..4098)
            .map(|i| record(1, 100, 10.0, 0.0, [None, Some(0), Some(100)][i % 3]))
            .collect();
        rows.push(record(1, 100, 1.0, 0.0, Some(100)));
        let view = render(&rows);
        let bounds = view.bounds.as_ref().unwrap();
        let pixel_rgba = |raster: &Raster, index: u64| {
            let bytes = STANDARD
                .decode(raster.png_data_uri.split_once(',').unwrap().1)
                .unwrap();
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            reader.next_frame(&mut pixels).unwrap();
            <[u8; 4]>::try_from(&pixels[index as usize * 4..index as usize * 4 + 4]).unwrap()
        };
        let overview = view.overview.as_ref().unwrap();
        assert_eq!(overview.counts.len(), 1);
        assert_eq!(overview.counts[0][1], 4099);
        let color = quality_color(1.0, bounds);
        assert_eq!(
            pixel_rgba(overview, overview.counts[0][0]),
            [color[0], color[1], color[2], 255]
        );
        let scatter = view.scatter.as_ref().unwrap();
        let dense = scatter.counts.iter().find(|c| c[1] == 4098).unwrap();
        let colors = [
            events_color(None, bounds),
            events_color(Some(0.0), bounds),
            events_color(Some(1.0), bounds),
        ];
        let mean: [u8; 3] = std::array::from_fn(|i| {
            (colors.iter().map(|c| u16::from(c[i])).sum::<u16>() / 3) as u8
        });
        assert_eq!(
            pixel_rgba(scatter, dense[0]),
            [mean[0], mean[1], mean[2], 255]
        );
    }
    #[test]
    #[ignore = "optional public MinION validation supplies PG_MINION_INPUT/OUTPUT"]
    fn export_public_visualization() {
        use std::{fs::File, time::Instant};
        let input = std::env::var("PG_MINION_INPUT").unwrap();
        let read = || {
            let mut r = crate::input::SummaryReader::new(File::open(&input).unwrap()).unwrap();
            r.allow_multiple_runs();
            r
        };
        let begin = Instant::now();
        let mut tracker = Tracker::default();
        let mut reader = read();
        while let Some(r) = reader.next_record().unwrap() {
            assert!((1..=512).contains(&r.channel));
            tracker.observe(&r);
        }
        let bounds_pass_seconds = begin.elapsed().as_secs_f64();
        let begin = Instant::now();
        let mut renderer = tracker.prepare();
        if renderer.needs_pass() {
            let mut reader = read();
            while let Some(r) = reader.next_record().unwrap() {
                renderer.add(&r).unwrap();
            }
        }
        let pixel_pass_seconds = begin.elapsed().as_secs_f64();
        let begin = Instant::now();
        let visualization = renderer.finish().unwrap();
        let rasterization_seconds = begin.elapsed().as_secs_f64();
        let value = serde_json::json!({"visualization":visualization,"timings":{
            "bounds_pass_seconds":bounds_pass_seconds,"pixel_pass_seconds":pixel_pass_seconds,
            "rasterization_seconds":rasterization_seconds}});
        serde_json::to_writer(
            File::create(std::env::var("PG_MINION_OUTPUT").unwrap()).unwrap(),
            &value,
        )
        .unwrap();
    }
}
