import { validateImportedTasks } from "../domain/taskImport";
import { AppearancePreferences, defaultAppearance, parseAppearance } from "../domain/appearance";
import { isSyncOwner, requestOwnerSync } from "../sync/owner";
import { create } from "zustand";
import { parseCommand } from "../domain/commandParser";
import { getTodayFocus, sortDeadlineTasks } from "../domain/taskSorting";
import { DeadlineTask, NewTaskInput } from "../domain/task";
import { AppLanguage } from "../i18n";
import { loadSnapshot } from "../storage/storage";
import { getStateRevision, loadSetting, loadTasks, patchTask, removeTask, replaceTasks, saveAppearancePreferences, saveSetting, saveTask, saveTasks } from "../storage/storage";
import { getSyncNamespace, loadSupabaseSyncSettings, syncTasksWithSupabase } from "../sync/supabaseSync";

export type FocusLimit = 3 | 5 | 10;

interface SyncOptions {
  silent?: boolean;
}

interface LoadOptions {
  silent?: boolean;
}

interface DeadlineState {
  tasks: DeadlineTask[];
  focusLimit: FocusLimit;
  language: AppLanguage;
  appearance: AppearancePreferences;
  setAppearance: (changes: Partial<AppearancePreferences>) => Promise<void>;
  isLoading: boolean;
  error: string | null;
  notificationError: string | null;
  commandMessage: string | null;
  load: (options?: LoadOptions) => Promise<void>;
  setFocusLimit: (limit: FocusLimit) => Promise<void>;
  setLanguage: (language: AppLanguage) => Promise<void>;
  addTask: (input: NewTaskInput) => Promise<boolean>;
  bulkAddTasks: (inputs: NewTaskInput[]) => Promise<boolean>;
  importTasks: (tasks: DeadlineTask[]) => Promise<boolean>;
  updateTask: (id: string, fields: Partial<DeadlineTask>) => Promise<boolean>;
  completeTask: (id: string) => Promise<boolean>;
  restoreTask: (id: string) => Promise<boolean>;
  postponeTask: (id: string, dueAt: string) => Promise<boolean>;
  toggleCurrentTask: (id: string) => Promise<boolean>;
  deleteTask: (id: string) => Promise<boolean>;
  syncWithCloud: (options?: SyncOptions) => Promise<void>;
  runCommand: (input: string) => Promise<boolean>;
  clearCommandMessage: () => void;
}

const FOCUS_LIMIT_SETTING_KEY = "focus_limit";
const LANGUAGE_SETTING_KEY = "app_language";
let syncFlight: Promise<void> | null = null;
let syncAgain = false;
let loadEpoch = 0;

