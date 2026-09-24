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

const TARGETS: [[f64; 2]; 17] = [
    [0.0, 0.0],
    [-0.20, 0.15],
    [0.0, 0.15],
    [0.20, 0.15],
    [0.20, 0.0],
    [0.20, -0.15],
    [0.0, -0.15],
    [-0.20, -0.15],
    [-0.20, 0.0],
    [-0.40, 0.30],
    [0.0, 0.30],
    [0.40, 0.30],
    [0.40, 0.0],
    [0.40, -0.30],
    [0.0, -0.30],
    [-0.40, -0.30],
    [-0.40, 0.0],
];

fn main() -> Result<()> {
    let preview = std::env::args().any(|argument| argument == "--preview");
    println!("Eye pointer calibration: wear the headset and look at each purple dot.");
    println!("Seventeen targets take about a minute. Press Escape to cancel.");
    let vr = SteamVr::connect_overlay()?;
    let overlay = vr.eye_calibration_overlay()?;
    if preview {
        overlay.show_target([0.0, 0.0], 0, TARGETS.len())?;
        thread::sleep(Duration::from_secs(6));
        return Ok(());
    }
    ensure!(
        vr.headset_gaze_sample()?.is_some(),
        "No fresh gaze. Restore eye tracking before calibrating."
    );
    let mut targets = Vec::new();
    for (index, expected) in TARGETS.into_iter().enumerate() {
        overlay.show_target(expected, index, TARGETS.len())?;
        println!("Target {} of {}", index + 1, TARGETS.len());
        let observed = collect(&vr)?;
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

fn collect(vr: &SteamVr) -> Result<[f64; 2]> {
    // Let the eye settle on the newly drawn target before sampling.
    thread::sleep(Duration::from_millis(650));
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
