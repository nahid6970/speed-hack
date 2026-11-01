use dll_syringe::{process::OwnedProcess, Syringe};
// Referencing the crate keeps Cargo building its cdylib artifact, which is
// injected into the selected game process at runtime.
use tempo_speed_hook as _;
use serde::Serialize;
use std::{fs, fs::File, io::{Read, Seek, SeekFrom}, path::PathBuf, thread, time::{Duration, Instant}};
use sysinfo::System;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ADD, VK_DECIMAL, VK_DELETE, VK_END, VK_INSERT, VK_MENU, VK_NUMPAD0,
    VK_NUMPAD1, VK_SUBTRACT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunningProcess {
    pid: u32,
    name: String,
    executable: Option<String>,
}

#[tauri::command]
fn list_processes() -> Result<Vec<RunningProcess>, String> {
    let mut system = System::new_all();
    system.refresh_all();
    let own_pid = std::process::id();
    let mut processes: Vec<RunningProcess> = system.processes().iter().filter_map(|(pid, process)| {
        let process_id = pid.as_u32();
        if process_id == own_pid { return None; }
        let name = process.name().trim().to_owned();
        if name.is_empty() { return None; }
        let executable = process.exe().map(|path| path.to_string_lossy().into_owned());
        executable.as_ref()?;
        Some(RunningProcess { pid: process_id, name, executable })
    }).collect();
    processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.pid.cmp(&b.pid)));
    Ok(processes)
}

#[tauri::command]
fn get_foreground_process() -> Result<RunningProcess, String> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() { return Err("Could not identify the foreground app.".into()); }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut pid); }
    if pid == 0 { return Err("Could not identify the foreground app process.".into()); }
    list_processes()?.into_iter().find(|process| process.pid == pid)
        .ok_or_else(|| "The foreground window is not a selectable app. Press Alt+Num0 while the game is active.".into())
}

fn key_is_down(key: u16) -> bool {
    // GetAsyncKeyState's high bit is set while the key is currently held.
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

fn start_hotkey_poller(app: AppHandle) {
    thread::spawn(move || {
        let mut was_pressed = [false; 5];
        let events = ["attach", "increase", "decrease", "toggle", "reset"];
        loop {
            let alt_down = key_is_down(VK_MENU);
            let pressed = [
                alt_down && (key_is_down(VK_NUMPAD0) || key_is_down(VK_INSERT)),
                alt_down && key_is_down(VK_ADD),
                alt_down && key_is_down(VK_SUBTRACT),
                alt_down && (key_is_down(VK_DECIMAL) || key_is_down(VK_DELETE)),
                alt_down && (key_is_down(VK_NUMPAD1) || key_is_down(VK_END)),
            ];
            for (index, is_pressed) in pressed.iter().enumerate() {
                if *is_pressed && !was_pressed[index] {
                    let _ = app.emit("game-speed-hotkey", events[index]);
                }
            }
            was_pressed = pressed;
            thread::sleep(Duration::from_millis(8));
        }
    });
}

fn control_path(pid: u32) -> PathBuf { std::env::temp_dir().join(format!("tempo-speed-{pid}.cfg")) }
fn status_path(pid: u32) -> PathBuf { std::env::temp_dir().join(format!("tempo-speed-{pid}.status")) }
fn write_control(pid: u32, speed: f64) -> Result<(), String> {
    fs::write(control_path(pid), format!("{speed:.4}\n")).map_err(|e| format!("Could not update the game speed: {e}"))
}
#[derive(Clone, Copy)]
enum ProcessArchitecture { X86, X64 }

fn process_architecture(pid: u32) -> Result<ProcessArchitecture, String> {
    let executable = list_processes()?.into_iter().find(|process| process.pid == pid)
        .and_then(|process| process.executable)
        .ok_or("Cannot locate the selected process executable")?;
    let mut file = File::open(&executable).map_err(|e| format!("Cannot inspect the selected executable: {e}"))?;
    let mut dos_header = [0u8; 64];
    file.read_exact(&mut dos_header).map_err(|e| format!("Cannot read the selected executable header: {e}"))?;
    if &dos_header[0..2] != b"MZ" { return Err("The selected process is not a supported Windows executable".into()); }
    let pe_offset = u32::from_le_bytes(dos_header[0x3c..0x40].try_into().unwrap()) as u64;
    file.seek(SeekFrom::Start(pe_offset)).map_err(|e| format!("Cannot inspect executable architecture: {e}"))?;
    let mut pe_header = [0u8; 6];
    file.read_exact(&mut pe_header).map_err(|e| format!("Cannot read executable architecture: {e}"))?;
    if &pe_header[0..4] != b"PE\0\0" { return Err("The selected process has an invalid executable header".into()); }
    match u16::from_le_bytes([pe_header[4], pe_header[5]]) {
        0x014c => Ok(ProcessArchitecture::X86),
        0x8664 => Ok(ProcessArchitecture::X64),
        _ => Err("Only x86 and x64 Windows games are supported".into()),
    }
}

fn hook_dll_path(architecture: ProcessArchitecture) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate Tempo: {e}"))?;
    let exe_dir = exe.parent().ok_or("Tempo executable directory is unavailable")?;
    let (arch_name, marker) = match architecture {
        ProcessArchitecture::X86 => ("x86", "tempo_speed_hook_x86.current"),
        ProcessArchitecture::X64 => ("x64", "tempo_speed_hook_x64.current"),
    };
    let versioned_dll = fs::read_to_string(exe_dir.join(marker))
        .ok()
        .map(|filename| exe_dir.join(filename.trim()));
    let mut candidates = versioned_dll.into_iter().chain([
        exe_dir.join(format!("tempo_speed_hook_{arch_name}.dll")),
        exe_dir.join("deps").join(format!("tempo_speed_hook_{arch_name}.dll")),
    ]);
    candidates.find(|path| path.is_file()).ok_or_else(|| {
        "Speed-hook DLL is missing. Run `npm run tauri dev` or rebuild with `npm run tauri -- build`.".into()
    })
}

