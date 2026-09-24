use anyhow::{Context, Result, ensure};
use rig_companion::{
    eye_pointer_calibration::{self, Target},
    steamvr::SteamVr,
};
use std::{
    fs, thread,
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
    println!("Seventeen targets take about a minute. Press Escape to cancel.");
    let vr = SteamVr::connect_overlay()?;
    let target_positions = targets();
    if preview {
        let overlay = vr.eye_calibration_overlay()?;
        overlay.show_target(&target_positions, 0)?;
        thread::sleep(Duration::from_secs(6));
        return Ok(());
    }
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
        overlay.show_target(&target_positions, index)?;
        println!("Target {} of {}", index + 1, target_positions.len());
        let observed = collect(&vr, if index == 0 { 2000 } else { 650 })?;
        targets.push(Target { observed, expected });
    }
    drop(overlay);
    let profile = eye_pointer_calibration::fit(&targets)?;
    let path = eye_pointer_calibration::profile_path()?;
    let previous = fs::read(&path).ok();
    fs::create_dir_all(path.parent().context("Missing profile folder")?)?;
    fs::write(&path, serde_json::to_vec_pretty(&profile)?)?;
    if let Err(error) = vr.reload_gaze_pointer_calibration() {
        match previous {
            Some(bytes) => {
                let _ = fs::write(&path, bytes);
            }
            None => {
                let _ = fs::remove_file(&path);
            }
        }
        return Err(error.context("The previous pointer calibration was restored"));
    }
    println!("Dashboard eye pointer calibrated. Game eye tracking is unchanged.");
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

fn collect(vr: &SteamVr, settle_ms: u64) -> Result<[f64; 2]> {
    // Let the eye settle on the newly drawn target before sampling.
    wait_or_cancel(Duration::from_millis(settle_ms))?;
    let started = Instant::now();
    let mut last_sequence = None;
    let mut samples = Vec::new();
    while started.elapsed() < Duration::from_secs(8) {
        // SAFETY: read-only keyboard state; no keyboard hooks are installed.
        if unsafe { GetAsyncKeyState(VK_ESCAPE.into()) < 0 } {
            anyhow::bail!("Calibration cancelled. Previous profile was kept.");
        }
        if let Some((sequence, x, y)) = vr.headset_gaze_sample()?
            && last_sequence != Some(sequence)
        {
            last_sequence = Some(sequence);
            samples.push([x, y]);
        }
        if samples.len() >= 35 && started.elapsed() >= Duration::from_millis(1400) {
            if let Ok(center) = stable_median(&samples) {
                return Ok(center);
            }
            samples.drain(..20);
        }
        thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("Gaze was missing or unstable at a target. Previous profile was kept.")
}

fn stable_median(samples: &[[f64; 2]]) -> Result<[f64; 2]> {
    ensure!(samples.len() >= 25, "Not enough gaze samples");
    let mut xs = samples.iter().map(|sample| sample[0]).collect::<Vec<_>>();
    let mut ys = samples.iter().map(|sample| sample[1]).collect::<Vec<_>>();
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    let center = [xs[xs.len() / 2], ys[ys.len() / 2]];
    let stable = samples
        .iter()
        .filter(|sample| {
            (sample[0] - center[0]).abs() < 0.075 && (sample[1] - center[1]).abs() < 0.075
        })
        .count();
    ensure!(
        stable >= samples.len() * 3 / 4,
        "Eye movement was too large for one target; try again"
    );
    Ok(center)
}
