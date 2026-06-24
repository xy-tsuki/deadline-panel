import { DeadlineTask } from "./task";

export const seedTasks: DeadlineTask[] = [
  {
    id: "seed-graph-mining-quiz",
    title: "示例：课程小测",
    dueAt: "2026-06-23T23:59:00+09:00",
    priority: "high",
    status: "active",
    notes: "示例任务，可直接修改或删除",
    source: "seed",
    isCurrent: false,
    createdAt: "2026-06-21T09:00:00+09:00",
    updatedAt: "2026-06-21T09:00:00+09:00",
    completedAt: null
  },
  {
    id: "seed-lab-report",
    title: "示例：提交报告",
    dueAt: "2026-06-25T18:00:00+09:00",
    priority: "medium",
    status: "active",
    notes: "示例任务，可直接修改或删除",
    source: "seed",
    isCurrent: false,
    createdAt: "2026-06-21T09:05:00+09:00",
    updatedAt: "2026-06-21T09:05:00+09:00",
    completedAt: null
  },
  {
    id: "seed-paper-reading",
    title: "示例：阅读材料",
    dueAt: "2026-06-28T23:59:00+09:00",
    priority: "medium",
    status: "active",
    notes: "示例任务，可直接修改或删除",
    source: "seed",
    isCurrent: false,
    createdAt: "2026-06-21T09:10:00+09:00",
    updatedAt: "2026-06-21T09:10:00+09:00",
    completedAt: null
  }
];
