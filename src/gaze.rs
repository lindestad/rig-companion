//! SteamVR's default gamepad binding maps right trigger to the gaze pointer click.
use anyhow::{Context, Result};
use vigem_client::{Client, TargetId, XGamepad, Xbox360Wired};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

pub struct GazeClick {
    target: Xbox360Wired<Client>,
    index: u32,
}

impl GazeClick {
    pub fn connect() -> Result<Self> {
        let client = Client::connect().context("Cannot connect to ViGEmBus for VR click")?;
        let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
        target.plugin().context("Cannot attach virtual gamepad")?;
        target
            .wait_ready()
            .context("Virtual gamepad is not ready")?;
        target.update(&XGamepad::default())?;
        // Give SteamVR time to discover the newly attached XInput device.
        std::thread::sleep(std::time::Duration::from_millis(700));
        let index = target
            .get_user_index()
            .context("Cannot identify virtual XInput slot")?;
        Ok(Self { target, index })
    }

    pub fn trigger_state(&self) -> Result<(u32, u8)> {
        let mut state = XINPUT_STATE::default();
        // SAFETY: XInput writes a correctly sized state structure for our own slot.
        let status = unsafe { XInputGetState(self.index, &mut state) };
        anyhow::ensure!(
            status == 0,
            "Windows cannot read virtual XInput slot {} (error {status})",
            self.index
        );
        Ok((self.index, state.Gamepad.bRightTrigger))
    }

    pub fn click(&mut self) -> Result<()> {
        let pressed = XGamepad {
            right_trigger: 255,
            ..Default::default()
        };
        let press_result = self.target.update(&pressed);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let observed = self.trigger_state();
        let release_result = self.target.update(&XGamepad::default());
        press_result.context("Could not press the virtual trigger")?;
        release_result.context("Could not release the virtual trigger")?;
        let (_, trigger) = observed?;
        anyhow::ensure!(
            trigger == 255,
            "Virtual trigger was written but Windows reported {trigger}; check input filtering"
        );
        Ok(())
    }
}

impl Drop for GazeClick {
    fn drop(&mut self) {
        let _ = self.target.update(&XGamepad::default());
        let _ = self.target.unplug();
    }
}