export const useDeadlineStore = create<DeadlineState>((set, get) => ({
  tasks: [],
  focusLimit: 3,
  language: "system",
  appearance: { ...defaultAppearance },
  isLoading: true,
  error: null,
  notificationError: null,
  commandMessage: null,

  async load(options) {
    const epoch = ++loadEpoch;
    const isSilent = options?.silent ?? false;
    if (!isSilent) {
      set({ isLoading: true, error: null });
    }
    try {
      const snapshot = await loadSnapshot();
      if (epoch !== loadEpoch) return;
      const { tasks, focusLimit: storedLimit, language: storedLanguage } = snapshot;
      const appearance = parseAppearance(snapshot.appearance);
      set({
        appearance,
        tasks: sortDeadlineTasks(tasks),
        focusLimit: parseFocusLimit(storedLimit),
        language: parseLanguage(storedLanguage),
        isLoading: false
      });
    } catch (error) {
      if (epoch !== loadEpoch) return;
      set({
        error: getErrorMessage(error),
        isLoading: false
      });
    }
  },

  async setAppearance(changes) {
    try {
      const appearance = await saveAppearancePreferences(changes);
      set({ appearance, error: null });
    } catch (error) { set({ error: getErrorMessage(error) }); }
  },

  async setFocusLimit(limit) {
    try {
      await saveSetting(FOCUS_LIMIT_SETTING_KEY, String(limit));
      set({ focusLimit: limit, error: null });
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async setLanguage(language) {
    try {
      await saveSetting(LANGUAGE_SETTING_KEY, language);
      set({ language, error: null });
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async addTask(input) {
    const task = createTaskFromInput(input);

    try {
      await saveTask(task);
      void get().syncWithCloud({ silent: true }).catch(() => undefined);
      set({ error: null, tasks: sortDeadlineTasks(await loadTasks()) });
      return true;
    } catch (error) {
      set({ error: getErrorMessage(error) });
      return false;
    }
  },

  async bulkAddTasks(inputs) {
    const tasks = inputs.map(createTaskFromInput);
    if (tasks.length === 0) return false;

    try {
      await saveTasks(tasks);
      void get().syncWithCloud({ silent: true }).catch(() => undefined);
      set({ error: null, commandMessage: `已导入 ${tasks.length} 条 Deadline`, tasks: sortDeadlineTasks(await loadTasks()) });
      return true;
    } catch (error) {
      set({ error: getErrorMessage(error) });
      return false;
    }
  },

  async importTasks(importedTasks) {
    let normalizedTasks: DeadlineTask[];
    try { normalizedTasks = validateImportedTasks(importedTasks); }
    catch (error) { set({ error: getErrorMessage(error) }); return false; }
    if (normalizedTasks.length === 0) {
      set({ commandMessage: "没有可导入的事项" });
      return false;
    }

    try {
      await saveTasks(normalizedTasks);
      void get().syncWithCloud({ silent: true }).catch(() => undefined);
      set({ error: null, commandMessage: `已导入 ${normalizedTasks.length} 条事项`, tasks: sortDeadlineTasks(await loadTasks()) });
      return true;
    } catch (error) {
      set({ error: getErrorMessage(error) });
      return false;
    }
  },

  async updateTask(id, fields) {
    const current = get().tasks.find((task) => task.id === id);
    if (!current) return false;

    try {
      await patchTask(id, fields);
      void get().syncWithCloud({ silent: true }).catch(() => undefined);
      set({ error: null, tasks: sortDeadlineTasks(await loadTasks()) });
      return true;
    } catch (error) {
      set({ error: getErrorMessage(error) });
      return false;
    }
  },

  async completeTask(id) {
    return get().updateTask(id, {
      status: "completed",
      isCurrent: false,
      completedAt: new Date().toISOString()
    });
  },

  async restoreTask(id) {
    return get().updateTask(id, {
      status: "active",
      completedAt: null
    });
  },

  async postponeTask(id, dueAt) {
    return get().updateTask(id, {
      dueAt,
      status: "postponed"
    });
  },

  async toggleCurrentTask(id) {
    const tasks = get().tasks;
    const current = tasks.find((task) => task.id === id);
    if (!current || current.status === "completed") return false;

    if (!current.isCurrent && tasks.filter((task) => task.isCurrent && task.status !== "completed").length >= 2) {
      set({ commandMessage: "当前任务最多两个，请先取消一个" });
      return false;
    }

    const ok = await get().updateTask(id, { isCurrent: !current.isCurrent });
    if (ok) set({ commandMessage: null });
    return ok;
  },

  async deleteTask(id) {
    try {
      await removeTask(id, await getSyncNamespace());
      void get().syncWithCloud({ silent: true }).catch(() => undefined);
      set((state) => ({ error: null, tasks: state.tasks.filter((task) => task.id !== id) }));
      return true;
    } catch (error) {
      set({ error: getErrorMessage(error) });
      return false;
    }
  },

  async syncWithCloud(options) {
    if (!isSyncOwner()) return requestOwnerSync(options?.silent ?? false);
    if (syncFlight) { syncAgain = true; return syncFlight; }
    const isSilent = options?.silent ?? false;
    const run = async () => {
    try {
      const settings = await loadSupabaseSyncSettings();
      if (!settings.url || !settings.anonKey || !settings.syncCode) {
        if (isSilent) return;
        throw new Error("Supabase sync is not configured.");
      }
      const revision = await getStateRevision();
      const currentTasks = await loadTasks();
      const prepared = await syncTasksWithSupabase(currentTasks);
      await prepared.assertCurrent();
      const mergedTasks = sortDeadlineTasks(prepared.tasks);
      if (JSON.stringify(sortDeadlineTasks(currentTasks)) !== JSON.stringify(mergedTasks)) await replaceTasks(mergedTasks, revision);
      else if (await getStateRevision() !== revision) throw new Error("revision_conflict");
      await prepared.upload();
      set({
        error: null,
        commandMessage: isSilent ? get().commandMessage : `已同步 ${mergedTasks.length} 条事项`,
        tasks: sortDeadlineTasks(await loadTasks())
      });
    } catch (error) {
      set({ error: getErrorMessage(error) });
      throw error;
    }
    };
    syncFlight = (async () => {
      for (let attempt = 0; attempt < 3; attempt++) {
        syncAgain = false;
        try { await run(); }
        catch (error) { if (String(error).includes("revision_conflict") && attempt < 2) continue; throw error; }
        if (!syncAgain) return;
      }
    })();
    try { await syncFlight; } finally { syncFlight = null; }
  },

  async runCommand(input) {
    const result = parseCommand(input);
    if (!result.ok || !result.command) {
      set({ commandMessage: result.error ?? "命令无法识别" });
      return false;
    }

    const command = result.command;
    if (command.kind === "add") {
      if (!await get().addTask(command.payload)) return false;
      set({ commandMessage: "已添加 Deadline" });
      return true;
    }

    const target = findTask(get().tasks, command.idOrTitle);
    if (!target) {
      set({ commandMessage: "没有找到对应任务" });
      return false;
    }

    if (command.kind === "complete") {
      if (!await get().completeTask(target.id)) return false;
      set({ commandMessage: "已完成" });
      return true;
    }

    if (command.kind === "delete") {
      if (!await get().deleteTask(target.id)) return false;
      set({ commandMessage: "已删除" });
      return true;
    }

    if (!await get().updateTask(target.id, command.fields)) return false;
    set({ commandMessage: "已更新" });
    return true;
  },

  clearCommandMessage() {
    set({ commandMessage: null });
  }
}));

export function selectFocusTasks(tasks: DeadlineTask[], limit: FocusLimit): DeadlineTask[] {
  return getTodayFocus(tasks, limit);
}

export async function resetToTasks(tasks: DeadlineTask[]): Promise<void> {
  await replaceTasks(tasks);
}

function createTaskFromInput(input: NewTaskInput): DeadlineTask {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    title: input.title.trim(),
    dueAt: input.dueAt,
    priority: input.priority,
    status: "active",
    notes: input.notes?.trim() ?? "",
    source: input.source ?? "manual",
    isCurrent: false,
    createdAt: now,
    updatedAt: now,
    completedAt: null
  };
}

function parseFocusLimit(value: string | null): FocusLimit {
  if (value === "5") return 5;
  if (value === "10") return 10;
  return 3;
}

function parseLanguage(value: string | null): AppLanguage {
  if (value === "zh" || value === "ja" || value === "en" || value === "system") {
    return value;
  }
  return "system";
}

function findTask(tasks: DeadlineTask[], idOrTitle: string): DeadlineTask | undefined {
  const needle = idOrTitle.toLowerCase();
  return tasks.find((task) => task.id === idOrTitle || task.title.toLowerCase() === needle);
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : typeof error === "string" ? error : "未知错误";
}
