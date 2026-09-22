//! One owner of the physical USB port; SimHub sends speed over localhost UDP.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    io::{Read, Write},
    net::UdpSocket,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub const BRIDGE_PORT: u16 = 29814;
pub const HEADERS: [&str; 4] = ["L1", "L2", "R1", "R2"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunMode {
    Always,
    SteamVr,
    Iracing,
}
impl RunMode {
    pub const ALL: [Self; 3] = [Self::Always, Self::SteamVr, Self::Iracing];
    pub fn allows(self, steamvr: bool, iracing: bool) -> bool {
        match self {
            Self::Always => true,
            Self::SteamVr => steamvr,
            Self::Iracing => iracing,
        }
    }
}
impl fmt::Display for RunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Always => "Always (minimum when idle)",
            Self::SteamVr => "While SteamVR is running",
            Self::Iracing => "Only while iRacing simulator is running",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub enabled: bool,
    pub mode: RunMode,
    pub minimum: u16,
    pub maximum: u16,
    pub full_speed_kmh: u16,
    pub port: Option<String>,
    pub left_enabled: bool,
    pub right_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            mode: RunMode::Iracing,
            minimum: 20,
            maximum: 80,
            full_speed_kmh: 180,
            port: None,
            left_enabled: true,
            right_enabled: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "Unsupported wind settings version");
        ensure!(
            self.minimum <= self.maximum && self.maximum <= 100,
            "Minimum must not exceed maximum (0–100%)"
        );
        ensure!(
            self.minimum == 0 || self.minimum >= 5,
            "Minimum must be 0 or at least 5%"
        );
        ensure!(self.maximum >= 5, "Maximum must be at least 5%");
        ensure!(
            (10..=500).contains(&self.full_speed_kmh),
            "Full wind speed must be 10–500 km/h"
        );
        if let Some(port) = &self.port {
            ensure!(
                port.strip_prefix("COM")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())),
                "Select a COM port"
            );
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                let value: Self = serde_json::from_slice(&bytes)?;
                value.validate()?;
                Ok(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path.parent().context("No settings directory")?;
        fs::create_dir_all(parent)?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        tmp.write_all(&serde_json::to_vec_pretty(self)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn demand(&self, steamvr: bool, iracing: bool, speed: Option<f64>) -> [u16; 2] {
        if !self.enabled || !self.mode.allows(steamvr, iracing) {
            return [0, 0];
        }
        let fraction = speed.filter(|v| v.is_finite()).unwrap_or(0.0).max(0.0)
            / f64::from(self.full_speed_kmh);
        let value = ((f64::from(self.minimum)
            + fraction.clamp(0.0, 1.0) * f64::from(self.maximum - self.minimum))
            * 10.0)
            .round() as u16;
        let value = if value < 50 { 0 } else { value };
        [
            if self.left_enabled { value } else { 0 },
            if self.right_enabled { value } else { 0 },
        ]
    }
}

pub fn settings_path(demo: bool) -> PathBuf {
    crate::profile::default_path(demo).with_file_name(if demo {
        "demo-wind.json"
    } else {
        "wind.json"
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Telemetry {
    pub mode: u8,
    pub warnings: u8,
    pub uptime_ms: u32,
    pub accepted: u32,
    pub rejected: u32,
    pub rpm: [u32; 4],
    pub demand: [u16; 4],
}
impl Telemetry {
    pub fn parse(line: &str) -> Option<Self> {
        let fields: Vec<_> = line.trim_end_matches('\r').split(',').collect();
        if fields.len() != 15 || fields[0] != "T" || fields[1] != "2" {
            return None;
        }
        // Strict grammar: no signs, spaces, NaNs or ignored extra fields.
        if fields
            .iter()
            .enumerate()
            .skip(2)
            .any(|(i, f)| f.is_empty() || (i != 3 && !f.bytes().all(|b| b.is_ascii_digit())))
        {
            return None;
        }
        if fields[3].len() != 1 {
            return None;
        }
        let mode = fields[2].parse().ok()?;
        let warnings = u8::from_str_radix(fields[3], 16).ok()?;
        let mut rpm = [0; 4];
        let mut demand = [0; 4];
        for i in 0..4 {
            rpm[i] = fields[7 + i].parse().ok()?;
            demand[i] = fields[11 + i].parse().ok()?;
        }
        if mode > 4
            || warnings > 15
            || rpm.iter().any(|&v| v > 30000)
            || demand.iter().any(|&v| v > 1000)
        {
            return None;
        }
        Some(Self {
            mode,
            warnings,
            uptime_ms: fields[4].parse().ok()?,
            accepted: fields[5].parse().ok()?,
            rejected: fields[6].parse().ok()?,
            rpm,
            demand,
        })
    }
    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            0 => "Stopped",
            1 => "Starting",
            2 => "Running",
            3 => "Command timed out",
            _ => "Fault — reset controller",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeFrame {
    pub version: u8,
    pub running: bool,
    pub speed_kmh: f64,
}
impl BridgeFrame {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let f: Self = serde_json::from_slice(bytes).ok()?;
        (f.version == 1 && f.speed_kmh.is_finite() && (0.0..=1500.0).contains(&f.speed_kmh))
            .then_some(f)
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub connection: String,
    pub error: Option<String>,
    pub ports: Vec<String>,
    pub telemetry: Option<Telemetry>,
    pub telemetry_at: Option<Instant>,
    pub bridge_at: Option<Instant>,
    pub bridge_running: bool,
    pub speed_kmh: f64,
    pub steamvr: bool,
    pub iracing: bool,
    pub demand: [u16; 2],
    pub suspended: bool,
    pub bridge_error: Option<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connection: "Looking for controller…".into(),
            error: None,
            ports: vec![],
            telemetry: None,
            telemetry_at: None,
            bridge_at: None,
            bridge_running: false,
            speed_kmh: 0.0,
            steamvr: false,
            iracing: false,
            demand: [0, 0],
            suspended: false,
            bridge_error: None,
        }
    }
}
impl Snapshot {
    pub fn requested(&self, settings: &Settings) -> [u16; 2] {
        if !self.fresh() || self.suspended {
            return [0, 0];
        }
        let speed = (self.bridge_fresh() && self.bridge_running).then_some(self.speed_kmh);
        settings.demand(self.steamvr, self.iracing, speed)
    }
    pub fn fresh(&self) -> bool {
        self.telemetry_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(3))
    }
    pub fn bridge_fresh(&self) -> bool {
        self.bridge_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
    }
}

enum Command {
    Settings(Settings),
    Suspend(bool),
    Shutdown,
}
pub struct Worker {
    tx: mpsc::Sender<Command>,
    state: Arc<Mutex<Snapshot>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn spawn(settings: Settings, demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let shared = state.clone();
        let join = thread::spawn(move || run(settings, demo, rx, shared));
        Self {
            tx,
            state,
            join: Some(join),
        }
    }
    pub fn configure(&self, settings: Settings) {
        let _ = self.tx.send(Command::Settings(settings));
    }
    pub fn suspend(&self, suspend: bool) {
        let _ = self.tx.send(Command::Suspend(suspend));
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn stop(port: &mut Option<Box<dyn serialport::SerialPort>>) {
    if let Some(p) = port {
        let _ = p.write_all(b"W,0,0\n");
    }
    *port = None;
}

fn run(
    mut settings: Settings,
    demo: bool,
    rx: mpsc::Receiver<Command>,
    state: Arc<Mutex<Snapshot>>,
) {
    let mut view = Snapshot::default();
    let socket = if demo {
        None
    } else {
        match UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, BRIDGE_PORT)) {
            Ok(s) => {
                let _ = s.set_nonblocking(true);
                Some(s)
            }
            Err(e) => {
                view.bridge_error = Some(format!(
                    "SimHub bridge unavailable: {e}. Restart Rig Companion after freeing port {BRIDGE_PORT}."
                ));
                None
            }
        }
    };
    let mut port: Option<Box<dyn serialport::SerialPort>> = None;
    let mut pending = Vec::new();
    let mut discard = false;
    let mut handshake = None;
    let mut identified = false;
    let mut next_scan = Instant::now();
    let mut next_process = Instant::now();
    let mut next_send = Instant::now();
    loop {
        for command in rx.try_iter() {
            match command {
                Command::Shutdown => {
                    stop(&mut port);
                    return;
                }
                Command::Suspend(value) => {
                    view.suspended = value;
                    if value {
                        stop(&mut port);
                        view.telemetry_at = None;
                    }
                }
                Command::Settings(value) => {
                    if value.validate().is_ok() {
                        if value.port != settings.port {
                            stop(&mut port);
                            view.telemetry_at = None;
                            next_scan = Instant::now();
                        }
                        settings = value;
                    }
                }
            }
        }
        let now = Instant::now();
        if !demo && now >= next_process {
            view.steamvr = crate::startup::process_running("vrserver.exe").unwrap_or(false);
            view.iracing = [
                "iRacingSim64DX11.exe",
                "iRacingSim64DX11_EAC.exe",
                "iRacingSim64DX11_EOS.exe",
            ]
            .iter()
            .any(|name| crate::startup::process_running(name).unwrap_or(false));
            next_process = now + Duration::from_secs(2);
        }
        if let Some(socket) = &socket {
            let mut buf = [0; 512];
            // Bound work if another local process floods the bridge.
            for _ in 0..16 {
                match socket.recv_from(&mut buf) {
                    Ok((size, source)) if source.ip().is_loopback() => {
                        if let Some(frame) = BridgeFrame::parse(&buf[..size]) {
                            view.bridge_at = Some(now);
                            view.bridge_running = frame.running;
                            view.speed_kmh = frame.speed_kmh;
                        }
                    }
                    _ => break,
                }
            }
        }
        if !demo && !view.suspended && now >= next_scan {
            match serialport::available_ports() {
                Ok(ports) => {
                    view.ports = ports.into_iter().filter(|p| matches!(&p.port_type, serialport::SerialPortType::UsbPort(u) if u.vid == 0x303a && u.pid == 0x1001)).map(|p| p.port_name).collect();
                    view.ports.sort();
                    if port.is_none() {
                        let name = settings
                            .port
                            .clone()
                            .filter(|p| view.ports.contains(p))
                            .or_else(|| {
                                if settings.port.is_none() && view.ports.len() == 1 {
                                    view.ports.first().cloned()
                                } else {
                                    None
                                }
                            });
                        if let Some(name) = name {
                            match serialport::new(&name, 115200)
                                .timeout(Duration::from_millis(10))
                                .dtr_on_open(false)
                                .open()
                            {
                                Ok(mut p) => {
                                    let _ = p.write_request_to_send(false);
                                    let _ = p.clear(serialport::ClearBuffer::Input);
                                    port = Some(p);
                                    pending.clear();
                                    discard = false;
                                    identified = false;
                                    handshake = Some(now);
                                    view.telemetry = None;
                                    view.telemetry_at = None;
                                    view.connection = format!("Identifying {name}…");
                                }
                                Err(e) => {
                                    view.connection = format!("Cannot open {name}");
                                    view.error =
                                        Some(format!("{e}. Close any other app using this port."));
                                }
                            }
                        } else {
                            view.connection = if view.ports.len() > 1 {
                                "Choose a controller port"
                            } else {
                                "Controller disconnected"
                            }
                            .into();
                        }
                    }
                }
                Err(e) => view.error = Some(format!("USB discovery: {e}")),
            }
            next_scan = now + Duration::from_secs(3);
        }
        let mut failed = None;
        if let Some(p) = &mut port {
            let mut buf = [0; 256];
            match p.read(&mut buf) {
                Ok(n) => {
                    for &byte in &buf[..n] {
                        if byte == b'\n' {
                            if !discard
                                && let Ok(line) = std::str::from_utf8(&pending)
                                && let Some(t) = Telemetry::parse(line)
                            {
                                identified = true;
                                view.telemetry = Some(t);
                                view.telemetry_at = Some(now);
                                view.connection =
                                    format!("Connected · {}", p.name().unwrap_or_default());
                                view.error = None;
                            }
                            pending.clear();
                            discard = false;
                        } else if !discard {
                            if pending.len() >= 160 {
                                pending.clear();
                                discard = true;
                            } else {
                                pending.push(byte);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => failed = Some(format!("USB disconnected: {e}")),
            }
            let fresh = view.fresh();
            view.demand = if identified {
                view.requested(&settings)
            } else {
                [0, 0]
            };
            if identified && now >= next_send {
                let frame = format!("W,{},{}\n", view.demand[0], view.demand[1]);
                if let Err(e) = p.write_all(frame.as_bytes()) {
                    failed = Some(format!("USB write: {e}"));
                }
                next_send = now + Duration::from_millis(100);
            }
            if (!identified
                && handshake.is_some_and(|t| now.duration_since(t) > Duration::from_secs(4)))
                || (identified && !fresh)
            {
                failed = Some(
                    "No fresh RPM telemetry. Update controller firmware or reconnect USB.".into(),
                );
            }
        } else {
            view.demand = [0, 0];
        }
        if let Some(error) = failed {
            if identified {
                stop(&mut port);
            } else {
                port = None;
            }
            identified = false;
            view.telemetry_at = None;
            view.connection = "Controller unavailable".into();
            view.error = Some(error);
            next_scan = now + Duration::from_secs(3);
        }
        if demo {
            view.connection = "Demo · no hardware connection".into();
        }
        if view.suspended {
            view.connection = "USB released · ready for firmware update".into();
        }
        *state.lock().unwrap() = view.clone();
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_gates_minimum_and_speed_bounds() {
        let mut s = Settings {
            enabled: true,
            ..Settings::default()
        };
        assert_eq!(s.demand(true, false, Some(180.0)), [0, 0]);
        assert_eq!(s.demand(false, true, None), [200, 200]);
        assert_eq!(s.demand(false, true, Some(90.0)), [500, 500]);
        assert_eq!(s.demand(false, true, Some(500.0)), [800, 800]);
        s.mode = RunMode::SteamVr;
        assert_eq!(s.demand(false, true, Some(500.0)), [0, 0]);
        assert_eq!(s.demand(true, false, Some(f64::NAN)), [200, 200]);
        s.mode = RunMode::Always;
        s.left_enabled = false;
        assert_eq!(s.demand(false, false, Some(-10.0)), [0, 200]);
        s.enabled = false;
        assert_eq!(s.demand(true, true, Some(200.0)), [0, 0]);
    }
    #[test]
    fn rejects_invalid_settings_preserves_corrupt_file() {
        let mut s = Settings {
            minimum: 90,
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        s.minimum = 1;
        assert!(s.validate().is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wind.json");
        fs::write(&path, b"broken").unwrap();
        assert!(Settings::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        Settings::default().save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
    }
    #[test]
    fn strict_telemetry_and_bridge_parsing() {
        let t = Telemetry::parse("T,2,2,3,1000,20,0,0,0,1200,1230,0,0,300,300").unwrap();
        assert_eq!(t.rpm, [0, 0, 1200, 1230]);
        assert_eq!(t.warnings, 3);
        for bad in [
            "S,1,0,0",
            "T,2,9,0,0,0,0,0,0,0,0,0,0,0,0",
            "T,2,0,0,0,0,0,0,0,0,0,0,0,1001,0",
            "T,2,0,0,-1,0,0,0,0,0,0,0,0,0,0",
        ] {
            assert!(Telemetry::parse(bad).is_none());
        }
        assert!(BridgeFrame::parse(br#"{"version":1,"running":true,"speed_kmh":123.4}"#).is_some());
        for bad in [
            br#"{"version":2,"running":true,"speed_kmh":123}"#.as_slice(),
            br#"{"version":1,"running":true,"speed_kmh":-1}"#,
            br#"{"version":1,"running":true,"speed_kmh":1,"extra":1}"#,
        ] {
            assert!(BridgeFrame::parse(bad).is_none());
        }
    }

    #[test]
    fn stale_bridge_falls_to_minimum_but_stale_controller_stops() {
        let settings = Settings {
            enabled: true,
            mode: RunMode::Always,
            ..Settings::default()
        };
        let mut s = Snapshot {
            telemetry_at: Some(Instant::now()),
            bridge_at: Some(Instant::now()),
            bridge_running: true,
            speed_kmh: 500.0,
            ..Snapshot::default()
        };
        assert_eq!(s.requested(&settings), [800, 800]);
        s.bridge_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(s.requested(&settings), [200, 200]);
        s.telemetry_at = Some(Instant::now() - Duration::from_secs(4));
        assert_eq!(s.requested(&settings), [0, 0]);
        s.telemetry_at = Some(Instant::now());
        s.suspended = true;
        assert_eq!(s.requested(&settings), [0, 0]);
    }
}
