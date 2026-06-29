import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultKeyPath = path.join(rootDir, "src-tauri", "updater.key");

if (!process.env.TAURI_SIGNING_PRIVATE_KEY && !process.env.TAURI_SIGNING_PRIVATE_KEY_PATH && existsSync(defaultKeyPath)) {
  process.env.TAURI_SIGNING_PRIVATE_KEY_PATH = defaultKeyPath;
  process.env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(defaultKeyPath, "utf8").trim();
}
if (process.env.TAURI_SIGNING_PRIVATE_KEY_PATH && process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD === undefined) {
  process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "";
}

const npx = process.platform === "win32" ? "npx.cmd" : "npx";
execFileSync(npx, ["tauri", "build"], {
  cwd: rootDir,
  env: process.env,
  stdio: "inherit"
});
execFileSync(process.execPath, [path.join(rootDir, "scripts", "patch-nsis-update-mode.mjs")], {
  cwd: rootDir,
  env: process.env,
  stdio: "inherit"
});
execFileSync(process.execPath, [path.join(rootDir, "scripts", "make-latest-json.mjs")], {
  cwd: rootDir,
  env: process.env,
  stdio: "inherit"
});
