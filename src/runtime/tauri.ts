import { invoke } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

export interface PanelPointerState {
  inTrigger: boolean;
  inWindow: boolean;
  leftDown: boolean;
  rightDown: boolean;
  cursorX: number;
  cursorY: number;
  windowX: number;
  windowY: number;
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

export async function setPanelExpanded(expanded: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("set_panel_expanded", { expanded });
}

export async function hidePanelTemporarily(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("hide_panel_temporarily");
}

export async function hidePanelForMinutes(minutes: number): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("hide_panel_for_minutes", { minutes });
}

export async function setPanelAutoHidden(hidden: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("set_panel_auto_hidden", { hidden });
}

export async function isForegroundWindowFullscreen(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  return invoke<boolean>("is_foreground_window_fullscreen");
}

export async function isCursorInPanelTrigger(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  return invoke<boolean>("cursor_in_panel_trigger");
}

export async function getPanelPointerState(): Promise<PanelPointerState | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  return invoke<PanelPointerState>("panel_pointer_state");
}

export async function setPanelAcceptsInput(acceptsInput: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("set_panel_accepts_input", { acceptsInput });
}

export async function getDataFilePath(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  return invoke<string>("data_file_path");
}

export async function openDataDir(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("open_data_dir");
}

export async function backupDatabase(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  return invoke<string>("backup_database");
}

export async function movePanelWindow(x: number, y: number): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("move_panel_window", { x: Math.round(x), y: Math.round(y) });
}

export async function rememberPanelPosition(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("remember_panel_position");
}

export async function resetPanelPosition(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("reset_panel_position");
}

export async function showPanelContextMenu(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("show_panel_context_menu");
}

export async function startPanelDrag(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("start_panel_drag");
}

export async function finishPanelDrag(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("finish_panel_drag");
}

export async function quitApp(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("quit_app");
}

export async function getAutostartEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  return isEnabled();
}

export async function setAutostartEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  if (enabled) {
    await enable();
  } else {
    await disable();
  }

  return isEnabled();
}
