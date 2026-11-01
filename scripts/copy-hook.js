import { copyFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const projectRoot = process.cwd();
const source = resolve(projectRoot, "speed-hook", "target", "release", "tempo_speed_hook.dll");
const destination = resolve(projectRoot, "src-tauri", "target", "release", "tempo_speed_hook.dll");

await mkdir(dirname(destination), { recursive: true });
await copyFile(source, destination);
console.log(`Copied speed hook to ${destination}`);
