import { NewTaskInput, TaskPriority } from "./task";

export interface QuickAddParseResult {
  ok: boolean;
  input?: NewTaskInput;
  error?: string;
}

const priorityAliases: Array<[RegExp, TaskPriority]> = [
  [/\b(urgent|asap)\b|紧急|至急|急ぎ/i, "urgent"],
  [/\bhigh\b|高优先级|高優先度|重要/i, "high"],
  [/\bmedium\b|中优先级|中優先度|普通/i, "medium"],
  [/\blow\b|低优先级|低優先度|低め/i, "low"]
];

const weekdayAliases: Array<[RegExp, number]> = [
  [/\b(mon|monday)\b|周一|星期一|月曜|月曜日/i, 1],
  [/\b(tue|tuesday)\b|周二|星期二|火曜|火曜日/i, 2],
  [/\b(wed|wednesday)\b|周三|星期三|水曜|水曜日/i, 3],
  [/\b(thu|thursday)\b|周四|星期四|木曜|木曜日/i, 4],
  [/\b(fri|friday)\b|周五|星期五|金曜|金曜日/i, 5],
  [/\b(sat|saturday)\b|周六|星期六|土曜|土曜日/i, 6],
  [/\b(sun|sunday)\b|周日|周天|星期日|星期天|日曜|日曜日/i, 0]
];

export function parseQuickAdd(text: string, now = new Date()): QuickAddParseResult {
  let remaining = text.trim();
  if (!remaining) {
    return { ok: false, error: "empty" };
  }

  const priority = extractPriority(remaining);
  remaining = priority.text;

  const time = extractTime(remaining);
  remaining = time.text;

  const date = extractDate(remaining, now);
  if (!date.date) {
    return { ok: false, error: "missing-date" };
  }
  remaining = date.text;

  const title = normalizeTitle(remaining);
  if (!title) {
    return { ok: false, error: "missing-title" };
  }

  const dueAt = new Date(date.date);
  dueAt.setHours(time.hour, time.minute, 0, 0);

  return {
    ok: true,
    input: {
      title,
      dueAt: dueAt.toISOString(),
      priority: priority.priority,
      source: "manual"
    }
  };
}

function extractPriority(text: string): { priority: TaskPriority; text: string } {
  for (const [pattern, priority] of priorityAliases) {
    if (pattern.test(text)) {
      return { priority, text: text.replace(pattern, " ") };
    }
  }
  return { priority: "medium", text };
}

function extractTime(text: string): { hour: number; minute: number; text: string } {
  const match = text.match(/\b([01]?\d|2[0-3])[:：]([0-5]\d)\b/);
  if (!match) {
    return { hour: 23, minute: 59, text };
  }

  return {
    hour: Number(match[1]),
    minute: Number(match[2]),
    text: text.replace(match[0], " ")
  };
}

function extractDate(text: string, now: Date): { date: Date | null; text: string } {
  const iso = text.match(/\b(20\d{2})[-/](\d{1,2})[-/](\d{1,2})\b/);
  if (iso) {
    return {
      date: makeDate(Number(iso[1]), Number(iso[2]), Number(iso[3])),
      text: text.replace(iso[0], " ")
    };
  }

  const monthDay = text.match(/(?:^|\s)(\d{1,2})[-/](\d{1,2})(?:\s|$)/);
  if (monthDay) {
    return {
      date: makeDate(now.getFullYear(), Number(monthDay[1]), Number(monthDay[2])),
      text: text.replace(monthDay[0], " ")
    };
  }

  const relative = [
    { pattern: /\b(day after tomorrow)\b|后天|後天|あさって/i, days: 2 },
    { pattern: /\b(today)\b|今天|今日|きょう|今日/i, days: 0 },
    { pattern: /\b(tomorrow|tmr)\b|明天|明日|あした|明日/i, days: 1 }
  ];
  for (const item of relative) {
    if (item.pattern.test(text)) {
      const date = new Date(now);
      date.setDate(date.getDate() + item.days);
      return { date, text: text.replace(item.pattern, " ") };
    }
  }

  for (const [pattern, weekday] of weekdayAliases) {
    if (pattern.test(text)) {
      return { date: nextWeekday(now, weekday), text: text.replace(pattern, " ") };
    }
  }

  return { date: null, text };
}

function makeDate(year: number, month: number, day: number): Date | null {
  const date = new Date(year, month - 1, day);
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day) {
    return null;
  }
  return date;
}

function nextWeekday(now: Date, weekday: number): Date {
  const date = new Date(now);
  const diff = (weekday - date.getDay() + 7) % 7 || 7;
  date.setDate(date.getDate() + diff);
  return date;
}

function normalizeTitle(text: string): string {
  return text
    .replace(/[|｜,，]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
