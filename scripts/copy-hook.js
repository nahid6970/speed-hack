import { createHash } from "node:crypto";
import { access, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const projectRoot = process.cwd();
const source = resolve(projectRoot, "speed-hook", "target", "release", "tempo_speed_hook.dll");
const releaseDir = resolve(projectRoot, "src-tauri", "target", "release");
const bytes = await readFile(source);
const hash = createHash("sha256").update(bytes).digest("hex");
const filename = `tempo_speed_hook-${hash}.dll`;
const destination = resolve(releaseDir, filename);

await mkdir(releaseDir, { recursive: true });
try {
  await access(destination);
} catch {
  await copyFile(source, destination);
}
await writeFile(resolve(releaseDir, "tempo_speed_hook.current"), filename, "utf8");
console.log(`Copied speed hook to ${destination}`);
