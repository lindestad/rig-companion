//! Isolate vendor discovery from the UI and the SteamVR worker.
use anyhow::{Context, Result, bail};
use std::{
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn inspect() -> Result<serde_json::Value> {
    let exe = std::env::current_exe()?.with_file_name("eye-probe.exe");
    let mut child = Command::new(exe)
        .creation_flags(0x08000000)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Build the calibration helper with cargo build --release --bins")?;
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            let value: serde_json::Value = serde_json::from_slice(&output.stdout)
                .context("Calibration helper did not return a result")?;
            return Ok(value);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Tracker discovery timed out. No calibration was started.");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn summary(value: &serde_json::Value) -> String {
    if value["ok"] != true {
        return value["error"]
            .as_str()
            .unwrap_or("Calibration discovery failed")
            .into();
    }
    let Some(devices) = value["devices"].as_array().filter(|d| !d.is_empty()) else {
        return "No Tobii tracker found. Enable eye tracking in Pimax EVO, wake the headset, then check again. Calibration has not started.".into();
    };
    devices.iter().map(|d| {
        if d["connected"] != true {
            return format!("Tracker connection: {}", d["error"].as_str().unwrap_or("unavailable"));
        }
        format!("{} / {} · 3D calibration: {} · backup: {}. Native calibration is experimental; no changes applied.",
            d["model"].as_str().unwrap_or("Unknown model"), d["generation"].as_str().unwrap_or("Unknown generation"),
            if d["calibration_3d"] == true { "available" } else { "unavailable" },
            d["backup"]["result"].as_str().unwrap_or("unverified"))
    }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_is_not_presented_as_calibration_success() {
        assert!(summary(&serde_json::json!({"ok":true,"devices":[]})).contains("No Tobii tracker"));
        assert_eq!(
            summary(&serde_json::json!({"ok":false,"error":"Unsupported ABI"})),
            "Unsupported ABI"
        );
        assert!(summary(&serde_json::json!({"ok":true,"devices":[{"connected":true,"model":"test","generation":"XR5","calibration_3d":true,"backup":{"result":"ok"}}]})).contains("no changes applied"));
    }
}
