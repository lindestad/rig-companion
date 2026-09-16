//! Attach and release a neutral controller without sending any click.
fn main() -> anyhow::Result<()> {
    let mut gamepad = rig_companion::gaze::GazeClick::connect()?;
    println!(
        "Neutral virtual controller XInput (slot, trigger): {:?}; no buttons pressed.",
        gamepad.trigger_state()?
    );
    if std::env::args().any(|a| a == "--pulse") {
        gamepad.click()?;
        println!(
            "Trigger press read back as 255; released state: {:?}",
            gamepad.trigger_state()?
        );
    }
    Ok(())
}
