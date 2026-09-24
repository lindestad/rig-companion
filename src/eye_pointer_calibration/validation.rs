//! Independent validation of candidate, saved and raw gaze on identical samples.
use super::{Profile, angular_error, median};
use anyhow::{Result, ensure};

pub fn positions() -> Vec<[f64; 2]> {
    let mut points = (0..6)
        .map(|i| {
            let angle = std::f64::consts::FRAC_PI_8 + i as f64 * std::f64::consts::PI / 3.0;
            [0.375 * angle.cos(), 0.18 * angle.sin()]
        })
        .collect::<Vec<_>>();
    points.push([0.0, 0.0]);
    points
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub error_deg: f64,
    pub bias_deg: f64,
    pub spread_deg: f64,
}

#[derive(Debug)]
pub struct Comparison {
    pub candidate: Metrics,
    pub current: Option<Metrics>,
    pub raw: Metrics,
}

fn measure(expected: [f64; 2], samples: &[[f64; 2]], profile: Option<&Profile>) -> Metrics {
    let points = samples
        .iter()
        .map(|point| profile.map_or(*point, |p| p.corrected(*point)))
        .collect::<Vec<_>>();
    let center = median(&points);
    let mut spread = points
        .iter()
        .map(|point| angular_error(*point, center))
        .collect::<Vec<_>>();
    spread.sort_by(f64::total_cmp);
    Metrics {
        error_deg: points
            .iter()
            .map(|point| angular_error(*point, expected))
            .sum::<f64>()
            / points.len() as f64,
        bias_deg: angular_error(center, expected),
        spread_deg: spread[(spread.len() * 9 / 10).min(spread.len() - 1)],
    }
}

pub fn compare(
    expected: [f64; 2],
    samples: &[[f64; 2]],
    candidate: &Profile,
    current: Option<&Profile>,
) -> Result<Comparison> {
    ensure!(
        !samples.is_empty()
            && samples
                .iter()
                .flatten()
                .chain(expected.iter())
                .all(|v| v.is_finite()),
        "Invalid validation samples"
    );
    candidate.check()?;
    if let Some(profile) = current {
        profile.check()?;
    }
    Ok(Comparison {
        candidate: measure(expected, samples, Some(candidate)),
        current: current.map(|profile| measure(expected, samples, Some(profile))),
        raw: measure(expected, samples, None),
    })
}

pub struct Report {
    pub points: Vec<Comparison>,
    pub center_shift_deg: f64,
}

impl Report {
    pub fn summary(&self) -> String {
        let average = |values: Vec<f64>| values.iter().sum::<f64>() / values.len() as f64;
        let candidate = average(self.points.iter().map(|p| p.candidate.error_deg).collect());
        let raw = average(self.points.iter().map(|p| p.raw.error_deg).collect());
        let worst = self
            .points
            .iter()
            .map(|p| p.candidate.error_deg)
            .fold(0.0, f64::max);
        let current = if self.points.iter().all(|p| p.current.is_some()) {
            format!(
                "{:.2}°",
                average(
                    self.points
                        .iter()
                        .map(|p| p.current.unwrap().error_deg)
                        .collect()
                )
            )
        } else {
            "none".into()
        };
        format!(
            "Fresh validation mean error: new {candidate:.2}°, saved {current}, raw {raw:.2}°. Worst new target {worst:.2}°; center shift {:.2}°.",
            self.center_shift_deg
        )
    }

