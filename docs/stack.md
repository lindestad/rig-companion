# Stack recommendation

Researched 14 September 2026. Recommendation is provisional until the Windows lifecycle and OpenVR integration prototypes pass.

## Choice

Use **Rust + iced** for this utility. It has a small settings window and spends most of its life receiving hotkeys or USB commands in the background. An entirely Rust application suits the user's preference, without introducing a frontend package manager or a local web server.

Tauri is the easier integrated desktop shell if rapid web-UI development is the priority. Keep it as the fallback if iced's tray/event-loop integration takes disproportionate effort. The Rust calibration and OpenVR modules should be reusable with either shell.

## Comparison

| Stack | Fit | Main tradeoff |
| --- | --- | --- |
| Rust + iced | Recommended for an all-Rust utility with a modest UI | Tray and global hotkeys need additional crates and explicit Windows event-loop integration |
| Tauri 2 + Rust + Svelte/TypeScript | Best alternative for fast, polished web UI and integrated desktop facilities | Adds web frontend tooling and uses Windows WebView2 |
| Electron + TypeScript + Rust worker | Feasible, especially for a team already using Electron | Chromium/Node process architecture plus a native bridge or sidecar for our Rust VR logic |
| Rust + Axum + browser UI | Useful if a phone/browser dashboard becomes a requirement | Axum is a web server framework; it does not supply tray, hotkeys, desktop lifecycle or packaging |

These are architectural judgments, not measured startup, RAM, GPU or frame-time comparisons. Benchmark the actual app while racing before making performance claims. An iced UI also renders graphics; removing a browser does not make it free.

## Evidence

- [iced 0.14 daemon](https://docs.rs/iced/0.14.0/iced/fn.daemon.html) can start without a window and remain running after all windows close. This matches a tray utility. iced draws its own widgets; all-Rust does not mean standard Win32 widgets.
- [tray-icon](https://docs.rs/tray-icon/latest/tray_icon/) supports Windows and requires an event loop on the owning thread.
- [global-hotkey](https://docs.rs/global-hotkey/latest/global_hotkey/) requires a Windows message loop on the same thread that creates its manager. A dedicated Windows message-loop thread is an option; bridge events into iced instead of idle polling.
- [Tauri architecture](https://v2.tauri.app/concept/architecture/) provides a Rust core and a system webview. Its [tray API](https://v2.tauri.app/learn/system-tray/) and [global shortcut plugin](https://v2.tauri.app/plugin/global-shortcut/) supply the integration directly. Rust commands can own the critical path while the frontend is closed.
- [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model) uses a main process and renderer processes with Chromium/Node architecture. It is viable, but brings little advantage for this particular Rust-first project.
- [Axum documentation](https://docs.rs/axum/latest/axum/) describes its HTTP routing/extraction/middleware model. Add it only for a concrete remote/browser requirement; a desktop settings window does not require HTTP.

## Proposed Rust components

| Area | Initial choice |
| --- | --- |
| Toolchain | Stable Rust, edition 2024, Windows x86_64 MSVC; pin the tested release when implementation starts |
| Settings UI | iced 0.14 stable API; daemon lifecycle |
| Tray / shortcuts | tray-icon + global-hotkey; shared Windows message-loop ownership |
| OpenVR | Narrow safe adapter over openvr_sys bindings, subject to compilation/interface validation |
| Calibration maths | glam double-precision transforms, with explicit conversion to OpenVR's matrix layout |
| Profile storage | serde + serde_json; versioned profile, atomic replacement in the user's application-data directory |
| Diagnostics | tracing + tracing-subscriber; bounded/rotated file logs when persistent logging is added |
| Errors | thiserror for library boundaries; contextual errors at the executable boundary |
| Prototype commands | clap CLI before UI implementation |
| USB, later | hidapi for a vendor-defined HID collection; CDC serial is a development alternative |
| Development | Cargo, rustfmt, Clippy, rust-analyzer and just; optional cargo-nextest once tests warrant it |

Only introduce dependencies when their feature is implemented. Commit Cargo.lock for the application. Avoid a web bundler, Node requirement, Tokio runtime or Windows service unless a concrete feature needs one. Rust threads and message channels are sufficient for the initial worker design.

## OpenVR binding decision needs a prototype

The documented [openvr 0.9 high-level wrapper](https://docs.rs/openvr/0.9.0/openvr/) exposes Chaperone but does not expose ChaperoneSetup in its public API listing. The lower-level [VR_IVRChaperoneSetup_FnTable](https://docs.rs/openvr_sys/latest/openvr_sys/struct.VR_IVRChaperoneSetup_FnTable.html) includes the required setup operations. Do not assume that adding the high-level crate completes the integration.

Prefer one OpenVR lifetime owner and one SDK/binding version. Prototype function-table acquisition, calling conventions, matrix conversion, interface availability and shutdown before locking in the adapter. Keep unsafe calls confined to this module. If bindings are insufficient, use a small maintained C/C++ bridge against Valve's SDK rather than duplicating large ABI declarations by hand.

The [rust-openvr build instructions](https://github.com/rust-openvr/rust-openvr) require MSVC, C++ and CMake for its sys build. An all-Rust application can still require native SDK/build tooling. Verify these prerequisites before scaffolding the UI. A desktop iced/Tauri window is not automatically an in-headset OpenVR overlay; any custom VR overlay is a separate rendering feature.

## Local tooling observed

Rustup's active toolchain is `stable-x86_64-pc-windows-msvc`. Cargo, rustc, Git and just are on PATH; just reports 1.58.0. MSVC linker, Windows SDK and CMake have not yet been validated. No toolchains or dependencies were installed in this planning pass.
