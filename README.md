# Tempo — offline game speed control

Tempo is a Windows Tauri desktop app for adjusting an offline game's clock through a timing-hook adapter.

## What it does

- Lists running processes and lets you select one.
- On explicit enable, detects whether the selected game is x86 or x64 and loads the matching timing adapter.
- Scales `QueryPerformanceCounter`, `GetTickCount`, `GetTickCount64`, and `timeGetTime` from 0.25× to 10×, with 1× centered among the presets.
- Sends a heartbeat while connected. If Tempo closes or loses the connection, the adapter returns the multiplier to 1× within about two seconds.
- Global numpad shortcuts are read from Windows key state so they can work while a game has focus: `Alt+Num0` attaches the foreground app, `Alt+Num+` and `Alt+Num-` adjust one preset step, `Alt+NumDel` toggles speed control, and `Alt+Num1` resets to normal speed. With Num Lock off, Num0/Num1/NumDel also work as Insert/End/Delete.

## Limits and safety

- Use only with offline, single-player games. Never use this against an online session or anti-cheat protected mode.
- The adapter supports x86 and x64 Windows processes. Some games use other clocks, block hooks, or may crash; compatibility varies by game.
- Disabling control restores 1× immediately, but the hook DLL stays loaded until the game exits.
- Process injection may be blocked by Windows permissions or security software. Do not elevate Tempo just to bypass a game's protections.
- This has not been verified with Far Cry yet. First try it with a small offline test game and keep your game progress saved.

## Setup and run

Requirements: Node.js/npm, Rust nightly for the GNU Windows host, MinGW-w64 compilers for both x86_64 and i686 Windows GNU targets, and the Windows Tauri prerequisites. `dll-syringe` currently uses nightly-only Rust features, so this project pins the nightly GNU toolchain in `rust-toolchain.toml`. The project uses Tauri 2, Vite, `sysinfo`, `dll-syringe`, and the MinHook library. The x64 controller injects the x86 adapter into 32-bit games using `dll-syringe`'s `into-x86-from-x64` support.

Install the Rust x86 Windows target for the pinned toolchain:

```powershell
rustup target add i686-pc-windows-gnu --toolchain nightly-x86_64-pc-windows-gnu
```

Make sure both `x86_64-w64-mingw32-gcc` and `i686-w64-mingw32-gcc` are available on `PATH`. The build compiles separate x64 and x86 hooks and copies both beside the app.

From PowerShell in `D:\speed`:

```powershell
npm install
npm run tauri dev
```

`npm run tauri dev` builds both hook architectures, starts the Vite server, and launches the development app. The debug EXE depends on that server, so do not launch the debug EXE by itself.

To build a standalone production executable, run:

```powershell
npm run tauri -- build
```

The build runs Vite's production build and separately compiles x64 and x86 speed-hook DLLs, then copies them under content-versioned filenames into `src-tauri/target/release` and writes architecture-specific pointer files for the app. This avoids overwriting a DLL that a game still has loaded. Bundling is disabled in `tauri.conf.json`, so this command creates the executable without an installer.

Launch the release executable with:

```powershell
& .\src-tauri\target\release\tempo-speed-control.exe
```

Keep the EXE beside `tempo_speed_hook_x64.current`, `tempo_speed_hook_x86.current`, and both versioned hook DLLs in the release folder; moving only the EXE prevents the adapter from finding its DLL. The release app does not need Vite running. If the game requires administrator privileges, run the controller at the same privilege level.

The first install/build may fetch the declared frontend and Rust dependencies.
