import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(readFileSync(path.join(rootDir, "package.json"), "utf8"));
const version = packageJson.version;
const repo = "xy-tsuki/deadline-panel";
const bundleDir = path.join(rootDir, "src-tauri", "target", "release", "bundle", "nsis");
const setupName = `Deadline Panel_${version}_x64-setup.exe`;
const setupPath = path.join(bundleDir, setupName);
const signatureCandidates = [
  `${setupPath}.sig`,
  path.join(bundleDir, `${setupName}.sig`),
  path.join(bundleDir, `Deadline Panel_${version}_x64-setup.exe.sig`)
];
const signaturePath = signatureCandidates.find((candidate) => existsSync(candidate));

if (!existsSync(setupPath)) {
  throw new Error(`Installer not found: ${setupPath}`);
}

if (!signaturePath) {
  throw new Error(`Updater signature not found next to installer: ${setupPath}.sig`);
}

const assetName = encodeURIComponent(setupName);
const manifest = {
  version,
  notes: `Deadline Panel v${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(signaturePath, "utf8").trim(),
      url: `https://github.com/${repo}/releases/download/v${version}/${assetName}`
    }
  }
};

const manifestText = `${JSON.stringify(manifest, null, 2)}\n`;
const bundleOutputPath = path.join(bundleDir, "latest.json");
const repoOutputDir = path.join(rootDir, "updates");
const repoOutputPath = path.join(repoOutputDir, "latest.json");

mkdirSync(repoOutputDir, { recursive: true });
writeFileSync(bundleOutputPath, manifestText, "utf8");
writeFileSync(repoOutputPath, manifestText, "utf8");
console.log(`Wrote updater manifest: ${bundleOutputPath}`);
console.log(`Wrote repository updater manifest: ${repoOutputPath}`);
