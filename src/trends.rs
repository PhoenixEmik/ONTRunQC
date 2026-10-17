//! Optional raw-read fits. Bounded knot selection plus a second streaming pass;
//! never regress interval means or retain an unbounded time/length/Q dataset.
use crate::record::Record;
use serde::Serialize;
use std::collections::BTreeMap;

const K: usize = 10;
const UNIQUE_LIMIT: usize = 4096;
type Matrix = [[f64; K]; K];

#[derive(Clone, Debug, Serialize)]
pub struct Trends {
    pub verification: &'static str,
    pub single: Vec<Curve>,
    pub combined: Vec<Curve>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Curve {
    pub population: &'static str,
    pub metric: &'static str,
    pub method: &'static str,
    pub observations: u64,
    pub exact_knots: bool,
    pub confidence_interval: Option<&'static str>,
    pub unavailable_reason: Option<&'static str>,
    pub points: Vec<Point>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Point {
    pub hours: f64,
    pub value: f64,
}
#[derive(Clone, Copy)]
struct Observation {
    x: f64,
    length: f64,
    q: f64,
}
#[derive(Clone, Default)]
struct Population {
    count: u64,
    min: f64,
    max: f64,
    length_scale: f64,
    q_scale: f64,
    q_min: f64,
    q_max: f64,
    length_min: f64,
    length_max: f64,
    unique: BTreeMap<(u64, u64), f64>,
    truncated: bool,
    small: Vec<Observation>,
}
impl Population {
    fn add(&mut self, v: Observation) {
        if self.count == 0 {
            self.min = v.x;
            self.max = v.x;
            self.q_min = v.q;
            self.q_max = v.q;
            self.length_min = v.length;
            self.length_max = v.length;
        }
        self.q_min = self.q_min.min(v.q);
        self.q_max = self.q_max.max(v.q);
        self.length_min = self.length_min.min(v.length);
        self.length_max = self.length_max.max(v.length);
        self.count += 1;
        self.min = self.min.min(v.x);
        self.max = self.max.max(v.x);
        self.length_scale = self.length_scale.max(v.length.abs());
        self.q_scale = self.q_scale.max(v.q.abs());
        if self.count < 1000 {
            self.small.push(v);
        } else {
            self.small.clear();
        }
        let bits = if v.x == 0.0 { 0 } else { v.x.to_bits() };
        let mut hash = bits.wrapping_add(0x9e3779b97f4a7c15);
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
        hash ^= hash >> 31;
        let key = (hash, bits);
        if self.unique.contains_key(&key) {
            return;
        }
        if self.unique.len() < UNIQUE_LIMIT {
            self.unique.insert(key, v.x);
        } else {
            self.truncated = true;
            if key < *self.unique.last_key_value().unwrap().0 {
                self.unique.pop_last();
                self.unique.insert(key, v.x);
            }
        }
    }
}
#[derive(Default)]
pub struct Tracker {
    all: Population,
    positive_q: Population,
    selected: Population,
}
impl Tracker {
    pub fn add(&mut self, r: &Record<'_>, cutoff: f64) {
        if r.is_control {
            return;
        }
        let v = Observation {
            x: r.start_time / 3600.0,
            length: r.length as f64,
            q: r.qscore,
        };
        self.all.add(v);
        if v.q >= 0.0 {
            self.positive_q.add(v);
        }
        if v.q >= cutoff {
            self.selected.add(v);
        }
    }
    pub fn count(&self) -> u64 {
        self.all.count
    }
    pub fn prepare(self, combined_gam: bool) -> Plan {
        let single_gam = self.all.count >= 1000;
        let mut datasets = Vec::new();
        for (population, tracker, quality, single) in [
            ("all_summary_records", self.all.clone(), false, false),
            ("all_summary_records", self.all, true, false),
            ("all_summary_records", self.positive_q, true, true),
            (
                "qscore_selected_summary_records",
                self.selected.clone(),
                false,
                false,
            ),
            (
                "qscore_selected_summary_records",
                self.selected,
                true,
                false,
            ),
        ] {
            datasets.push(Dataset::new(population, tracker, quality, single));
        }
        Plan {
            datasets,
            single_gam,
            combined_gam,
        }
    }
}
pub struct Plan {
    datasets: Vec<Dataset>,
    single_gam: bool,
    combined_gam: bool,
}
impl Plan {
    pub fn add(&mut self, r: &Record<'_>, cutoff: f64) {
        if r.is_control {
            return;
        }
        for d in &mut self.datasets {
            if (d.population.starts_with("qscore") && r.qscore < cutoff)
                || (d.single && r.qscore < 0.0)
            {
                continue;
            }
            let Some(basis) = &d.basis else {
                continue;
            };
            let b = basis.evaluate(r.start_time / 3600.0);
            let y = ((if d.quality { r.qscore } else { r.length as f64 }) - d.offset) / d.scale;
            d.n += 1;
            d.yy += y * y;
            for i in 0..K {
                d.rhs[i] += b[i] * y;
                for j in 0..K {
                    d.xx[i][j] += b[i] * b[j];
                }
            }
        }
    }
    pub fn finish(self) -> Trends {
        let mut single = Vec::new();
        let mut combined = Vec::new();
        for (i, d) in self.datasets.iter().enumerate() {
            if i != 1 {
                single.push(d.fit(self.single_gam));
            }
            if i != 2 {
                combined.push(d.fit(self.combined_gam));
            }
        }
        Trends {
            verification: "R_differential_partial; bounded_GAM_knots_singular_LOESS_and_confidence_bands_differ",
            single,
            combined,
        }
    }
}
struct Dataset {
    population: &'static str,
    tracker: Population,
    quality: bool,
    single: bool,
    basis: Option<Basis>,
    scale: f64,
    offset: f64,
    n: u64,
    xx: Matrix,
    rhs: [f64; K],
    yy: f64,
}
impl Dataset {
    fn new(population: &'static str, tracker: Population, quality: bool, single: bool) -> Self {
        let basis = Basis::new(&tracker);
        let scale = if quality {
            tracker.q_scale
        } else {
            tracker.length_scale
        }
        .max(1.0);
        let offset = if quality {
            tracker.q_min / 2.0 + tracker.q_max / 2.0
        } else {
            tracker.length_min / 2.0 + tracker.length_max / 2.0
        };
        Self {
            offset,
            population,
            tracker,
            quality,
            single,
            basis,
            scale,
            n: 0,
            xx: [[0.0; K]; K],
            rhs: [0.0; K],
            yy: 0.0,
        }
    }
    fn fit(&self, gam: bool) -> Curve {
        let mut curve = Curve {
            population: self.population,
            metric: if self.quality { "qscore" } else { "length_bp" },
            method: if gam {
                "cubic_shrinkage_spline_REML_k10"
            } else {
                "loess_degree2_span0.75_interpolate_cell0.2"
            },
            observations: self.tracker.count,
            exact_knots: !self.tracker.truncated,
            confidence_interval: None,
            unavailable_reason: None,
            points: Vec::new(),
        };
        if self.tracker.count < 3 || self.tracker.min == self.tracker.max {
            curve.unavailable_reason =
                Some("fewer than three observations or fewer than two distinct times");
            return curve;
        }
        let coefficients = if gam { self.gam() } else { None };
        if gam && coefficients.is_none() {
            curve.unavailable_reason =
                Some("GAM requires ten distinct times and a finite nonsingular fit");
            return curve;
        }
        let surface = if gam {
            None
        } else {
            LoessSurface::new(&self.tracker.small, self.quality)
        };
        for i in 0..80 {
            let x = self.tracker.min + (self.tracker.max - self.tracker.min) * i as f64 / 79.0;
            let value = if let Some(c) = &coefficients {
                Some(dot(&self.basis.as_ref().unwrap().evaluate(x), c) * self.scale + self.offset)
            } else {
                surface.as_ref().map(|surface| surface.predict(x))
            };
            let Some(value) = value.filter(|v| v.is_finite()) else {
                curve.points.clear();
                curve.unavailable_reason =
                    Some("singular local fit or nonrepresentable prediction");
                return curve;
            };
            curve.points.push(Point { hours: x, value });
        }
        curve
    }
    fn gam(&self) -> Option<[f64; K]> {
        let basis = self.basis.as_ref()?;
        let n = self.n as f64;
        if n <= K as f64 {
            return None;
        }
        // Absorb the sample-weighted centering constraint via a Householder
        // orthogonal complement; intercept remains completely unpenalized.
        let mean: [f64; K] = std::array::from_fn(|i| self.xx[i].iter().sum::<f64>() / n);
        let norm = dot(&mean, &mean).sqrt();
        let mut v = mean;
        v[0] += norm.copysign(v[0]);
        let vn = dot(&v, &v);
        if vn == 0.0 {
            return None;
        }
        let q: Matrix = std::array::from_fn(|i| {
            std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } - 2.0 * v[i] * v[j] / vn)
        });
        let mut xx = [[0.0; K]; K];
        let mut penalty = [[0.0; K]; K];
        let mut rhs = [0.0; K];
        let ymean = self.rhs.iter().sum::<f64>() / n;
        for a in 1..K {
            for i in 0..K {
                rhs[a - 1] += q[i][a] * (self.rhs[i] - mean[i] * ymean * n);
            }
            for b in 1..K {
                for i in 0..K {
                    for j in 0..K {
                        xx[a - 1][b - 1] +=
                            q[i][a] * (self.xx[i][j] - n * mean[i] * mean[j]) * q[j][b];
                        penalty[a - 1][b - 1] += q[i][a] * basis.penalty[i][j] * q[j][b];
                    }
                }
            }
        }
        let centered_yy = (self.yy - n * ymean * ymean).max(0.0);
        let objective = |log_lambda: f64| -> Option<(f64, [f64; K])> {
            let lambda = log_lambda.exp();
            let mut a = xx;
            for i in 0..K - 1 {
                for j in 0..K - 1 {
                    a[i][j] += lambda * penalty[i][j];
                }
            }
            let (beta, logdet) = solve(a, rhs, K - 1)?;
            let rss = (centered_yy - dot(&beta, &rhs)).max(1e-300);
            Some((
                (n - 1.0) * (rss / (n - 1.0)).ln() + logdet - (K - 1) as f64 * log_lambda,
                beta,
            ))
        };
        let (mut lo, mut hi) = (-30.0, 30.0);
        for _ in 0..100 {
            let a = lo + (hi - lo) / 3.0;
            let b = hi - (hi - lo) / 3.0;
            if objective(a)?.0 < objective(b)?.0 {
                hi = b;
            } else {
                lo = a;
            }
        }
        let (_, beta) = objective((lo + hi) / 2.0)?;
        let coefficients =
            std::array::from_fn(|i| ymean + (1..K).map(|a| q[i][a] * beta[a - 1]).sum::<f64>());
        coefficients
            .iter()
            .all(|c| c.is_finite())
            .then_some(coefficients)
    }
}
fn dot(a: &[f64; K], b: &[f64; K]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
// Small SPD solve only: natural splines, local quadratics and REML coefficients.
fn solve(a: Matrix, b: [f64; K], size: usize) -> Option<([f64; K], f64)> {
    let mut l = [[0.0; K]; K];
    let mut logdet = 0.0;
    for i in 0..size {
        for j in 0..=i {
            let value = a[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            l[i][j] = if i == j {
                if value <= 0.0 || !value.is_finite() {
                    return None;
                }
                logdet += value.ln();
                value.sqrt()
            } else {
                value / l[j][j]
            };
        }
    }
    let mut y = [0.0; K];
    for i in 0..size {
        y[i] = (b[i] - (0..i).map(|j| l[i][j] * y[j]).sum::<f64>()) / l[i][i];
    }
    let mut x = [0.0; K];
    for i in (0..size).rev() {
        x[i] = (y[i] - (i + 1..size).map(|j| l[j][i] * x[j]).sum::<f64>()) / l[i][i];
    }
    Some((x, logdet))
}
struct Basis {
    knots: [f64; K],
    second: Matrix,
    penalty: Matrix,
    min: f64,
    range: f64,
}
impl Basis {
    #[allow(clippy::needless_range_loop)] // Column-wise tridiagonal solves share neighboring rows.
    fn new(pop: &Population) -> Option<Self> {
        if pop.unique.len() < K || pop.max <= pop.min {
            return None;
        }
        let mut x: Vec<_> = pop.unique.values().copied().collect();
        x.sort_by(f64::total_cmp);
        x[0] = pop.min;
        *x.last_mut().unwrap() = pop.max;
        let knots = std::array::from_fn(|i| {
            let p = (x.len() - 1) as f64 * i as f64 / (K - 1) as f64;
            let j = p.floor() as usize;
            (x[j] + (x[p.ceil() as usize] - x[j]) * p.fract() - pop.min) / (pop.max - pop.min)
        });
        if knots.windows(2).any(|p| !p[0].is_finite() || p[1] <= p[0]) {
            return None;
        }
        let mut second = [[0.0; K]; K];
        for column in 0..K {
            let y: [f64; K] = std::array::from_fn(|i| if i == column { 1.0 } else { 0.0 });
            let mut diag = [0.0; K];
            let mut rhs = [0.0; K];
            for i in 1..K - 1 {
                let h = knots[i] - knots[i - 1];
                let g = knots[i + 1] - knots[i];
                diag[i] = 2.0 * (h + g);
                rhs[i] = 6.0 * ((y[i + 1] - y[i]) / g - (y[i] - y[i - 1]) / h);
                if i > 1 {
                    let ratio = h / diag[i - 1];
                    diag[i] -= ratio * h;
                    rhs[i] -= ratio * rhs[i - 1];
                }
            }
            for i in (1..K - 1).rev() {
                second[i][column] =
                    (rhs[i] - (knots[i + 1] - knots[i]) * second[i + 1][column]) / diag[i];
            }
        }
        let penalty = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                (0..K - 1)
                    .map(|k| {
                        (knots[k + 1] - knots[k]) / 6.0
                            * (2.0 * second[k][i] * second[k][j]
                                + second[k][i] * second[k + 1][j]
                                + second[k + 1][i] * second[k][j]
                                + 2.0 * second[k + 1][i] * second[k + 1][j])
                    })
                    .sum()
            })
        });
        let (eigen, vectors) = symmetric_eigen(penalty);
        let maximum = eigen[K - 1];
        if maximum <= 0.0 {
            return None;
        }
        let mut adjusted = eigen;
        adjusted[1] = eigen[2] * 0.1;
        adjusted[0] = eigen[2] * 0.01;
        let penalty = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                (0..K)
                    .map(|k| vectors[i][k] * adjusted[k] / maximum * vectors[j][k])
                    .sum()
            })
        });
        Some(Self {
            knots,
            second,
            penalty,
            min: pop.min,
            range: pop.max - pop.min,
        })
    }
    fn evaluate(&self, x: f64) -> [f64; K] {
        let x = (x - self.min) / self.range;
        let i = (self.knots.partition_point(|k| *k <= x).saturating_sub(1)).min(K - 2);
        let h = self.knots[i + 1] - self.knots[i];
        let a = (self.knots[i + 1] - x) / h;
        let b = (x - self.knots[i]) / h;
        std::array::from_fn(|j| {
            let linear = (if j == i { a } else { 0.0 }) + (if j == i + 1 { b } else { 0.0 });
            linear
                + h * h / 6.0
                    * ((a * a * a - a) * self.second[i][j]
                        + (b * b * b - b) * self.second[i + 1][j])
        })
    }
}
#[allow(clippy::needless_range_loop)] // Coupled row/column rotations of a fixed 10×10 matrix.
fn symmetric_eigen(mut a: Matrix) -> ([f64; K], Matrix) {
    let mut v: Matrix =
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }));
    for _ in 0..2000 {
        let (mut p, mut q) = (0, 1);
        for i in 0..K {
            for j in i + 1..K {
                if a[i][j].abs() > a[p][q].abs() {
                    p = i;
                    q = j;
                }
            }
        }
        if a[p][q].abs() < 1e-12 {
            break;
        }
        let angle = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (c, s) = (angle.cos(), angle.sin());
        for row in &mut a {
            let (x, y) = (row[p], row[q]);
            row[p] = c * x - s * y;
            row[q] = s * x + c * y;
        }
        for j in 0..K {
            let (x, y) = (a[p][j], a[q][j]);
            a[p][j] = c * x - s * y;
            a[q][j] = s * x + c * y;
        }
        for row in &mut v {
            let (x, y) = (row[p], row[q]);
            row[p] = c * x - s * y;
            row[q] = s * x + c * y;
        }
    }
    let mut indices: Vec<_> = (0..K).collect();
    indices.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]));
    (
        std::array::from_fn(|i| a[indices[i]][indices[i]]),
        std::array::from_fn(|i| std::array::from_fn(|j| v[i][indices[j]])),
    )
}
// One-dimensional LOESS interpolation: median-split cells, padded boundaries
// and cubic Hermite interpolation of vertex predictions/derivatives. Algorithm:
// R stats loessf.f ehg124/126/128, control(cell=0.2), span=0.75.
struct LoessSurface {
    vertices: Vec<(f64, f64, f64)>,
}
impl LoessSurface {
    fn new(rows: &[Observation], quality: bool) -> Option<Self> {
        let mut times: Vec<_> = rows.iter().map(|v| v.x).collect();
        times.sort_by(f64::total_cmp);
        let first = *times.first()?;
        let last = *times.last()?;
        if first == last {
            return None;
        }
        let padding = 0.005 * (last - first).max(1e-10 * first.abs().max(last.abs()) + 1e-30);
        let lower = first - padding;
        let upper = last + padding;
        let mut knots = vec![lower, upper];
        let cell = (rows.len() as f64 * 0.75 * 0.2).floor() as usize;
        fn split(times: &[f64], lower: f64, upper: f64, cell: usize, knots: &mut Vec<f64>) {
            if times.len() <= cell || times.is_empty() {
                return;
            }
            let middle = (times.len() - 1) / 2;
            // R searches alternately right and left when median times tie.
            let cut = (0..times.len())
                .filter_map(|i| {
                    let offset = i.div_ceil(2) as isize * if i % 2 == 1 { 1 } else { -1 };
                    middle.checked_add_signed(offset)
                })
                .take_while(|&i| i + 1 < times.len())
                .find(|&i| times[i] != times[i + 1]);
            // With cell=0 R also splits singleton leaves at their observation;
            // all-tied leaves retain the median until it meets a boundary.
            let i = cut.unwrap_or(middle);
            let point = times[i];
            if point <= lower || point >= upper {
                return;
            }
            knots.push(point);
            split(&times[..=i], lower, point, cell, knots);
            split(&times[i + 1..], point, upper, cell, knots);
        }
        split(&times, lower, upper, cell, &mut knots);
        knots.sort_by(f64::total_cmp);
        let vertices = knots
            .into_iter()
            .map(|x| local_quadratic(rows, x, quality).map(|(y, slope)| (x, y, slope)))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { vertices })
    }
    fn predict(&self, x: f64) -> f64 {
        let i = self
            .vertices
            .partition_point(|v| v.0 <= x)
            .saturating_sub(1)
            .min(self.vertices.len() - 2);
        let (a, ya, da) = self.vertices[i];
        let (b, yb, db) = self.vertices[i + 1];
        let h = (x - a) / (b - a);
        (1.0 - h).powi(2) * (1.0 + 2.0 * h) * ya
            + h * h * (3.0 - 2.0 * h) * yb
            + (h * (1.0 - h).powi(2) * da + h * h * (h - 1.0) * db) * (b - a)
    }
}
fn local_quadratic(rows: &[Observation], x: f64, quality: bool) -> Option<(f64, f64)> {
    let mut distances: Vec<_> = rows.iter().map(|v| (v.x - x).powi(2)).collect();
    distances.sort_by(f64::total_cmp);
    let neighbors = (rows.len() as f64 * 0.75).floor() as usize;
    if neighbors < 2 {
        return None;
    }
    let radius_squared = distances[neighbors.min(rows.len()) - 1];
    if radius_squared <= 0.0 {
        return None;
    }
    let scale = rows
        .iter()
        .map(|v| if quality { v.q.abs() } else { v.length })
        .fold(1.0, f64::max);
    // R equilibrates weighted columns and solves by SVD. Working directly on
    // the <=999-by-3 design also handles rank-deficient local quadratics;
    // normal equations can appear nonsingular after roundoff in these cases.
    let mut design = Vec::new();
    let mut response = Vec::new();
    for row in rows {
        let dx = row.x - x;
        let u = (dx * dx / radius_squared).sqrt();
        if u >= 1.0 {
            continue;
        }
        let w = (1.0 - u.powi(3)).powi(3).sqrt();
        design.push([w, w * dx, w * dx * dx]);
        response.push(w * (if quality { row.q } else { row.length }) / scale);
    }
    if design.len() < 3 {
        design.resize(3, [0.0; 3]);
        response.resize(3, 0.0);
    }
    let coefficients = local_pseudoinverse(&mut design, &mut response)?;
    Some((coefficients[0] * scale, coefficients[1] * scale))
}

