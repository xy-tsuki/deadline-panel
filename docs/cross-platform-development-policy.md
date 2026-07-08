# Cross-Platform Development Policy

This document defines how Windows and macOS development should coexist in this repository without breaking each other's builds, release files, or updater behavior.

## Goals

- Windows and macOS development can happen in parallel.
- Platform-specific work must not break the other platform.
- The production updater must never offer a Windows package to macOS, or a macOS package to Windows.
- Development builds must not accidentally publish or consume production updates.
- `main` stays release-oriented and stable.

## Branch Rules

- `main` is the stable release branch.
- `macos-port-probe` is the macOS migration branch until the macOS version is release-ready.
- Windows maintenance should use short-lived branches such as `windows/fix-startup`.
- macOS maintenance should use short-lived branches from `macos-port-probe`, such as `macos/fix-hover-expand`.
- Do not do long-running development directly on `main`.
- Do not force-push shared branches unless the other side has explicitly agreed.

Recommended workflow:

```bash
git fetch origin
git checkout macos-port-probe
git pull --rebase origin macos-port-probe
```

For Windows maintenance:

```bash
git fetch origin
git checkout main
git pull --ff-only origin main
git checkout -b windows/fix-something
```

## Configuration Separation

The shared config should stay in:

```text
src-tauri/tauri.conf.json
```

Platform-specific config should be split into:

```text
src-tauri/tauri.windows.conf.json
src-tauri/tauri.macos.conf.json
```

Windows-only bundle settings belong in `tauri.windows.conf.json`:

```json
{
  "bundle": {
    "targets": ["nsis"],
    "icon": ["icons/icon.ico"],
    "windows": {
      "nsis": {}
    }
  }
}
```

macOS-only bundle settings belong in `tauri.macos.conf.json`:

```json
{
  "bundle": {
    "targets": ["app", "dmg"],
    "icon": ["icons/icon.icns"]
  }
}
```

Do not make Windows builds depend on macOS bundle settings. Do not make macOS builds depend on NSIS settings.

## Code Separation

Platform-specific native behavior must be behind `cfg` gates or platform modules.

Preferred structure:

```text
src-tauri/src/platform/mod.rs
src-tauri/src/platform/windows.rs
src-tauri/src/platform/macos.rs
```

Rules:

- Windows behavior must remain behind `#[cfg(windows)]`.
- macOS behavior must remain behind `#[cfg(target_os = "macos")]`.
- Shared task, database, sync, import/export, and UI logic should stay platform-neutral.
- Do not replace working Windows native behavior while implementing macOS behavior.
- macOS stubs are acceptable during early migration if they are clearly temporary and do not alter Windows behavior.

## Updater Separation

The production updater may use one shared static manifest only if all platform entries are valid and complete.

Expected production shape:

```json
{
  "version": "0.6.0",
  "notes": "Deadline Panel v0.6.0",
  "pub_date": "2026-07-09T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "...",
      "url": "https://github.com/xy-tsuki/deadline-panel/releases/download/v0.6.0/Deadline.Panel_0.6.0_x64-setup.exe"
    },
    "darwin-aarch64": {
      "signature": "...",
      "url": "https://github.com/xy-tsuki/deadline-panel/releases/download/v0.6.0/Deadline.Panel_0.6.0_aarch64.app.tar.gz"
    }
  }
}
```

Rules:

- Windows release scripts may update `platforms.windows-x86_64`.
- macOS release scripts may update `platforms.darwin-aarch64` and, if needed, `platforms.darwin-x86_64`.
- A platform release script must not delete entries for another platform.
- `updates/latest.json` must not point to assets that have not been uploaded to GitHub Releases.
- Do not publish a candidate build by updating the production updater manifest.

## Development Updater Rules

During macOS migration:

- macOS development builds should not use the production updater endpoint.
- Prefer disabling updater checks in macOS development builds until packaging is stable.
- If updater testing is required, use a separate dev manifest such as:

```text
updates/latest-dev.json
```

and a separate dev endpoint on the migration branch.

Development builds may use a distinct identifier and product name:

```json
{
  "identifier": "local.deadline-panel.macos-dev",
  "productName": "Deadline Panel Dev"
}
```

This avoids confusing installed production builds with migration builds.

## Release Rules

Before publishing a release:

- Confirm the platform build artifact exists.
- Confirm its `.sig` exists.
- Confirm `updates/latest.json` points to the uploaded release asset.
- Confirm the release tag exists and points to the intended commit.
- Confirm `main` is clean.

For Windows:

```text
Deadline.Panel_<version>_x64-setup.exe
```

For macOS Apple Silicon:

```text
Deadline.Panel_<version>_aarch64.dmg
Deadline Panel.app.tar.gz
Deadline Panel.app.tar.gz.sig
```

The updater should reference the macOS `.app.tar.gz`, not the `.dmg`.

## Local-Only Files

Never commit:

```text
src-tauri/updater.key
.env
.env.local
*.p12
*.cer
*.mobileprovision
.DS_Store
dist/
src-tauri/target/
```

Use `.git/info/exclude` for machine-specific temporary files when they should not become repository-wide ignore rules.

