//! Start the existing VR stack once; reconnecting does not relaunch apps the user closed.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Seek, SeekFrom},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    compositor_command("system_dashboard_toggle", "dashboard toggle")
}

pub fn toggle_passthrough() -> Result<()> {
    compositor_command("camera_room_view_toggle", "camera toggle")
}

fn compositor_command(command: &str, label: &str) -> Result<()> {
    run_vr_helper(
        Command::new(runtime_dir()?.join("bin/win64/vrcmd.exe")).args(["--compositorcmd", command]),
        label,
        Duration::from_secs(3),
    )
}

fn run_vr_helper(command: &mut Command, label: &str, timeout: Duration) -> Result<()> {
    // A file avoids filling a pipe while we poll for exit. Do not wait on inherited
    // stdout/stderr handles: a child can exit without every writer closing them.
    let mut stderr = tempfile::tempfile().context("Cannot capture SteamVR helper errors")?;
    let mut child = command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()
        .with_context(|| format!("Cannot start SteamVR {label}"))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => {
                // Terminate only our vrcmd helper, never SteamVR or the Pimax runtime.
                child
                    .kill()
                    .with_context(|| format!("Cannot stop stalled SteamVR {label} helper"))?;
                // Windows termination is asynchronous. Bound cleanup as well so a
                // broken runtime cannot replace the original wait with a new one.
                let cleanup_deadline = Instant::now() + Duration::from_secs(1);
                while child
                    .try_wait()
                    .context("Cannot check stopped SteamVR helper")?
                    .is_none()
                {
                    ensure!(
                        Instant::now() < cleanup_deadline,
                        "SteamVR {label} helper did not exit after termination; its outcome is unknown"
                    );
                    std::thread::sleep(Duration::from_millis(25));
                }
                if let Err(error) = result {
                    return Err(error).context("Cannot check SteamVR helper status");
                }
                bail!(
                    "SteamVR {label} timed out after {:.0} seconds. The command may already have taken effect; check the headset before retrying.",
                    timeout.as_secs_f64()
                );
            }
        }
    };
    // Keep failures readable even if SteamVR emitted a very large diagnostic log.
    let length = stderr.metadata()?.len();
    stderr.seek(SeekFrom::Start(length.saturating_sub(4096)))?;
    let mut detail = Vec::new();
    stderr.take(4096).read_to_end(&mut detail)?;
    ensure!(
        status.success(),
        "SteamVR {label} failed ({status}): {}",
        String::from_utf8_lossy(&detail).trim()
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

    fn helper_fixture(mode: &str, marker: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "startup::tests::vr_helper_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("RIG_VR_HELPER_TEST_MODE", mode)
            .env("RIG_VR_HELPER_TEST_MARKER", marker);
        command
    }

    #[test]
    #[ignore = "subprocess fixture invoked by the helper regression tests"]
    fn vr_helper_fixture() {
        use std::io::Write;
        let mode = std::env::var("RIG_VR_HELPER_TEST_MODE").unwrap();
        std::fs::write(
            std::env::var_os("RIG_VR_HELPER_TEST_MARKER").unwrap(),
            std::process::id().to_string(),
        )
        .unwrap();
        if mode == "stall" {
            std::thread::sleep(Duration::from_secs(5));
        } else {
            // Exceed pipe capacity on both streams: waiting before draining pipes can deadlock.
            let noise = vec![b'x'; 256 * 1024];
            std::io::stdout().write_all(&noise).unwrap();
            std::io::stderr().write_all(&noise).unwrap();
            if mode == "fail" {
                eprintln!("fixture failure");
                std::process::exit(23);
            }
        }
    }

    #[test]
    fn stalled_vr_helper_times_out_and_next_command_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("pid");
        let started = Instant::now();
        let error = run_vr_helper(
            &mut helper_fixture("stall", &marker),
            "dashboard toggle",
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"), "{error:#}");
        assert!(started.elapsed() < Duration::from_secs(3));
        let pid: u32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
        let exe = std::env::current_exe().unwrap();
        assert!(!process_matches(exe.file_name().unwrap().to_str().unwrap(), Some(pid)).unwrap());
        run_vr_helper(
            &mut helper_fixture("success", &marker),
            "dashboard toggle",
            Duration::from_secs(3),
        )
        .unwrap();
    }

    #[test]
    fn vr_helper_reports_failure_after_large_output() {
        let dir = tempfile::tempdir().unwrap();
        let error = run_vr_helper(
            &mut helper_fixture("fail", &dir.path().join("pid")),
            "camera toggle",
            Duration::from_secs(3),
        )
        .unwrap_err();
        assert!(error.to_string().contains("camera toggle failed"));
        assert!(error.to_string().contains("fixture failure"));
    }
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
