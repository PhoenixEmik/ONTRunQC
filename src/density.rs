//! R 4.4-style Gaussian density on a shared ggplot scale domain. Independent
//! discrete convolution equals the padded FFT's convolution without an FFT crate.
use crate::metrics::{DensityEstimate, DensityPoint};
#[derive(Clone, Copy, Default)]
pub struct Bounds {
    pub length: Option<[f64; 2]>,
    pub qscore: Option<[f64; 2]>,
}
impl Bounds {
    pub fn merge(&mut self, other: Self) {
        for (to, from) in [
            (&mut self.length, other.length),
            (&mut self.qscore, other.qscore),
        ] {
            if let Some(v) = from {
                *to = Some(to.map_or(v, |a| [a[0].min(v[0]), a[1].max(v[1])]));
            }
        }
    }
}
pub fn range(values: impl Iterator<Item = f64>) -> Option<[f64; 2]> {
    values.fold(None, |bounds, v| {
        Some(bounds.map_or([v, v], |b: [f64; 2]| [b[0].min(v), b[1].max(v)]))
    })
}
pub fn estimate<I: Iterator<Item = f64>>(
    values: impl Fn() -> I,
    domain: Option<[f64; 2]>,
) -> Option<DensityEstimate> {
    let n = values().count();
    if n < 2 {
        return None;
    }
    let first = values().next()?;
    let last = values().last()?;
    let origin = first / 2.0 + last / 2.0;
    let scale = if first == last {
        first.abs().max(1.0)
    } else {
        (first - origin).abs().max((last - origin).abs())
    };
    let quantile = |p: f64| {
        let rank = (n - 1) as f64 * p;
        let mut it = values();
        let a = (it.nth(rank.floor() as usize).unwrap() - origin) / scale;
        let b = if rank.fract() == 0.0 {
            a
        } else {
            (it.next().unwrap() - origin) / scale
        };
        a + (b - a) * rank.fract()
    };
    let (mut mean, mut m2) = (0.0, 0.0);
    for (i, v) in values().enumerate() {
        let z = (v - origin) / scale;
        let d = z - mean;
        mean += d / (i + 1) as f64;
        m2 += d * (z - mean);
    }
    let sd = (m2 / (n - 1) as f64).sqrt();
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
    let bandwidth = 0.9 * spread * (n as f64).powf(-0.2) * scale;
    if !bandwidth.is_finite() || bandwidth <= 0.0 {
        return None;
    }
    let [mut from, mut to] = domain.unwrap_or([first, last]);
    if from == to {
        from -= 0.5;
        to += 0.5;
    } // ggplot's degenerate continuous scale width
    // Compute in bandwidth units for stable nearby/large finite observations.
    let lower = (from - origin) / bandwidth - 4.0;
    let upper = (to - origin) / bandwidth + 4.0;
    let step = (upper - lower) / 511.0;
    if !lower.is_finite() || !upper.is_finite() || !step.is_finite() || step <= 0.0 {
        return None;
    }
    let mut mass = [0.0; 512];
    for value in values() {
        let p = (((value - origin) / bandwidth - lower) / step).clamp(0.0, 511.0);
        let i = p.floor() as usize;
        mass[i] += (1.0 - p.fract()) / n as f64;
        if i < 511 {
            mass[i + 1] += p.fract() / n as f64;
        }
    }
    let kernel: [f64; 512] = std::array::from_fn(|i| {
        (-0.5 * (i as f64 * step).powi(2)).exp() / (2.0 * std::f64::consts::PI).sqrt() / bandwidth
    });
    let grid: [f64; 512] = std::array::from_fn(|i| {
        mass.iter()
            .enumerate()
            .map(|(j, w)| w * kernel[i.abs_diff(j)])
            .sum()
    });
    let points: Vec<_> = (0..512)
        .map(|i| {
            let f = i as f64 / 511.0;
            let x = from * (1.0 - f) + to * f;
            let position = (((x - origin) / bandwidth - lower) / step).clamp(0.0, 511.0);
            let j = (position.floor() as usize).min(510);
            let weight = position - j as f64;
            DensityPoint {
                x,
                density: grid[j] * (1.0 - weight) + grid[j + 1] * weight,
            }
        })
        .collect();
    points
        .iter()
        .all(|p| p.x.is_finite() && p.density.is_finite())
        .then_some(DensityEstimate {
            observations: n,
            bandwidth,
            points,
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaussian_convolution_preserves_mass_symmetry_and_peaks() {
        let x = [-2.0, -2.0, -1.9, 1.9, 2.0, 2.0];
        let d = estimate(|| x.into_iter(), Some([-10.0, 10.0])).unwrap();
        let area: f64 = d
            .points
            .windows(2)
            .map(|p| (p[1].x - p[0].x) * (p[0].density + p[1].density) / 2.0)
            .sum();
        assert!((area - 1.0).abs() < 0.001);
        for (a, b) in d.points.iter().zip(d.points.iter().rev()) {
            assert!((a.density - b.density).abs() < 1e-12);
        }
        let peak = d
            .points
            .iter()
            .max_by(|a, b| a.density.total_cmp(&b.density))
            .unwrap();
        assert!(peak.x.abs() > 1.0);
    }
    #[test]
    fn nrd0_uses_sample_sd_type7_iqr_and_sparse_rules() {
        assert!(estimate(|| [1.0].into_iter(), None).is_none());
        let d = estimate(|| [1.0, 2.0, 3.0, 4.0].into_iter(), None).unwrap();
        let expected = 0.9 * (1.5_f64 / 1.34).min((5.0_f64 / 3.0).sqrt()) * 4.0_f64.powf(-0.2);
        assert!((d.bandwidth - expected).abs() < 1e-14);
        assert_eq!(d.points[0].x, 1.0);
        assert_eq!(d.points[511].x, 4.0);
        assert!(estimate(|| [20.0, 20.0, 20.0].into_iter(), None).is_some());
        assert!(estimate(|| [0.0, 0.0].into_iter(), None).is_some());
    }
    #[test]
    fn close_values_long_tails_and_shared_domains_remain_explicit() {
        let close = [20.0, 20.00000001, 20.00000002];
        assert!(estimate(|| close.into_iter(), None).unwrap().bandwidth < 1e-7);
        let x = [0.0, 0.001, 1.0, 100.0, 1e9];
        let d = estimate(|| x.into_iter(), Some([-1.0, 1e9])).unwrap();
        assert_eq!(d.observations, 5);
        assert_eq!(d.points[0].x, -1.0);
        assert_eq!(d.points[511].x, 1e9);
        assert!(d.points.iter().all(|p| p.density >= 0.0));
    }
}
