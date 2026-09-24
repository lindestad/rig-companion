use std::sync::mpsc;
use windows_sys::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
            VK_F7, VK_F8, VK_F9, VK_F13, VK_F14, VK_F15, VK_F16, VK_F17, VK_F18,
        },
        WindowsAndMessaging::{
            GetMessageW, KillTimer, MSG, PostThreadMessageW, SetTimer, WM_HOTKEY, WM_QUIT, WM_TIMER,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Shortcut(u32),
    GazeDown,
    GazeUp,
    Joystick(i8),
}

pub struct Hotkeys {
    thread_id: u32,
    pub events: mpsc::Receiver<HotkeyEvent>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Hotkeys {
    pub fn start() -> anyhow::Result<Self> {
        let (events_tx, events) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || {
                // SAFETY: registrations and message pump live on this dedicated owning thread.
                unsafe {
                    let keys = [
                        (VK_F8, MOD_CONTROL | MOD_ALT),
                        (VK_F9, MOD_CONTROL | MOD_ALT),
                        (VK_F7, MOD_CONTROL | MOD_ALT),
                        (VK_F13, 0),
                        (VK_F14, 0),
                        (VK_F15, 0),
                        (VK_F16, 0),
                        (VK_F17, 0),
                        (VK_F18, 0),
                    ];
                    for (index, (key, modifiers)) in keys.iter().enumerate() {
                        if RegisterHotKey(
                            std::ptr::null_mut(),
                            index as i32 + 1,
                            *modifiers | MOD_NOREPEAT,
                            *key as u32,
                        ) == 0
                        {
                            for id in 1..=index as i32 {
                                UnregisterHotKey(std::ptr::null_mut(), id);
                            }
                            let _ = ready_tx.send(Err(
                                "A global shortcut is already in use. Shortcuts remain disabled."
                                    .to_string(),
                            ));
                            return;
                        }
                    }
                    // A windowless timer gets a system-assigned ID; use the returned value.
                    let timer = SetTimer(std::ptr::null_mut(), 0, 10, None);
                    if timer == 0 {
                        for id in 1..=9 {
                            UnregisterHotKey(std::ptr::null_mut(), id);
                        }
                        let _ = ready_tx.send(Err("Could not watch held-key releases.".into()));
                        return;
                    }
                    let _ = ready_tx.send(Ok(GetCurrentThreadId()));
                    let mut gaze_down = false;
                    let mut joystick_direction = 0;
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        if msg.message == WM_HOTKEY {
                            if msg.wParam == 5 {
                                if !gaze_down {
                                    gaze_down = true;
                                    let _ = events_tx.send(HotkeyEvent::GazeDown);
                                }
                            } else if msg.wParam != 8 && msg.wParam != 9 {
                                let _ = events_tx.send(HotkeyEvent::Shortcut(msg.wParam as u32));
                            }
                        } else if msg.message == WM_TIMER && msg.wParam == timer {
                            if gaze_down && GetAsyncKeyState(VK_F14.into()) >= 0 {
                                gaze_down = false;
                                let _ = events_tx.send(HotkeyEvent::GazeUp);
                            }
                            let up = GetAsyncKeyState(VK_F17.into()) < 0;
                            let down = GetAsyncKeyState(VK_F18.into()) < 0;
                            let next_direction = (up as i8) - (down as i8);
                            if next_direction != joystick_direction {
                                joystick_direction = next_direction;
                                let _ = events_tx.send(HotkeyEvent::Joystick(next_direction));
                            }
                        }
                    }
                    if gaze_down {
                        let _ = events_tx.send(HotkeyEvent::GazeUp);
                    }
                    if joystick_direction != 0 {
                        let _ = events_tx.send(HotkeyEvent::Joystick(0));
                    }
                    KillTimer(std::ptr::null_mut(), timer);
                    for id in 1..=9 {
                        UnregisterHotKey(std::ptr::null_mut(), id);
                    }
                }
            })?;
        let thread_id = ready_rx.recv()?.map_err(anyhow::Error::msg)?;
        Ok(Self {
            thread_id,
            events,
            join: Some(join),
        })
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        // SAFETY: only signals our own thread's existing message queue to exit.
        unsafe {
            PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    };

    #[test]
    #[ignore = "requires an interactive Windows desktop with unused global shortcuts"]
    fn f14_press_and_release_are_both_delivered() {
        let hotkeys = Hotkeys::start().unwrap();
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F14,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        // SAFETY: SendInput receives initialized keyboard input structures.
        unsafe {
            assert_eq!(SendInput(1, &key(0), size_of::<INPUT>() as i32), 1);
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
        unsafe {
            assert_eq!(
                SendInput(1, &key(KEYEVENTF_KEYUP), size_of::<INPUT>() as i32),
                1
            );
        }
        assert_eq!(
            hotkeys
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::GazeDown
        );
        assert_eq!(
            hotkeys
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::GazeUp
        );
    }

    #[test]
    #[ignore = "requires an interactive Windows desktop with unused global shortcuts"]
    fn joystick_keys_return_to_neutral_on_release() {
        let hotkeys = Hotkeys::start().unwrap();
        let key = |vk, flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        for (vk, expected) in [(VK_F17, 1), (VK_F18, -1)] {
            unsafe {
                assert_eq!(SendInput(1, &key(vk, 0), size_of::<INPUT>() as i32), 1);
            }
            assert_eq!(
                hotkeys
                    .events
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap(),
                HotkeyEvent::Joystick(expected)
            );
            unsafe {
                assert_eq!(
                    SendInput(1, &key(vk, KEYEVENTF_KEYUP), size_of::<INPUT>() as i32),
                    1
                );
            }
            assert_eq!(
                hotkeys
                    .events
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap(),
                HotkeyEvent::Joystick(0)
            );
        }
    }
}
