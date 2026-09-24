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
    pub offset_x: [f64; 6],
    pub offset_y: [f64; 6],
    /// Observed gaze bounds used to prevent polynomial extrapolation.
    pub bounds: [f64; 4],
}

impl Profile {
    pub fn corrected(&self, gaze: [f64; 2]) -> [f64; 2] {
        let x = gaze[0].clamp(self.bounds[0], self.bounds[1]);
        let y = gaze[1].clamp(self.bounds[2], self.bounds[3]);
        let basis = [1.0, x, y, x * x, x * y, y * y];
        let correction = |coefficients: &[f64; 6]| {
            coefficients
                .iter()
                .zip(basis)
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
        targets.len() >= 9,
        "Collect at least nine calibration targets"
    );
    let mut normal = [[0.0; 6]; 6];
    let mut rhs_x = [0.0; 6];
    let mut rhs_y = [0.0; 6];
    let mut bounds = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
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
        let basis = [1.0, x, y, x * x, x * y, y * y];
        for i in 0..6 {
            rhs_x[i] += basis[i] * (wanted_x - x);
            rhs_y[i] += basis[i] * (wanted_y - y);
            for j in 0..6 {
                normal[i][j] += basis[i] * basis[j];
            }
        }
    }
    ensure!(
        bounds[1] - bounds[0] > 0.20 && bounds[3] - bounds[2] > 0.15,
        "Calibration targets did not span enough of the view"
    );
    // Favor an offset/scale correction; curvature is available but strongly regularized.
    for (i, penalty) in [0.0001, 0.002, 0.002, 0.03, 0.03, 0.03]
        .into_iter()
        .enumerate()
    {
        normal[i][i] += penalty;
    }
    let profile = Profile {
        version: 1,
        offset_x: solve(normal, rhs_x)?,
        offset_y: solve(normal, rhs_y)?,
        bounds,
    };
    let mut squared_error = 0.0;
    for target in targets {
        let corrected = profile.corrected(target.observed);
        squared_error += (corrected[0] - target.expected[0]).powi(2)
            + (corrected[1] - target.expected[1]).powi(2);
    }
    let rms = (squared_error / targets.len() as f64).sqrt();
    ensure!(
        rms < 0.055,
        "Calibration fit is inconsistent (about {:.1}° error); try again",
        rms.to_degrees()
    );
    ensure!(
        profile
            .offset_x
            .iter()
            .chain(profile.offset_y.iter())
            .all(|v| v.is_finite() && v.abs() <= 2.0),
        "Calibration fit is unstable"
    );
    Ok(profile)
}

fn solve(mut matrix: [[f64; 6]; 6], mut rhs: [f64; 6]) -> Result<[f64; 6]> {
    for column in 0..6 {
        let pivot = (column..6)
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
        let mut targets = Vec::new();
        for y in [-0.18, 0.0, 0.18] {
            for x in [-0.25, 0.0, 0.25] {
                targets.push(Target {
                    observed: [x - 0.04 + 0.03 * x, y - 0.025],
                    expected: [x, y],
                });
            }
        }
        let profile = fit(&targets).unwrap();
        let actual = profile.corrected([-0.04, -0.025]);
        assert!(actual[0].abs() < 0.005 && actual[1].abs() < 0.005);
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
}
