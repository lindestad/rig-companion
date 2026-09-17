//! Installed sboys profile catalogue. Preview uses published knots, not a lens simulation.
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub source: String,
    pub data: Value,
}
impl Profile {
    pub fn compatible(&self, device: &str) -> bool {
        match self.data.get("device") {
            None | Some(Value::Null) => true,
            Some(Value::String(s)) => s.is_empty() || s == device,
            Some(Value::Array(a)) => a.is_empty() || a.iter().any(|s| s.as_str() == Some(device)),
            _ => false,
        }
    }
    pub fn description(&self) -> &str {
        self.data
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
    }
    pub fn points(&self, key: &str) -> Result<Vec<[f32; 2]>, String> {
        if self.data.get("type").and_then(Value::as_str) != Some("RadialBezier") {
            return Err(
                "This profile uses a runtime distortion function; no radial points are published."
                    .into(),
            );
        }
        let Some(value) = self.data.get(key) else {
            return Ok(vec![]);
        };
        let a = value.as_array().ok_or("Profile points must be an array")?;
        if a.len() % 2 != 0 {
            return Err("Profile has an incomplete angle/value pair".into());
        }
        let mut points = Vec::new();
        for pair in a.as_chunks::<2>().0 {
            let x = pair[0].as_f64().ok_or("Invalid profile angle")? as f32;
            let y = pair[1].as_f64().ok_or("Invalid profile value")? as f32;
            if !x.is_finite()
                || !y.is_finite()
                || x < 0.0
                || points.last().is_some_and(|p: &[f32; 2]| p[0] >= x)
            {
                return Err(
                    "Profile points need finite values and increasing nonnegative angles".into(),
                );
            }
            points.push([x, y]);
        }
        Ok(points)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    pub profiles: BTreeMap<String, Profile>,
    pub warnings: Vec<String>,
}
impl Catalogue {
    pub fn load(info: &Value, directory: &Path) -> Self {
        let mut out = Self::default();
        if let Some(profiles) = info
            .get("builtInDistortionProfiles")
            .and_then(Value::as_object)
        {
            for (name, data) in profiles {
                out.profiles.insert(
                    name.clone(),
                    Profile {
                        name: name.clone(),
                        source: "Built-in".into(),
                        data: data.clone(),
                    },
                );
            }
        }
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
            Err(e) => {
                out.warnings
                    .push(format!("Cannot read custom profiles: {e}"));
                return out;
            }
        };
        for entry in entries {
            let result = (|| -> anyhow::Result<()> {
                let path = entry?.path();
                if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
                    return Ok(());
                }
                let name = path.file_stem().unwrap().to_string_lossy().into_owned();
                // The driver resolves built-ins before files with the same name.
                if out.profiles.contains_key(&name) {
                    return Ok(());
                }
                let bytes = std::fs::read(&path)?;
                let data: Value =
                    serde_json::from_reader(json_comments::StripComments::new(bytes.as_slice()))
                        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
                anyhow::ensure!(
                    data.is_object(),
                    "{} is not a profile object",
                    path.display()
                );
                out.profiles.insert(
                    name.clone(),
                    Profile {
                        name,
                        source: "Custom file".into(),
                        data,
                    },
                );
                Ok(())
            })();
            if let Err(e) = result {
                out.warnings.push(e.to_string());
            }
        }
        out
    }
    pub fn choices(
        &self,
        device: &str,
        current: &str,
        default: &str,
        filtered: bool,
    ) -> Vec<String> {
        let mut names: Vec<_> = self
            .profiles
            .values()
            .filter(|p| !filtered || p.compatible(device) || p.name == current || p.name == default)
            .map(|p| p.name.clone())
            .collect();
        for value in [current, default] {
            if !names.iter().any(|n| n == value) {
                names.push(value.into());
            }
        }
        names.sort_by_key(|n| (n != default, n.to_lowercase()));
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_matches_devices_preserves_unknown_selection_and_builtin_precedence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Default.json"),
            r#"{"description":"wrong"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Custom.json"),
            r#"{/*comment*/"device":["DreamAir"]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("broken.json"), "{").unwrap();
        let info = serde_json::json!({"builtInDistortionProfiles":{"Default":{"device":"DreamAir","description":"right"},"Other":{"device":"Other"}}});
        let c = Catalogue::load(&info, dir.path());
        assert_eq!(c.profiles["Default"].description(), "right");
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(
            c.choices("DreamAir", "Missing", "Default", true),
            vec!["Default", "Custom", "Missing"]
        );
        assert!(
            c.choices("DreamAir", "Other", "Default", true)
                .contains(&"Other".into())
        );
    }
    #[test]
    fn malformed_and_runtime_profiles_are_not_drawn_as_valid_curves() {
        let mut p = Profile {
            name: "x".into(),
            source: "test".into(),
            data: serde_json::json!({"type":"RadialBezier","distortions":[0,0,20,50]}),
        };
        assert_eq!(p.points("distortions").unwrap(), vec![[0., 0.], [20., 50.]]);
        p.data["distortions"] = serde_json::json!([0, 0, 0, 1]);
        assert!(p.points("distortions").is_err());
        p.data["distortions"] = serde_json::json!([0, 0, 20]);
        assert!(p.points("distortions").is_err());
        p.data["type"] = serde_json::json!("Pimax");
        assert!(p.points("distortions").is_err());
    }
}
