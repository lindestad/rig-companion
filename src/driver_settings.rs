//! Edit overrides against defaults published by the installed sboys driver.
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Field {
    pub path: String,
    pub group: String,
    pub label: String,
    pub default: Value,
    pub value: Value,
    pub overridden: bool,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub profiles: crate::distortion::Catalogue,
    pub path: PathBuf,
    pub dream_air_fov_limits: [Option<f64>; 2],
    original: Vec<u8>,
    overrides: Value,
    pub fields: Vec<Field>,
}

pub fn relevant(path: &str) -> bool {
    let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
    match parts[0] {
        "dreamAir" | "generalHeadset" => true,
        "customShader" => !matches!(
            parts.get(1).copied(),
            Some("enableForMeganeX8K" | "enableForOther" | "lensColorCorrection")
        ),
        _ => parts.len() == 1,
    }
}

pub fn label(key: &str) -> String {
    let mut result = String::new();
    let mut previous = ' ';
    for c in key.chars() {
        if c.is_uppercase() && (previous.is_lowercase() || previous.is_ascii_digit()) {
            result.push(' ');
        }
        if result.is_empty() {
            result.extend(c.to_uppercase());
        } else {
            result.push(c);
        }
        previous = c;
    }
    result
}

fn parse(bytes: &[u8]) -> Result<Value> {
    let v: Value = serde_json::from_reader(json_comments::StripComments::new(bytes))?;
    ensure!(v.is_object(), "Settings must be a JSON object");
    Ok(v)
}

impl Settings {
    pub fn load() -> Result<Self> {
        let root = PathBuf::from(std::env::var_os("APPDATA").context("APPDATA unavailable")?)
            .join("CustomHeadset");
        Self::load_from(&root.join("settings.json"), &root.join("info.json"))
    }

    pub fn load_from(path: &Path, info_path: &Path) -> Result<Self> {
        let original = std::fs::read(path)
            .context("Cannot read sboys settings.json. Launch the custom driver first.")?;
        let overrides = parse(&original).context("Invalid sboys settings; no changes made")?;
        let info = parse(
            &std::fs::read(info_path)
                .context("Driver defaults unavailable. Launch SteamVR once first.")?,
        )?;
        let defaults = info
            .get("defaultSettings")
            .filter(|v| v.is_object())
            .context("Driver did not publish default settings")?;
        let mut fields = Vec::new();
        fn visit(value: &Value, path: String, overrides: &Value, fields: &mut Vec<Field>) {
            if let Some(object) = value.as_object() {
                for (key, value) in object {
                    visit(
                        value,
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                        overrides,
                        fields,
                    );
                }
            } else {
                let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
                let group = if parts.len() == 1 {
                    "Global".into()
                } else {
                    label(parts[0])
                };
                let name = parts[if parts.len() == 1 { 0 } else { 1 }..]
                    .iter()
                    .map(|p| label(p))
                    .collect::<Vec<_>>()
                    .join(" / ");
                fields.push(Field {
                    path: path.clone(),
                    group,
                    label: name,
                    default: value.clone(),
                    value: overrides.pointer(&path).unwrap_or(value).clone(),
                    overridden: overrides.pointer(&path).is_some(),
                });
            }
        }
        visit(defaults, String::new(), &overrides, &mut fields);
        Ok(Self {
            profiles: crate::distortion::Catalogue::load(
                &info,
                &info_path
                    .parent()
                    .context("Missing driver directory")?
                    .join("Distortion"),
            ),
            path: path.into(),
            dream_air_fov_limits: ["fovMaxX", "fovMaxY"].map(|axis| {
                (info.get("connectedHeadset").and_then(Value::as_u64) == Some(4))
                    .then(|| {
                        info.get("resolution")?
                            .get(axis)?
                            .as_f64()
                            .filter(|v| v.is_finite() && *v > 1.0)
                            .map(f64::ceil)
                    })
                    .flatten()
            }),
            original,
            overrides,
            fields,
        })
    }

