use clap::{Parser, Subcommand};
use rig_companion::{
    profile,
    service::{Command, Engine},
};
use std::{path::PathBuf, time::Duration};

#[derive(Parser)]
#[command(
    version,
    about = "Rig Companion calibration CLI (close the GUI before live use)"
)]
struct Args {
    #[arg(long, global = true)]
    demo: bool,
    #[arg(long, global = true)]
    profile: Option<PathBuf>,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Read one eye diagnostic snapshot; may run alongside the GUI.
    EyeStatus,
    /// Query calibration capabilities in an isolated helper; no calibration writes.
    EyeCalibrationStatus,
    /// Check the modified headset driver's command protocol without clicking.
    HeadsetBridgeStatus,
    /// Enable SteamVR's gamepad input driver. Restart SteamVR afterward.
    EnableGamepad,
    /// Inspect SteamVR gamepad setting and active input device without clicking.
    GamepadStatus,
    /// Inspect current tracking without modifying it. Output is JSON.
    Status,
    /// Store a desired seated height in centimetres; does not modify SteamVR.
    SetHeight {
        cm: f64,
    },
    /// Restore the saved height in SteamVR.
    Restore,
    /// Capture the current correct height, position and forward direction.
    Capture,
    /// Restore a previously captured position and heading.
    RestoreFull,
    /// Raise/lower viewpoint by signed centimetres (maximum 10).
    Nudge {
        #[arg(allow_hyphen_values = true)]
        cm: f64,
    },
    Dashboard,
    /// Exercise save, restore, nudge and undo using a temporary simulated profile.
    DemoCheck,
}

fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}",
            serde_json::json!({"ok": false, "error": format!("{error:#}")})
        );
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    if matches!(args.command, Action::EyeCalibrationStatus) {
        anyhow::ensure!(
            !args.demo,
            "Calibration discovery requires the live vendor runtime"
        );
        println!("{}", rig_companion::eye_calibration::inspect()?);
        return Ok(());
    }
    if matches!(args.command, Action::EyeStatus) {
        anyhow::ensure!(!args.demo, "Eye diagnostics require the live driver");
        println!(
            "{}",
            serde_json::to_string_pretty(&rig_companion::eyes::read()?)?
        );
        return Ok(());
    }
    if matches!(args.command, Action::HeadsetBridgeStatus) {
        anyhow::ensure!(!args.demo, "Headset bridge status requires live SteamVR");
        let vr = rig_companion::steamvr::SteamVr::connect()?;
        let reply = vr.headset_bridge_request(c"rigcompanion:capabilities:v1")?;
        println!(
            "{}",
            serde_json::json!({"supported":reply == "ok:rigcompanion:gaze-click:v1", "reply":reply})
        );
        return Ok(());
    }
    if matches!(args.command, Action::EnableGamepad | Action::GamepadStatus) {
        anyhow::ensure!(!args.demo, "Gamepad configuration requires live SteamVR");
        let vr = rig_companion::steamvr::SteamVr::connect()?;
        let was_enabled = vr.gamepad_enabled()?;
        if matches!(args.command, Action::EnableGamepad) {
            vr.enable_gamepad()?;
        }
        println!(
            "{}",
            serde_json::json!({"was_enabled":was_enabled,"enabled":vr.gamepad_enabled()?,"connected":vr.gamepad_connected()?,"restart_required":!was_enabled && matches!(args.command, Action::EnableGamepad)})
        );
        return Ok(());
    }
    if matches!(args.command, Action::DemoCheck) {
        let temp = tempfile::tempdir()?;
        let mut engine = Engine::new(temp.path().join("profile.json"), true)?;
        for command in [
            Command::Connect,
            Command::SaveHeight(1.15),
            Command::RestoreHeight,
            Command::Nudge(0.01),
            Command::Undo,
        ] {
            engine.run(command)?;
        }
        anyhow::ensure!(
            (engine.state.pose.unwrap().position[1] - 1.15).abs() < 1e-8,
            "Unexpected final height"
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"ok":true,"state":engine.state}))?
        );
        return Ok(());
    }
    let _lock = rig_companion::lock_instance(args.demo)?;
    let mut engine = Engine::new(
        args.profile
            .unwrap_or_else(|| profile::default_path(args.demo)),
        args.demo,
    )?;
    if let Action::SetHeight { cm } = args.command {
        engine.run(Command::SaveHeight(cm / 100.0))?;
    } else {
        engine.run(Command::Connect)?;
        if !matches!(args.command, Action::Status | Action::Dashboard) && !args.demo {
            std::thread::sleep(Duration::from_millis(3200));
            engine.refresh();
        }
        match args.command {
            Action::Status => {}
            Action::Restore => {
                engine.run(Command::RestoreHeight)?;
            }
            Action::Capture => {
                engine.run(Command::Capture)?;
            }
            Action::RestoreFull => {
                engine.run(Command::RestoreReference)?;
            }
            Action::Nudge { cm } => {
                engine.run(Command::Nudge(cm / 100.0))?;
            }
            Action::Dashboard => {
                engine.run(Command::Dashboard)?;
            }
            _ => unreachable!(),
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"ok":true,"state":engine.state}))?
    );
    Ok(())
}
