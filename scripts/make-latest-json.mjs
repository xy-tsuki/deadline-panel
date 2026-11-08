import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(readFileSync(path.join(rootDir, "package.json"), "utf8"));
const version = packageJson.version;
const repo = "xy-tsuki/deadline-panel";
const args = process.argv.slice(2);
function argument(name, fallback) {
  const index = args.indexOf(name);
  if (index < 0) return fallback;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`Missing value for ${name}`);
  return value;
}
// Windows candidates use their own release tag; v<version> belongs to macOS.
const releaseTag = argument("--release-tag", `windows-v${version}`);
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

const releaseAssetName = setupName.replaceAll(" ", ".");
const assetName = encodeURIComponent(releaseAssetName);
const manifest = {
  version,
  notes: `Deadline Panel v${version} for Windows`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(signaturePath, "utf8").trim(),
      url: `https://github.com/${repo}/releases/download/${encodeURIComponent(releaseTag)}/${assetName}`
    }
  }
};

const manifestText = `${JSON.stringify(manifest, null, 2)}\n`;
const outputPath = path.resolve(rootDir, argument("--output", path.join(bundleDir, "latest-candidate.json")));
const productionPath = path.join(rootDir, "updates", "latest.json");
if (outputPath.toLowerCase() === productionPath.toLowerCase()) {
  throw new Error("Generate a candidate file, then review the production manifest separately.");
}
mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, manifestText, "utf8");
console.log(`Wrote candidate updater manifest: ${outputPath}`);
