import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(readFileSync(path.join(rootDir, "package.json"), "utf8"));
const version = packageJson.version;
const nsisDir = path.join(rootDir, "src-tauri", "target", "release", "nsis", "x64");
const nsiPath = path.join(nsisDir, "installer.nsi");
const outputPath = path.join(nsisDir, "nsis-output.exe");
const setupPath = path.join(
  rootDir,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "nsis",
  `Deadline Panel_${version}_x64-setup.exe`
);

if (!existsSync(nsiPath)) {
  throw new Error(`NSIS script not found: ${nsiPath}`);
}

const marker = '  ${IfThen} "$R0$R1" == "" ${|} Abort ${|}';
const patch = `${marker}\r\n  StrCpy $UpdateMode 1\r\n  Abort`;
const reinstallPatchPattern =
  /  \$\{IfThen\} "\$R0\$R1" == "" \$\{\|\} Abort \$\{\|\}(?:\r?\n  StrCpy \$UpdateMode 1\r?\n  Abort)*/;
const directoryPagePattern =
  /!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive\r?\n!insertmacro MUI_PAGE_DIRECTORY/;
const patchedDirectoryPagePattern =
  /!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassiveOrUpdate\r?\n!insertmacro MUI_PAGE_DIRECTORY/;
const directoryPagePatch =
  "!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassiveOrUpdate\r\n!insertmacro MUI_PAGE_DIRECTORY";
const skipFunctionPattern =
  /Function SkipIfPassiveOrUpdate\r?\n(?:.*\r?\n)*?FunctionEnd\r?\n?/;
const skipFunctionPatch =
  "Function SkipIfPassiveOrUpdate\r\n" +
  "  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}\r\n" +
  "  ${IfThen} $UpdateMode = 1  ${|} Abort ${|}\r\n" +
  "FunctionEnd\r\n";
const original = readFileSync(nsiPath, "utf8");

if (!original.includes(marker)) {
  throw new Error("Could not find the NSIS reinstall-page marker to patch.");
}

let patched = original.replace(reinstallPatchPattern, patch);
if (directoryPagePattern.test(patched)) {
  patched = patched.replace(directoryPagePattern, directoryPagePatch);
} else if (!patchedDirectoryPagePattern.test(patched)) {
  throw new Error("Could not find the NSIS directory-page marker to patch.");
}
patched = patched.replace(skipFunctionPattern, "");
patched = patched.replace(
  "Function un.SkipIfPassive",
  `${skipFunctionPatch}Function un.SkipIfPassive`
);
if (patched !== original) {
  writeFileSync(nsiPath, patched, "utf8");
}

const makensisPath = path.join(process.env.LOCALAPPDATA ?? "", "tauri", "NSIS", "makensis.exe");
if (!existsSync(makensisPath)) {
  throw new Error(`makensis.exe not found: ${makensisPath}`);
}

execFileSync(makensisPath, ["installer.nsi"], {
  cwd: nsisDir,
  stdio: "inherit"
});

copyFileSync(outputPath, setupPath);
console.log(`Patched update-mode installer: ${setupPath}`);
