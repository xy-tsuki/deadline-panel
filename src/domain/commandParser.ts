import { NewTaskInput, TaskPriority } from "./task";

export type ParsedCommand =
  | { kind: "add"; payload: NewTaskInput }
  | { kind: "complete"; idOrTitle: string }
  | { kind: "delete"; idOrTitle: string }
  | { kind: "update"; idOrTitle: string; fields: Partial<NewTaskInput> };

export interface CommandParseResult {
  ok: boolean;
  command?: ParsedCommand;
  error?: string;
}

const supportedPriorities = new Set<TaskPriority>(["low", "medium", "high", "urgent"]);

export function parseCommand(input: string): CommandParseResult {
  const trimmed = input.trim();
  if (!trimmed.startsWith("/")) {
    return { ok: false, error: "命令需要以 / 开头" };
  }

  const firstSpace = trimmed.search(/\s/);
  const name = firstSpace === -1 ? trimmed : trimmed.slice(0, firstSpace);
  const restText = firstSpace === -1 ? "" : trimmed.slice(firstSpace + 1);
  const args = parseKeyValues(restText);

  if (name === "/add") {
    const title = args.title;
    const due = args.due;
    const priority = normalizePriority(args.priority ?? "medium");

    if (!title) return { ok: false, error: "/add 需要 title" };
    if (!due) return { ok: false, error: "/add 需要 due" };
    if (!priority) return { ok: false, error: "priority 只能是 low, medium, high, urgent" };

    const dueAt = normalizeDue(due);
    if (!dueAt) return { ok: false, error: "due 时间无法识别" };

    return {
      ok: true,
      command: {
        kind: "add",
        payload: {
          title,
          dueAt,
          priority,
          notes: args.notes ?? "",
          source: "command"
        }
      }
    };
  }

  if (name === "/complete" || name === "/delete") {
    const idOrTitle = args.id ?? args.title ?? restText.trim();
    if (!idOrTitle) return { ok: false, error: `${name} 需要 id 或 title` };
    return { ok: true, command: { kind: name.slice(1) as "complete" | "delete", idOrTitle } };
  }

  if (name === "/update") {
    const idOrTitle = args.id ?? args.title;
    if (!idOrTitle) return { ok: false, error: "/update 需要 id 或 title" };

    const fields: Partial<NewTaskInput> = {};
    if (args.newTitle) fields.title = args.newTitle;
    if (args.due) {
      const dueAt = normalizeDue(args.due);
      if (!dueAt) return { ok: false, error: "due 时间无法识别" };
      fields.dueAt = dueAt;
    }
    if (args.priority) {
      const priority = normalizePriority(args.priority);
      if (!priority) return { ok: false, error: "priority 只能是 low, medium, high, urgent" };
      fields.priority = priority;
    }
    if (args.notes) fields.notes = args.notes;

    return { ok: true, command: { kind: "update", idOrTitle, fields } };
  }

  return { ok: false, error: "支持 /add, /update, /delete, /complete" };
}

function normalizePriority(value: string): TaskPriority | null {
  const lower = value.toLowerCase() as TaskPriority;
  return supportedPriorities.has(lower) ? lower : null;
}

function normalizeDue(value: string): string | null {
  const normalized = value.replace(/\//g, "-").replace(" ", "T");
  const date = new Date(normalized);
  if (Number.isNaN(date.getTime())) {
    return null;
  }
  return date.toISOString();
}

function parseKeyValues(text: string): Record<string, string> {
  const args: Record<string, string> = {};
  const pattern = /(\w+)=("([^"]*)"|[^\s"]+)/g;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    args[match[1]] = match[3] ?? match[2];
  }

  return args;
}