    pub fn save(&self, changes: &BTreeMap<String, Option<String>>) -> Result<PathBuf> {
        ensure!(!changes.is_empty(), "No changes to apply");
        let mut updated = self.overrides.clone();
        for (path, input) in changes {
            let field = self
                .fields
                .iter()
                .find(|f| &f.path == path)
                .context("Unknown setting")?;
            let value = input.as_ref().map(|s| validate(field, s)).transpose()?;
            assign(&mut updated, path, value)?;
        }
        let bytes = serde_json::to_vec_pretty(&updated)?;
        ensure!(
            std::fs::read(&self.path)? == self.original,
            "Settings changed outside this app. Reload before applying to avoid overwriting them."
        );
        let backup_dir =
            crate::profile::default_path(false).with_file_name("driver-settings-backups");
        self.write_with_backup(&bytes, &backup_dir)
    }

    fn write_with_backup(&self, bytes: &[u8], backup_dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(backup_dir)?;
        let mut backup = tempfile::Builder::new()
            .prefix("settings-")
            .suffix(".json")
            .tempfile_in(backup_dir)?;
        backup.write_all(&self.original)?;
        backup.as_file().sync_all()?;
        let (_, backup_path) = backup.keep().map_err(|e| e.error)?;
        let mut temp = tempfile::NamedTempFile::new_in(
            self.path.parent().context("Settings directory missing")?,
        )?;
        temp.write_all(bytes)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        // Recheck after preparing the files to narrow the race with external editors.
        ensure!(
            std::fs::read(&self.path)? == self.original,
            "Settings changed during save. Reload before applying."
        );
        temp.persist(&self.path)
            .map_err(|e| e.error)
            .context("Cannot replace driver settings")?;
        Ok(backup_path)
    }
}

fn assign(root: &mut Value, path: &str, value: Option<Value>) -> Result<()> {
    let mut parts = path
        .trim_start_matches('/')
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .peekable();
    let mut cursor = root;
    while let Some(key) = parts.next() {
        let map = cursor
            .as_object_mut()
            .context("A settings group is not an object")?;
        if parts.peek().is_none() {
            if let Some(value) = value {
                map.insert(key, value);
            } else {
                map.remove(&key);
            }
            return Ok(());
        }
        cursor = map.entry(key).or_insert_with(|| serde_json::json!({}));
    }
    Ok(())
}

pub fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

