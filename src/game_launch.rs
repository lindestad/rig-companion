//! Launch games only after their rig software is running.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Game {
    Iracing,
    ContentManager,
    Rally,
    Evo,
    Lmu,
}
impl Game {
    pub const ALL: [Self; 5] = [
        Self::Iracing,
        Self::ContentManager,
        Self::Rally,
        Self::Evo,
        Self::Lmu,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Iracing => "iRacing",
            Self::ContentManager => "Content Manager",
            Self::Rally => "AC Rally",
            Self::Evo => "AC EVO",
            Self::Lmu => "Le Mans Ultimate",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Target {
    Exe(PathBuf),
    Steam(u32),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub name: String,
    pub processes: Vec<String>,
    pub target: Target,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub simpro: Program,
    pub simhub: Program,
    pub maira: Program,
    pub games: BTreeMap<Game, Program>,
}
fn exe(name: &str, path: impl Into<PathBuf>, process: &str) -> Program {
    Program {
        name: name.into(),
        processes: vec![process.into()],
        target: Target::Exe(path.into()),
    }
}
impl Default for Config {
    fn default() -> Self {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            simpro: exe(
                "SimPro Manager 3",
                r"C:\Program Files (x86)\SIMAGIC\Daemon\simdaemon.exe",
                "simpro3.exe",
            ),
            simhub: exe(
                "SimHub",
                r"C:\Program Files (x86)\SimHub\SimHubWPF.exe",
                "SimHubWPF.exe",
            ),
            maira: exe(
                "MAIRA",
                local.join(
                    "Programs/Marvins Awesome iRacing App - Refactored/MarvinsAIRARefactored.exe",
                ),
                "MarvinsAIRARefactored.exe",
            ),
            games: BTreeMap::from([
                (
                    Game::Iracing,
                    exe(
                        "iRacing launcher",
                        r"C:\Program Files (x86)\iRacing\ui\iRacingUI.exe",
                        "iRacingUI.exe",
                    ),
                ),
                (
                    Game::ContentManager,
                    exe(
                        "Content Manager",
                        r"E:\Games\Assetto Corsa Content Manager\Content Manager.exe",
                        "Content Manager.exe",
                    ),
                ),
                (
                    Game::Rally,
                    Program {
                        name: "Assetto Corsa Rally".into(),
                        processes: vec!["acr.exe".into()],
                        target: Target::Steam(3917090),
                    },
                ),
                (
                    Game::Evo,
                    Program {
                        name: "Assetto Corsa EVO".into(),
                        processes: vec!["AssettoCorsaEVO.exe".into()],
                        target: Target::Steam(3058630),
                    },
                ),
                (
                    Game::Lmu,
                    Program {
                        name: "Le Mans Ultimate".into(),
                        processes: vec![
                            "Le Mans Ultimate.exe".into(),
                            "Launch Le Mans Ultimate.exe".into(),
                        ],
                        target: Target::Steam(2399420),
                    },
                ),
            ]),
        }
    }
}
pub fn config_path() -> PathBuf {
    crate::profile::default_path(false).with_file_name("game-launchers.json")
}
pub fn config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        return serde_json::from_slice(&std::fs::read(&path)?)
            .context("Invalid game-launchers.json");
    }
    let config = Config::default();
    std::fs::create_dir_all(path.parent().context("Missing config directory")?)?;
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    file.write_all(&serde_json::to_vec_pretty(&config)?)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(&path)
        .map_err(|e| e.error)
        .context("Cannot create game-launchers.json")?;
    Ok(config)
}
pub fn plan(config: &Config, game: Game) -> Result<Vec<&Program>> {
    let mut items = vec![&config.simpro, &config.simhub];
    if game == Game::Iracing {
        items.push(&config.maira);
    }
    items.push(
        config
            .games
            .get(&game)
            .context("Game missing from game-launchers.json")?,
    );
    Ok(items)
}
trait Host {
    fn running(&mut self, program: &Program) -> Result<bool>;
    fn start(&mut self, program: &Program, prerequisite: bool) -> Result<()>;
    fn wait(&mut self, program: &Program, cancelled: &AtomicBool) -> Result<()>;
}
struct Windows;
impl Host for Windows {
    fn running(&mut self, p: &Program) -> Result<bool> {
        ensure!(
            !p.processes.is_empty(),
            "No process names configured for {}",
            p.name
        );
        for name in &p.processes {
            if crate::startup::process_running(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn start(&mut self, p: &Program, prerequisite: bool) -> Result<()> {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let (target, dir) = match &p.target {
            Target::Exe(path) => {
                ensure!(
                    path.is_file(),
                    "{} executable missing: {}. Edit {}.",
                    p.name,
                    path.display(),
                    config_path().display()
                );
                (
                    path.to_string_lossy().into_owned(),
                    path.parent().map(|p| p.to_string_lossy().into_owned()),
                )
            }
            Target::Steam(id) => {
                ensure!(*id > 0, "Invalid Steam app ID");
                (format!("steam://rungameid/{id}"), None)
            }
        };
        ensure!(!target.contains('\0'), "Invalid launch target");
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let target = wide(&target);
        let dir = dir.map(|s| wide(&s));
        let verb = wide("open");
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
                if prerequisite { 7 } else { 1 },
            )
        };
        ensure!(
            result as isize > 32,
            "Windows could not launch {} (code {}).",
            p.name,
            result as isize
        );
        Ok(())
    }
    fn wait(&mut self, p: &Program, cancelled: &AtomicBool) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            ensure!(!cancelled.load(Ordering::Relaxed), "Launch cancelled");
            if self.running(p)? {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "{} did not start within 30 seconds. Check its window or any Windows permission prompt, then retry.",
                p.name
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
fn execute(
    host: &mut impl Host,
    items: &[&Program],
    cancelled: &AtomicBool,
    mut progress: impl FnMut(String),
) -> Result<String> {
    ensure!(!items.is_empty(), "Empty launch plan");
    for (index, p) in items.iter().enumerate() {
        ensure!(!cancelled.load(Ordering::Relaxed), "Launch cancelled");
        let prerequisite = index + 1 < items.len();
        if host.running(p)? {
            progress(format!("{} is running", p.name));
            if !prerequisite {
                return Ok(format!("{} is already running", p.name));
            }
            continue;
        }
        progress(format!("Starting {}…", p.name));
        host.start(p, prerequisite)?;
        if prerequisite {
            host.wait(p, cancelled)?;
        }
    }
    Ok(format!("{} launch requested", items.last().unwrap().name))
}
pub fn launch(game: Game, cancelled: &AtomicBool, progress: impl FnMut(String)) -> Result<String> {
    let config = config()?;
    execute(&mut Windows, &plan(&config, game)?, cancelled, progress)
}
pub fn status(game: Game) -> Result<serde_json::Value> {
    let c = config()?;
    let items=plan(&c,game)?.iter().map(|p|Ok(serde_json::json!({"name":p.name,"target":p.target,"running":Windows.running(p)?,"executable_exists":match &p.target{Target::Exe(path)=>Some(path.is_file()),Target::Steam(_)=>None}}))).collect::<Result<Vec<_>>>()?;
    Ok(serde_json::json!({"game":game,"config":config_path(),"plan":items}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        running: Vec<String>,
        started: Vec<String>,
        fail: Option<String>,
    }
    impl Host for Fake {
        fn running(&mut self, p: &Program) -> Result<bool> {
            Ok(self.running.contains(&p.name))
        }
        fn start(&mut self, p: &Program, _: bool) -> Result<()> {
            self.started.push(p.name.clone());
            Ok(())
        }
        fn wait(&mut self, p: &Program, _: &AtomicBool) -> Result<()> {
            ensure!(self.fail.as_ref() != Some(&p.name), "Not ready");
            self.running.push(p.name.clone());
            Ok(())
        }
    }
    #[test]
    fn every_game_waits_for_rig_software_and_only_iracing_needs_maira() {
        let c = Config::default();
        for game in Game::ALL {
            let mut host = Fake::default();
            execute(
                &mut host,
                &plan(&c, game).unwrap(),
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            assert_eq!(&host.started[..2], &["SimPro Manager 3", "SimHub"]);
            assert_eq!(
                host.started.contains(&"MAIRA".into()),
                game == Game::Iracing
            );
            assert_eq!(host.started.last(), Some(&c.games[&game].name));
        }
    }
    #[test]
    fn running_dependencies_are_reused_and_failure_prevents_game_start() {
        let c = Config::default();
        let mut host = Fake {
            running: vec![c.simpro.name.clone()],
            fail: Some(c.simhub.name.clone()),
            ..Default::default()
        };
        assert!(
            execute(
                &mut host,
                &plan(&c, Game::Iracing).unwrap(),
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(host.started, vec!["SimHub"]);
    }
    #[test]
    fn cancellation_and_already_running_game_do_not_start_duplicates() {
        let c = Config::default();
        let plan = plan(&c, Game::Iracing).unwrap();
        let mut host = Fake {
            running: plan.iter().map(|p| p.name.clone()).collect(),
            ..Default::default()
        };
        execute(&mut host, &plan, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(host.started.is_empty());
        assert!(execute(&mut Fake::default(), &plan, &AtomicBool::new(true), |_| {}).is_err());
    }
}
