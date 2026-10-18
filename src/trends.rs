//! Optional raw-read fits. Exact bounded-memory knot selection and a second pass;
//! never regress interval means or retain an unbounded time/length/Q dataset.
use crate::record::Record;
use anyhow::Result;
use serde::Serialize;

const K: usize = 10;
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
    pub knots_hours: Option<[f64; K]>,
    pub confidence_interval: Option<&'static str>,
    pub unavailable_reason: Option<&'static str>,
    pub points: Vec<Point>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Point {
    pub hours: f64,
    pub value: f64,
    pub standard_error: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
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
    knots: Option<[f64; K]>,
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
    }
}
#[derive(Default)]
pub struct Tracker {
    all: Population,
    positive_q: Population,
    selected: Population,
    times: crate::trend_knots::Times,
}
impl Tracker {
    pub fn add(&mut self, r: &Record<'_>, cutoff: f64) -> Result<()> {
        if r.is_control {
            return Ok(());
        }
        let v = Observation {
            x: r.start_time / 3600.0,
            length: r.length as f64,
            q: r.qscore,
        };
        self.times.add(
            v.x,
            1 | if v.q >= 0.0 { 2 } else { 0 } | if v.q >= cutoff { 4 } else { 0 },
        )?;
        self.all.add(v);
        if v.q >= 0.0 {
            self.positive_q.add(v);
        }
        if v.q >= cutoff {
            self.selected.add(v);
        }
        Ok(())
    }
    pub fn count(&self) -> u64 {
        self.all.count
    }
    pub fn prepare(mut self, combined_gam: bool) -> Result<Plan> {
        let [all, positive, selected] = std::mem::take(&mut self.times).finish()?;
        self.all.knots = all;
        self.positive_q.knots = positive;
        self.selected.knots = selected;
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
        Ok(Plan {
            datasets,
            single_gam,
            combined_gam,
        })
    }
}
pub struct Plan {
    datasets: Vec<Dataset>,
    single_gam: bool,
    combined_gam: bool,
}
impl Plan {
    #[cfg(test)]
    pub fn audit(&self) -> serde_json::Value {
        serde_json::json!(self.datasets.iter().map(|d| {
            serde_json::json!({"population":d.population,"quality":d.quality,"single":d.single,
                "basis":d.basis.as_ref().map(|b| &b.audit),
                "basis_at_predictions":d.basis.as_ref().map(|b|(0..80).map(|i|
                    b.evaluate(d.tracker.min+(d.tracker.max-d.tracker.min)*i as f64/79.0)).collect::<Vec<_>>()),
                "nodal_rhs":d.rhs,"nodal_yy":d.yy,
                "fit":d.gam().map(|f| f.audit)})
        }).collect::<Vec<_>>())
    }
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
            verification: "R_differential_partial; canonical_cs_null_space; LOESS_intervals_unavailable",
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
            exact_knots: true,
            knots_hours: if gam { self.tracker.knots } else { None },
            confidence_interval: None,
            unavailable_reason: None,
            points: Vec::new(),
        };
        if self.tracker.count < 3 || self.tracker.min == self.tracker.max {
            curve.unavailable_reason =
                Some("fewer than three observations or fewer than two distinct times");
            return curve;
        }
        let fitted = if gam { self.gam() } else { None };
        if gam && fitted.is_none() {
            curve.unavailable_reason =
                Some("GAM requires ten distinct times and a finite nonsingular fit");
            return curve;
        }
        let surface = if gam {
            None
        } else {
            LoessSurface::new(&self.tracker.small, self.quality)
        };
        if fitted.is_some() {
            curve.confidence_interval = Some("pointwise_95_percent_bayesian_normal");
        }
        for i in 0..80 {
            let x = self.tracker.min + (self.tracker.max - self.tracker.min) * i as f64 / 79.0;
            let value = if let Some(fitted) = &fitted {
                Some(
                    dot(
                        &self.basis.as_ref().unwrap().evaluate(x),
                        &fitted.coefficients,
                    ) * self.scale
                        + self.offset,
                )
            } else {
                surface.as_ref().map(|surface| surface.predict(x))
            };
            let Some(value) = value.filter(|v| v.is_finite()) else {
                curve.points.clear();
                curve.unavailable_reason =
                    Some("singular local fit or nonrepresentable prediction");
                return curve;
            };
            let standard_error = fitted.as_ref().and_then(|fit| {
                let b = self.basis.as_ref().unwrap().evaluate(x);
                let variance: f64 = (0..K)
                    .map(|i| {
                        (0..K)
                            .map(|j| b[i] * fit.covariance[i][j] * b[j])
                            .sum::<f64>()
                    })
                    .sum();
                if !variance.is_finite() {
                    return None;
                }
                let se = variance.max(0.0).sqrt() * self.scale;
                se.is_finite().then_some(se)
            });
            curve.points.push(Point {
                hours: x,
                value,
                standard_error,
                lower: standard_error
                    .map(|s| value - 1.959963984540054 * s)
                    .filter(|v| v.is_finite()),
                upper: standard_error
                    .map(|s| value + 1.959963984540054 * s)
                    .filter(|v| v.is_finite()),
            });
        }
        curve
    }
    fn gam(&self) -> Option<GamFit> {
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
        let reml = GaussianReml {
            xx,
            penalty,
            rhs,
            yy: centered_yy,
            n,
            scale: self.scale,
        };
        let (optimum, optimized, _iterations) = if centered_yy > 0.0 {
            reml.optimize().or_else(|| reml.profile_fallback())?
        } else {
            (
                [30.0, 0.0],
                RemlPoint {
                    score: 0.0,
                    gradient: [0.0; 2],
                    hessian: [[0.0; 2]; 2],
                    beta: [0.0; K],
                },
                0,
            )
        };
        let beta = optimized.beta;
        let coefficients =
            std::array::from_fn(|i| ymean + (1..K).map(|a| q[i][a] * beta[a - 1]).sum::<f64>());
        let mut a = xx;
        for i in 0..K - 1 {
            for j in 0..K - 1 {
                a[i][j] += optimum[0].exp() * penalty[i][j];
            }
        }
        let mut inverse = [[0.0; K]; K];
        for j in 0..K - 1 {
            let mut e = [0.0; K];
            e[j] = 1.0;
            let solution = solve(a, e, K - 1)?.0;
            for i in 0..K - 1 {
                inverse[i][j] = solution[i];
            }
        }
        let edf = 1.0
            + (0..K - 1)
                .map(|i| (0..K - 1).map(|j| inverse[i][j] * xx[j][i]).sum::<f64>())
                .sum::<f64>();
        let rss = (centered_yy - 2.0 * dot(&beta, &rhs)
            + (0..K - 1)
                .map(|i| {
                    (0..K - 1)
                        .map(|j| beta[i] * xx[i][j] * beta[j])
                        .sum::<f64>()
                })
                .sum::<f64>())
        .max(0.0);
        let variance = rss / (n - edf);
        let covariance = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                variance
                    * (1.0 / n
                        + (1..K)
                            .map(|a| {
                                (1..K)
                                    .map(|b| q[i][a] * inverse[a - 1][b - 1] * q[j][b])
                                    .sum::<f64>()
                            })
                            .sum::<f64>())
            })
        });
        (variance.is_finite() && variance >= 0.0 && coefficients.iter().all(|c| c.is_finite()))
            .then_some(GamFit {
                coefficients,
                covariance,
                #[cfg(test)]
                audit: serde_json::json!({"n":self.n,"offset":self.offset,"response_scale":self.scale,
                    "nodal_crossproduct":self.xx,"centered_crossproduct":xx,
                    "centered_penalty":penalty,"centered_rhs":rhs,"centered_yy":centered_yy,
                    "transform":q,"mean":mean,"log_lambda":optimum[0],"optimized_log_variance":optimum[1]+2.0*self.scale.ln(),
                    "reml_gradient":optimized.gradient,"reml_hessian":optimized.hessian,"reml":optimized.score,"iterations":_iterations,"edf":edf,"residual_sum_squares":rss*self.scale*self.scale,
                    "residual_variance":variance*self.scale*self.scale,
                    "nodal_coefficients":coefficients.map(|v|v*self.scale+self.offset),
                    "nodal_covariance":covariance.map(|r|r.map(|v|v*self.scale*self.scale))}),
            })
    }
}
// Gaussian identity-link REML only: one cs penalty and one estimated variance.
// Match mgcv's joint log(lambda)/log(variance) Newton stopping rule; solving a
// profiled score to machine precision gives different finite-tolerance curves.
struct GaussianReml {
    xx: Matrix,
    penalty: Matrix,
    rhs: [f64; K],
    yy: f64,
    n: f64,
    scale: f64,
}
struct RemlPoint {
    score: f64,
    gradient: [f64; 2],
    hessian: [[f64; 2]; 2],
    beta: [f64; K],
}
impl GaussianReml {
    fn profile_fallback(&self) -> Option<([f64; 2], RemlPoint, usize)> {
        // Degenerate/noiseless responses can make the joint Hessian singular.
        // Retain a finite profiled REML fit rather than losing existing views.
        let at = |rho: f64| {
            let mut a = self.xx;
            for (i, row) in a.iter_mut().enumerate().take(K - 1) {
                for (j, value) in row.iter_mut().enumerate().take(K - 1) {
                    *value += rho.exp() * self.penalty[i][j];
                }
            }
            let beta = solve(a, self.rhs, K - 1)?.0;
            let rss = (self.yy - dot(&beta, &self.rhs)).max(1e-300);
            let parameters = [rho, (rss / (self.n - 1.0)).ln()];
            Some((parameters, self.evaluate(parameters)?))
        };
        let (mut lo, mut hi) = (-30.0, 30.0);
        let left = at(lo)?;
        let right = at(hi)?;
        if left.1.gradient[0] >= 0.0 {
            return Some((left.0, left.1, 0));
        }
        if right.1.gradient[0] <= 0.0 {
            return Some((right.0, right.1, 0));
        }
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if at(mid)?.1.gradient[0] > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let result = at((lo + hi) / 2.0)?;
        Some((result.0, result.1, 0))
    }
    fn evaluate(&self, parameters: [f64; 2]) -> Option<RemlPoint> {
        let [rho, log_variance] = parameters;
        let lambda = rho.exp();
        let variance = log_variance.exp();
        let mut a = self.xx;
        for (i, row) in a.iter_mut().enumerate().take(K - 1) {
            for (j, value) in row.iter_mut().enumerate().take(K - 1) {
                *value += lambda * self.penalty[i][j];
            }
        }
        let (beta, logdet) = solve(a, self.rhs, K - 1)?;
        let rss = (self.yy - dot(&beta, &self.rhs)).max(1e-300);
        let mut inverse = [[0.0; K]; K];
        for j in 0..K - 1 {
            let mut e = [0.0; K];
            e[j] = 1.0;
            let column = solve(a, e, K - 1)?.0;
            for i in 0..K - 1 {
                inverse[i][j] = column[i];
            }
        }
        let h: Matrix = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..K - 1).map(|k| inverse[i][k] * self.xx[k][j]).sum())
        });
        let trace: f64 = (0..K - 1).map(|i| h[i][i]).sum();
        let trace2: f64 = (0..K - 1)
            .map(|i| (0..K - 1).map(|j| h[i][j] * h[j][i]).sum::<f64>())
            .sum();
        let pb: [f64; K] = std::array::from_fn(|i| dot(&self.penalty[i], &beta));
        let t = lambda * dot(&beta, &pb);
        let u = lambda * lambda * dot(&pb, &solve(a, pb, K - 1)?.0);
        let penalty_logdet = solve(self.penalty, [0.0; K], K - 1)?.1;
        let score = 0.5
            * (rss / variance
                + (self.n - 1.0)
                    * (log_variance + 2.0 * self.scale.ln() + (2.0 * std::f64::consts::PI).ln())
                + self.n.ln()
                + logdet
                - (K - 1) as f64 * rho
                - penalty_logdet);
        let gradient = [
            0.5 * (t / variance - trace),
            0.5 * (self.n - 1.0 - rss / variance),
        ];
        let cross = -0.5 * t / variance;
        let hessian = [
            [0.5 * ((t - 2.0 * u) / variance + trace - trace2), cross],
            [cross, 0.5 * rss / variance],
        ];
        (score.is_finite()
            && gradient.iter().all(|g| g.is_finite())
            && hessian.iter().flatten().all(|h| h.is_finite()))
        .then_some(RemlPoint {
            score,
            gradient,
            hessian,
            beta,
        })
    }
    fn optimize(&self) -> Option<([f64; 2], RemlPoint, usize)> {
        let mut lambda = (0..K - 1).map(|i| self.xx[i][i]).sum::<f64>()
            / (0..K - 1).map(|i| self.penalty[i][i]).sum::<f64>();
        if !lambda.is_finite() || lambda <= 0.0 {
            return None;
        }
        let fraction = |lambda: f64| {
            (0..K - 1)
                .map(|i| self.xx[i][i] / (self.xx[i][i] + lambda * self.penalty[i][i]))
                .sum::<f64>()
                / (K - 1) as f64
        };
        while fraction(lambda) > 0.4 {
            lambda *= 10.0;
            if !lambda.is_finite() {
                return None;
            }
        }
        while fraction(lambda) < 0.4 {
            lambda /= 10.0;
            if lambda == 0.0 {
                return None;
            }
        }
        let mut parameters = [lambda.ln(), (self.yy / self.n / 10.0).ln()];
        let mut point = self.evaluate(parameters)?;
        let scale =
            |p: &RemlPoint, x: [f64; 2]| p.score.abs() + (x[1] + 2.0 * self.scale.ln()).abs();
        let mut active = point
            .gradient
            .map(|g| g.abs() > scale(&point, parameters) * 1e-6);
        if !active.iter().any(|v| *v) {
            active = [true; 2];
        }
        for iteration in 0..200 {
            if !active.iter().any(|v| *v) {
                active[usize::from(point.gradient[1].abs() > point.gradient[0].abs())] = true;
            }
            let score_scale = scale(&point, parameters);
            let (mut step, positive) = newton_step(point.gradient, point.hessian, active);
            let maximum = step[0].abs().max(step[1].abs());
            if maximum > 5.0 {
                step = step.map(|v| v * 5.0 / maximum);
            }
            let grad_max = point.gradient[0].abs().max(point.gradient[1].abs());
            if grad_max == 0.0 {
                return Some((parameters, point, iteration));
            }
            let steepest = point.gradient.map(|g| -g / grad_max);
            let trial = |step: [f64; 2]| -> Option<([f64; 2], RemlPoint, f64)> {
                let x = std::array::from_fn(|i| parameters[i] + step[i]);
                let p = self.evaluate(x)?;
                let change = p.score - point.score;
                let predicted = (0..2).map(|i| point.gradient[i] * step[i]).sum::<f64>()
                    + 0.5
                        * (0..2)
                            .map(|i| {
                                (0..2)
                                    .map(|j| step[i] * point.hessian[i][j] * step[j])
                                    .sum::<f64>()
                            })
                            .sum::<f64>();
                let error = (predicted - change).abs()
                    / (predicted.abs().max(change.abs()) + score_scale * 1e-6);
                Some((x, p, error))
            };
            let mut candidate = trial(step)?;
            let mut halves = 0;
            let mut steepest_unused = true;
            while (candidate.1.score >= point.score || candidate.2 >= 0.8) && halves < 30 {
                if halves == 3 && iteration < 9 {
                    let length = (step[0] * step[0] + step[1] * step[1]).sqrt().min(2.0);
                    let norm = (steepest[0] * steepest[0] + steepest[1] * steepest[1]).sqrt();
                    step = steepest.map(|v| v * length / norm);
                    steepest_unused = false;
                } else {
                    step = step.map(|v| v / 2.0);
                }
                candidate = trial(step)?;
                if halves > 4 {
                    candidate.2 = 0.4;
                }
                if candidate.1.score < point.score && candidate.2 < 0.8 {
                    break;
                }
                halves += 1;
            }
            if !positive && steepest_unused {
                for k in 1..=40 {
                    let alternate = trial(steepest.map(|v| v * 2.0_f64.powi(1 - k)))?;
                    if alternate.2 < 0.8 && alternate.1.score <= candidate.1.score {
                        candidate = alternate;
                    } else if candidate.1.score < point.score
                        && alternate.1.score > candidate.1.score
                    {
                        break;
                    }
                }
            }
            let change = (point.score - candidate.1.score).abs();
            parameters = candidate.0;
            point = candidate.1;
            let score_scale = scale(&point, parameters);
            active = std::array::from_fn(|i| {
                point.gradient[i].abs() > score_scale * 1e-7
                    || point.hessian[i][i].abs() > score_scale * 1e-7
            });
            if (positive
                && change <= score_scale * 1e-6
                && point.gradient.iter().all(|g| g.abs() <= score_scale * 5e-6))
                || halves == 30
            {
                return Some((parameters, point, iteration + 1));
            }
        }
        None
    }
}
fn newton_step(g: [f64; 2], h: [[f64; 2]; 2], active: [bool; 2]) -> ([f64; 2], bool) {
    if !active[0] || !active[1] {
        let i = usize::from(!active[0]);
        let mut step = [0.0; 2];
        step[i] = -g[i] / h[i][i].abs();
        return (step, h[i][i] > 0.0);
    }
    let angle = 0.5 * (2.0 * h[0][1]).atan2(h[0][0] - h[1][1]);
    let (s, c) = angle.sin_cos();
    let eigen = [
        c * c * h[0][0] + 2.0 * c * s * h[0][1] + s * s * h[1][1],
        s * s * h[0][0] - 2.0 * c * s * h[0][1] + c * c * h[1][1],
    ];
    let floor = eigen[0].abs().max(eigen[1].abs()) * f64::EPSILON.powf(0.7);
    let a = -(c * g[0] + s * g[1]) / eigen[0].abs().max(floor);
    let b = -(-s * g[0] + c * g[1]) / eigen[1].abs().max(floor);
    (
        [c * a - s * b, s * a + c * b],
        eigen.iter().all(|e| *e > 0.0),
    )
}

