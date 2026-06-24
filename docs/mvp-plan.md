# MVP Plan

## Product Principle

Deadline Panel is a persistent environment cue, not a list manager.

The app should answer only three questions by default:

1. What should I do now?
2. What is the nearest deadline?
3. How much time is left?

Everything else stays secondary.

## First Phase Deliverables

### 1. Floating Panel

- Tiny collapsed strip at the bottom-right edge.
- Expanded panel slides in when the pointer enters the trigger zone.
- Pointer leaving the panel collapses it again.
- Window should be always-on-top in Tauri mode.
- Window should avoid focus stealing where the platform allows it.

### 2. Deadline Sorting

Sort active/postponed tasks by:

1. Due datetime ascending.
2. Priority descending.
3. Created time ascending.

Completed tasks are excluded from the Top 3 and nearest-deadline view.

### 3. Today Focus

The panel only shows the top three actionable tasks. This is intentional: long
lists create pressure and avoidance.

### 4. Local Storage

MVP has two persistence layers:

- Browser/dev fallback: `localStorage`.
- Desktop target: SQLite through Tauri commands.

The frontend talks through one storage adapter so the UI does not care which
backend is active.

## UI Prototype

ASCII layout:

```text
Collapsed:

┌──────────────────────────────────────────────┐
│ 今天 3 件 | 最近截止: Graph Mining Quiz (2天) │
└──────────────────────────────────────────────┘

Expanded:

┌────────────────────────────────────┐
│ 现在该做                           │
│ Graph Mining Quiz                  │
│ 剩余 2天 | high                    │
├────────────────────────────────────┤
│ 最近 Deadline                      │
│ 1. Graph Mining Quiz               │
│    2026-06-23 23:59 | 剩余 2天     │
│ 2. 研究室报告                      │
│    2026-06-25 18:00 | 剩余 4天     │
│ 3. 文献阅读                        │
│    2026-06-28 23:59 | 剩余 7天     │
├────────────────────────────────────┤
│ + Add deadline                     │
└────────────────────────────────────┘
```

An HTML mockup is available at `docs/ui-mockup.html`.

## Development Roadmap

### Step 1: Stable MVP Shell

- Done: React collapsed/expanded panel states.
- Done: Top 3 deadline-first task selection.
- Done: Local persistence adapter with browser fallback.
- Done: Seed tasks for first launch.

### Step 2: Desktop Integration

- Done: SQLite commands in Tauri.
- Done: Transparent, borderless, always-on-top window.
- Done: No-focus collapsed mode with work-area bottom-right positioning.
- Done: Fixed-size overlay with cursor passthrough to avoid WebView2 resize flicker.
- Done: Tray menu for show, pause, reposition, and quit.
- Done: Autostart toggle through the Tauri autostart plugin.
- Done: Single-instance guard that re-shows the existing panel.

### Step 3: Command Import Foundation

- Done: Parse `/add`, `/complete`, `/delete`, `/update`.
- Done: Basic validation and user-facing result messages.
- Next: Add a preview/confirm layer before command mutations.

### Step 4: Codex-Assisted Import

- User sends screenshots to Codex.
- Codex emits standard command strings.
- User pastes commands into the app.

### Step 5: One-Click Screenshot Import

- Add image picker.
- Call OpenAI API.
- Show extraction preview.
- Require explicit confirmation before database writes.

## Non-Goals for MVP

- Calendar sync.
- Pomodoro timers.
- Gamification.
- Large dashboards.
- Complex categories or project trees.
- Direct OpenAI API screenshot processing.
