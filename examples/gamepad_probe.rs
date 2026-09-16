//! Attach and release a neutral controller without sending any click.
fn main() -> anyhow::Result<()> {
    let _gamepad = rig_companion::gaze::GazeClick::connect()?;
    println!("ViGEmBus virtual controller attached and ready; no buttons pressed.");
    Ok(())
}
