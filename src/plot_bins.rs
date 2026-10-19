//! Explicit ggplot2 3.5.1 StatBin/StatBin2d layers, separate from core QC.
use crate::{metrics::ChannelPopulation, record::Record};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub length: Option<[f64; 2]>,
    pub q: Option<[f64; 2]>,
}
impl Bounds {
    pub fn add(&mut self, r: &Record<'_>) {
        if r.is_control {
            return;
        }
        if r.length > 0 {
            extend(&mut self.length, (r.length as f64).log10());
        }
        extend(&mut self.q, r.qscore);
    }
    pub fn merge(&mut self, other: Self) {
        for (a, b) in [(&mut self.length, other.length), (&mut self.q, other.q)] {
            if let Some([lo, hi]) = b {
                extend(a, lo);
                extend(a, hi);
            }
        }
    }
}
fn extend(range: &mut Option<[f64; 2]>, x: f64) {
    *range = Some(range.map_or([x, x], |[lo, hi]| [lo.min(x), hi.max(x)]));
}
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}
fn zero_range([a, b]: [f64; 2]) -> bool {
    a == b
        || (a.abs().min(b.abs()) > 0.0
            && ((a - b) / a.abs().min(b.abs())).abs() < 1000.0 * f64::EPSILON)
}
#[derive(Clone, Debug)]
struct Grid {
    breaks: Vec<f64>,
    fuzzy: Vec<f64>,
}
impl Grid {
    fn new(range: [f64; 2], n: usize, two_d: bool) -> Result<Self> {
        let width = if !two_d && zero_range(range) {
            0.1
        } else {
            (range[1] - range[0]) / (if two_d { n } else { n - 1 }) as f64
        };
        ensure!(
            width.is_finite() && width > 0.0,
            "reference plot range cannot be represented by finite positive bin widths"
        );
        let origin = if two_d {
            (range[0] / width).floor() * width
        } else {
            let boundary = width / 2.0;
            boundary + ((range[0] - boundary) / width).floor() * width
        };
        let end = range[1] + if two_d { width } else { (1.0 - 1e-8) * width };
        ensure!(
            origin.is_finite() && end.is_finite(),
            "reference plot boundaries overflow; disable --minionqc-plot-bins for this extreme range"
        );
        let steps = ((end - origin) / width + 1e-10).floor() as usize;
        ensure!(steps <= n + 4, "unexpected reference bin extent");
        let mut breaks: Vec<_> = (0..=steps).map(|i| origin + i as f64 * width).collect();
        if two_d && breaks.len() > 1 && breaks[breaks.len() - 2] >= range[1] {
            breaks.pop();
        }
        if breaks.len() == 1 {
            breaks.push(origin + width);
        }
        ensure!(
            breaks
                .windows(2)
                .all(|v| v[1] > v[0] && ((v[0] + v[1]) / 2.0).is_finite()),
            "reference bin boundaries collapse or midpoints overflow at this floating-point scale; disable --minionqc-plot-bins"
        );
        let fuzz = median(breaks.windows(2).map(|v| v[1] - v[0]).collect())
            * if two_d { 1e-7 } else { 1e-8 };
        let fuzzy = breaks
            .iter()
            .enumerate()
            .map(|(i, b)| b + if i == 0 { -fuzz } else { fuzz })
            .collect::<Vec<_>>();
        if two_d {
            breaks.clone_from(&fuzzy);
        }
        Ok(Self { breaks, fuzzy })
    }
    fn index(&self, x: f64) -> Option<usize> {
        if !x.is_finite() || x < self.fuzzy[0] || x > *self.fuzzy.last()? {
            return None;
        }
        Some(
            self.fuzzy[1..]
                .partition_point(|b| *b < x)
                .min(self.fuzzy.len() - 2),
        )
    }
    fn bin(&self, i: usize, count: u64) -> Bin {
        let width = self.breaks[i + 1] - self.breaks[i];
        let x = (self.breaks[i] + self.breaks[i + 1]) / 2.0;
        Bin {
            x,
            xmin: x - width / 2.0,
            xmax: x + width / 2.0,
            width,
            count,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Bin {
    pub x: f64,
    pub xmin: f64,
    pub xmax: f64,
    pub width: f64,
    pub count: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Histogram {
    pub transformation: &'static str,
    pub requested_bins: usize,
    pub all: Vec<Bin>,
    pub q_selected: Vec<Bin>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Cell {
    pub x: f64,
    pub xmin: f64,
    pub xmax: f64,
    pub y: f64,
    pub ymin: f64,
    pub ymax: f64,
    pub count: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Bin2d {
    pub requested_bins: usize,
    pub x_breaks: Vec<f64>,
    pub y_breaks: Vec<f64>,
    pub cells: Vec<Cell>,
    pub unavailable_reason: Option<&'static str>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ChannelHistogram {
    pub metric: &'static str,
    pub histogram: Histogram,
}
#[derive(Clone, Debug, Serialize)]
pub struct PlotBins {
    pub method: &'static str,
    pub summary_records: u64,
    pub q_selected_summary_records: u64,
    pub zero_length_records: u64,
    pub length: Histogram,
    pub quality: Histogram,
    pub length_vs_q: Bin2d,
    pub channels: Vec<ChannelHistogram>,
    #[serde(skip)]
    pub selected_zeros: BTreeMap<u32, u64>,
}
struct Counter {
    grid: Option<Grid>,
    all: Vec<u64>,
    selected: Vec<u64>,
}
impl Counter {
    fn new(range: Option<[f64; 2]>, n: usize) -> Result<Self> {
        let grid = range.map(|r| Grid::new(r, n, false)).transpose()?;
        let len = grid.as_ref().map_or(0, |g| g.breaks.len() - 1);
        Ok(Self {
            grid,
            all: vec![0; len],
            selected: vec![0; len],
        })
    }
    fn add(&mut self, x: f64, selected: bool) -> Result<()> {
        ensure!(
            self.grid.is_some(),
            "new finite observation in a previously empty reference population; input changed"
        );
        if let Some(g) = &self.grid {
            let i = g.index(x).ok_or_else(|| {
                anyhow::anyhow!(
                    "observation outside frozen reference histogram limits; input changed"
                )
            })?;
            self.all[i] = self.all[i]
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("plot count overflow"))?;
            if selected {
                self.selected[i] += 1;
            }
        }
        Ok(())
    }
    fn finish(self, transformation: &'static str, requested_bins: usize) -> Histogram {
        let convert = |counts: Vec<u64>| {
            if counts.iter().any(|n| *n > 0) {
                counts
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| self.grid.as_ref().unwrap().bin(i, c))
                    .collect()
            } else {
                Vec::new()
            }
        };
        Histogram {
            transformation,
            requested_bins,
            all: convert(self.all),
            q_selected: convert(self.selected),
        }
    }
}
pub struct Plan {
    length: Counter,
    q: Counter,
    xy: Option<(Grid, Grid)>,
    cells: Vec<u64>,
    total: u64,
    selected: u64,
    zero: u64,
    selected_zeros: BTreeMap<u32, u64>,
}
impl Plan {
    pub fn new(bounds: Bounds) -> Result<Self> {
        let xy = match (bounds.length, bounds.q) {
            (Some(x), Some(y)) if x[0] != x[1] && y[0] != y[1] => {
                Some((Grid::new(x, 30, true)?, Grid::new(y, 30, true)?))
            }
            _ => None,
        };
        let size = xy
            .as_ref()
            .map_or(0, |(x, y)| (x.breaks.len() - 1) * (y.breaks.len() - 1));
        Ok(Self {
            length: Counter::new(bounds.length, 300)?,
            q: Counter::new(bounds.q, 300)?,
            xy,
            cells: vec![0; size],
            total: 0,
            selected: 0,
            zero: 0,
            selected_zeros: BTreeMap::new(),
        })
    }
    pub fn add(&mut self, r: &Record<'_>, cutoff: f64) -> Result<()> {
        if r.is_control {
            return Ok(());
        }
        self.total += 1;
        let selected = r.qscore >= cutoff;
        self.selected += u64::from(selected);
        self.q.add(r.qscore, selected)?;
        if r.length == 0 {
            self.zero += 1;
            if selected {
                *self.selected_zeros.entry(r.channel).or_default() += 1;
            }
            return Ok(());
        }
        let x = (r.length as f64).log10();
        self.length.add(x, selected)?;
        if let Some((gx, gy)) = &self.xy {
            let ix = gx
                .index(x)
                .ok_or_else(|| anyhow::anyhow!("length outside reference Bin2D limits"))?;
            let iy = gy
                .index(r.qscore)
                .ok_or_else(|| anyhow::anyhow!("Q-score outside reference Bin2D limits"))?;
            self.cells[iy * (gx.breaks.len() - 1) + ix] += 1;
        }
        Ok(())
    }
    pub fn finish(self) -> PlotBins {
        let mut bin2d = Bin2d {
            requested_bins: 30,
            x_breaks: Vec::new(),
            y_breaks: Vec::new(),
            cells: Vec::new(),
            unavailable_reason: None,
        };
        if let Some((x, y)) = self.xy {
            for (i, count) in self.cells.into_iter().enumerate().filter(|(_, c)| *c > 0) {
                let xb = x.bin(i % (x.breaks.len() - 1), count);
                let yb = y.bin(i / (x.breaks.len() - 1), count);
                bin2d.cells.push(Cell {
                    x: xb.x,
                    xmin: xb.xmin,
                    xmax: xb.xmax,
                    y: yb.x,
                    ymin: yb.xmin,
                    ymax: yb.xmax,
                    count,
                });
            }
            bin2d.x_breaks = x.breaks;
            bin2d.y_breaks = y.breaks;
        } else {
            bin2d.unavailable_reason = Some(
                "Empty or constant transformed axis; upstream StatBin2d cannot form a positive-width grid.",
            );
        }
        PlotBins {
            method: "ggplot2_3.5.1_StatBin_StatBin2d_right_closed",
            summary_records: self.total,
            q_selected_summary_records: self.selected,
            zero_length_records: self.zero,
            length: self.length.finish("log10_bp", 300),
            quality: self.q.finish("identity_qscore", 300),
            length_vs_q: bin2d,
            channels: Vec::new(),
            selected_zeros: self.selected_zeros,
        }
    }
}
/// Summary-read channel means/medians, including selected zero lengths. Counts
/// remain true counts: upstream's triangular channel-count defect is not copied.
pub fn channels(
    values: &[(ChannelPopulation, ChannelPopulation)],
) -> Result<Vec<ChannelHistogram>> {
    let mut result = Vec::new();
    for metric in [
        "total_yield_bp",
        "read_count",
        "mean_read_length_bp",
        "median_read_length_bp",
    ] {
        let value = |p: &ChannelPopulation| match metric {
            "total_yield_bp" => Some(p.total_yield_bp as f64),
            "read_count" => Some(p.read_count as f64),
            "mean_read_length_bp" => p.mean_read_length_bp,
            _ => p.median_read_length_bp,
        };
        let mut bounds = None;
        for (all, selected) in values {
            for p in [all, selected] {
                if p.read_count > 0 {
                    if let Some(v) = value(p) {
                        extend(&mut bounds, v);
                    }
                }
            }
        }
        let mut histogram = Counter::new(bounds, 30)?;
        // Each channel is one observation in its population, not a read weight.
        for (all, selected) in values {
            if let Some(v) = value(all).filter(|_| all.read_count > 0) {
                histogram.add(v, false)?;
            }
            if let Some(v) = value(selected).filter(|_| selected.read_count > 0) {
                if let Some(g) = &histogram.grid {
                    histogram.selected[g.index(v).unwrap()] += 1;
                }
            }
        }
        result.push(ChannelHistogram {
            metric,
            histogram: histogram.finish("identity", 30),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn numbers(v: &serde_json::Value) -> Vec<f64> {
        if let Some(a) = v.as_array() {
            a.iter().map(|x| x.as_f64().unwrap()).collect()
        } else {
            vec![v.as_f64().unwrap()]
        }
    }
    fn close(a: f64, b: f64) {
        assert!(
            (a - b).abs() <= 1e-12 + 1e-13 * b.abs(),
            "native {a:.17} R {b:.17}"
        );
    }
    #[test]
    fn actual_pinned_r_histogram_edges_counts_and_bin2d_cells() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../tests/reference/bin-layers-golden.json"))
                .unwrap();
        for (name, c) in golden["cases"].as_object().unwrap() {
            let lengths = numbers(&c["length"]);
            let q = numbers(&c["q"]);
            let records: Vec<_> = lengths
                .iter()
                .zip(q)
                .map(|(l, q)| Record {
                    line: 1,
                    is_control: false,
                    num_events_template: None,
                    flow_cell_id: None,
                    run_id: None,
                    channel: 1,
                    mux: None,
                    start_time: 0.0,
                    template_duration: None,
                    passed: None,
                    length: *l as u64,
                    qscore: q,
                    end_reason: None,
                })
                .collect();
            let mut bounds = Bounds::default();
            for r in &records {
                bounds.add(r);
            }
            let mut plan = Plan::new(bounds).unwrap();
            for r in &records {
                plan.add(r, c["cutoff"].as_f64().unwrap()).unwrap();
            }
            let report = plan.finish();
            for (key, h) in [("length_bins", &report.length), ("q_bins", &report.quality)] {
                let rows = c[key].as_array().unwrap();
                for (population, bins) in [("all", &h.all), ("q_selected", &h.q_selected)] {
                    let expected: Vec<_> = rows
                        .iter()
                        .filter(|r| r["population"] == population)
                        .collect();
                    assert_eq!(bins.len(), expected.len(), "{name} {key} {population}");
                    for (b, r) in bins.iter().zip(expected) {
                        assert_eq!(b.count, r["count"].as_u64().unwrap(), "{name} {key}");
                        close(b.x, r["x"].as_f64().unwrap());
                        close(b.xmin, r["xmin"].as_f64().unwrap());
                        close(b.xmax, r["xmax"].as_f64().unwrap());
                    }
                }
            }
            let rows = c["cells"].as_array().unwrap();
            assert_eq!(report.length_vs_q.cells.len(), rows.len(), "{name} cells");
            for b in &report.length_vs_q.cells {
                let r = rows
                    .iter()
                    .find(|r| {
                        (r["x"].as_f64().unwrap() - b.x).abs() < 1e-10
                            && (r["y"].as_f64().unwrap() - b.y).abs() < 1e-10
                    })
                    .unwrap();
                assert_eq!(b.count, r["count"].as_u64().unwrap());
                for (a, key) in [
                    (b.x, "x"),
                    (b.xmin, "xmin"),
                    (b.xmax, "xmax"),
                    (b.y, "y"),
                    (b.ymin, "ymin"),
                    (b.ymax, "ymax"),
                ] {
                    close(a, r[key].as_f64().unwrap());
                }
            }
        }
    }
    #[test]
    fn actual_r_channel_bins_keep_correct_counts_and_shared_population_limits() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../tests/reference/bin-layers-golden.json"))
                .unwrap();
        let parse = |v: &serde_json::Value| ChannelPopulation {
            read_count: v["read_count"].as_u64().unwrap(),
            total_yield_bp: v["total_yield_bp"].as_u64().unwrap(),
            mean_read_length_bp: v["mean_read_length_bp"].as_f64(),
            median_read_length_bp: v["median_read_length_bp"].as_f64(),
        };
        let values: Vec<_> = golden["channel_values"]["all"]
            .as_array()
            .unwrap()
            .iter()
            .zip(golden["channel_values"]["q_selected"].as_array().unwrap())
            .map(|(a, q)| (parse(a), parse(q)))
            .collect();
        for h in channels(&values).unwrap() {
            let reference = golden["channel_bins"][h.metric].as_array().unwrap();
            for (name, bins) in [
                ("all", h.histogram.all),
                ("q_selected", h.histogram.q_selected),
            ] {
                let rows: Vec<_> = reference
                    .iter()
                    .filter(|r| r["population"] == name)
                    .collect();
                assert_eq!(rows.len(), bins.len());
                for (r, b) in rows.iter().zip(bins) {
                    assert_eq!(r["count"].as_u64().unwrap(), b.count);
                    close(b.x, r["x"].as_f64().unwrap());
                    close(b.xmin, r["xmin"].as_f64().unwrap());
                    close(b.xmax, r["xmax"].as_f64().unwrap());
                }
            }
        }
    }
    #[test]
    fn right_closed_fuzz_includes_lowest_edge_and_boundary_ties() {
        assert!(Counter::new(None, 300).unwrap().add(1.0, false).is_err());
        let grid = Grid::new([0.0, 29.0], 30, false).unwrap();
        let b = grid.breaks[10];
        assert_eq!(grid.index(b), Some(9));
        assert_eq!(grid.index(b + 1e-9), Some(9));
        assert_eq!(grid.index(b + 1e-6), Some(10));
        assert_eq!(grid.index(grid.fuzzy[0]), Some(0));
        assert_eq!(grid.index(f64::NEG_INFINITY), None);
    }
}
