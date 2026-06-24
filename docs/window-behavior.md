# Window Behavior Notes

## Why The Window Does Not Resize On Hover

Windows + WebView2 can flicker when a transparent, borderless Tauri window is
shown, hidden, or resized. This is especially visible for a small always-on-top
overlay because the whole point is to sit quietly on top of other apps.

The MVP therefore uses this rule:

- The native Tauri window keeps one fixed expanded-size rectangle.
- Collapsed and expanded states are CSS states inside that fixed rectangle.
- In collapsed mode, the native window ignores cursor events, so it does not
  block the apps underneath the transparent area.
- A lightweight Windows cursor-position command checks whether the pointer is
  inside the collapsed strip trigger zone.
- When the pointer enters that zone, the frontend expands the panel and the
  window starts receiving cursor events again.
- When the pointer leaves, CSS collapses the panel and cursor passthrough is
  restored.

This keeps ordinary hover animation out of the native window manager path, which
avoids the resize/show/hide flicker seen in transparent WebView2 windows.

## Hidden States

There are two hidden states:

- Manual hidden: triggered by the in-panel hide button or tray pause command.
- Auto hidden: triggered when the foreground window appears fullscreen.

Manual hidden is only cleared by tray show/reposition. Auto hidden is cleared
when the fullscreen foreground window goes away.

## Positioning

The panel is positioned against the monitor work area, not the full monitor
bounds. This keeps the collapsed strip above the Windows taskbar.
