#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod distortion_ui;
mod hotkeys;
mod settings_ui;
mod ui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Restore your seated reference in SteamVR")]
struct Args {
    /// Simulate a headset without accessing SteamVR. Uses a separate profile.
    #[arg(long)]
    demo: bool,
    /// Connect to an existing VR session without launching Pimax or SteamVR.
    #[arg(long)]
    no_launch: bool,
    /// Override the profile location.
    #[arg(long)]
    profile: Option<PathBuf>,
}

fn main() {
    if let Err(error) = run() {
        let message: Vec<u16> = format!("{error:#}\0").encode_utf16().collect();
        let title: Vec<u16> = "Rig Companion\0".encode_utf16().collect();
        // SAFETY: null-terminated strings remain alive for this synchronous dialog.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0x10,
            );
        }
    }
}

fn run() -> anyhow::Result<()> {
    let app_id: Vec<u16> = "RigCompanion.Desktop\0".encode_utf16().collect();
    unsafe {
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
    let args = Args::parse();
    let lock = match rig_companion::lock_instance(args.demo) {
        Ok(lock) => lock,
        Err(error) => {
            if !args.demo && focus_existing() {
                return Ok(());
            }
            return Err(error);
        }
    };
    let worker = rig_companion::service::Worker::spawn_with_launch(
        args.profile
            .unwrap_or_else(|| rig_companion::profile::default_path(args.demo)),
        args.demo,
        !args.no_launch,
    )?;
    let app = std::cell::RefCell::new(Some(ui::App::new(worker, lock)));
    iced::application(
        move || app.borrow_mut().take().expect("application boots once"),
        ui::App::update,
        ui::App::view,
    )
    .title("Rig Companion")
    .theme(ui::App::theme)
    .subscription(ui::App::subscription)
    .window(iced::window::Settings {
        exit_on_close_request: false,
        icon: iced::window::icon::from_rgba(include_bytes!("../assets/icon.rgba").to_vec(), 64, 64)
            .ok(),
        size: iced::Size::new(1120.0, 980.0),
        min_size: Some(iced::Size::new(900.0, 650.0)),
        position: iced::window::Position::Centered,
        ..Default::default()
    })
    .default_font(iced::Font::with_name("Segoe UI"))
    .run()?;
    Ok(())
}

fn focus_existing() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let title: Vec<u16> = "Rig Companion\0".encode_utf16().collect();
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if window.is_null() {
            return false;
        }
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
    }
    true
}
