import { isValidAbsoluteTimestamp } from "./timestamp";
import { DeadlineTask } from "./task";

export function validateImportedTasks(values: unknown[], now = new Date().toISOString()): DeadlineTask[] {
  const tasks = new Map<string, DeadlineTask>();
  const errors: string[] = [];
  for (const [index, value] of values.entries()) {
    try {
      if (!value || typeof value !== "object") throw new Error("invalid task object");
      const item = value as Partial<DeadlineTask>;
      if (typeof item.title !== "string" || !item.title.trim()) throw new Error("missing title");
      if (!["urgent", "high", "medium", "low"].includes(String(item.priority))) throw new Error("invalid priority");
      const status = item.status ?? "active";
      if (!["active", "completed", "postponed"].includes(status)) throw new Error("invalid status");
      const source = item.source ?? "manual";
      if (!["manual", "command", "codex", "seed"].includes(source)) throw new Error("invalid source");
      const timestamp = (value: unknown, label: string): string => {
        if (!isValidAbsoluteTimestamp(value)) throw new Error(`invalid ${label}`);
        return value;
      };
      const createdAt = timestamp(item.createdAt ?? now, "createdAt");
      const updatedAt = timestamp(item.updatedAt ?? createdAt, "updatedAt");
      const dueAt = timestamp(item.dueAt, "dueAt");
      const completedAt = item.completedAt == null ? null : timestamp(item.completedAt, "completedAt");
      if (item.notes != null && typeof item.notes !== "string") throw new Error("invalid notes");
      if (item.id != null && (typeof item.id !== "string" || !item.id.trim())) throw new Error("invalid id");
      if (item.isCurrent != null && typeof item.isCurrent !== "boolean") throw new Error("invalid isCurrent");
      const task: DeadlineTask = { id: item.id ?? crypto.randomUUID(), title: item.title.trim(), priority: item.priority!, status, source, notes: item.notes ?? "", isCurrent: item.isCurrent ?? false, createdAt, updatedAt, dueAt, completedAt };
      const existing = tasks.get(task.id);
      if (!existing || Date.parse(task.updatedAt) >= Date.parse(existing.updatedAt)) tasks.set(task.id, task);
    } catch (error) { errors.push(`#${index + 1}: ${String(error)}`); }
  }
  if (errors.length) throw new Error(errors.join("\n"));
  return [...tasks.values()];
}
