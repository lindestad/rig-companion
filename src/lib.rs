pub mod calibration;
pub mod profile;
pub mod service;
pub mod startup;
pub mod steamvr;

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
pub mod gaze;
