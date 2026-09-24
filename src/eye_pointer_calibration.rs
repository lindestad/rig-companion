//! A small spatial correction for the dashboard pointer. Raw game gaze is unchanged.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub observed: [f64; 2],
    pub expected: [f64; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub version: u8,
    pub offset_x: [f64; 10],
    pub offset_y: [f64; 10],
    /// Observed gaze bounds used to prevent polynomial extrapolation.
    pub bounds: [f64; 4],
    /// Tangent-space target radii used to keep the fit well-conditioned.
    pub scale: [f64; 2],
}

fn basis(gaze: [f64; 2], scale: [f64; 2]) -> [f64; 10] {
    let x = gaze[0] / scale[0];
    let y = gaze[1] / scale[1];
    [
        1.0,
        x,
        y,
        x * x,
        x * y,
        y * y,
        x * x * x,
        x * x * y,
        x * y * y,
        y * y * y,
    ]
}

impl Profile {
    pub fn corrected(&self, gaze: [f64; 2]) -> [f64; 2] {
        let x = gaze[0].clamp(self.bounds[0], self.bounds[1]);
        let y = gaze[1].clamp(self.bounds[2], self.bounds[3]);
        let terms = basis([x, y], self.scale);
        let correction = |coefficients: &[f64; 10]| {
            coefficients
                .iter()
                .zip(terms)
                .map(|(coefficient, term)| coefficient * term)
                .sum::<f64>()
                .clamp(-0.25, 0.25)
        };
        [
            gaze[0] + correction(&self.offset_x),
            gaze[1] + correction(&self.offset_y),
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
        targets.len() >= 17,
        "Collect the center and two eight-point rings"
    );
    let mut normal = [[0.0; 10]; 10];
    let mut rhs_x = [0.0; 10];
    let mut rhs_y = [0.0; 10];
    let mut bounds = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut scale = [0.0f64; 2];
    for target in targets {
        scale[0] = scale[0].max(target.expected[0].abs());
        scale[1] = scale[1].max(target.expected[1].abs());
    }
    ensure!(
        scale[0].is_finite()
            && scale[1].is_finite()
            && (0.2..=0.8).contains(&scale[0])
            && (0.15..=0.8).contains(&scale[1]),
        "Calibration targets do not cover the expected view"
    );
    for target in targets {
        let [x, y] = target.observed;
        let [wanted_x, wanted_y] = target.expected;
        ensure!(
            x.is_finite() && y.is_finite() && wanted_x.is_finite() && wanted_y.is_finite(),
            "Invalid calibration point"
        );
        ensure!(
            x.abs() < 1.0 && y.abs() < 1.0,
            "Gaze point outside calibration range"
        );
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].max(x);
        bounds[2] = bounds[2].min(y);
        bounds[3] = bounds[3].max(y);
        let terms = basis([x, y], scale);
        for i in 0..10 {
            rhs_x[i] += terms[i] * (wanted_x - x);
            rhs_y[i] += terms[i] * (wanted_y - y);
            for j in 0..10 {
                normal[i][j] += terms[i] * terms[j];
            }
        }
    }
    ensure!(
        bounds[1] - bounds[0] > 0.40 && bounds[3] - bounds[2] > 0.30,
        "Calibration targets did not span enough of the view"
    );
    // Favor an offset/scale correction; higher-order terms add smooth local variation.
    for (i, penalty) in [
        0.0001, 0.001, 0.001, 0.02, 0.02, 0.02, 0.08, 0.08, 0.08, 0.08,
    ]
    .into_iter()
    .enumerate()
    {
        normal[i][i] += penalty;
    }
    let profile = Profile {
        version: 2,
        offset_x: solve(normal, rhs_x)?,
        offset_y: solve(normal, rhs_y)?,
        bounds,
        scale,
    };
    let mut squared_error = 0.0;
    let mut largest_error = 0.0f64;
    for target in targets {
        let corrected = profile.corrected(target.observed);
        let error = ((corrected[0] - target.expected[0]).powi(2)
            + (corrected[1] - target.expected[1]).powi(2))
        .sqrt();
        squared_error += error * error;
        largest_error = largest_error.max(error);
    }
    let rms = (squared_error / targets.len() as f64).sqrt();
    ensure!(
        rms < 0.055,
        "Calibration fit is inconsistent (about {:.1}° error); try again",
        rms.to_degrees()
    );
    ensure!(
        largest_error < 0.075,
        "One target disagreed with the others; keep looking at the small center dot and try again"
    );
    ensure!(
        profile
            .offset_x
            .iter()
            .chain(profile.offset_y.iter())
            .all(|v| v.is_finite() && v.abs() <= 2.0),
        "Calibration fit is unstable"
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
            let correction_gradient = [
                (right[0] - left[0]) / (2.0 * STEP) - 1.0,
                (up[0] - down[0]) / (2.0 * STEP),
                (right[1] - left[1]) / (2.0 * STEP),
                (up[1] - down[1]) / (2.0 * STEP) - 1.0,
            ];
            let strength = correction_gradient
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

fn solve<const N: usize>(mut matrix: [[f64; N]; N], mut rhs: [f64; N]) -> Result<[f64; N]> {
    for column in 0..N {
        let pivot = (column..N)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))
            .unwrap();
        ensure!(
            matrix[pivot][column].abs() > 1e-10,
            "Calibration points are degenerate"
        );
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let scale = matrix[column][column];
        for item in matrix[column].iter_mut().skip(column) {
            *item /= scale;
        }
        rhs[column] /= scale;
        let pivot_row = matrix[column];
        for (row, values) in matrix.iter_mut().enumerate() {
            if row == column {
                continue;
            }
            let factor = values[column];
            for (value, pivot_value) in values.iter_mut().zip(pivot_row).skip(column) {
                *value -= factor * pivot_value;
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    Ok(rhs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_smooth_offset_and_scale_without_large_extrapolation() {
        let mut points = vec![[0.0, 0.0]];
        for [rx, ry] in [[0.2, 0.15], [0.4, 0.3]] {
            for [dx, dy] in [
                [-1.0, 1.0],
                [0.0, 1.0],
                [1.0, 1.0],
                [1.0, 0.0],
                [1.0, -1.0],
                [0.0, -1.0],
                [-1.0, -1.0],
                [-1.0, 0.0],
            ] {
                points.push([dx * rx, dy * ry]);
            }
        }
        let observed = |[x, y]: [f64; 2]| {
            [
                x - 0.04 + 0.03 * x + 0.06 * x * x * x,
                y - 0.025 + 0.04 * y * y * y,
            ]
        };
        let targets = points
            .into_iter()
            .map(|expected| Target {
                observed: observed(expected),
                expected,
            })
            .collect::<Vec<_>>();
        let profile = fit(&targets).unwrap();
        assert_eq!(profile.version, 2);
        let actual = profile.corrected(observed([0.0, 0.0]));
        assert!(actual[0].abs() < 0.005 && actual[1].abs() < 0.005);
        let holdout = profile.corrected(observed([0.29, -0.12]));
        assert!((holdout[0] - 0.29).abs() < 0.008 && (holdout[1] + 0.12).abs() < 0.008);
        let distant = profile.corrected([2.0, -2.0]);
        assert!((distant[0] - 2.0).abs() <= 0.25 && (distant[1] + 2.0).abs() <= 0.25);
    }

    #[test]
    fn refuses_incoherent_targets() {
        let targets = (0..9)
            .map(|_| Target {
                observed: [0.0, 0.0],
                expected: [0.0, 0.0],
            })
            .collect::<Vec<_>>();
        assert!(fit(&targets).is_err());
    }

    #[test]
    fn rejects_center_captured_before_the_user_looked_at_it() {
        let mut targets = vec![Target {
            observed: [0.12, -0.10],
            expected: [0.0, 0.0],
        }];
        for [rx, ry] in [[0.25, 0.15], [0.5, 0.3]] {
            for [dx, dy] in [
                [-1.0, 1.0],
                [0.0, 1.0],
                [1.0, 1.0],
                [1.0, 0.0],
                [1.0, -1.0],
                [0.0, -1.0],
                [-1.0, -1.0],
                [-1.0, 0.0],
            ] {
                let expected = [dx * rx, dy * ry];
                targets.push(Target {
                    observed: [expected[0] - 0.02, expected[1] - 0.03],
                    expected,
                });
            }
        }
        let error = fit(&targets).unwrap_err().to_string();
        assert!(
            error.contains("One target")
                || error.contains("bends too sharply")
                || error.contains("offset is too large"),
            "{error}"
        );
    }
}
