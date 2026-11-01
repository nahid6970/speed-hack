# Tempo — offline game speed control

Tempo is a Windows Tauri desktop app for adjusting an offline game's clock through a timing-hook adapter.

## What it does

- Lists running processes and lets you select one.
- On explicit enable, loads the x64 timing adapter into the selected process.
- Scales `QueryPerformanceCounter`, `GetTickCount`, `GetTickCount64`, and `timeGetTime` from 0.25× to 5×.
- Sends a heartbeat while connected. If Tempo closes or loses the connection, the adapter returns the multiplier to 1× within about two seconds.

## Limits and safety

- Use only with offline, single-player games. Never use this against an online session or anti-cheat protected mode.
- The adapter currently targets x64 Windows processes. Some games use other clocks, block hooks, or may crash; compatibility varies by game.
- Disabling control restores 1× immediately, but the hook DLL stays loaded until the game exits.
- Process injection may be blocked by Windows permissions or security software. Do not elevate Tempo just to bypass a game's protections.
- This has not been verified with Far Cry 4 yet. First try it with a small offline test game and keep your game progress saved.

## Setup and run

Requirements: Node.js/npm, Rust nightly for the GNU Windows host, MinGW-w64, and the Windows Tauri prerequisites. `dll-syringe` currently uses nightly-only Rust features, so this project pins the nightly GNU toolchain in `rust-toolchain.toml`. The project uses Tauri 2, Vite, `sysinfo`, `dll-syringe`, and the MinHook library.

From PowerShell in `D:\speed`:

```powershell
npm install
npm run tauri dev
```

The first run fetches the declared frontend and Rust dependencies. Package installation and build commands are left for you to run.
