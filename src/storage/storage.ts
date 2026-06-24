import { invoke } from "@tauri-apps/api/core";
import { DeadlineTask } from "../domain/task";
import { seedTasks } from "../domain/seed";
import { isTauriRuntime } from "../runtime/tauri";

const STORAGE_KEY = "adhd-deadline-panel.tasks.v1";
const SETTING_KEY_PREFIX = "adhd-deadline-panel.setting.";

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

export async function removeTask(id: string): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("delete_task", { id });
    return;
  }

  const tasks = await loadTasks();
  localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks.filter((task) => task.id !== id)));
}

export async function replaceTasks(tasks: DeadlineTask[]): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("replace_tasks", { tasks });
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
