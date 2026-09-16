//! SteamVR's default gamepad binding maps right trigger to the gaze pointer click.
use anyhow::{Context, Result};
use vigem_client::{Client, TargetId, XGamepad, Xbox360Wired};

pub struct GazeClick {
    target: Xbox360Wired<Client>,
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
        Ok(Self { target })
    }

    pub fn click(&mut self) -> Result<()> {
        let pressed = XGamepad {
            right_trigger: 255,
            ..Default::default()
        };
        let press_result = self.target.update(&pressed);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let release_result = self.target.update(&XGamepad::default());
        press_result.context("Could not press the virtual trigger")?;
        release_result.context("Could not release the virtual trigger")?;
        Ok(())
    }
}

impl Drop for GazeClick {
    fn drop(&mut self) {
        let _ = self.target.update(&XGamepad::default());
        let _ = self.target.unplug();
    }
}
