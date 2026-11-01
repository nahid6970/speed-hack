use minhook_sys::{
    MH_CreateHookApiEx, MH_EnableHook, MH_Initialize, MH_OK,
};
use std::{
    ffi::{c_char, c_void, CString},
    fs,
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{HINSTANCE, TRUE},
    System::{
        LibraryLoader::{DisableThreadLibraryCalls, LoadLibraryW},
        Performance::{QueryPerformanceFrequency},
        Threading::{CreateThread, GetCurrentProcessId},
    },
};

// Win32 BOOL is a signed 32-bit integer.
type BOOL = i32;

type QpcFn = unsafe extern "system" fn(*mut i64) -> BOOL;
type Tick64Fn = unsafe extern "system" fn() -> u64;
type Tick32Fn = unsafe extern "system" fn() -> u32;
type TimeGetTimeFn = unsafe extern "system" fn() -> u32;

static ORIGINAL_QPC: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_TICK64: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_TICK32: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_TIMEGET: AtomicU64 = AtomicU64::new(0);
static SPEED_BITS: AtomicU64 = AtomicU64::new(1.0f64.to_bits());
static CLOCK: OnceLock<Mutex<ClockState>> = OnceLock::new();

#[derive(Clone, Copy)]
struct ClockState {
    frequency: i64,
    base_real_qpc: i64,
    last_real_qpc: i64,
    virtual_qpc: f64,
    base_tick64: u64,
    base_tick32: u32,
    base_timeget: u32,
}

fn speed() -> f64 {
    f64::from_bits(SPEED_BITS.load(Ordering::Relaxed)).clamp(0.25, 5.0)
}

unsafe fn original_qpc() -> Option<QpcFn> {
    let value = ORIGINAL_QPC.load(Ordering::Acquire);
    (value != 0).then(|| std::mem::transmute::<usize, QpcFn>(value as usize))
}
unsafe fn original_tick64() -> Option<Tick64Fn> {
    let value = ORIGINAL_TICK64.load(Ordering::Acquire);
    (value != 0).then(|| std::mem::transmute::<usize, Tick64Fn>(value as usize))
}
unsafe fn original_tick32() -> Option<Tick32Fn> {
    let value = ORIGINAL_TICK32.load(Ordering::Acquire);
    (value != 0).then(|| std::mem::transmute::<usize, Tick32Fn>(value as usize))
}
unsafe fn original_timeget() -> Option<TimeGetTimeFn> {
    let value = ORIGINAL_TIMEGET.load(Ordering::Acquire);
    (value != 0).then(|| std::mem::transmute::<usize, TimeGetTimeFn>(value as usize))
}

unsafe extern "system" fn hooked_qpc(out: *mut i64) -> BOOL {
    let Some(call_original) = original_qpc() else { return 0 };
    if out.is_null() { return call_original(out); }
    let mut real = 0i64;
    let ok = call_original(&mut real);
    if ok == 0 { return ok; }
    if let Some(clock) = CLOCK.get() {
        if let Ok(mut state) = clock.lock() {
            let delta = real.saturating_sub(state.last_real_qpc).max(0);
            state.virtual_qpc += delta as f64 * speed();
            state.last_real_qpc = real;
            *out = state.virtual_qpc.round() as i64;
            return ok;
        }
    }
    *out = real;
    ok
}

fn scaled_elapsed_ms() -> Option<u64> {
    let call_original = unsafe { original_qpc()? };
    let mut real = 0i64;
    if unsafe { call_original(&mut real) } == 0 { return None; }
    let clock = CLOCK.get()?;
    let mut state = clock.lock().ok()?;
    let delta = real.saturating_sub(state.last_real_qpc).max(0);
    state.virtual_qpc += delta as f64 * speed();
    state.last_real_qpc = real;
    let elapsed = (state.virtual_qpc - state.base_real_qpc as f64).max(0.0);
    Some((elapsed * 1000.0 / state.frequency.max(1) as f64).max(0.0) as u64)
}

unsafe extern "system" fn hooked_tick64() -> u64 {
    let Some(base) = original_tick64().map(|f| f()) else { return 0 };
    let Some(elapsed) = scaled_elapsed_ms() else { return base };
    let Some(clock) = CLOCK.get() else { return base };
    let Ok(state) = clock.lock() else { return base };
    state.base_tick64.saturating_add(elapsed)
}

unsafe extern "system" fn hooked_tick32() -> u32 {
    let Some(base) = original_tick32().map(|f| f()) else { return 0 };
    let Some(elapsed) = scaled_elapsed_ms() else { return base };
    let Some(clock) = CLOCK.get() else { return base };
    let Ok(state) = clock.lock() else { return base };
    state.base_tick32.wrapping_add(elapsed as u32)
}

unsafe extern "system" fn hooked_timeget() -> u32 {
    let Some(base) = original_timeget().map(|f| f()) else { return 0 };
    let Some(elapsed) = scaled_elapsed_ms() else { return base };
    let Some(clock) = CLOCK.get() else { return base };
    let Ok(state) = clock.lock() else { return base };
    state.base_timeget.wrapping_add(elapsed as u32)
}

