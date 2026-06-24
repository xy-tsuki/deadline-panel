# Deadline Panel

Windows desktop deadline panel for attention-friendly deadline awareness.

This is not a traditional to-do app. The MVP keeps a tiny always-visible status
strip near the bottom-right corner and expands into a low-density deadline panel
when the pointer enters the corner area.

## MVP Scope

- Bottom-right floating status strip.
- Hover/corner-triggered slide-out panel.
- Top 3 / 5 / 10 deadline-focused tasks.
- Deadline-first sorting with priority as a tie breaker.
- Local persistence with a browser fallback and a Tauri + SQLite backend.
- Basic task lifecycle: active, completed, postponed.
- Windows tray menu for show, pause, reposition, and quit.
- Desktop autostart toggle in Tauri mode.
- Single-instance guard so a second launch re-shows the existing panel.
- Fixed-size overlay with cursor passthrough to avoid transparent-window resize flicker.
- Quick add parser for simple Chinese, Japanese, and English deadline phrases.
- Chinese / Japanese / English UI language setting.
- NSIS `.exe` installer with Chinese / Japanese / English language selector.
- Command import preview for Codex/GPT-assisted text imports.

## Tech Stack

- React + TypeScript + Vite
- Tauri shell
- SQLite via Rust backend commands
- Zustand state store

The frontend can run in a browser for fast MVP verification. Desktop mode uses
the Tauri backend under `src-tauri`.

## Development

```powershell
npm install
npm run dev
```

Then open the Vite URL shown in the terminal.

Desktop mode after Rust is installed:

```powershell
npm run tauri:dev
```

If `cargo` is not found after installing Rust, restart the terminal so the Rust
toolchain path is picked up.

Build the Windows installer:

```powershell
npm run tauri:build -- --bundles nsis
```

## Project Docs

- [MVP plan](docs/mvp-plan.md)
- [Database design](docs/database.md)
- [Window behavior notes](docs/window-behavior.md)
- [UI mockup](docs/ui-mockup.html)
- [Generated UI concept](docs/assets/ui-concept.png)
