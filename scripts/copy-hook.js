import { createHash } from "node:crypto";
import { access, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const projectRoot = process.cwd();
const variants = [
  { arch: "x64", source: resolve(projectRoot, "speed-hook", "target", "release", "tempo_speed_hook.dll") },
  { arch: "x86", source: resolve(projectRoot, "speed-hook", "target", "i686-pc-windows-gnu", "release", "tempo_speed_hook.dll") },
];

for (const directory of ["debug", "release"]) {
  const destinationDir = resolve(projectRoot, "src-tauri", "target", directory);
  await mkdir(destinationDir, { recursive: true });
  for (const { arch, source } of variants) {
    const bytes = await readFile(source);
    const hash = createHash("sha256").update(bytes).digest("hex");
    const filename = `tempo_speed_hook_${arch}-${hash}.dll`;
    const destination = resolve(destinationDir, filename);
    try {
      await access(destination);
    } catch {
      await copyFile(source, destination);
    }
    await writeFile(resolve(destinationDir, `tempo_speed_hook_${arch}.current`), filename, "utf8");
    console.log(`${arch} speed hook ready at ${destination}`);
  }
}