#[tauri::command]
fn connect_speed_adapter(pid: u32, speed: f64) -> Result<String, String> {
    if !(0.25..=10.0).contains(&speed) || !speed.is_finite() { return Err("Choose a speed between 0.25 and 10.".into()); }
    let architecture = process_architecture(pid)?;
    let process = OwnedProcess::from_pid(pid).map_err(|e| format!("Cannot open selected process: {e}"))?;
    let status_file = status_path(pid);
    let already_ready = fs::read_to_string(&status_file).map(|s| s.trim() == "ready").unwrap_or(false);
    write_control(pid, speed)?;
    if already_ready { return Ok("Speed adapter is already active for this process.".into()); }

    let dll = hook_dll_path(architecture)?;
    let syringe = Syringe::for_process(process);
    syringe.find_or_inject(&dll).map_err(|e| format!("Could not load the timing adapter into the selected process: {e}"))?;

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        if let Ok(status) = fs::read_to_string(&status_file) {
            let status = status.trim();
            if status == "ready" { return Ok("Timing adapter connected.".into()); }
            if let Some(error) = status.strip_prefix("error:") { return Err(format!("The game rejected the timing adapter: {error}")); }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("The adapter was loaded, but its timer hooks did not report ready. This game may not support the current hook set.".into())
}

#[tauri::command]
fn set_speed_for_process(pid: u32, speed: f64, enabled: bool) -> Result<(), String> {
    if !speed.is_finite() || !(0.25..=10.0).contains(&speed) { return Err("Speed must be between 0.25 and 10.".into()); }
    let status = fs::read_to_string(status_path(pid)).map_err(|_| "Connect the timing adapter first.".to_string())?;
    if status.trim() != "ready" { return Err("The timing adapter is not ready for this process.".into()); }
    write_control(pid, if enabled { speed } else { 1.0 })
}

#[tauri::command]
fn disconnect_speed_adapter(pid: u32) -> Result<(), String> {
    // The in-process hooks stay loaded until the game exits, but immediately return its clock to 1×.
    write_control(pid, 1.0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            start_hotkey_poller(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_processes, get_foreground_process, connect_speed_adapter, set_speed_for_process, disconnect_speed_adapter])
        .run(tauri::generate_context!())
        .expect("error while running Game Speed Control");
}


