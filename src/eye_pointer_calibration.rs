//! Local alignment for the dashboard pointer. Raw game gaze is unchanged.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const TARGET_COUNT: usize = 17;
const BANDWIDTH: f64 = 0.32;

#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub observed: [f64; 2],
    pub expected: [f64; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub version: u8,
    pub observed: [[f64; 2]; TARGET_COUNT],
    pub offsets: [[f64; 2]; TARGET_COUNT],
    /// Observed gaze bounds keep the correction local outside the sampled area.
    pub bounds: [f64; 4],
    pub scale: [f64; 2],
    pub bandwidth: f64,
}

impl Profile {
    pub fn corrected(&self, gaze: [f64; 2]) -> [f64; 2] {
        let x = gaze[0].clamp(self.bounds[0], self.bounds[1]);
        let y = gaze[1].clamp(self.bounds[2], self.bounds[3]);
        let mut weight_sum = 0.0;
        let mut offset = [0.0, 0.0];
        for (point, delta) in self.observed.iter().zip(self.offsets.iter()) {
            let dx = (x - point[0]) / self.scale[0];
            let dy = (y - point[1]) / self.scale[1];
            let weight = (-(dx * dx + dy * dy) / (2.0 * self.bandwidth.powi(2))).exp();
            weight_sum += weight;
            offset[0] += weight * delta[0];
            offset[1] += weight * delta[1];
        }
        if weight_sum > 1e-12 {
            offset[0] /= weight_sum;
            offset[1] /= weight_sum;
        }
        [
            gaze[0] + offset[0].clamp(-0.25, 0.25),
            gaze[1] + offset[1].clamp(-0.25, 0.25),
        ]
    }
}

pub fn profile_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA").context("Windows roaming app data unavailable")?;
    Ok(PathBuf::from(appdata)
        .join("Rig Companion")
        .join("eye-pointer-calibration.json"))
}

pub fn fit(targets: &[Target]) -> Result<Profile> {
    ensure!(
        targets.len() == TARGET_COUNT,
        "Collect the center and both eight-point rings"
    );
    let mut observed = [[0.0; 2]; TARGET_COUNT];
    let mut offsets = [[0.0; 2]; TARGET_COUNT];
    let mut bounds = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut scale = [0.0f64; 2];
    for (index, target) in targets.iter().enumerate() {
        let [x, y] = target.observed;
        let [expected_x, expected_y] = target.expected;
        ensure!(
            x.is_finite() && y.is_finite() && expected_x.is_finite() && expected_y.is_finite(),
            "Invalid calibration point"
        );
        ensure!(
            x.abs() < 1.0 && y.abs() < 1.0,
            "Gaze point outside calibration range"
        );
        observed[index] = [x, y];
        offsets[index] = [expected_x - x, expected_y - y];
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].max(x);
        bounds[2] = bounds[2].min(y);
        bounds[3] = bounds[3].max(y);
        scale[0] = scale[0].max(expected_x.abs());
        scale[1] = scale[1].max(expected_y.abs());
    }
    ensure!(
        (0.2..=0.8).contains(&scale[0]) && (0.1..=0.8).contains(&scale[1]),
        "Calibration targets do not cover the expected view"
    );
    ensure!(
        bounds[1] - bounds[0] > 0.65 && bounds[3] - bounds[2] > 0.28,
        "Calibration targets did not span enough of the view"
    );
    ensure!(
        offsets.iter().flatten().all(|value| value.abs() < 0.18),
        "A target was captured too far from its expected position; try again"
    );
    let mut sorted_x = offsets.iter().map(|delta| delta[0]).collect::<Vec<_>>();
    let mut sorted_y = offsets.iter().map(|delta| delta[1]).collect::<Vec<_>>();
    sorted_x.sort_by(f64::total_cmp);
    sorted_y.sort_by(f64::total_cmp);
    let median = [sorted_x[TARGET_COUNT / 2], sorted_y[TARGET_COUNT / 2]];
    ensure!(
        offsets
            .iter()
            .all(|delta| { (delta[0] - median[0]).hypot(delta[1] - median[1]) < 0.14 }),
        "One target disagreed with the others; look at each small center dot and try again"
    );
    let profile = Profile {
        version: 3,
        observed,
        offsets,
        bounds,
        scale,
        bandwidth: BANDWIDTH,
    };
    let mut squared_error = 0.0;
    let mut largest_error = 0.0f64;
    for target in targets {
        let corrected = profile.corrected(target.observed);
        let error = (corrected[0] - target.expected[0]).hypot(corrected[1] - target.expected[1]);
        squared_error += error * error;
        largest_error = largest_error.max(error);
    }
    ensure!(
        (squared_error / TARGET_COUNT as f64).sqrt() < 0.055 && largest_error < 0.075,
        "Calibration points do not agree well enough for a smooth correction; try again"
    );
    validate_shape(&profile)?;
    Ok(profile)
}