fn local_pseudoinverse(design: &mut [[f64; 3]], response: &mut [f64]) -> Option<[f64; K]> {
    let norms: [f64; 3] =
        std::array::from_fn(|j| design.iter().map(|r| r[j] * r[j]).sum::<f64>().sqrt());
    for row in design.iter_mut() {
        for j in 0..3 {
            row[j] /= norms[j].max(f64::MIN_POSITIVE);
        }
    }
    // Householder QR before the small SVD, as in R's dqrdc/dqrsl path.
    // This preserves tiny weighted responses near singular neighborhoods;
    // applying SVD rotations to the full design loses them by cancellation.
    let mut reduced = [[0.0; 3]; 3];
    for j in 0..3 {
        let norm = design[j..].iter().fold(0.0_f64, |n, r| n.hypot(r[j]));
        if norm > 0.0 {
            let norm = norm.copysign(design[j][j]);
            for row in &mut design[j..] {
                row[j] /= norm;
            }
            design[j][j] += 1.0;
            for k in j + 1..3 {
                let t = -design[j..].iter().map(|r| r[j] * r[k]).sum::<f64>() / design[j][j];
                for row in &mut design[j..] {
                    row[k] += t * row[j];
                }
            }
            let t = -design[j..]
                .iter()
                .zip(&response[j..])
                .map(|(r, y)| r[j] * y)
                .sum::<f64>()
                / design[j][j];
            for (row, y) in design[j..].iter().zip(&mut response[j..]) {
                *y += t * row[j];
            }
            design[j][j] = -norm;
        }
        for i in 0..=j {
            reduced[i][j] = design[i][j];
        }
    }
    let design = &mut reduced;
    let mut vectors: [[f64; 3]; 3] =
        std::array::from_fn(|i| std::array::from_fn(|j| f64::from(i == j)));
    // One-sided Jacobi SVD of three equilibrated columns. The singular-value
    // cutoff is R loess's 100*epsilon; no ridge or invented observations.
    for _ in 0..50 {
        let mut changed = false;
        for p in 0..3 {
            for q in p + 1..3 {
                let a = design.iter().map(|r| r[p] * r[p]).sum::<f64>();
                let b = design.iter().map(|r| r[q] * r[q]).sum::<f64>();
                let c = design.iter().map(|r| r[p] * r[q]).sum::<f64>();
                if c.abs() <= f64::EPSILON * (a * b).sqrt() {
                    continue;
                }
                let (s, c) = (0.5 * (2.0 * c).atan2(b - a)).sin_cos();
                for row in design.iter_mut().chain(vectors.iter_mut()) {
                    let (a, b) = (row[p], row[q]);
                    row[p] = c * a - s * b;
                    row[q] = s * a + c * b;
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let sigma: [f64; 3] =
        std::array::from_fn(|j| design.iter().map(|r| r[j] * r[j]).sum::<f64>().sqrt());
    let cutoff = sigma.into_iter().fold(0.0, f64::max) * 100.0 * f64::EPSILON;
    if cutoff == 0.0 {
        return None;
    }
    let mut coefficients = [0.0; K];
    for k in 0..3 {
        if sigma[k] <= cutoff {
            continue;
        }
        let component = design
            .iter()
            .zip(response.iter())
            .map(|(r, y)| r[k] * y)
            .sum::<f64>()
            / sigma[k].powi(2);
        for j in 0..3 {
            coefficients[j] += vectors[j][k] * component / norms[j].max(f64::MIN_POSITIVE);
        }
    }
    coefficients
        .iter()
        .all(|v| v.is_finite())
        .then_some(coefficients)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_r_synthetic_loess_and_gam_predictions() {
        use crate::input::SummaryReader;
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../tests/reference/trends-golden.json")).unwrap();
        let (mut compared, mut maximum_error) = (0, 0.0_f64);
        for case in golden["cases"].as_array().unwrap() {
            let input = case["input"].as_str().unwrap().as_bytes();
            let mut tracker = Tracker::default();
            let mut reader = SummaryReader::new(input).unwrap();
            while let Some(record) = reader.next_record().unwrap() {
                tracker.add(&record, 7.0);
            }
            let mut plan = tracker.prepare(false);
            let mut reader = SummaryReader::new(input).unwrap();
            while let Some(record) = reader.next_record().unwrap() {
                plan.add(&record, 7.0);
            }
            let trends = plan.finish();
            for reference in case["fits"].as_array().unwrap() {
                let curve = trends
                    .single
                    .iter()
                    .find(|c| {
                        (c.metric == "qscore") == reference["quality"].as_bool().unwrap()
                            && c.population.starts_with("qscore")
                                == reference["selected"].as_bool().unwrap()
                    })
                    .unwrap();
                let points = reference["points"].as_array().unwrap();
                assert_eq!(
                    curve.points.len(),
                    points.len(),
                    "{} {:?}",
                    case["name"],
                    curve
                );
                for (actual, expected) in curve.points.iter().zip(points) {
                    assert!((actual.hours - expected[0].as_f64().unwrap()).abs() < 1e-10);
                    if let Some(y) = expected[1].as_f64() {
                        compared += 1;
                        maximum_error = maximum_error.max((actual.value - y).abs());
                        assert!(
                            (actual.value - y).abs() <= 1e-6 + 1e-6 * y.abs(),
                            "{} {} {} at {}: {} != {}",
                            case["name"],
                            curve.population,
                            curve.metric,
                            actual.hours,
                            actual.value,
                            y
                        );
                    }
                }
            }
        }
        eprintln!(
            "Actual R fitted values compared: {compared}; max absolute error: {maximum_error:e}"
        );
    }
    fn population(n: usize) -> Population {
        let mut p = Population::default();
        for i in 0..n {
            let x = i as f64 / (n - 1).max(1) as f64;
            p.add(Observation {
                x,
                length: 100.0 + 50.0 * x * x,
                q: 8.0 + 2.0 * x,
            });
        }
        p
    }
    #[test]
    fn direct_loess_reproduces_quadratics_including_boundaries() {
        let p = population(99);
        for x in [0.0, 0.01, 0.3, 0.9, 1.0] {
            assert!(
                (local_quadratic(&p.small, x, false).unwrap().0 - (100.0 + 50.0 * x * x)).abs()
                    < 1e-9
            );
        }
    }
    #[test]
    fn interpolated_loess_preserves_quadratics_and_handles_tied_times() {
        let mut p = population(99);
        let fit = LoessSurface::new(&p.small, false).unwrap();
        for i in 0..80 {
            let x = i as f64 / 79.0;
            assert!((fit.predict(x) - (100.0 + 50.0 * x * x)).abs() < 1e-8);
        }
        // Replicating observations must preserve predictions, not create
        // zero-width cells at repeated median times.
        let originals = p.small.clone();
        for v in originals {
            p.small.push(v);
        }
        let tied = LoessSurface::new(&p.small, false).unwrap();
        assert!(tied.vertices.windows(2).all(|p| p[0].0 < p[1].0));
        for i in 0..80 {
            let x = i as f64 / 79.0;
            assert!((tied.predict(x) - (100.0 + 50.0 * x * x)).abs() < 1e-8);
        }
    }
    #[test]
    fn natural_basis_interpolates_knots_and_partitions_unity() {
        let p = population(100);
        let b = Basis::new(&p).unwrap();
        for (j, &x) in b.knots.iter().enumerate() {
            let values = b.evaluate(x);
            for (i, v) in values.iter().enumerate() {
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-10);
            }
        }
        for i in 0..100 {
            assert!((b.evaluate(i as f64 / 99.0).iter().sum::<f64>() - 1.0).abs() < 1e-10);
        }
    }
    #[test]
    fn gam_uses_all_raw_observations_and_preserves_constant_intercept() {
        let mut p = population(1500);
        for row in &mut p.small {
            row.length = 42.0;
        }
        let mut d = Dataset::new("all_summary_records", p, false, false);
        for i in 0..1500 {
            let b = d.basis.as_ref().unwrap().evaluate(i as f64 / 1499.0);
            let y = (42.0 - d.offset) / d.scale;
            d.n += 1;
            d.yy += y * y;
            for j in 0..K {
                d.rhs[j] += b[j] * y;
                for k in 0..K {
                    d.xx[j][k] += b[j] * b[k];
                }
            }
        }
        let curve = d.fit(true);
        assert_eq!(curve.points.len(), 80);
        assert!(curve.points.iter().all(|p| (p.value - 42.0).abs() < 1e-7));
        assert!(curve.confidence_interval.is_none());
    }
    #[test]
    fn gam_is_stable_under_large_response_offsets() {
        let fit = |offset: f64| {
            let mut pop = Population::default();
            for i in 0..1200 {
                let x = i as f64 / 1199.0;
                pop.add(Observation {
                    x,
                    length: 100.0,
                    q: offset + 8.0 + 2.0 * x,
                });
            }
            let mut d = Dataset::new("all_summary_records", pop, true, false);
            for i in 0..1200 {
                let x = i as f64 / 1199.0;
                let b = d.basis.as_ref().unwrap().evaluate(x);
                let y = (offset + 8.0 + 2.0 * x - d.offset) / d.scale;
                d.n += 1;
                d.yy += y * y;
                for j in 0..K {
                    d.rhs[j] += b[j] * y;
                    for k in 0..K {
                        d.xx[j][k] += b[j] * b[k];
                    }
                }
            }
            d.fit(true)
        };
        let a = fit(0.0);
        let b = fit(1e6);
        assert_eq!(a.points.len(), 80);
        assert_eq!(b.points.len(), 80);
        for (a, b) in a.points.iter().zip(&b.points) {
            assert!((b.value - a.value - 1e6).abs() < 1e-7);
        }
    }
    #[test]
    fn unique_time_storage_is_bounded_and_method_switch_is_global() {
        let p = population(20_000);
        assert_eq!(p.unique.len(), UNIQUE_LIMIT);
        assert!(p.truncated);
        assert!(p.small.is_empty());
        let tracker = Tracker {
            all: population(99),
            positive_q: population(99),
            selected: population(60),
        };
        let plan = tracker.prepare(true);
        assert!(!plan.single_gam);
        assert!(plan.combined_gam);
        let result = plan.finish();
        assert!(result.single.iter().all(|c| c.method.starts_with("loess")));
        assert!(result.combined.iter().all(|c| c.method.contains("REML")));
    }
}
