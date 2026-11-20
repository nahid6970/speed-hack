# Tempo — offline game speed control

Tempo is a Windows desktop app with a native GPUI interface. It adjusts the clock speed of an offline game through a timing-hook adapter.

## What it does

- Lists visible Windows processes so you can select a game.
- Loads the matching x86 or x64 timing adapter when you enable speed control.
- Scales `QueryPerformanceCounter`, `GetTickCount`, `GetTickCount64`, and `timeGetTime` from 0.25× to 10×.
- Resets the game clock to 1× when you disconnect. The hook DLL stays loaded until the game exits.
- Supports `Alt+Num0` to attach the foreground app, `Num+` / `Num-` to adjust speed, `NumDel` to toggle control, and `Alt+Num1` to reset.

Use this only with offline, single-player games. Compatibility varies; some games use other clocks, block hooks, or may crash. Do not use it in online or anti-cheat protected sessions, and do not elevate Tempo to bypass game protections.

## Requirements

- Windows 10 or newer.
- Windows 10/11 SDK, including `fxc.exe` (the GPUI release shader compiler). Set `GPUI_FXC_PATH` if it is not on `PATH`.
- Rust nightly GNU toolchain (pinned in `rust-toolchain.toml`) and Cargo.
- MinGW-w64 x64 and i686 compilers. The 32-bit compiler is needed to build the x86 timing adapter.
- MSYS2 MINGW32 for the i686 compiler. Install it with `winget install --id MSYS2.MSYS2 -e`, then run `pacman -Syu` and `pacman -S --needed mingw-w64-i686-gcc` in the MSYS2 MINGW32 terminal.
- Node.js/npm for the helper script that copies built hook DLLs beside Tempo.

GPUI uses Win32 and DirectWrite on Windows; no WebView or Tauri installation is needed.

## Run in development

In PowerShell from the project folder, ensure your x64 MinGW `bin` directory appears before `C:\msys64\mingw32\bin` on `PATH`. For example:

```powershell
$env:Path = "C:\Users\nahid\scoop\apps\mingw-msvcrt\current\bin;C:\msys64\mingw32\bin;$env:Path"
npm run build:hook
npm run dev
```

The GPUI app opens as a native desktop window. The first Cargo build downloads GPUI and its dependencies.

## Build a release executable

```powershell
$env:GPUI_FXC_PATH = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.18362.0\x64\fxc.exe"
npm run build:windows
```

The standalone executable is `src-tauri\target\release\tempo-speed-control.exe`. Keep it beside the `tempo_speed_hook_x64.current`, `tempo_speed_hook_x86.current`, and versioned hook DLL files in that release folder. The app needs no running Vite server or WebView runtime.
