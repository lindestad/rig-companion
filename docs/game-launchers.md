# Game launchers

The top of the main page has logo buttons for iRacing, Content Manager, Assetto Corsa Rally, Assetto Corsa EVO and Le Mans Ultimate. Camera lives below calibration, beside the dashboard and gaze-click controls.

Each launch checks exact process names for SimPro Manager 3 and SimHub, starts missing programs and waits up to 30 seconds for each process. iRacing also checks MAIRA. A failed prerequisite stops the launch and displays the error. Existing processes are reused. Buttons are disabled during launch and for five seconds afterward to prevent repeated requests. Quit cancels any remaining launch steps; it leaves these helper applications and Pimax running.

Defaults were resolved from this PC's installed shortcuts and Steam manifests. SimPro starts through SIMAGIC's `Daemon/simdaemon.exe` and waits for `simpro3.exe`. SimHub uses `SimHubWPF.exe`. iRacing opens `iRacing/ui/iRacingUI.exe`; Content Manager opens `E:/Games/Assetto Corsa Content Manager/Content Manager.exe`. Steam app IDs are Rally 3917090, EVO 3058630 and LMU 2399420. Steam handles updates and launch-option dialogs; a launch request does not establish that a game is ready to play.

Paths and process names can be edited in `%LOCALAPPDATA%/RigCompanion/data/game-launchers.json`, created on first use. It is reloaded for every launch. Each target has type `exe` with an absolute path, or `steam` with an app ID. Windows handles any permission prompt required by an executable.

`rigctl launcher-status iracing` reports the configured launch order, process state and executable existence without launching anything. Other values: `content-manager`, `rally`, `evo`, `lmu`. It can run beside the GUI.

Verification: all five installed targets/configuration plans checked; tests cover dependency order, MAIRA scope, reusing running processes, failure stopping the game, cancellation and avoiding duplicates. Desktop automation could not attach to the app's window (reported the same expected/current owner while rejecting it), so final visual inspection and live tile clicks remain unverified. No games were launched by the verification commands.
