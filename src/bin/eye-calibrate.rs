use anyhow::{Context, Result, ensure};
use rig_companion::{
    eye_pointer_calibration::{
        self, Profile, Target, angular_error,
        capture::{CaptureWindow, Fixation},
        validation::{self, Report},
    },
    steamvr::{EyeCalibrationOverlay, SteamVr},
};
use std::{
    fs,
    io::Write,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

fn targets() -> Vec<[f64; 2]> {
    let mut points = vec![[0.0, 0.0]];
    for [horizontal, vertical] in [[0.25, 0.12], [0.50, 0.24]] {
        for step in 0..8 {
            let angle = step as f64 * std::f64::consts::FRAC_PI_4;
            points.push([horizontal * angle.cos(), vertical * angle.sin()]);
        }
    }
    points
}

fn main() -> Result<()> {
    let preview = std::env::args().any(|argument| argument == "--preview");
    println!("Eye pointer calibration: wear the headset and look at each purple dot.");
    println!(
        "Seventeen alignment targets, then seven checks, take about a minute. Press Escape to cancel."
    );
    let vr = SteamVr::connect_overlay()?;
    let target_positions = targets();
    if preview {
        let overlay = vr.eye_calibration_overlay()?;
        overlay.show_target(&target_positions, 0, false)?;
        thread::sleep(Duration::from_secs(6));
        return Ok(());
    }
    let path = eye_pointer_calibration::profile_path()?;
    let previous = read_profile(&path)?;
    let current = previous
        .as_deref()
        .map(|bytes| -> Result<Profile> {
            let profile: Profile = serde_json::from_slice(bytes)?;
            profile.check()?;
            Ok(profile)
        })
        .transpose()
        .context("Cannot compare the saved pointer alignment; it was kept unchanged")?;
    ensure!(
        vr.headset_gaze_sample()?.is_some(),
        "No fresh gaze. Restore eye tracking before calibrating."
    );
    if vr.close_dashboard_if_visible()? {
        println!("SteamVR dashboard closed for alignment.");
    }
    println!("First target appears in five seconds.");
    wait_or_cancel(Duration::from_secs(5))?;
    let overlay = vr.eye_calibration_overlay()?;
    let mut targets = Vec::new();
    for (index, expected) in target_positions.iter().copied().enumerate() {
        overlay.show_target(&target_positions, index, false)?;
        println!("Target {} of {}", index + 1, target_positions.len());
        let fixation = collect(
            &vr,
            &overlay,
            &target_positions,
            index,
            if index == 0 { 2000 } else { 650 },
            true,
        )?;
        targets.push(Target {
            observed: fixation.center,
            expected,
        });
    }
    let profile = eye_pointer_calibration::fit(&targets).context("Previous alignment kept")?;
    let checks = validation::positions();
    let mut report = Report {
        points: Vec::new(),
        center_shift_deg: 0.0,
    };
    println!(
        "Seven fresh checks follow. Keep looking at each center dot; the new alignment is not saved yet."
    );
    for (index, expected) in checks.iter().copied().enumerate() {
        overlay.show_target(&checks, index, false)?;
        println!("Validation {} of {}", index + 1, checks.len());
        let fixation = collect(&vr, &overlay, &checks, index, 650, false)?;
        let comparison =
            validation::compare(expected, &fixation.samples, &profile, current.as_ref())?;
        let describe = |m: validation::Metrics| {
            format!(
                "error {:.2}°, bias {:.2}°, spread (90%) {:.2}°",
                m.error_deg, m.bias_deg, m.spread_deg
            )
        };
        println!(
            "Check {}: new {}; saved {}; raw {}",
            index + 1,
            describe(comparison.candidate),
            comparison
                .current
                .map(describe)
                .unwrap_or_else(|| "none".into()),
            describe(comparison.raw)
        );
        if index == checks.len() - 1 {
            report.center_shift_deg = angular_error(targets[0].observed, fixation.center);
        }
        report.points.push(comparison);
    }
    drop(overlay);
    // All acceptance checks precede any profile write or driver reload.
    report.accept()?;
    save_profile(&path, previous.as_deref(), &profile, || {
        vr.reload_gaze_pointer_calibration()
    })?;
    // The GUI displays this final line; detailed per-target diagnostics stay on stdout.
    println!(
        "Pointer alignment saved. {} Game eye tracking is unchanged.",
        report.summary()
    );
    Ok(())
}

fn read_profile(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("Cannot read the saved alignment; it was kept unchanged"),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing profile folder")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn save_profile(
    path: &Path,
    previous: Option<&[u8]>,
    profile: &Profile,
    reload: impl FnOnce() -> Result<()>,
) -> Result<()> {
    ensure!(
        read_profile(path)?.as_deref() == previous,
        "Saved alignment changed while calibrating. It was kept; start again."
    );
    atomic_write(path, &serde_json::to_vec_pretty(profile)?)?;
    if let Err(error) = reload() {
        let restored = match previous {
            Some(bytes) => atomic_write(path, bytes),
            None => fs::remove_file(path).map_err(Into::into),
        };
        restored.context("Driver reload failed and the previous profile could not be restored")?;
        return Err(
            error.context("Driver reload failed; the previous profile was restored on disk")
        );
    }
    Ok(())
}

fn wait_or_cancel(duration: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < duration {
        // SAFETY: read-only keyboard state; no keyboard hooks are installed.
        if unsafe { GetAsyncKeyState(VK_ESCAPE.into()) < 0 } {
            anyhow::bail!("Calibration cancelled. Previous profile was kept.");
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn collect(
    vr: &SteamVr,
    overlay: &EyeCalibrationOverlay<'_>,
    positions: &[[f64; 2]],
    index: usize,
    settle_ms: u64,
    acquisition_gate: bool,
) -> Result<Fixation> {
    // Retry this target once, rather than throwing away the entire run for one blink.
    for attempt in 0..2 {
        wait_or_cancel(Duration::from_millis(if attempt == 0 {
            settle_ms
        } else {
            650
        }))?;
        let started = Instant::now();
        let mut capture = CaptureWindow::new(acquisition_gate.then_some(positions[index]));
        let mut focused = false;
        let mut last_sequence = None;
        let mut polls = 0;
        let mut fresh = 0;
        while started.elapsed() < Duration::from_secs(8) {
            // SAFETY: read-only keyboard state; no keyboard hooks are installed.
            if unsafe { GetAsyncKeyState(VK_ESCAPE.into()) < 0 } {
                anyhow::bail!("Calibration cancelled. Previous profile was kept.");
            }
            let reading = vr.headset_gaze_sample()?;
            polls += 1;
            if let Some((sequence, _, _)) = reading
                && last_sequence != Some(sequence)
            {
                fresh += 1;
                last_sequence = Some(sequence);
            }
            let fixation = capture.poll(started.elapsed(), reading);
            if capture.focused() != focused {
                focused = capture.focused();
                overlay.show_target(positions, index, focused)?;
                capture.discard_window();
            } else if let Some(fixation) = fixation {
                println!(
                    "Captured {} stable samples; {fresh}/{polls} polls had new gaze.",
                    fixation.samples.len()
                );
                return Ok(fixation);
            }
            thread::sleep(Duration::from_millis(25));
        }
        ensure!(
            fresh > 0,
            "No fresh gaze on target {}. Restore eye tracking and try again. Previous alignment kept.",
            index + 1
        );
        if attempt == 0 {
            println!(
                "Retrying target {}: keep looking at its small center dot.",
                index + 1
            );
            overlay.show_target(positions, index, false)?;
        }
    }
    anyhow::bail!(
        "Gaze did not stay steady on target {} after a retry. Check headset fit and look at the small center dot. Previous alignment kept.",
        index + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_reload_restores_exact_previous_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profile.json");
        let previous = b"previous profile bytes";
        fs::write(&path, previous).unwrap();
        let profile = test_profile();
        assert!(
            save_profile(&path, Some(previous), &profile, || anyhow::bail!(
                "reload failed"
            ))
            .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), previous);
    }

    fn test_profile() -> Profile {
        let points = targets()
            .into_iter()
            .map(|expected| Target {
                observed: expected,
                expected,
            })
            .collect::<Vec<_>>();
        eye_pointer_calibration::fit(&points).unwrap()
    }

    #[test]
    fn failed_first_reload_leaves_no_profile_and_concurrent_change_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profile.json");
        assert!(
            save_profile(&path, None, &test_profile(), || anyhow::bail!(
                "reload failed"
            ))
            .is_err()
        );
        assert!(!path.exists());
        fs::write(&path, b"changed elsewhere").unwrap();
        assert!(save_profile(&path, None, &test_profile(), || panic!("must not reload")).is_err());
        assert_eq!(fs::read(path).unwrap(), b"changed elsewhere");
    }
}
