//! Start the existing VR stack once; reconnecting does not relaunch apps the user closed.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchConfig {
    pub pimax_exe: PathBuf,
    pub driver_dir: PathBuf,
}

pub fn runtime_dir() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?;
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(
        PathBuf::from(local).join("openvr/openvrpaths.vrpath"),
    )?)?;
    Ok(PathBuf::from(
        data["runtime"][0]
            .as_str()
            .context("SteamVR runtime path is unavailable")?,
    ))
}

fn config() -> Result<LaunchConfig> {
    let path = crate::profile::default_path(false).with_file_name("launch-config.json");
    if path.exists() {
        return serde_json::from_slice(&std::fs::read(&path)?)
            .context("Cannot read launch-config.json");
    }
    let home = directories::UserDirs::new().context("Cannot locate home directory")?;
    let config = LaunchConfig {
        pimax_exe: PathBuf::from(r"C:\Program Files\Pimax\PimaxClient\pimaxui\PimaxClient.exe"),
        driver_dir: home
            .home_dir()
            .join("dev/CustomHeadsetOpenVR/output/CustomHeadsetOpenVR"),
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_vec_pretty(&config)?)?;
    Ok(config)
}

pub fn process_running(name: &str) -> Result<bool> {
    process_matches(name, None)
}

pub fn process_matches(name: &str, pid: Option<u32>) -> Result<bool> {
    // SAFETY: snapshot owns its handle; PROCESSENTRY32W is initialized to the ABI size.
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        ensure!(
            handle != INVALID_HANDLE_VALUE,
            "Cannot inspect running processes: {}",
            std::io::Error::last_os_error()
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        let mut next = Process32FirstW(handle, &mut entry);
        while next != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|&v| v == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(name)
                && pid.is_none_or(|pid| pid == entry.th32ProcessID)
            {
                found = true;
                break;
            }
            next = Process32NextW(handle, &mut entry);
        }
        CloseHandle(handle);
        Ok(found)
    }
}

fn launch_if_missing(exe: &Path, process: &str) -> Result<bool> {
    if process_running(process)? {
        return Ok(false);
    }
    ensure!(
        exe.is_file(),
        "Executable missing: {}. Check launch-config.json.",
        exe.display()
    );
    Command::new(exe)
        .current_dir(exe.parent().context("Missing executable directory")?)
        .spawn()
        .with_context(|| format!("Cannot launch {}", exe.display()))?;
    Ok(true)
}

pub fn run(cancelled: &AtomicBool, mut progress: impl FnMut(&str)) -> Result<()> {
    let config = config()?;
    let runtime = runtime_dir()?;
    let log_path = crate::profile::default_path(false).with_file_name("launch.log");
    let mut report = |message: &str| {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = writeln!(file, "{message}");
        }
        progress(message);
    };
    report("Checking Pimax EVO…");
    if launch_if_missing(&config.pimax_exe, "PimaxClient.exe")? {
        report("Pimax EVO launched. Waiting for its runtime…");
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    while !process_running("pi_server.exe")? {
        ensure!(!cancelled.load(Ordering::Relaxed), "Startup cancelled");
        ensure!(
            Instant::now() < deadline,
            "Pimax runtime did not start within 30 seconds. Finish Pimax startup, then reopen Rig Companion."
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    ensure!(!cancelled.load(Ordering::Relaxed), "Startup cancelled");
    report("Pimax runtime running. Checking the custom headset driver…");
    ensure!(
        config
            .driver_dir
            .join("bin/win64/driver_CustomHeadsetOpenVR.dll")
            .is_file(),
        "Custom headset build is missing: {}",
        config.driver_dir.display()
    );
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?;
    let registration: serde_json::Value = serde_json::from_slice(&std::fs::read(
        PathBuf::from(local).join("openvr/openvrpaths.vrpath"),
    )?)?;
    let registered = registration["external_drivers"]
        .as_array()
        .is_some_and(|paths| {
            paths
                .iter()
                .filter_map(|p| p.as_str())
                .any(|p| same_path(Path::new(p), &config.driver_dir))
        });
    ensure!(
        !runtime
            .join("drivers/CustomHeadsetOpenVR/driver.vrdrivermanifest")
            .exists(),
        "A second CustomHeadset driver is installed inside SteamVR. Resolve the duplicate before launching."
    );
    if !registered {
        ensure!(
            !process_running("vrserver.exe")?,
            "The custom driver is not registered. Close SteamVR and reopen Rig Companion to register it."
        );
        let result = Command::new(runtime.join("bin/win64/vrpathreg.exe"))
            .args(["adddriver"])
            .arg(&config.driver_dir)
            .creation_flags(0x08000000)
            .output()?;
        ensure!(
            result.status.success(),
            "Could not register the custom driver: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        report("Custom headset driver registered.");
    }
    ensure!(!cancelled.load(Ordering::Relaxed), "Startup cancelled");
    if !process_running("vrserver.exe")? && !process_running("vrstartup.exe")? {
        launch_if_missing(&runtime.join("bin/win64/vrstartup.exe"), "vrstartup.exe")?;
        report("SteamVR launched; it will load the custom headset driver. Waiting for connection…");
    } else {
        report("SteamVR already running. Connecting to your headset…");
    }
    Ok(())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}

pub fn toggle_dashboard_closed() -> Result<()> {
    let result = Command::new(runtime_dir()?.join("bin/win64/vrcmd.exe"))
        .args(["--compositorcmd", "system_dashboard_toggle"])
        .creation_flags(0x08000000)
        .output()?;
    ensure!(
        result.status.success(),
        "SteamVR dashboard toggle failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

pub fn toggle_passthrough() -> Result<()> {
    let result = Command::new(runtime_dir()?.join("bin/win64/vrcmd.exe"))
        .args(["--compositorcmd", "camera_room_view_toggle"])
        .creation_flags(0x08000000)
        .output()?;
    ensure!(
        result.status.success(),
        "SteamVR rejected the camera toggle"
    );
    Ok(())
}

pub fn quit_steamvr() -> Result<()> {
    if !process_running("vrserver.exe")? && !process_running("vrmonitor.exe")? {
        return Ok(());
    }
    Command::new(runtime_dir()?.join("bin/win64/vrmonitor.exe"))
        .arg("vrmonitor://quit")
        .creation_flags(0x08000000)
        .spawn()
        .context("Cannot request SteamVR shutdown")?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while process_running("vrserver.exe")? || process_running("vrmonitor.exe")? {
        ensure!(
            Instant::now() < deadline,
            "SteamVR has not finished shutting down. Close any VR application prompts, then press Quit again."
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_compares_equivalent_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(same_path(dir.path(), &dir.path().join(".")));
        assert!(!same_path(dir.path(), &dir.path().join("missing")));
    }
    #[test]
    fn process_detection_finds_our_actual_executable() {
        let exe = std::env::current_exe().unwrap();
        assert!(process_running(exe.file_name().unwrap().to_str().unwrap()).unwrap());
        assert!(!process_running("rig-companion-nonexistent-test-process.exe").unwrap());
    }
}