    pub fn accept(&self) -> Result<()> {
        ensure!(
            self.points.len() == positions().len(),
            "Validation is incomplete; previous alignment kept"
        );
        let summary = self.summary();
        ensure!(
            self.center_shift_deg.is_finite() && self.center_shift_deg <= 1.0,
            "Gaze shifted during alignment. Check headset fit and try again. Previous alignment kept. {summary}"
        );
        let average = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
        let candidate = self
            .points
            .iter()
            .map(|p| p.candidate.error_deg)
            .collect::<Vec<_>>();
        // Conservative usability/regression guards, not hardware accuracy claims.
        ensure!(
            candidate.iter().all(|v| v.is_finite() && *v <= 4.0) && average(&candidate) <= 2.5,
            "Alignment is still too far from the validation targets. Check headset fit and look at each center dot. Previous alignment kept. {summary}"
        );
        let mut baselines = vec![(
            "raw gaze",
            self.points
                .iter()
                .map(|p| p.raw.error_deg)
                .collect::<Vec<_>>(),
        )];
        if let Some(current) = self
            .points
            .iter()
            .map(|p| p.current.map(|m| m.error_deg))
            .collect::<Option<Vec<_>>>()
        {
            baselines.push(("saved alignment", current));
        }
        for (name, baseline) in baselines {
            ensure!(
                average(&candidate) <= average(&baseline) + 0.35f64.max(average(&baseline) * 0.15),
                "New alignment is worse than {name} on fresh targets. Previous alignment kept. {summary}"
            );
            if let Some(index) = candidate
                .iter()
                .zip(&baseline)
                .position(|(new, old)| *new > old + 0.8f64.max(old * 0.30))
            {
                anyhow::bail!(
                    "Validation target {} is worse than {name}. Previous alignment kept. {summary}",
                    index + 1
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(offset: [f64; 2]) -> Profile {
        Profile {
            version: 3,
            observed: [[0.0; 2]; 17],
            offsets: [offset; 17],
            bounds: [-0.5, 0.5, -0.24, 0.24],
            scale: [0.5, 0.24],
            bandwidth: 0.32,
        }
    }

    fn report(candidate: &Profile, current: Option<&Profile>) -> Report {
        let points = positions()
            .into_iter()
            .map(|expected| {
                let samples = (0..50)
                    .map(|i| {
                        [
                            expected[0] - 0.03 + if i % 2 == 0 { 0.002 } else { -0.002 },
                            expected[1],
                        ]
                    })
                    .collect::<Vec<_>>();
                compare(expected, &samples, candidate, current).unwrap()
            })
            .collect();
        Report {
            points,
            center_shift_deg: 0.0,
        }
    }

    #[test]
    fn improved_profile_passes_on_identical_fresh_samples() {
        let report = report(&profile([0.03, 0.0]), Some(&profile([0.01, 0.0])));
        assert!(
            report
                .points
                .iter()
                .all(|p| p.candidate.error_deg < p.current.unwrap().error_deg
                    && p.current.unwrap().error_deg < p.raw.error_deg)
        );
        assert!(report.accept().is_ok());
    }

    #[test]
    fn rejects_regression_against_saved_alignment_even_when_better_than_raw() {
        let report = report(&profile([0.015, 0.0]), Some(&profile([0.03, 0.0])));
        assert!(
            report
                .points
                .iter()
                .all(|p| p.candidate.error_deg < p.raw.error_deg)
        );
        assert!(
            report
                .accept()
                .unwrap_err()
                .to_string()
                .contains("saved alignment")
        );
    }

    #[test]
    fn rejects_regression_against_raw_without_saved_profile() {
        assert!(report(&profile([-0.01, 0.0]), None).accept().is_err());
    }

    #[test]
    fn allows_small_variation_but_catches_local_regression() {
        assert!(
            report(&profile([0.028, 0.0]), Some(&profile([0.03, 0.0])))
                .accept()
                .is_ok()
        );
        let mut report = report(&profile([0.03, 0.0]), Some(&profile([0.03, 0.0])));
        report.points[0].candidate.error_deg = 1.2;
        assert!(
            report
                .accept()
                .unwrap_err()
                .to_string()
                .contains("target 1")
        );
    }

    #[test]
    fn rejects_session_drift_and_incomplete_validation() {
        let mut report = report(&profile([0.03, 0.0]), None);
        report.center_shift_deg = 1.1;
        assert!(report.accept().is_err());
        report.center_shift_deg = 0.0;
        report.points.pop();
        assert!(report.accept().is_err());
    }

    #[test]
    fn separates_bias_from_scatter_and_checks_profile() {
        let expected = [0.0, 0.0];
        let points = [[-0.01, 0.0], [0.01, 0.0]];
        let comparison = compare(expected, &points, &profile([0.0, 0.0]), None).unwrap();
        assert!(comparison.raw.bias_deg < 1e-6);
        assert!(comparison.raw.error_deg > 0.5 && comparison.raw.spread_deg > 0.5);
        let mut bad = profile([0.0, 0.0]);
        bad.scale[0] = 0.0;
        assert!(compare(expected, &points, &bad, None).is_err());
    }
}
