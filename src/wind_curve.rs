//! Normalized control points shared by the editor and the actual fan mapping.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_POINTS: usize = 16;
pub const MIN_GAP: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curve {
    pub points: Vec<Point>,
    pub smoothing: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Linear,
    EarlyBoost,
    Balanced,
    Gentle,
    SCurve,
}
impl Preset {
    pub const ALL: [Self; 5] = [
        Self::Linear,
        Self::EarlyBoost,
        Self::Balanced,
        Self::Gentle,
        Self::SCurve,
    ];
    pub fn curve(self) -> Curve {
        match self {
            Self::Linear => {
                Curve::from_pairs(&[(0., 0.), (0.25, 0.25), (0.5, 0.5), (0.75, 0.75), (1., 1.)])
            }
            Self::EarlyBoost => Curve::from_pairs(&[
                (0., 0.),
                (0.1, 0.4),
                (0.25, 0.65),
                (0.5, 0.83),
                (0.75, 0.93),
                (1., 1.),
            ]),
            Self::Balanced => Curve::from_exponent(0.6),
            Self::Gentle => {
                Curve::from_pairs(&[(0., 0.), (0.25, 0.1), (0.5, 0.3), (0.75, 0.6), (1., 1.)])
            }
            Self::SCurve => Curve::from_pairs(&[
                (0., 0.),
                (0.2, 0.08),
                (0.4, 0.32),
                (0.6, 0.68),
                (0.8, 0.92),
                (1., 1.),
            ]),
        }
    }
}
impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Linear => "Linear",
            Self::EarlyBoost => "Early boost",
            Self::Balanced => "Balanced",
            Self::Gentle => "Gentle",
            Self::SCurve => "S-curve",
        })
    }
}
impl Default for Curve {
    fn default() -> Self {
        Preset::Balanced.curve()
    }
}
impl Curve {
    fn from_pairs(pairs: &[(f64, f64)]) -> Self {
        Self {
            points: pairs.iter().map(|&(x, y)| Point { x, y }).collect(),
            smoothing: 1.0,
        }
    }
    /// Sample the previous exponent setting when migrating, retaining its character.
    pub fn from_exponent(exponent: f64) -> Self {
        Self {
            points: [0.0_f64, 0.03, 0.1, 0.25, 0.4, 0.6, 0.8, 1.0]
                .into_iter()
                .map(|x| Point {
                    x,
                    y: x.powf(exponent),
                })
                .collect(),
            smoothing: 1.0,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (2..=MAX_POINTS).contains(&self.points.len()),
            "Curve needs 2–16 points"
        );
        ensure!(
            self.smoothing.is_finite() && (0.0..=1.0).contains(&self.smoothing),
            "Smoothing must be 0–100%"
        );
        ensure!(
            self.points.iter().all(|p| p.x.is_finite()
                && p.y.is_finite()
                && (0.0..=1.0).contains(&p.x)
                && (0.0..=1.0).contains(&p.y)),
            "Curve coordinates must be finite fractions from 0 to 1"
        );
        ensure!(
            self.points.first() == Some(&Point { x: 0., y: 0. })
                && self.points.last() == Some(&Point { x: 1., y: 1. }),
            "Curve endpoints must match minimum and maximum"
        );
        ensure!(
            self.points
                .windows(2)
                .all(|p| p[1].x - p[0].x >= MIN_GAP - 1e-9),
            "Curve speed positions must be ordered and at least 1% apart"
        );
        Ok(())
    }
    pub fn movable(&self, i: usize) -> bool {
        i > 0 && i + 1 < self.points.len()
    }
    pub fn move_point(&mut self, i: usize, y: f64) {
        if self.movable(i) && y.is_finite() {
            self.points[i].y = y.clamp(0.0, 1.0);
        }
    }
    pub fn insert(&mut self, x: f64, y: f64) -> Option<usize> {
        if self.points.len() >= MAX_POINTS
            || !x.is_finite()
            || !y.is_finite()
            || !(MIN_GAP..=1.0 - MIN_GAP).contains(&x)
            || self.points.iter().any(|p| (p.x - x).abs() < MIN_GAP - 1e-9)
        {
            return None;
        }
        let i = self.points.partition_point(|p| p.x < x);
        self.points.insert(
            i,
            Point {
                x,
                y: y.clamp(0., 1.),
            },
        );
        Some(i)
    }
    pub fn add_in_largest_gap(&mut self) -> Option<usize> {
        let gap = self
            .points
            .windows(2)
            .max_by(|a, b| (a[1].x - a[0].x).total_cmp(&(b[1].x - b[0].x)))?;
        let x = (gap[0].x + gap[1].x) / 2.;
        self.insert(x, self.sample(x))
    }
    pub fn remove(&mut self, i: usize) -> bool {
        if !self.movable(i) {
            return false;
        }
        self.points.remove(i);
        true
    }
    fn slope(&self, i: usize) -> f64 {
        let a = self.points[i];
        let b = self.points[i + 1];
        (b.y - a.y) / (b.x - a.x)
    }
    fn tangent(&self, i: usize) -> f64 {
        if i == 0 {
            return self.slope(0);
        }
        if i + 1 == self.points.len() {
            return self.slope(i - 1);
        }
        let a = self.slope(i - 1);
        let b = self.slope(i);
        // A plateau or turning point must not acquire an overshooting tangent.
        if a * b <= 0. {
            return 0.;
        }
        let h0 = self.points[i].x - self.points[i - 1].x;
        let h1 = self.points[i + 1].x - self.points[i].x;
        let w0 = 2. * h1 + h0;
        let w1 = h1 + 2. * h0;
        (w0 + w1) / (w0 / a + w1 / b)
    }
    pub fn sample(&self, x: f64) -> f64 {
        let x = if x.is_finite() { x.clamp(0., 1.) } else { 0. };
        if x <= 0. {
            return 0.;
        }
        if x >= 1. {
            return 1.;
        }
        let i = self
            .points
            .partition_point(|p| p.x <= x)
            .saturating_sub(1)
            .min(self.points.len() - 2);
        let a = self.points[i];
        let b = self.points[i + 1];
        let h = b.x - a.x;
        let t = (x - a.x) / h;
        let linear = a.y + (b.y - a.y) * t;
        let t2 = t * t;
        let t3 = t2 * t;
        let smooth = (2. * t3 - 3. * t2 + 1.) * a.y
            + (t3 - 2. * t2 + t) * h * self.tangent(i)
            + (-2. * t3 + 3. * t2) * b.y
            + (t3 - t2) * h * self.tangent(i + 1);
        (linear + (smooth - linear) * self.smoothing).clamp(a.y.min(b.y), a.y.max(b.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_interpolate_points_and_stay_monotone_for_every_smoothing() {
        for preset in Preset::ALL {
            let mut curve = preset.curve();
            curve.validate().unwrap();
            for smoothing in [0., 0.25, 0.5, 1.] {
                curve.smoothing = smoothing;
                for p in &curve.points {
                    assert!((curve.sample(p.x) - p.y).abs() < 1e-10);
                }
                let mut prev = 0.;
                for i in 0..=1000 {
                    let y = curve.sample(i as f64 / 1000.);
                    assert!(y + 1e-10 >= prev && (0.0..=1.0).contains(&y));
                    prev = y;
                }
            }
        }
    }
    #[test]
    fn smoothing_changes_transitions_without_overshoot_even_at_peaks() {
        let mut c = Curve::from_pairs(&[(0., 0.), (0.1, 0.8), (0.2, 0.8), (0.6, 0.1), (1., 1.)]);
        let rounded = c.sample(0.05);
        c.smoothing = 0.;
        assert!((c.sample(0.05) - 0.4).abs() < 1e-10);
        assert!((rounded - 0.4).abs() > 0.01);
        for smoothing in [0., 0.5, 1.] {
            c.smoothing = smoothing;
            for gap in c.points.windows(2) {
                for j in 0..=100 {
                    let y = c.sample(gap[0].x + (gap[1].x - gap[0].x) * j as f64 / 100.);
                    assert!(
                        y >= gap[0].y.min(gap[1].y) - 1e-10 && y <= gap[0].y.max(gap[1].y) + 1e-10
                    );
                }
            }
        }
    }
    #[test]
    fn edit_limits_endpoints_and_duplicate_positions() {
        let mut c = Preset::Linear.curve();
        c.move_point(0, 0.8);
        assert_eq!(c.points[0].y, 0.);
        assert!(!c.remove(0));
        assert!(!c.remove(c.points.len() - 1));
        assert!(c.insert(0.25, 0.8).is_none());
        assert!(c.insert(f64::NAN, 0.5).is_none());
        let i = c.insert(0.4, 0.7).unwrap();
        c.move_point(i, 2.);
        assert_eq!(c.points[i].y, 1.);
        assert!(c.remove(i));
        while c.points.len() < MAX_POINTS {
            assert!(c.add_in_largest_gap().is_some());
        }
        assert!(c.add_in_largest_gap().is_none());
        c.validate().unwrap();
        c.points[1].x = c.points[0].x;
        assert!(c.validate().is_err());
    }
    #[test]
    fn rejects_corrupt_saved_curves() {
        for fault in 0..6 {
            let mut c = Curve::default();
            match fault {
                0 => c.points.clear(),
                1 => c.points[1].y = f64::NAN,
                2 => c.points[1].x = 0.999,
                3 => c.smoothing = 1.1,
                4 => c.points[0].y = 0.1,
                _ => c.points[1].y = -0.1,
            };
            assert!(c.validate().is_err());
        }
    }
}
