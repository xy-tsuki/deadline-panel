export type TaskPriority = "low" | "medium" | "high" | "urgent";
export type TaskStatus = "active" | "completed" | "postponed";

export interface DeadlineTask {
  id: string;
  title: string;
  dueAt: string;
  priority: TaskPriority;
  status: TaskStatus;
  notes: string;
  source: "manual" | "command" | "codex" | "seed";
  isCurrent: boolean;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface NewTaskInput {
  title: string;
  dueAt: string;
  priority: TaskPriority;
  notes?: string;
  source?: DeadlineTask["source"];
}

export const priorityWeight: Record<TaskPriority, number> = {
  urgent: 4,
  high: 3,
  medium: 2,
  low: 1
};

export const priorityLabel: Record<TaskPriority, string> = {
  urgent: "urgent",
  high: "high",
  medium: "medium",
  low: "low"
};

export const statusLabel: Record<TaskStatus, string> = {
  active: "未完成",
  completed: "已完成",
  postponed: "已延期"
};
