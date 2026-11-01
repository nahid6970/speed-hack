const invoke = window.__TAURI__?.core?.invoke;
const storageKeys = { process: "tempo.selectedProcess", speed: "tempo.selectedSpeed" };
const slider = document.getElementById("speed-slider");
const speedValues = [0.25, 0.5, 0.75, 1, 2, 5, 10];
const speedText = document.getElementById("speed-number");
const presets = [...document.querySelectorAll("[data-testid='speed-preset']")];
const dialog = document.getElementById("process-picker-dialog");
const processList = document.getElementById("process-list");
const searchInput = document.getElementById("process-search-input");
const toast = document.getElementById("status-toast");
const toggle = document.getElementById("enable-speed-toggle");
let processes = [];
let toastTimer;
let speedSyncTimer;
let heartbeatBusy = false;
let connectedPid = null;
let currentProcess = readStoredProcess();

function readStoredProcess() {
  try { return JSON.parse(localStorage.getItem(storageKeys.process) || "null"); }
  catch { return null; }
}
function notify(message) {
  toast.textContent = message;
  toast.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove("show"), 2000);
}
function setConnection(label, connected = false) {
  document.getElementById("connection-label").textContent = label;
  document.getElementById("connection-status").classList.toggle("connected", connected);
  document.getElementById("adapter-notice").classList.toggle("adapter-ready", connected);
  const note = document.getElementById("adapter-notice-text");
  note.innerHTML = connected
    ? "<strong>Adapter connected.</strong> Use only with an offline, single-player game. If the app closes, the clock returns to 1×."
    : "<strong>Offline single-player only.</strong> Enabling loads a timing hook into the selected game. Compatibility varies.";
}
function updateEnabled(enabled) {
  toggle.setAttribute("aria-checked", String(enabled));
  toggle.classList.toggle("off", !enabled);
  document.getElementById("switch-label").textContent = enabled ? "Speed control on" : "Speed control off";
}
function selectedSpeed() { return speedValues[Number(slider.value)] ?? 1; }
function renderSelectedProcess() {
  const name = currentProcess?.name || "No game selected";
  document.getElementById("game-name").textContent = name;
  document.getElementById("game-process").textContent = currentProcess
    ? `${currentProcess.executable || currentProcess.name} · PID ${currentProcess.pid}`
    : "Choose a running game process";
  document.getElementById("game-icon").textContent = currentProcess
    ? name.split(/\s+/).slice(0, 2).map(part => part[0]).join("").toUpperCase()
    : "TG";
}
function applySpeed(value, { announce = false } = {}) {
  const speed = Math.min(10, Math.max(0.25, Number(value)));
  const rounded = speedValues.reduce((nearest, candidate) => Math.abs(candidate - speed) < Math.abs(nearest - speed) ? candidate : nearest, speedValues[0]);
  const index = speedValues.indexOf(rounded);
  const percent = (index / (speedValues.length - 1)) * 100;
  slider.value = String(index);
  slider.style.background = `linear-gradient(90deg,var(--mint),#59cbb4 ${percent}%,#303944 ${percent}%)`;
  speedText.textContent = String(rounded);
  presets.forEach(button => button.classList.toggle("selected", Number(button.dataset.speed) === rounded));
  localStorage.setItem(storageKeys.speed, String(rounded));
  if (connectedPid !== null) queueSpeedUpdate();
  else if (announce) notify(`Selected ${rounded}×. Connect the adapter to apply it.`);
}
async function refreshProcesses() {
  document.getElementById("process-count").textContent = "Reading running apps…";
  processList.replaceChildren();
  if (!invoke) {
    processes = [];
    document.getElementById("process-count").textContent = "Desktop app required";
    const empty = document.createElement("div");
    empty.className = "empty-list";
    empty.id = "process-list-empty-state";
    empty.textContent = "Launch Game Speed Control as a desktop app to see running Windows processes.";
    processList.append(empty);
    return;
  }
  try {
    processes = await invoke("list_processes");
    renderProcesses();
    if (currentProcess && !processes.some(process => process.pid === currentProcess.pid)) {
      if (connectedPid === currentProcess.pid) await stopAdapter(currentProcess.pid);
      currentProcess = null;
      localStorage.removeItem(storageKeys.process);
      renderSelectedProcess();
    }
  } catch (error) {
    processes = [];
    document.getElementById("process-count").textContent = "Could not read apps";
    const empty = document.createElement("div");
    empty.className = "empty-list";
    empty.id = "process-list-empty-state";
    empty.textContent = `Process list failed: ${String(error)}`;
    processList.append(empty);
  }
}
function renderProcesses() {
  const query = searchInput.value.trim().toLocaleLowerCase();
  const visible = processes.filter(process => `${process.name} ${process.executable || ""}`.toLocaleLowerCase().includes(query));
  document.getElementById("process-count").textContent = `${visible.length} app${visible.length === 1 ? "" : "s"} found`;
  processList.replaceChildren();
  if (!visible.length) {
    const empty = document.createElement("div");
    empty.className = "empty-list";
    empty.id = "process-list-empty-state";
    empty.textContent = processes.length ? "No apps match that search." : "No visible app processes found. Start a game, then refresh the list.";
    processList.append(empty);
    return;
  }
  for (const process of visible) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "process-choice";
    button.id = `process-choice-${process.pid}`;
    button.dataset.testid = "process-choice";
    button.setAttribute("role", "option");
    button.setAttribute("aria-selected", String(currentProcess?.pid === process.pid));
    const icon = document.createElement("span");
    icon.className = "process-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = process.name.replace(/\.exe$/i, "").slice(0, 2).toUpperCase();
    const copy = document.createElement("span");
    copy.className = "process-copy";
    const name = document.createElement("strong");
    name.textContent = process.name;
    const path = document.createElement("small");
    path.textContent = process.executable || "Executable path unavailable";
    copy.append(name, path);
    const pid = document.createElement("span");
    pid.className = "process-pid";
    pid.textContent = `PID ${process.pid}`;
    button.append(icon, copy, pid);
    button.addEventListener("click", () => selectProcess(process));
    processList.append(button);
  }
}
async function selectProcess(process) {
  if (connectedPid !== null && connectedPid !== process.pid) await stopAdapter(connectedPid);
  currentProcess = process;
  localStorage.setItem(storageKeys.process, JSON.stringify(process));
  renderSelectedProcess();
  dialog.close();
  notify("Game selected");
}
async function enableAdapter() {
  if (!invoke) throw new Error("Start Game Speed Control as a desktop app first.");
  if (!currentProcess) throw new Error("Choose a running game first.");
  setConnection("Loading adapter…");
  toggle.disabled = true;
  try {
    const message = await invoke("connect_speed_adapter", { pid: currentProcess.pid, speed: selectedSpeed() });
    connectedPid = currentProcess.pid;
    updateEnabled(true);
    setConnection("Adapter connected", true);
    notify(message || "Game speed adapter connected");
  } catch (error) {
    connectedPid = null;
    updateEnabled(false);
    setConnection("Adapter unavailable");
    throw new Error(String(error));
  } finally {
    toggle.disabled = false;
  }
}
async function stopAdapter(pid) {
  try {
    if (invoke) await invoke("disconnect_speed_adapter", { pid });
  } catch (error) {
    notify(`Could not reset game to normal speed: ${String(error)}`);
  }
  connectedPid = null;
  updateEnabled(false);
  setConnection("Not connected");
}
async function pushSpeed(enabled = true) {
  if (connectedPid === null || !invoke) return;
  try {
    await invoke("set_speed_for_process", { pid: connectedPid, speed: selectedSpeed(), enabled });
  } catch (error) {
    connectedPid = null;
    updateEnabled(false);
    setConnection("Adapter disconnected");
    notify(String(error));
  }
}
function queueSpeedUpdate() {
  clearTimeout(speedSyncTimer);
  speedSyncTimer = setTimeout(() => pushSpeed(true), 100);
}
async function handleToggle() {
  const turnOn = toggle.getAttribute("aria-checked") !== "true";
  if (turnOn) {
    try { await enableAdapter(); }
    catch (error) { notify(error.message); }
    return;
  }
  if (connectedPid !== null) await stopAdapter(connectedPid);
  else { updateEnabled(false); setConnection("Not connected"); }
  notify("Game clock returned to normal speed");
}

 document.getElementById("choose-game-button").addEventListener("click", async () => {
  dialog.showModal();
  searchInput.value = "";
  await refreshProcesses();
  searchInput.focus();
});
document.getElementById("close-picker-button").addEventListener("click", () => dialog.close());
document.getElementById("refresh-process-list-button").addEventListener("click", refreshProcesses);
searchInput.addEventListener("input", renderProcesses);
dialog.addEventListener("click", event => { if (event.target === dialog) dialog.close(); });
slider.addEventListener("input", event => applySpeed(speedValues[Number(event.target.value)]));
presets.forEach(button => button.addEventListener("click", () => applySpeed(button.dataset.speed, { announce: true })));
document.getElementById("normal-speed-button").addEventListener("click", () => applySpeed(1, { announce: true }));
toggle.addEventListener("click", handleToggle);

// A heartbeat lets the injected adapter fail back to 1× if the app closes unexpectedly.
setInterval(async () => {
  if (connectedPid === null || heartbeatBusy || !invoke) return;
  heartbeatBusy = true;
  try { await invoke("set_speed_for_process", { pid: connectedPid, speed: selectedSpeed(), enabled: true }); }
  catch (error) {
    connectedPid = null;
    updateEnabled(false);
    setConnection("Adapter disconnected");
    notify(String(error));
  } finally { heartbeatBusy = false; }
}, 500);

renderSelectedProcess();
applySpeed(localStorage.getItem(storageKeys.speed) || 1);
updateEnabled(false);
setConnection("Not connected");