fn create_hook(module: &[u16], procedure: &str, detour: *mut c_void) -> Result<(*mut c_void, *mut c_void), String> {
    let proc = CString::new(procedure).map_err(|e| e.to_string())?;
    let mut original = ptr::null_mut();
    let mut target = ptr::null_mut();
    let status = unsafe {
        MH_CreateHookApiEx(
            module.as_ptr(),
            proc.as_ptr() as *const c_char,
            detour,
            &mut original,
            &mut target,
        )
    };
    if status != MH_OK || original.is_null() || target.is_null() {
        return Err(format!("MinHook could not attach {procedure} (status {status})"));
    }
    Ok((original, target))
}

fn install_hooks(pid: u32) -> Result<(), String> {
    let winmm: Vec<u16> = "winmm.dll\0".encode_utf16().collect();
    unsafe { LoadLibraryW(winmm.as_ptr()); }

    let status = unsafe { MH_Initialize() };
    if status != MH_OK {
        return Err(format!("MinHook initialization failed (status {status})"));
    }
    let kernel32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    let mut targets = Vec::new();
    let (qpc, target) = create_hook(&kernel32, "QueryPerformanceCounter", hooked_qpc as *mut c_void)?;
    ORIGINAL_QPC.store(qpc as usize as u64, Ordering::Release);
    targets.push(target);
    let (tick64, target) = create_hook(&kernel32, "GetTickCount64", hooked_tick64 as *mut c_void)?;
    ORIGINAL_TICK64.store(tick64 as usize as u64, Ordering::Release);
    targets.push(target);
    let (tick32, target) = create_hook(&kernel32, "GetTickCount", hooked_tick32 as *mut c_void)?;
    ORIGINAL_TICK32.store(tick32 as usize as u64, Ordering::Release);
    targets.push(target);
    let mm = "winmm.dll\0".encode_utf16().collect::<Vec<_>>();
    let (timeget, target) = create_hook(&mm, "timeGetTime", hooked_timeget as *mut c_void)?;
    ORIGINAL_TIMEGET.store(timeget as usize as u64, Ordering::Release);
    targets.push(target);

    let mut real_qpc = 0i64;
    let mut frequency = 0i64;
    let qpc_call = unsafe { original_qpc().ok_or("QPC trampoline missing")? };
    if unsafe { qpc_call(&mut real_qpc) } == 0 || unsafe { QueryPerformanceFrequency(&mut frequency) } == 0 || frequency <= 0 {
        return Err("Windows high-resolution timer is unavailable".into());
    }
    let base_tick64 = unsafe { original_tick64().ok_or("GetTickCount64 trampoline missing")?() };
    let base_tick32 = unsafe { original_tick32().ok_or("GetTickCount trampoline missing")?() };
    let base_timeget = unsafe { original_timeget().ok_or("timeGetTime trampoline missing")?() };
    CLOCK.set(Mutex::new(ClockState {
        frequency,
        base_real_qpc: real_qpc,
        last_real_qpc: real_qpc,
        virtual_qpc: real_qpc as f64,
        base_tick64,
        base_tick32,
        base_timeget,
    })).map_err(|_| "Clock hooks already initialized".to_string())?;

    let enabled = unsafe { MH_EnableHook(ptr::null_mut()) };
    if enabled != MH_OK { return Err(format!("MinHook could not enable hooks (status {enabled})")); }
    write_status(pid, "ready")?;
    Ok(())
}

fn config_path(pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("tempo-speed-{pid}.cfg"))
}
fn status_path(pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("tempo-speed-{pid}.status"))
}
fn write_status(pid: u32, status: &str) -> Result<(), String> {
    fs::write(status_path(pid), status).map_err(|e| format!("Could not write adapter status: {e}"))
}

fn control_thread(pid: u32) {
    let path = config_path(pid);
    loop {
        let next_speed = fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .filter(|modified| modified.elapsed().map(|age| age < Duration::from_secs(2)).unwrap_or(false))
            .and_then(|_| fs::read_to_string(&path).ok())
            .and_then(|text| text.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite() && (0.25..=5.0).contains(value))
            .unwrap_or(1.0);
        SPEED_BITS.store(next_speed.to_bits(), Ordering::Relaxed);
        thread::sleep(Duration::from_millis(100));
    }
}

unsafe extern "system" fn initialize_thread(_: *mut c_void) -> u32 {
    let pid = unsafe { GetCurrentProcessId() };
    match install_hooks(pid) {
        Ok(()) => {
            thread::spawn(move || control_thread(pid));
        }
        Err(error) => {
            let safe_error = error.replace('\r', " ").replace('\n', " ");
            let _ = write_status(pid, &format!("error:{safe_error}"));
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(instance: HINSTANCE, reason: u32, _: *mut c_void) -> BOOL {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        unsafe { DisableThreadLibraryCalls(instance); }
        let handle = unsafe { CreateThread(ptr::null(), 0, Some(initialize_thread), ptr::null_mut(), 0, ptr::null_mut()) };
        if !handle.is_null() { unsafe { windows_sys::Win32::Foundation::CloseHandle(handle); } }
    }
    TRUE
}