fn validate_shape(profile: &Profile) -> Result<()> {
    const STEP: f64 = 0.0001;
    for ix in 0..9 {
        for iy in 0..9 {
            let x = profile.bounds[0]
                + (profile.bounds[1] - profile.bounds[0]) * (ix as f64 + 0.5) / 9.0;
            let y = profile.bounds[2]
                + (profile.bounds[3] - profile.bounds[2]) * (iy as f64 + 0.5) / 9.0;
            let corrected = profile.corrected([x, y]);
            ensure!(
                (corrected[0] - x).abs() < 0.18 && (corrected[1] - y).abs() < 0.18,
                "Alignment offset is too large in one area; try the targets again"
            );
            let right = profile.corrected([x + STEP, y]);
            let left = profile.corrected([x - STEP, y]);
            let up = profile.corrected([x, y + STEP]);
            let down = profile.corrected([x, y - STEP]);
            let gradient = [
                (right[0] - left[0]) / (2.0 * STEP) - 1.0,
                (up[0] - down[0]) / (2.0 * STEP),
                (right[1] - left[1]) / (2.0 * STEP),
                (up[1] - down[1]) / (2.0 * STEP) - 1.0,
            ];
            let strength = gradient
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            ensure!(
                strength < 0.8,
                "Alignment bends too sharply in one area; try the targets again"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring_points() -> Vec<[f64; 2]> {
        let mut points = vec![[0.0, 0.0]];
        for [rx, ry] in [[0.25, 0.12], [0.5, 0.24]] {
            for step in 0..8 {
                let angle = step as f64 * std::f64::consts::FRAC_PI_4;
                points.push([rx * angle.cos(), ry * angle.sin()]);
            }
        }
        points
    }

    #[test]
    fn smooth_offsets_cover_points_between_the_rings() {
        let targets = ring_points()
            .into_iter()
            .map(|expected| {
                let [x, y] = expected;
                Target {
                    observed: [x - 0.03 - 0.02 * x * x, y - 0.025 - 0.015 * y * y],
                    expected,
                }
            })
            .collect::<Vec<_>>();
        let profile = fit(&targets).unwrap();
        assert_eq!(profile.version, 3);
        let center = profile.corrected([-0.03, -0.025]);
        assert!(center[0].abs() < 0.01 && center[1].abs() < 0.01);
        let wanted = [0.34, -0.11];
        let corrected = profile.corrected([
            wanted[0] - 0.03 - 0.02 * wanted[0] * wanted[0],
            wanted[1] - 0.025 - 0.015 * wanted[1] * wanted[1],
        ]);
        assert!((corrected[0] - wanted[0]).abs() < 0.015);
        assert!((corrected[1] - wanted[1]).abs() < 0.015);
        let distant = profile.corrected([2.0, -2.0]);
        assert!((distant[0] - 2.0).abs() <= 0.18);
        assert!((distant[1] + 2.0).abs() <= 0.18);
    }

    #[test]
    fn rejects_center_captured_before_the_user_looked_at_it() {
        let targets = ring_points()
            .into_iter()
            .enumerate()
            .map(|(index, expected)| Target {
                observed: if index == 0 {
                    [0.12, -0.10]
                } else {
                    [expected[0] - 0.02, expected[1] - 0.03]
                },
                expected,
            })
            .collect::<Vec<_>>();
        assert!(fit(&targets).is_err());
    }

    #[test]
    fn refuses_incoherent_targets() {
        let targets = [Target {
            observed: [0.0, 0.0],
            expected: [0.0, 0.0],
        }; TARGET_COUNT];
        assert!(fit(&targets).is_err());
    }
}
