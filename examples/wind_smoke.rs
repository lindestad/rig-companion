//! Live, stopped-output integration check. Close Rig Companion; leave SimHub running.
use anyhow::{Result, ensure};
use rig_companion::wind::{Settings, Worker};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let _lock = rig_companion::lock_instance(false)?;
    let worker = Worker::spawn(Settings::default(), false);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let s = worker.snapshot();
        if s.fresh()
            && s.bridge_fresh()
            && s.telemetry
                .as_ref()
                .is_some_and(|t| t.accepted > 0 && t.mode == 0)
        {
            println!(
                "{}",
                serde_json::json!({"connection":s.connection,"simhub_heartbeat":true,"telemetry":s.telemetry,"demand":s.demand})
            );
            ensure!(s.demand == [0, 0], "Disabled worker requested airflow");
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "Integration did not become ready: {s:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    worker.suspend(true);
    std::thread::sleep(Duration::from_millis(250));
    let stopped = worker.snapshot();
    ensure!(
        stopped.suspended && !stopped.fresh() && stopped.demand == [0, 0],
        "USB release failed"
    );
    worker.suspend(false);
    let deadline = Instant::now() + Duration::from_secs(6);
    while !worker.snapshot().fresh() {
        ensure!(Instant::now() < deadline, "Controller did not reconnect");
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(worker);
    println!(
        "PASS: live SimHub heartbeat, controller telemetry, stopped outputs, release/reconnect and shutdown"
    );
    Ok(())
}
