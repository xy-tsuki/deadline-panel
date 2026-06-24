import { create } from "zustand";
import { parseCommand } from "../domain/commandParser";
import { getTodayFocus, sortDeadlineTasks } from "../domain/taskSorting";
import { DeadlineTask, NewTaskInput } from "../domain/task";
import { AppLanguage } from "../i18n";
import { loadSetting, loadTasks, removeTask, replaceTasks, saveSetting, saveTask } from "../storage/storage";

export type FocusLimit = 3 | 5 | 10;

interface DeadlineState {
  tasks: DeadlineTask[];
  focusLimit: FocusLimit;
  language: AppLanguage;
  isLoading: boolean;
  error: string | null;
  commandMessage: string | null;
  load: () => Promise<void>;
  setFocusLimit: (limit: FocusLimit) => Promise<void>;
  setLanguage: (language: AppLanguage) => Promise<void>;
  addTask: (input: NewTaskInput) => Promise<void>;
  bulkAddTasks: (inputs: NewTaskInput[]) => Promise<void>;
  importTasks: (tasks: DeadlineTask[]) => Promise<void>;
  updateTask: (id: string, fields: Partial<DeadlineTask>) => Promise<void>;
  completeTask: (id: string) => Promise<void>;
  restoreTask: (id: string) => Promise<void>;
  postponeTask: (id: string, dueAt: string) => Promise<void>;
  toggleCurrentTask: (id: string) => Promise<void>;
  deleteTask: (id: string) => Promise<void>;
  runCommand: (input: string) => Promise<void>;
  clearCommandMessage: () => void;
}

const FOCUS_LIMIT_SETTING_KEY = "focus_limit";
const LANGUAGE_SETTING_KEY = "app_language";

export const useDeadlineStore = create<DeadlineState>((set, get) => ({
  tasks: [],
  focusLimit: 3,
  language: "system",
  isLoading: true,
  error: null,
  commandMessage: null,

  async load() {
    set({ isLoading: true, error: null });
    try {
      const tasks = await loadTasks();
      const storedLimit = await loadSetting(FOCUS_LIMIT_SETTING_KEY).catch(() => null);
      const storedLanguage = await loadSetting(LANGUAGE_SETTING_KEY).catch(() => null);
      set({
        tasks: sortDeadlineTasks(tasks),
        focusLimit: parseFocusLimit(storedLimit),
        language: parseLanguage(storedLanguage),
        isLoading: false
      });
    } catch (error) {
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  async setFocusLimit(limit) {
    set({ focusLimit: limit });
    try {
      await saveSetting(FOCUS_LIMIT_SETTING_KEY, String(limit));
      set({ error: null });
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async setLanguage(language) {
    set({ language });
    try {
      await saveSetting(LANGUAGE_SETTING_KEY, language);
      set({ error: null });
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async addTask(input) {
    const task = createTaskFromInput(input);

    try {
      await saveTask(task);
      set((state) => ({ error: null, tasks: sortDeadlineTasks([...state.tasks, task]) }));
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async bulkAddTasks(inputs) {
    const tasks = inputs.map(createTaskFromInput);
    if (tasks.length === 0) return;

    try {
      await Promise.all(tasks.map((task) => saveTask(task)));
      set((state) => ({
        error: null,
        commandMessage: `已导入 ${tasks.length} 条 Deadline`,
        tasks: sortDeadlineTasks([...state.tasks, ...tasks])
      }));
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async importTasks(importedTasks) {
    const normalizedTasks = importedTasks.map(normalizeImportedTask).filter(Boolean) as DeadlineTask[];
    if (normalizedTasks.length === 0) {
      set({ commandMessage: "没有可导入的事项" });
      return;
    }

    try {
      await Promise.all(normalizedTasks.map((task) => saveTask(task)));
      set((state) => {
        const merged = new Map(state.tasks.map((task) => [task.id, task]));
        for (const task of normalizedTasks) {
          merged.set(task.id, task);
        }
        return {
          error: null,
          commandMessage: `已导入 ${normalizedTasks.length} 条事项`,
          tasks: sortDeadlineTasks([...merged.values()])
        };
      });
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async updateTask(id, fields) {
    const current = get().tasks.find((task) => task.id === id);
    if (!current) return;

    const updated: DeadlineTask = {
      ...current,
      ...fields,
      updatedAt: new Date().toISOString()
    };

    try {
      await saveTask(updated);
      set((state) => ({
        error: null,
        tasks: sortDeadlineTasks(state.tasks.map((task) => (task.id === id ? updated : task)))
      }));
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async completeTask(id) {
    await get().updateTask(id, {
      status: "completed",
      isCurrent: false,
      completedAt: new Date().toISOString()
    });
  },

  async restoreTask(id) {
    await get().updateTask(id, {
      status: "active",
      completedAt: null
    });
  },

  async postponeTask(id, dueAt) {
    await get().updateTask(id, {
      dueAt,
      status: "postponed"
    });
  },

  async toggleCurrentTask(id) {
    const tasks = get().tasks;
    const current = tasks.find((task) => task.id === id);
    if (!current || current.status === "completed") return;

    if (!current.isCurrent && tasks.filter((task) => task.isCurrent && task.status !== "completed").length >= 2) {
      set({ commandMessage: "当前任务最多两个，请先取消一个" });
      return;
    }

    await get().updateTask(id, { isCurrent: !current.isCurrent });
    set({ commandMessage: null });
  },

  async deleteTask(id) {
    try {
      await removeTask(id);
      set((state) => ({ error: null, tasks: state.tasks.filter((task) => task.id !== id) }));
    } catch (error) {
      set({ error: getErrorMessage(error) });
    }
  },

  async runCommand(input) {
    const result = parseCommand(input);
    if (!result.ok || !result.command) {
      set({ commandMessage: result.error ?? "命令无法识别" });
      return;
    }

    const command = result.command;
    if (command.kind === "add") {
      await get().addTask(command.payload);
      set({ commandMessage: "已添加 Deadline" });
      return;
    }

    const target = findTask(get().tasks, command.idOrTitle);
    if (!target) {
      set({ commandMessage: "没有找到对应任务" });
      return;
    }

    if (command.kind === "complete") {
      await get().completeTask(target.id);
      set({ commandMessage: "已完成" });
      return;
    }

    if (command.kind === "delete") {
      await get().deleteTask(target.id);
      set({ commandMessage: "已删除" });
      return;
    }

    await get().updateTask(target.id, command.fields);
    set({ commandMessage: "已更新" });
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

function normalizeImportedTask(task: DeadlineTask): DeadlineTask | null {
  if (!task || !task.title || !task.dueAt || !task.priority) {
    return null;
  }

  const now = new Date().toISOString();
  return {
    id: task.id || crypto.randomUUID(),
    title: String(task.title).trim(),
    dueAt: task.dueAt,
    priority: task.priority,
    status: task.status ?? "active",
    notes: task.notes ?? "",
    source: task.source ?? "manual",
    isCurrent: task.isCurrent ?? false,
    createdAt: task.createdAt ?? now,
    updatedAt: now,
    completedAt: task.completedAt ?? null
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
  return error instanceof Error ? error.message : "未知错误";
}