pub fn validate(field: &Field, input: &str) -> Result<Value> {
    let default = &field.default;
    let value = if default.is_string() {
        Value::String(input.into())
    } else {
        serde_json::from_str(input).with_context(|| {
            format!(
                "{}: enter a valid {}",
                field.label,
                if default.is_array() {
                    "JSON array"
                } else {
                    "value"
                }
            )
        })?
    };
    let correct = match default {
        Value::Bool(_) => value.is_boolean(),
        Value::Number(n) if n.is_i64() || n.is_u64() => value.is_i64() || value.is_u64(),
        Value::Number(_) => value.is_number(),
        Value::String(_) => value.is_string(),
        Value::Array(_) => value
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_number)),
        _ => false,
    };
    ensure!(correct, "{}: value has the wrong type", field.label);
    let key = field.path.rsplit('/').next().unwrap_or("");
    if key == "samplingFilter" {
        ensure!(
            [
                "None",
                "NearestNeighbor",
                "FXAA2",
                "FXAA2CAS",
                "LumaSharpen",
                "CAS"
            ]
            .contains(&input),
            "Unknown sampling filter"
        );
    }
    if let Some(n) = value.as_f64() {
        ensure!(n.is_finite(), "{} must be finite", field.label);
        if matches!(
            key,
            "fovZoom"
                | "flatFovZoom"
                | "distortionZoom"
                | "gamma"
                | "renderResolutionMultiplierX"
                | "renderResolutionMultiplierY"
        ) {
            ensure!(n > 0.0, "{} must be positive", field.label);
        }
        if matches!(key, "maxFovX" | "maxFovY") {
            ensure!(
                n > 0.0 && n < 180.0,
                "FOV must be between 0 and 180 degrees"
            );
        }
        if key == "ipd" {
            ensure!(
                (40.0..=90.0).contains(&n),
                "IPD must be between 40 and 90 mm"
            );
        }
    }
    if key == "subpixelOffsets" {
        ensure!(
            value.as_array().is_some_and(|v| v.len() == 6),
            "Subpixel offsets require six numbers"
        );
    }
    if key == "srgbColorCorrectionMatrix" {
        ensure!(
            value
                .as_array()
                .is_some_and(|v| v.is_empty() || v.len() == 9),
            "Color matrix requires zero or nine numbers"
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_distinguishes_dream_air_and_other_headsets() {
        for p in [
            "/dreamAir/ipd",
            "/forceTracking",
            "/generalHeadset/useViveBluetooth",
            "/customShader/gamma",
        ] {
            assert!(relevant(p));
        }
        for p in [
            "/dreamAirSE/ipd",
            "/meganeX8K/enable",
            "/customShader/enableForMeganeX8K",
            "/customShader/lensColorCorrection",
        ] {
            assert!(!relevant(p));
        }
    }
    #[test]
    fn edits_preserve_hidden_and_unknown_values_and_reset_removes_override() {
        let mut v =
            parse(br#"{/* keep values */"dreamAir":{"ipd":66.5},"other":{"future":17}}"#).unwrap();
        assign(&mut v, "/dreamAir/ipd", None).unwrap();
        assign(&mut v, "/customShader/gamma", Some(serde_json::json!(2.3))).unwrap();
        assert!(v.pointer("/dreamAir/ipd").is_none());
        assert_eq!(v["other"]["future"], 17);
        assert_eq!(v["customShader"]["gamma"], 2.3);
    }
    #[test]
    fn rejects_invalid_edits_and_preserves_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let info = dir.path().join("info.json");
        std::fs::write(&path, br#"{"dreamAir":{"ipd":66.5}}"#).unwrap();
        std::fs::write(
            &info,
            br#"{"defaultSettings":{"dreamAir":{"ipd":63.0,"enable":true}}}"#,
        )
        .unwrap();
        let s = Settings::load_from(&path, &info).unwrap();
        let f = s.fields.iter().find(|f| f.path.ends_with("/ipd")).unwrap();
        assert!(validate(f, "NaN").is_err());
        assert!(validate(f, "-1").is_err());
        assert!(validate(f, "67.5").is_ok());
        let backup = s
            .write_with_backup(br#"{}"#, &dir.path().join("backups"))
            .unwrap();
        assert_eq!(std::fs::read(backup).unwrap(), s.original);
        assert!(
            s.save(&BTreeMap::from([(
                "/dreamAir/ipd".into(),
                Some("67.5".into())
            )]))
            .is_err()
        );
    }
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}
#[derive(Debug, Default, serde::Deserialize)]
pub struct Control {
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub restart: String,
    pub step: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub dynamic_fov: Option<String>,
    #[serde(default)]
    pub options: Vec<Choice>,
}
pub fn control(path: &str) -> Option<&'static Control> {
    static CONTROLS: std::sync::OnceLock<BTreeMap<String, Control>> = std::sync::OnceLock::new();
    let controls = CONTROLS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/driver-controls.json"))
            .expect("embedded control metadata")
    });
    let key = path.trim_start_matches('/');
    controls.get(key).or_else(|| {
        let (group, field) = key.split_once('/')?;
        if matches!(group, "customShader" | "generalHeadset") {
            return None;
        }
        controls.get(&format!("headset/{field}"))
    })
}
pub fn number_text(value: f64) -> String {
    let v = (value * 1_000_000.0).round() / 1_000_000.0;
    if v == 0.0 { "0".into() } else { v.to_string() }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    #[test]
    fn controls_match_sboys_steps_and_choices() {
        let ipd = control("/dreamAir/ipd").unwrap();
        assert_eq!(
            (ipd.min, ipd.max, ipd.step),
            (Some(50.0), Some(80.0), Some(0.5))
        );
        assert_eq!(
            control("/dreamAir/secondsFromVsyncToPhotons").unwrap().step,
            Some(0.0005)
        );
        assert_eq!(
            control("/customShader/samplingFilter")
                .unwrap()
                .options
                .len(),
            6
        );
        assert!(control("/dreamAir/enablePimaxPassthrough").unwrap().restart == "steamvrRestart");
        assert!(control("/customShader/ipd").is_none());
        assert_eq!(number_text(1.7 + 0.1), "1.8");
        assert_eq!(number_text(0.00025 + 0.00025), "0.0005");
    }
}
