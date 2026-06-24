import { DeadlineTask, TaskPriority } from "./task";

interface SeedTaskSpec {
  id: string;
  title: string;
  dayOffset: number;
  hour: number;
  minute: number;
  priority: TaskPriority;
}

const seedTaskSpecs: SeedTaskSpec[] = [
  {
    id: "seed-graph-mining-quiz",
    title: "示例：课程小测",
    dayOffset: 1,
    hour: 23,
    minute: 59,
    priority: "high"
  },
  {
    id: "seed-lab-report",
    title: "示例：提交报告",
    dayOffset: 3,
    hour: 18,
    minute: 0,
    priority: "medium"
  },
  {
    id: "seed-paper-reading",
    title: "示例：阅读材料",
    dayOffset: 7,
    hour: 23,
    minute: 59,
    priority: "medium"
  }
];

export const seedTasks: DeadlineTask[] = createSeedTasks();

function createSeedTasks(): DeadlineTask[] {
  const now = new Date();
  return seedTaskSpecs.map((spec, index) => {
    const dueAt = new Date(now);
    dueAt.setDate(now.getDate() + spec.dayOffset);
    dueAt.setHours(spec.hour, spec.minute, 0, 0);

    const createdAt = new Date(now.getTime() + index * 60 * 1000).toISOString();
    return {
      id: spec.id,
      title: spec.title,
      dueAt: dueAt.toISOString(),
      priority: spec.priority,
      status: "active",
      notes: "示例任务，可直接修改或删除",
      source: "seed",
      isCurrent: false,
      createdAt,
      updatedAt: createdAt,
      completedAt: null
    };
  });
}
