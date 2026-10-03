import { AppearancePreferences, parseAppearance } from "../domain/appearance";
import { invoke } from "@tauri-apps/api/core";
import { DeadlineTask } from "../domain/task";
import { seedTasks } from "../domain/seed";
import { isTauriRuntime } from "../runtime/tauri";

const STORAGE_KEY = "adhd-deadline-panel.tasks.v1";
const SETTING_KEY_PREFIX = "adhd-deadline-panel.setting.";

export async function loadSettings(keys: string[]): Promise<Record<string, string | null>> {
  if (isTauriRuntime()) return invoke("get_app_settings", { keys });
  return Object.fromEntries(keys.map(key => [key, localStorage.getItem(SETTING_KEY_PREFIX + key)]));
}
export async function saveSettings(values: Record<string, string>): Promise<void> {
  if (isTauriRuntime()) return invoke("set_app_settings", { values });
  const previous = await loadSettings(Object.keys(values));
  try { for (const [key, value] of Object.entries(values)) localStorage.setItem(SETTING_KEY_PREFIX + key, value); }
  catch (error) {
    for (const [key, value] of Object.entries(previous)) { if (value === null) localStorage.removeItem(SETTING_KEY_PREFIX + key); else localStorage.setItem(SETTING_KEY_PREFIX + key, value); }
    throw error;
  }
}

export async function loadSnapshot(): Promise<{ tasks: DeadlineTask[]; focusLimit: string | null; language: string | null; appearance: string | null }> {
  if (isTauriRuntime()) return invoke("load_snapshot");
  const [tasks, focusLimit, language, appearance] = await Promise.all([loadTasks(), loadSetting("focus_limit"), loadSetting("app_language"), loadSetting("appearance_preferences")]);
  return { tasks, focusLimit, language, appearance };
}

export async function loadTasks(): Promise<DeadlineTask[]> {
  if (isTauriRuntime()) {
    const tasks = await invoke<DeadlineTask[]>("list_tasks");
    return normalizeTasks(tasks);
  }

  const stored = localStorage.getItem(STORAGE_KEY);
  if (!stored) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(seedTasks));
    return seedTasks;
  }

  try {
    return normalizeTasks(JSON.parse(stored) as DeadlineTask[]);
  } catch {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(seedTasks));
    return seedTasks;
  }
}

export async function saveTask(task: DeadlineTask): Promise<DeadlineTask> {
  if (isTauriRuntime()) {
    return invoke<DeadlineTask>("save_task", { task });
  }

  const tasks = await loadTasks();
  const next = tasks.some((item) => item.id === task.id)
    ? tasks.map((item) => (item.id === task.id ? task : item))
    : [...tasks, task];

  localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  return task;
}

export async function removeTask(id: string, namespace?: string): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("delete_task", { id, namespace });
    return;
  }

  if (namespace) localStorage.setItem(namespace, JSON.stringify([...new Set([...(await loadPendingDeletes(namespace)), id])]));
  const tasks = await loadTasks();
  localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks.filter((task) => task.id !== id)));
}

export async function getStateRevision(): Promise<number> {
  return isTauriRuntime() ? invoke<number>("state_revision") : Number(localStorage.getItem(`${STORAGE_KEY}.revision`) ?? 0);
}

export async function replaceTasks(tasks: DeadlineTask[], expectedRevision?: number): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("replace_tasks", { tasks, expectedRevision });
    return;
  }

  localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks));
}

export async function loadSetting(key: string): Promise<string | null> {
  if (isTauriRuntime()) {
    return invoke<string | null>("get_app_setting", { key });
  }

  return localStorage.getItem(`${SETTING_KEY_PREFIX}${key}`);
}

export async function saveSetting(key: string, value: string): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("set_app_setting", { key, value });
    return;
  }

  localStorage.setItem(`${SETTING_KEY_PREFIX}${key}`, value);
}

function normalizeTasks(tasks: DeadlineTask[]): DeadlineTask[] {
  return tasks.map((task) => ({
    ...task,
    isCurrent: task.isCurrent ?? false
  }));
}

export async function loadPendingDeletes(namespace: string): Promise<string[]> {
  return isTauriRuntime() ? invoke<string[]>("pending_deletes", { namespace }) : JSON.parse(localStorage.getItem(namespace) ?? "[]");
}
export async function acknowledgeDelete(namespace: string, id: string): Promise<void> {
  if (isTauriRuntime()) await invoke("acknowledge_delete", { namespace, id });
  else localStorage.setItem(namespace, JSON.stringify((await loadPendingDeletes(namespace)).filter((item) => item !== id)));
}

export async function saveTasks(tasks: DeadlineTask[]): Promise<void> {
  if (isTauriRuntime()) { await invoke("save_tasks", { tasks }); return; }
  const merged = new Map((await loadTasks()).map((task) => [task.id, task]));
  for (const task of tasks) merged.set(task.id, task);
  localStorage.setItem(STORAGE_KEY, JSON.stringify([...merged.values()]));
}

export async function patchTask(id: string, fields: Partial<DeadlineTask>): Promise<void> {
  const patch = { ...fields, updatedAt: new Date().toISOString() };
  if (isTauriRuntime()) { await invoke("patch_task", { id, fields: patch }); return; }
  const current = (await loadTasks()).find((task) => task.id === id);
  if (!current) throw new Error("Task no longer exists");
  await saveTask({ ...current, ...patch, id: current.id, createdAt: current.createdAt });
}

export async function saveAppearancePreferences(changes: Partial<AppearancePreferences>): Promise<AppearancePreferences> {
  if (isTauriRuntime()) return parseAppearance(await invoke<string>("set_appearance_preferences", { changes }));
  const key = SETTING_KEY_PREFIX + "appearance_preferences";
  const appearance = { ...parseAppearance(localStorage.getItem(key)), ...changes };
  localStorage.setItem(key, JSON.stringify(appearance));
  return appearance;
}
