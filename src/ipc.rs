use anyhow::{Result, ensure};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        CreateEventW, EVENT_MODIFY_STATE, OpenEventW, SetEvent, WaitForSingleObject,
    },
};
pub struct QuitSignal(HANDLE);
fn name(demo: bool) -> Vec<u16> {
    format!(
        "Local\\RigCompanion.Quit.{}\0",
        if demo { "Demo" } else { "Live" }
    )
    .encode_utf16()
    .collect()
}
impl QuitSignal {
    pub fn create(demo: bool) -> Result<Self> {
        let h = unsafe { CreateEventW(std::ptr::null(), 0, 0, name(demo).as_ptr()) };
        ensure!(!h.is_null(), "Cannot create quit signal");
        Ok(Self(h))
    }
    pub fn requested(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}
impl Drop for QuitSignal {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn request_quit(demo: bool) -> Result<()> {
    unsafe {
        let h = OpenEventW(EVENT_MODIFY_STATE, 0, name(demo).as_ptr());
        ensure!(
            !h.is_null(),
            "No running Rig Companion instance accepts quit requests"
        );
        let ok = SetEvent(h);
        CloseHandle(h);
        ensure!(ok != 0, "Cannot signal quit");
    }
    Ok(())
}
