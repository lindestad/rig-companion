//! Read existing driver diagnostics; never subscribe to or store tracker samples.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Angles {
    pub angle_x: f64,
    pub angle_y: f64,
}

impl Angles {
    pub fn degrees(self) -> [f64; 2] {
        [self.angle_x.to_degrees(), self.angle_y.to_degrees()]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gaze {
    pub valid: bool,
    pub left: Angles,
    pub right: Angles,
}

#[derive(Deserialize)]
struct Diagnostic {
    #[serde(rename = "vrserverPID")]
    pid: u32,
    #[serde(rename = "eyeTracking")]
    gaze: Gaze,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reading {
    pub gaze: Gaze,
    pub path: PathBuf,
    pub age_ms: u128,
    pub driver_running: bool,
}

impl Reading {
    pub fn live(&self) -> bool {
        self.driver_running && self.age_ms < 2000
    }
    pub fn usable(&self) -> bool {
        self.live() && self.gaze.valid
    }
}

fn parse(bytes: &[u8]) -> Result<Diagnostic> {
    let data: Diagnostic = serde_json::from_slice(bytes)?;
    ensure!(data.pid != 0, "Driver PID is missing");
    for angle in [data.gaze.left, data.gaze.right] {
        ensure!(
            angle
                .degrees()
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 180.0),
            "Invalid gaze angles"
        );
    }
    Ok(data)
}

pub fn read() -> Result<Reading> {
    let roaming = PathBuf::from(std::env::var_os("APPDATA").context("APPDATA unavailable")?);
    let mut paths: Vec<_> = [
        "CustomHeadset/diagnostic.json",
        "Pimax/CustomHeadset/diagnostic.json",
    ]
    .into_iter()
    .filter_map(|name| {
        let path = roaming.join(name);
        let modified = path.metadata().ok()?.modified().ok()?;
        Some((path, modified))
    })
    .collect();
    paths.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    let mut last_error = anyhow::anyhow!(
        "No driver eye diagnostics yet. Start SteamVR and enable eye tracking in Pimax."
    );
    let mut stale = None;
    for (path, modified) in paths {
        let result = (|| -> Result<Reading> {
            ensure!(
                path.metadata()?.len() <= 64 * 1024,
                "Diagnostic file is unexpectedly large"
            );
            let data = parse(&std::fs::read(&path)?)?;
            Ok(Reading {
                driver_running: crate::startup::process_matches("vrserver.exe", Some(data.pid))?,
                gaze: data.gaze,
                age_ms: SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or(Duration::MAX)
                    .as_millis(),
                path,
            })
        })();
        match result {
            Ok(reading) if reading.live() => return Ok(reading),
            Ok(reading) => {
                if stale.is_none() {
                    stale = Some(reading);
                }
            }
            Err(e) => last_error = e.context("Driver diagnostics unavailable; retrying"),
        }
    }
    stale.ok_or(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_writes_and_bad_angles_are_rejected() {
        assert!(parse(br#"{"vrserverPID":3,"eyeTracking":{"#).is_err());
        assert!(parse(br#"{"vrserverPID":3,"eyeTracking":{"valid":true,"left":{"angleX":9,"angleY":0},"right":{"angleX":0,"angleY":0}}}"#).is_err());
    }
    #[test]
    fn freshness_and_validity_are_independent() {
        let gaze = Gaze {
            valid: false,
            left: Angles {
                angle_x: 0.0,
                angle_y: 0.0,
            },
            right: Angles {
                angle_x: 0.0,
                angle_y: 0.0,
            },
        };
        let mut r = Reading {
            gaze,
            path: PathBuf::new(),
            age_ms: 100,
            driver_running: true,
        };
        assert!(r.live());
        assert!(!r.usable());
        r.gaze.valid = true;
        assert!(r.usable());
        r.age_ms = 2000;
        assert!(!r.usable());
        r.age_ms = 100;
        r.driver_running = false;
        assert!(!r.usable());
        assert!(
            (Angles {
                angle_x: std::f64::consts::FRAC_PI_2,
                angle_y: 0.0
            }
            .degrees()[0]
                - 90.0)
                .abs()
                < 1e-8
        );
    }
}
