import { DeadlineTask, priorityWeight } from "./task";

export function sortDeadlineTasks(tasks: DeadlineTask[]): DeadlineTask[] {
  return [...tasks].sort((a, b) => {
    const dueDiff = new Date(a.dueAt).getTime() - new Date(b.dueAt).getTime();
    if (dueDiff !== 0) return dueDiff;

    const priorityDiff = priorityWeight[b.priority] - priorityWeight[a.priority];
    if (priorityDiff !== 0) return priorityDiff;

    if (a.status !== b.status) {
      if (a.status === "active") return -1;
      if (b.status === "active") return 1;
    }

    return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
  });
}

export function getActionableTasks(tasks: DeadlineTask[]): DeadlineTask[] {
  return sortDeadlineTasks(tasks.filter((task) => task.status !== "completed"));
}

export function getTodayFocus(tasks: DeadlineTask[], limit = 3): DeadlineTask[] {
  return getActionableTasks(tasks).slice(0, limit);
}

export function getCurrentTasks(tasks: DeadlineTask[]): DeadlineTask[] {
  return sortDeadlineTasks(tasks.filter((task) => task.status !== "completed" && task.isCurrent)).slice(0, 2);
}

export function getNearestDeadline(tasks: DeadlineTask[]): DeadlineTask | undefined {
  return getActionableTasks(tasks)[0];
}
