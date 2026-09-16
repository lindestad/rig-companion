use crate::calibration::{Pose, validate_height};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub height_m: Option<f64>,
    pub reference: Option<Pose>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: 1,
            height_m: Some(0.98),
            reference: None,
        }
    }
}

impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1,
            "Unsupported profile version {}; expected 1",
            self.version
        );
        if let Some(h) = self.height_m {
            validate_height(h)?;
        }
        if let Some(p) = self.reference {
            p.validate()?;
            validate_height(p.position[1])?;
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                let p: Self = serde_json::from_slice(&bytes)
                    .context("Cannot read saved reference; original file was preserved")?;
                p.validate()?;
                Ok(p)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context("Cannot open profile"),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .context("Profile must have a parent directory")?;
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&serde_json::to_vec_pretty(self)?)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(path)
            .map_err(|e| e.error)
            .context("Could not atomically save reference")?;
        Ok(())
    }
}

pub fn default_path(demo: bool) -> PathBuf {
    directories::ProjectDirs::from("", "", "RigCompanion")
        .expect("Windows user data directory unavailable")
        .data_local_dir()
        .join(if demo {
            "demo-profile.json"
        } else {
            "profile.json"
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_replaces_and_reloads_reference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        let mut p = Profile {
            height_m: Some(1.15),
            ..Profile::default()
        };
        p.save(&path).unwrap();
        p.height_m = Some(1.2);
        p.save(&path).unwrap();
        assert_eq!(Profile::load(&path).unwrap().height_m, Some(1.2));
    }
    #[test]
    fn refuses_future_version_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        let contents = r#"{"version":99,"height_m":1.15,"reference":null}"#;
        fs::write(&path, contents).unwrap();
        assert!(Profile::load(&path).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }
}