struct GamFit {
    coefficients: [f64; K],
    covariance: Matrix,
    #[cfg(test)]
    audit: serde_json::Value,
}
fn dot(a: &[f64; K], b: &[f64; K]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
// Small SPD solve for REML coefficients and posterior covariance.
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
    #[cfg(test)]
    audit: serde_json::Value,
}
impl Basis {
    #[allow(clippy::needless_range_loop)] // Column-wise tridiagonal solves share neighboring rows.
    fn new(pop: &Population) -> Option<Self> {
        let knots = pop.knots?;
        if knots.windows(2).any(|p| !p[0].is_finite() || p[1] <= p[0]) {
            return None;
        }
        let widths: [f64; K - 1] = std::array::from_fn(|i| knots[i + 1] - knots[i]);
        let difference: Matrix = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                if i == 0 || i == K - 1 {
                    0.0
                } else if j == i - 1 {
                    1.0 / widths[i - 1]
                } else if j == i {
                    -1.0 / widths[i - 1] - 1.0 / widths[i]
                } else if j == i + 1 {
                    1.0 / widths[i]
                } else {
                    0.0
                }
            })
        });
        // LDL' solve of the natural cubic spline tridiagonal system B F = D,
        // followed by S = D' F. Keep original hour units, matching mgcv's
        // constructor rather than changing null-space rounding by rescaling.
        let mut diagonal: [f64; K] = std::array::from_fn(|i| {
            if i == 0 || i == K - 1 {
                0.0
            } else {
                (widths[i - 1] + widths[i]) / 3.0
            }
        });
        let mut lower = [0.0; K];
        for i in 1..K - 2 {
            let off = widths[i] / 6.0;
            lower[i] = off / diagonal[i];
            diagonal[i + 1] = (-lower[i]).mul_add(off, diagonal[i + 1]);
        }
        let mut second = [[0.0; K]; K];
        for column in 0..K {
            let mut rhs: [f64; K] = std::array::from_fn(|i| difference[i][column]);
            for i in 2..K - 1 {
                rhs[i] = (-lower[i - 1]).mul_add(rhs[i - 1], rhs[i]);
            }
            second[K - 2][column] = rhs[K - 2] / diagonal[K - 2];
            for i in (1..K - 2).rev() {
                second[i][column] =
                    (-lower[i]).mul_add(second[i + 1][column], rhs[i] / diagonal[i]);
            }
        }
        let raw: Matrix = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                if i == 0 {
                    difference[1][i] * second[1][j]
                } else if i == K - 1 {
                    difference[K - 2][i] * second[K - 2][j]
                } else if i == 1 {
                    second[1][j].mul_add(difference[1][i], second[2][j] * difference[2][i])
                } else if i == K - 2 {
                    second[K - 3][j].mul_add(
                        difference[K - 3][i],
                        second[K - 2][j] * difference[K - 2][i],
                    )
                } else {
                    second[i + 1][j].mul_add(
                        difference[i + 1][i],
                        second[i - 1][j]
                            .mul_add(difference[i - 1][i], second[i][j] * difference[i][i]),
                    )
                }
            })
        });
        let penalty =
            std::array::from_fn(|i| std::array::from_fn(|j| (raw[i][j] + raw[j][i]) / 2.0));
        let (eigen, mut vectors) = symmetric_eigen(penalty);
        // The mathematical null space is two-dimensional. Fix an explicit
        // endpoint-zero linear / orthogonal constant convention; numerical
        // sorting of roundoff-size eigenvalues rotates or swaps these axes.
        // This matches the verified PromethION case, but not every pinned-R
        // null-space ordering: those comparisons deliberately remain Partial.
        let mut linear = knots.map(|x| x - knots[0]);
        let linear_norm = dot(&linear, &linear).sqrt();
        for v in &mut linear {
            *v /= linear_norm;
        }
        let projection: f64 = linear.iter().sum();
        let mut constant = linear.map(|v| 1.0 - projection * v);
        let constant_norm = dot(&constant, &constant).sqrt();
        for i in 0..K {
            constant[i] /= constant_norm;
            vectors[i][0] = constant[i];
            vectors[i][1] = linear[i];
        }
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
            #[cfg(test)]
            audit: serde_json::json!({"knots":knots,"second":second,"raw_penalty":raw,
                "raw_eigenvalues":eigen,"shrink_eigenvectors":vectors,"null_space_convention":"endpoint_zero_linear_high","normalized_shrink_penalty":penalty}),
        })
    }
    fn evaluate(&self, x: f64) -> [f64; K] {
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
    let tolerance = a.iter().flatten().fold(0.0_f64, |m, v| m.max(v.abs())) * f64::EPSILON;
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
        if a[p][q].abs() <= tolerance {
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
    let active: Vec<_> = design
        .iter()
        .zip(response.iter())
        .filter(|(r, _)| r.iter().any(|v| *v != 0.0))
        .collect();
    if let [(r0, y0), (r1, y1)] = active.as_slice() {
        // The small-span edge case has two weighted equations for three
        // coefficients. Solve its row-space minimum-norm problem directly.
        // QR of a row with weight ~1e-24 followed by an O(1) row otherwise
        // loses its response by cancellation, corrupting vertex derivatives.
        let a: f64 = r0.iter().map(|v| v * v).sum();
        let b: f64 = r0.iter().zip(r1.iter()).map(|(a, b)| a * b).sum();
        let c: f64 = r1.iter().map(|v| v * v).sum();
        let remainder = c - b * b / a;
        if a > 0.0 && remainder > c * f64::EPSILON {
            let second = (**y1 - b * **y0 / a) / remainder;
            let first = **y0 / a - b * second / a;
            let mut solution = [0.0; K];
            for j in 0..3 {
                solution[j] = (r0[j] * first + r1[j] * second) / norms[j].max(f64::MIN_POSITIVE);
            }
            if solution.iter().all(|v| v.is_finite()) {
                return Some(solution);
            }
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
                tracker.add(&record, 7.0).unwrap();
            }
            let mut plan = tracker.prepare(false).unwrap();
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
                    if expected.as_array().unwrap().len() == 5 {
                        for (value, reference) in
                            [actual.standard_error, actual.lower, actual.upper]
                                .iter()
                                .zip(&expected.as_array().unwrap()[2..])
                        {
                            let r = reference.as_f64().unwrap();
                            assert!((value.unwrap() - r).abs() <= 1e-6 + r.abs() * 1e-6);
                        }
                    }
                }
            }
        }
        eprintln!(
            "Actual R fitted values compared: {compared}; max absolute error: {maximum_error:e}"
        );
    }
    #[test]
    fn actual_r_promethion_gam_endpoint_basis_and_uncertainty_regression() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../tests/reference/gam-golden.json")).unwrap();
        for case in golden["cases"].as_array().unwrap() {
            let knots: [f64; K] = serde_json::from_value(case["knots"].clone()).unwrap();
            let population = Population {
                count: case["n"].as_u64().unwrap(),
                min: knots[0],
                max: knots[K - 1],
                knots: Some(knots),
                ..Population::default()
            };
            let selected = case["selected"].as_bool().unwrap();
            let quality = case["quality"].as_bool().unwrap();
            let mut dataset = Dataset::new(
                if selected {
                    "qscore_selected_summary_records"
                } else {
                    "all_summary_records"
                },
                population,
                quality,
                quality && !selected,
            );
            dataset.n = case["n"].as_u64().unwrap();
            dataset.scale = case["scale"].as_f64().unwrap();
            dataset.offset = case["offset"].as_f64().unwrap();
            dataset.xx = serde_json::from_value(case["xx"].clone()).unwrap();
            dataset.rhs = serde_json::from_value(case["rhs"].clone()).unwrap();
            dataset.yy = case["yy"].as_f64().unwrap();
            let basis = dataset.basis.as_ref().unwrap();
            for (field, actual) in [
                ("second", basis.second),
                ("normalized_shrink_penalty", basis.penalty),
            ] {
                let reference: Matrix = serde_json::from_value(case[field].clone()).unwrap();
                for (a, r) in actual.iter().flatten().zip(reference.iter().flatten()) {
                    assert!(
                        (a - r).abs() <= 1e-12 + 1e-10 * r.abs(),
                        "{field}: {a} != {r}"
                    );
                }
            }
            let curve = dataset.fit(true);
            let reference = case["points"].as_array().unwrap();
            assert_eq!(curve.points.len(), 80);
            for (point, row) in curve.points.iter().zip(reference) {
                assert!((point.hours - row[0].as_f64().unwrap()).abs() < 1e-10);
                for (actual, expected) in [
                    Some(point.value),
                    point.standard_error,
                    point.lower,
                    point.upper,
                ]
                .iter()
                .zip(&row.as_array().unwrap()[1..])
                {
                    let (a, r) = (actual.unwrap(), expected.as_f64().unwrap());
                    assert!(
                        (a - r).abs() <= 1e-6 + 1e-6 * r.abs(),
                        "quality={quality}, selected={selected}, time={}: {a} != {r}",
                        point.hours
                    );
                }
            }
            if selected && !quality {
                // Previously 11961.667995 bp versus actual R 11750.650904 bp.
                let reference = 11750.65090436984_f64;
                assert!((curve.points[79].value - reference).abs() <= 1e-6 + 1e-6 * reference);
            }
        }
    }
    #[test]
    fn gaussian_reml_gradient_and_hessian_match_finite_differences() {
        let xx = std::array::from_fn(|i| {
            std::array::from_fn(|j| if i == j { 100.0 + i as f64 } else { 5.0 })
        });
        let penalty = std::array::from_fn(|i| {
            std::array::from_fn(|j| if i == j { 1.0 + i as f64 } else { 0.05 })
        });
        let reml = GaussianReml {
            xx,
            penalty,
            rhs: std::array::from_fn(|i| 3.0 + i as f64),
            yy: 500.0,
            n: 1500.0,
            scale: 100.0,
        };
        let parameters = [2.0, -1.0];
        let point = reml.evaluate(parameters).unwrap();
        let step = 1e-4;
        for i in 0..2 {
            let mut left = parameters;
            let mut right = parameters;
            left[i] -= step;
            right[i] += step;
            let a = reml.evaluate(left).unwrap();
            let b = reml.evaluate(right).unwrap();
            assert!(((b.score - a.score) / (2.0 * step) - point.gradient[i]).abs() < 1e-5);
            for j in 0..2 {
                assert!(
                    ((b.gradient[j] - a.gradient[j]) / (2.0 * step) - point.hessian[j][i]).abs()
                        < 1e-5
                );
            }
        }
    }
    fn population(n: usize) -> Population {
        let mut p = Population::default();
        let mut times = crate::trend_knots::Times::default();
        for i in 0..n {
            let x = i as f64 / (n - 1).max(1) as f64;
            times.add(x, 7).unwrap();
            p.add(Observation {
                x,
                length: 100.0 + 50.0 * x * x,
                q: 8.0 + 2.0 * x,
            });
        }
        p.knots = times.finish().unwrap()[0];
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
        assert!(curve.confidence_interval.is_some());
        assert!(
            curve
                .points
                .iter()
                .all(|p| p.standard_error.unwrap() < 1e-6)
        );
    }
    #[test]
    fn gam_is_stable_under_large_response_offsets() {
        let fit = |offset: f64| {
            let mut pop = Population::default();
            let mut times = crate::trend_knots::Times::default();
            for i in 0..1200 {
                let x = i as f64 / 1199.0;
                times.add(x, 7).unwrap();
                pop.add(Observation {
                    x,
                    length: 100.0,
                    q: offset + 8.0 + 2.0 * x,
                });
            }
            pop.knots = times.finish().unwrap()[0];
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
        assert!(p.knots.is_some());
        assert!(p.small.is_empty());
        let tracker = Tracker {
            all: population(99),
            positive_q: population(99),
            selected: population(60),
            times: crate::trend_knots::Times::default(),
        };
        let plan = tracker.prepare(true).unwrap();
        assert!(!plan.single_gam);
        assert!(plan.combined_gam);
        let result = plan.finish();
        assert!(result.single.iter().all(|c| c.method.starts_with("loess")));
        assert!(result.combined.iter().all(|c| c.method.contains("REML")));
    }
}
