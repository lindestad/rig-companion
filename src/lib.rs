pub mod calibration;
pub mod distortion;
pub mod driver_settings;
pub mod eye_calibration;
pub mod eyes;
pub mod ipc;
pub mod profile;
pub mod service;
pub mod settings_categories;
pub mod startup;
pub mod steamvr;
pub mod wind;

pub fn lock_instance(demo: bool) -> anyhow::Result<std::fs::File> {
    use anyhow::Context;
    let path = profile::default_path(demo).with_extension("lock");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock().context("Rig Companion is already running. Close it before starting another instance or using rigctl.")?;
    Ok(file)
}
pub mod game_launch;
pub mod gaze;
