//! Time-coherent fixation capture. No SteamVR calls, filtering, or profile writes.
use super::{angular_error, median};
use anyhow::{Result, ensure};
use std::{collections::VecDeque, time::Duration};

const MIN_SAMPLES: usize = 35;
const WINDOW: Duration = Duration::from_millis(1200);
const MAX_WINDOW: Duration = Duration::from_millis(1500);
const MAX_GAP: Duration = Duration::from_millis(150);
// Practical starting limits, not claimed Dream Air hardware specifications.
const MAX_SPREAD_DEG: f64 = 1.5;
const MAX_DRIFT_DEG: f64 = 0.6;
const EXCURSION_DEG: f64 = 3.0;

#[derive(Debug)]
pub struct Fixation {
    pub center: [f64; 2],
    pub samples: Vec<[f64; 2]>,
}

pub struct CaptureWindow {
    // Validation has no target-proximity gate: an inaccurate fixation must be measured.
    expected: Option<[f64; 2]>,
    last_sequence: Option<u64>,
    last_fresh: Option<Duration>,
    near_count: usize,
    samples: VecDeque<(Duration, [f64; 2])>,
}

impl CaptureWindow {
    pub fn new(expected: Option<[f64; 2]>) -> Self {
        Self {
            expected,
            last_sequence: None,
            last_fresh: None,
            near_count: 0,
            samples: VecDeque::new(),
        }
    }

    pub fn focused(&self) -> bool {
        self.near_count >= 5
    }

    /// Rendering a new focus ring can take time. Start a new window afterward,
    /// preserving the feedback state but never bridging the render interval.
    pub fn discard_window(&mut self) {
        self.samples.clear();
        self.last_fresh = None;
    }

    fn reacquire(&mut self) {
        self.samples.clear();
        self.near_count = 0;
    }

    pub fn poll(&mut self, now: Duration, reading: Option<(u64, f64, f64)>) -> Option<Fixation> {
        if self
            .last_fresh
            .is_some_and(|last| now.saturating_sub(last) > MAX_GAP)
        {
            self.reacquire();
        }
        let (sequence, x, y) = reading?;
        if self.last_sequence == Some(sequence) {
            return None;
        }
        self.last_sequence = Some(sequence);
        self.last_fresh = Some(now);
        if !x.is_finite() || !y.is_finite() {
            self.reacquire();
            return None;
        }
        // Only a coarse acquisition check, deliberately wider than calibration error.
        if self.expected.is_some_and(|expected| {
            ((x - expected[0]) / 0.16).powi(2) + ((y - expected[1]) / 0.13).powi(2) >= 1.0
        }) {
            self.reacquire();
            return None;
        }
        if self.samples.len() >= 5 {
            let recent = self
                .samples
                .iter()
                .rev()
                .take(9)
                .map(|(_, point)| *point)
                .collect::<Vec<_>>();
            if angular_error([x, y], median(&recent)) > EXCURSION_DEG {
                self.reacquire();
            }
        }
        self.near_count += 1;
        if !self.focused() {
            return None;
        }
        self.samples.push_back((now, [x, y]));
        while self
            .samples
            .front()
            .is_some_and(|(time, _)| now.saturating_sub(*time) > MAX_WINDOW)
        {
            self.samples.pop_front();
        }
        if self.samples.len() < MIN_SAMPLES || now.saturating_sub(self.samples.front()?.0) < WINDOW
        {
            return None;
        }
        let samples = self
            .samples
            .iter()
            .map(|(_, point)| *point)
            .collect::<Vec<_>>();
        let center = stable_median(&samples).ok()?;
        Some(Fixation { center, samples })
    }
}

fn stable_median(samples: &[[f64; 2]]) -> Result<[f64; 2]> {
    ensure!(
        samples.len() >= MIN_SAMPLES,
        "Not enough fresh gaze samples"
    );
    let center = median(samples);
    let mut distances = samples
        .iter()
        .map(|point| angular_error(*point, center))
        .collect::<Vec<_>>();
    distances.sort_by(f64::total_cmp);
    ensure!(
        distances[(distances.len() * 9 / 10).min(distances.len() - 1)] <= MAX_SPREAD_DEG,
        "Gaze is too spread out; keep looking at the small center dot"
    );
    let third = samples.len() / 3;
    ensure!(
        angular_error(
            median(&samples[..third]),
            median(&samples[samples.len() - third..])
        ) <= MAX_DRIFT_DEG,
        "Gaze moved during capture; hold your gaze on the small center dot"
    );
    Ok(center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_slow_sweep_even_inside_the_old_stability_box() {
        let samples = (0..57)
            .map(|i| [-0.03 + 0.06 * i as f64 / 56.0, 0.0])
            .collect::<Vec<_>>();
        assert!(stable_median(&samples).is_err());
    }

    #[test]
    fn captures_biased_but_steady_gaze() {
        let mut capture = CaptureWindow::new(Some([0.0, 0.0]));
        let result = (0..70)
            .find_map(|i| capture.poll(Duration::from_millis(i * 25), Some((i, 0.07, -0.03))));
        assert_eq!(result.unwrap().center, [0.07, -0.03]);
    }

    #[test]
    fn missing_or_duplicate_samples_cannot_finish_an_old_window() {
        for duplicate in [false, true] {
            let mut capture = CaptureWindow::new(None);
            for i in 0..40 {
                assert!(
                    capture
                        .poll(Duration::from_millis(i * 25), Some((i, 0.0, 0.0)))
                        .is_none()
                );
            }
            let reading = duplicate.then_some((39, 0.0, 0.0));
            assert!(capture.poll(Duration::from_millis(1600), reading).is_none());
            assert!(!capture.focused());
            assert!(
                capture
                    .poll(Duration::from_millis(1625), Some((40, 0.0, 0.0)))
                    .is_none()
            );
        }
    }

    #[test]
    fn excursion_requires_a_new_window_then_recovers() {
        let mut capture = CaptureWindow::new(None);
        for i in 0..40 {
            assert!(
                capture
                    .poll(Duration::from_millis(i * 25), Some((i, 0.0, 0.0)))
                    .is_none()
            );
        }
        assert!(
            capture
                .poll(Duration::from_millis(1000), Some((40, 0.1, 0.0)))
                .is_none()
        );
        for i in 41..85 {
            assert!(
                capture
                    .poll(Duration::from_millis(i * 25), Some((i, 0.1, 0.0)))
                    .is_none()
            );
        }
        assert!((85..110).any(|i| {
            capture
                .poll(Duration::from_millis(i * 25), Some((i, 0.1, 0.0)))
                .is_some()
        }));
    }

    #[test]
    fn validation_does_not_censor_large_target_error() {
        let mut capture = CaptureWindow::new(None);
        let result =
            (0..70).find_map(|i| capture.poll(Duration::from_millis(i * 25), Some((i, 0.3, 0.0))));
        assert_eq!(result.unwrap().center, [0.3, 0.0]);
    }

    #[test]
    fn rendering_delay_is_not_counted_as_fixation_time() {
        let mut capture = CaptureWindow::new(None);
        for i in 0..5 {
            capture.poll(Duration::from_millis(i * 25), Some((i, 0.0, 0.0)));
        }
        assert!(capture.focused());
        capture.discard_window();
        assert!(
            capture
                .poll(Duration::from_millis(2000), Some((5, 0.0, 0.0)))
                .is_none()
        );
        assert!(capture.focused());
    }
}
