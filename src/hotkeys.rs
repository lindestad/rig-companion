use std::sync::mpsc;
use windows_sys::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_F7, VK_F8,
            VK_F9, VK_F13, VK_F14,
        },
        WindowsAndMessaging::{GetMessageW, MSG, PostThreadMessageW, WM_HOTKEY, WM_QUIT},
    },
};

pub struct Hotkeys {
    thread_id: u32,
    pub events: mpsc::Receiver<u32>,
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
                    let _ = ready_tx.send(Ok(GetCurrentThreadId()));
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        if msg.message == WM_HOTKEY {
                            let _ = events_tx.send(msg.wParam as u32);
                        }
                    }
                    for id in 1..=5 {
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
