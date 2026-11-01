use dll_syringe::{process::OwnedProcess, Syringe};
// Referencing the crate keeps Cargo building its cdylib artifact, which is
// injected into the selected game process at runtime.
use tempo_speed_hook as _;
use serde::Serialize;
use std::{fs, path::PathBuf, thread, time::{Duration, Instant}};
use sysinfo::System;

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

fn control_path(pid: u32) -> PathBuf { std::env::temp_dir().join(format!("tempo-speed-{pid}.cfg")) }
fn status_path(pid: u32) -> PathBuf { std::env::temp_dir().join(format!("tempo-speed-{pid}.status")) }
fn write_control(pid: u32, speed: f64) -> Result<(), String> {
    fs::write(control_path(pid), format!("{speed:.4}\n")).map_err(|e| format!("Could not update the game speed: {e}"))
}
fn hook_dll_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate Tempo: {e}"))?;
    let exe_dir = exe.parent().ok_or("Tempo executable directory is unavailable")?;
    let candidates = [
        exe_dir.join("deps").join("tempo_speed_hook.dll"),
        exe_dir.join("tempo_speed_hook.dll"),
    ];
    candidates.into_iter().find(|path| path.is_file()).ok_or_else(|| {
        "Speed-hook DLL is missing from the build. Restart `npm run tauri dev` so Cargo builds the adapter.".into()
    })
}

#[tauri::command]
fn connect_speed_adapter(pid: u32, speed: f64) -> Result<String, String> {
    if !(0.25..=5.0).contains(&speed) || !speed.is_finite() { return Err("Choose a speed between 0.25× and 5×.".into()); }
    let process = OwnedProcess::from_pid(pid).map_err(|e| format!("Cannot open selected process: {e}"))?;
    let status_file = status_path(pid);
    let already_ready = fs::read_to_string(&status_file).map(|s| s.trim() == "ready").unwrap_or(false);
    write_control(pid, speed)?;
    if already_ready { return Ok("Speed adapter is already active for this process.".into()); }

    let dll = hook_dll_path()?;
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
    if !speed.is_finite() || !(0.25..=5.0).contains(&speed) { return Err("Speed must be between 0.25× and 5×.".into()); }
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
        .invoke_handler(tauri::generate_handler![list_processes, connect_speed_adapter, set_speed_for_process, disconnect_speed_adapter])
        .run(tauri::generate_context!())
        .expect("error while running Tempo");
}


