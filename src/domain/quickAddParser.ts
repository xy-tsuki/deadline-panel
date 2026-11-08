import { NewTaskInput, TaskPriority } from "./task";

export type QuickAddParseError = "empty" | "too-long" | "missing-date" | "missing-title"
  | "invalid-date" | "invalid-time" | "ambiguous-date" | "ambiguous-time" | "unsupported-timezone";

export interface QuickAddParseResult {
  ok: boolean;
  input?: NewTaskInput;
  error?: QuickAddParseError;
}

interface Span { start: number; end: number }
interface Clock { hour: number; minute: number; second: number }
interface DateToken extends Span {
  kind: "calendar" | "weekday" | "instant";
  weekday?: number;
  bareWeekday?: boolean;
  resolve: (clock: Clock) => Date | null;
}

const numeral = "(?:\\d{1,4}|[零〇一二两兩三四五六七八九十百廿]{1,6})";
const period = "凌晨|清晨|早上|早晨|上午|中午|下午|傍晚|晚上|晚间|晚間|夜里|夜裡|午前|午後";
const months = "january|jan|february|feb|march|mar|april|apr|may|june|jun|july|jul|august|aug|september|sept|sep|october|oct|november|nov|december|dec";
const monthNumbers: Record<string, number> = { jan: 1, feb: 2, mar: 3, apr: 4, may: 5, jun: 6, jul: 7, aug: 8, sep: 9, oct: 10, nov: 11, dec: 12 };
const englishNumbers: Record<string, number> = { a: 1, an: 1, one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9, ten: 10 };
const englishNumeral = "(?:\\d{1,4}|a|an|one|two|three|four|five|six|seven|eight|nine|ten)";

/** Local, deterministic parsing. Ambiguous dates/times never silently win. */
export function parseQuickAdd(text: string, now = new Date()): QuickAddParseResult {
  if (!text.trim()) return fail("empty");
  if (text.length > 4096) return fail("too-long");
  if (!Number.isFinite(now.getTime())) return fail("invalid-date");
  const note = text.match(/(?:^|[\s,，;；])(?:备注|備註|注記|メモ|notes?)\s*[:：]\s*/i);
  const notes = note ? text.slice(note.index! + note[0].length).trim() : "";
  const source = (note ? text.slice(0, note.index) : text).normalize("NFKC")
    .replace(/今晚|今夜/g, "今天晚上").replace(/明晚/g, "明天晚上")
    .replace(/今早/g, "今天早上").replace(/明早/g, "明天早上");
  // This entry point uses local time. Keep explicit offsets in JSON/commands.
  if (/\d{1,2}:\d{2}(?::\d{2})?\s*(?:Z\b|[+-]\d{2}:?\d{2})|\b(?:UTC|GMT|JST|CST|EST|PST)\b|北京时间|北京時間|日本时间|日本時間|东京时间|東京時間/i.test(source)) return fail("unsupported-timezone");

  const dates = extractDates(source, now);
  const time = extractTime(mask(source, dates));
  if (time.error) return fail(time.error);
  const clock = time.clock ?? { hour: 23, minute: 59, second: 0 };
  let dueAt: Date | null;
  if (dates.length) {
    const resolved = dates.map(token => token.resolve(clock));
    if (resolved.some(date => !date || !Number.isFinite(date.getTime()))) return fail("invalid-date");
    const reference = dates.findIndex(token => token.kind === "calendar");
    // A bare weekday alongside a date is an annotation, not a second deadline.
    if (reference >= 0) {
      for (const [index, token] of dates.entries()) {
        if (token.bareWeekday && token.weekday === resolved[reference]!.getDay()) resolved[index] = new Date(resolved[reference]!);
      }
    }
    if (resolved.some(date => date!.getTime() !== resolved[0]!.getTime())) return fail("ambiguous-date");
    if (dates.some(token => token.kind === "instant") && time.clock) return fail("ambiguous-time");
    dueAt = resolved[0];
  } else if (time.clock) {
    dueAt = atClock(now, clock);
    if (dueAt.getTime() <= now.getTime()) dueAt = atClock(addDays(now, 1), clock);
  } else {
    return fail("missing-date");
  }
  if (!dates.some(token => token.kind === "instant") && clock.hour < 24
    && (dueAt!.getHours() !== clock.hour || dueAt!.getMinutes() !== clock.minute)) return fail("invalid-time");

  const temporalSpans = [...dates, ...time.spans].map(span => includeGlue(source, span));
  const priority = extractPriority(mask(source, temporalSpans));
  const title = normalizeTitle(priority.text);
  if (!title) return fail("missing-title");
  return { ok: true, input: { title, dueAt: dueAt!.toISOString(), priority: priority.priority, source: "manual", ...(note ? { notes } : {}) } };
}

function extractDates(text: string, now: Date): DateToken[] {
  const tokens: DateToken[] = [];
  const collect = (pattern: RegExp, create: (match: RegExpExecArray) => Omit<DateToken, keyof Span>) => {
    for (const match of matches(text, pattern)) {
      const span = { start: match.index, end: match.index + match[0].length };
      if (!overlaps(span, tokens)) tokens.push({ ...span, ...create(match) });
    }
  };
  const calendar = (year: number | null, month: number, day: number): Omit<DateToken, keyof Span> => ({
    kind: "calendar",
    resolve: clock => {
      let date = makeDate(year ?? now.getFullYear(), month, day);
      if (!date) return null;
      if (year === null && atClock(date, clock).getTime() < now.getTime()) date = makeDate(now.getFullYear() + 1, month, day);
      return date && atClock(date, clock);
    }
  });
  collect(new RegExp(`(?<!\\d)(\\d{4}|[零〇一二三四五六七八九]{4})\\s*(?:年|[-/.])\\s*(${numeral})\\s*(?:月|[-/.])\\s*(${numeral})(?:日|号|號)?(?!\\d)`, "g"), m => calendar(number(m[1]), number(m[2]), number(m[3])));
  collect(new RegExp(`(今年|明年|来年|去年|后年|後年)?\\s*(${numeral})月\\s*(${numeral})(?:日|号|號)?`, "g"), m => calendar(m[1] ? now.getFullYear() + ({ 今年: 0, 明年: 1, 来年: 1, 去年: -1, 后年: 2, 後年: 2 }[m[1]] ?? 0) : null, number(m[2]), number(m[3])));
  collect(/(?<![\d/.-])(\d{1,2})[-/](\d{1,2})(?![\d/.-])/g, m => calendar(null, number(m[1]), number(m[2])));
  collect(new RegExp(`\\b(${months})\\s+(\\d{1,2})(?:st|nd|rd|th)?(?:,?\\s+(\\d{4}))?\\b`, "gi"), m => calendar(m[3] ? number(m[3]) : null, monthNumbers[m[1].slice(0, 3).toLowerCase()], number(m[2])));
  collect(new RegExp(`\\b(\\d{1,2})(?:st|nd|rd|th)?\\s+(${months})(?:,?\\s+(\\d{4}))?\\b`, "gi"), m => calendar(m[3] ? number(m[3]) : null, monthNumbers[m[2].slice(0, 3).toLowerCase()], number(m[1])));

  const relative = (days: number): Omit<DateToken, keyof Span> => ({ kind: "calendar", resolve: clock => atClock(addDays(now, days), clock) });
  collect(/\bday after tomorrow\b|大后天|大後天|后天|後天|あさって|明後日|\b(?:tomorrow|tmr)\b|明天|明日|あした|\btoday\b|今天|今日|きょう|大前天|前天|おととい|一昨日|\byesterday\b|昨天|昨日|きのう/gi, m => {
    const word = m[0].toLowerCase();
    const days = /大后天|大後天/.test(word) ? 3 : /day after tomorrow|后天|後天|あさって|明後日/.test(word) ? 2
      : /tomorrow|tmr|明天|明日|あした/.test(word) ? 1 : /大前天/.test(word) ? -3
        : /前天|おととい|一昨日/.test(word) ? -2 : /yesterday|昨天|昨日|きのう/.test(word) ? -1 : 0;
    return relative(days);
  });
  collect(new RegExp(`(${numeral})\\s*(?:个|個)?\\s*(天|日|周|週|星期)\\s*(后|後|以后|以後|之后|之後|前|之前)`, "g"), m => relative(number(m[1]) * (/周|週|星期/.test(m[2]) ? 7 : 1) * (/前/.test(m[3]) ? -1 : 1)));
  collect(new RegExp(`\\bin\\s+(${englishNumeral})\\s+(days?|weeks?)\\b|\\b(${englishNumeral})\\s+(days?|weeks?)\\s+(?:later|from now)\\b`, "gi"), m => relative(number(m[1] ?? m[3]) * (/week/i.test(m[2] ?? m[4]) ? 7 : 1)));

  const instant = (minutes: number): Omit<DateToken, keyof Span> => ({ kind: "instant", resolve: () => {
    const date = new Date(now.getTime() + minutes * 60_000);
    date.setSeconds(0, 0);
    return date;
  } });
  collect(new RegExp(`(${numeral}|半)\\s*(?:个|個)?\\s*(半)?\\s*(小时|小時|時間)(?:\\s*(${numeral})\\s*(?:分钟|分鐘|分)|\\s*(半))?\\s*(?:后|後|以后|以後|之后|之後)`, "g"), m => instant(((m[1] === "半" ? 0.5 : number(m[1])) + (m[2] ? 0.5 : 0)) * 60 + (m[4] ? number(m[4]) : m[5] ? 30 : 0)));
  collect(new RegExp(`(${numeral})\\s*(?:分钟|分鐘|分)\\s*(?:后|後|以后|以後|之后|之後)`, "g"), m => instant(number(m[1])));
  collect(new RegExp(`\\bin\\s+(${englishNumeral})\\s+(hours?|minutes?)\\b|\\b(${englishNumeral})\\s+(hours?|minutes?)\\s+(?:later|from now)\\b`, "gi"), m => instant(number(m[1] ?? m[3]) * (/hour/i.test(m[2] ?? m[4]) ? 60 : 1)));

  const weekday = (day: number, week: number | null): Omit<DateToken, keyof Span> => ({ kind: "weekday", weekday: day, bareWeekday: week === null, resolve: clock => {
    if (week !== null) {
      const monday = addDays(now, -((now.getDay() + 6) % 7));
      return atClock(addDays(monday, week * 7 + (day + 6) % 7), clock);
    }
    let date = atClock(addDays(now, (day - now.getDay() + 7) % 7), clock);
    if (date.getTime() <= now.getTime()) date = atClock(addDays(date, 7), clock);
    return date;
  } });
  collect(/(下下(?:个|個)?|下(?:个|個)?|这(?:个)?|這(?:個)?|本|上(?:个|個)?)?(?:周|週|星期|礼拜|禮拜)([一二三四五六日天1-7])/g, m => weekday(/日|天|7/.test(m[2]) ? 0 : number(m[2]), !m[1] ? null : m[1].startsWith("下下") ? 2 : m[1].startsWith("下") ? 1 : m[1].startsWith("上") ? -1 : 0));
  collect(/(再来週|来週|今週|先週)?\s*([月火水木金土日])(?:曜日|曜)/g, m => weekday("日月火水木金土".indexOf(m[2]), !m[1] ? null : ({ 再来週: 2, 来週: 1, 今週: 0, 先週: -1 }[m[1]] ?? 0)));
  collect(/\b(?:(next|this|last)\s+)?(monday|mon|tuesday|tue|wednesday|wed|thursday|thu|friday|fri|saturday|sat|sunday|sun)\b/gi, m => weekday({ mon: 1, tue: 2, wed: 3, thu: 4, fri: 5, sat: 6, sun: 0 }[m[2].slice(0, 3).toLowerCase()] ?? 0, m[1] ? ({ next: 1, this: 0, last: -1 }[m[1].toLowerCase()] ?? 0) : null));
  collect(/(下下(?:个|個)?|下(?:个|個)?|这(?:个)?|這(?:個)?|本|上(?:个|個)?)?(?:周末|週末)/g, m => weekday(0, !m[1] ? null : m[1].startsWith("下下") ? 2 : m[1].startsWith("下") ? 1 : m[1].startsWith("上") ? -1 : 0));
  collect(/\b(?:(next|this|last)\s+)?weekend\b/gi, m => weekday(0, m[1] ? ({ next: 1, this: 0, last: -1 }[m[1].toLowerCase()] ?? 0) : null));
  collect(/(?:(下(?:个|個)?月|上(?:个|個)?月|本月|这个月|這個月)(?:底|末)|月底|月末)/g, m => ({ kind: "calendar", resolve: clock => {
    const shift = m[1]?.startsWith("下") ? 1 : m[1]?.startsWith("上") ? -1 : 0;
    return atClock(new Date(now.getFullYear(), now.getMonth() + shift + 1, 0), clock);
  } }));
  collect(new RegExp(`(下(?:个|個)?月|上(?:个|個)?月|本月|这个月|這個月)?\\s*(${numeral})(?:号|號|日)(?![本期游遊])`, "g"), m => ({ kind: "calendar", resolve: clock => {
    const shift = m[1]?.startsWith("下") ? 1 : m[1]?.startsWith("上") ? -1 : 0;
    const anchor = new Date(now.getFullYear(), now.getMonth() + shift, 1);
    let date = makeDate(anchor.getFullYear(), anchor.getMonth() + 1, number(m[2]));
    if (!date) return null;
    if (!m[1] && atClock(date, clock).getTime() < now.getTime()) {
      const nextMonth = new Date(now.getFullYear(), now.getMonth() + 1, 1);
      date = makeDate(nextMonth.getFullYear(), nextMonth.getMonth() + 1, number(m[2]));
    }
    return date && atClock(date, clock);
  } }));
  return tokens;
}

function extractTime(text: string): { clock?: Clock; spans: Span[]; error?: QuickAddParseError } {
  const spans: Span[] = [];
  const clocks: Clock[] = [];
  let invalid = false;
  const collect = (pattern: RegExp, create: (match: RegExpExecArray) => Clock | null) => {
    for (const match of matches(text, pattern)) {
      const span = { start: match.index, end: match.index + match[0].length };
      if (overlaps(span, spans)) continue;
      spans.push(span);
      const clock = create(match);
      if (clock) clocks.push(clock); else invalid = true;
    }
  };
  collect(new RegExp(`(?<![\\da-z])(?:T)?(?:(${period})\\s*)?(\\d{1,2}):(\\d{1,2})(?::(\\d{1,2}))?\\s*(a\\.?m\\.?|p\\.?m\\.?)?(?![\\da-z])`, "gi"), m => makeClock(number(m[2]), number(m[3]), number(m[4] ?? "0"), m[1], m[5]));
  collect(new RegExp(`(?:(${period})\\s*)?(${numeral})\\s*(?:点|點|时|時)\\s*(?:(半|一刻|三刻)|(${numeral})\\s*(?:分(?:钟|鐘)?|(?=\\s|$|前|提交|交|完成)))?`, "g"), m => makeClock(number(m[2]), m[3] === "半" ? 30 : m[3] === "一刻" ? 15 : m[3] === "三刻" ? 45 : number(m[4] ?? "0"), 0, m[1]));
  collect(/\b(\d{1,2})\s*(a\.?m\.?|p\.?m\.?)\b/gi, m => makeClock(number(m[1]), 0, 0, undefined, m[2]));
  collect(/正午|中午|午夜|半夜/g, m => ({ hour: /正午|中午/.test(m[0]) ? 12 : 0, minute: 0, second: 0 }));
  collect(/晚上|晚间|晚間|夜里|夜裡/g, () => ({ hour: 23, minute: 59, second: 0 }));
  const remaining = mask(text, spans);
  if (invalid || new RegExp(`(?:^|\\s)(?:${period})(?=\\s|提交|交|完成|开会|参加|$)`).test(remaining) || /\d{1,4}\s*[:：]|(?:几|幾)点/.test(remaining)) return { spans, error: "invalid-time" };
  if (clocks.some(clock => clock.hour !== clocks[0].hour || clock.minute !== clocks[0].minute || clock.second !== clocks[0].second)) return { spans, error: "ambiguous-time" };
  return { spans, clock: clocks[0] };
}

function makeClock(hour: number, minute: number, second: number, part?: string, meridiem?: string): Clock | null {
  if (![hour, minute, second].every(Number.isInteger) || hour < 0 || hour > 24 || minute < 0 || minute > 59 || second < 0 || second > 59) return null;
  const am = /凌晨|清晨|早上|早晨|上午|午前/.test(part ?? "");
  const pm = /下午|傍晚|晚上|晚间|晚間|夜里|夜裡|午後/.test(part ?? "");
  if (meridiem) {
    const isPm = meridiem.toLowerCase().startsWith("p");
    if (hour < 1 || hour > 12 || (isPm && am) || (!isPm && pm)) return null;
    hour = hour % 12 + (isPm ? 12 : 0);
  } else if (am) {
    if (hour > 12) return null;
    hour %= 12;
  } else if (pm) {
    if (hour === 12 && /晚上|晚间|晚間|夜里|夜裡/.test(part!)) hour = 24;
    else if (hour > 0 && hour < 12) hour += 12;
  } else if (part === "中午") {
    if (hour >= 1 && hour <= 2) hour += 12;
    if (hour < 11 || hour > 14) return null;
  }
  if (hour === 24 && (minute !== 0 || second !== 0)) return null;
  return { hour, minute, second };
}

function extractPriority(text: string): { priority: TaskPriority; text: string } {
  const labels: Record<string, TaskPriority> = { urgent: "urgent", asap: "urgent", p0: "urgent", 紧急: "urgent", 緊急: "urgent", 至急: "urgent", 急ぎ: "urgent", 加急: "urgent", 尽快: "urgent", 盡快: "urgent", 最高: "urgent", high: "high", p1: "high", 高: "high", 重要: "high", medium: "medium", normal: "medium", p2: "medium", 中等: "medium", 中: "medium", 普通: "medium", low: "low", p3: "low", 低: "low", 不急: "low", 低め: "low" };
  const weights: Record<TaskPriority, number> = { urgent: 4, high: 3, medium: 2, low: 1 };
  const found: TaskPriority[] = [];
  const spans: Span[] = [];
  const collect = (pattern: RegExp, label: (m: RegExpExecArray) => string) => {
    for (const match of matches(text, pattern)) {
      const span = { start: match.index, end: match.index + match[0].length };
      if (!overlaps(span, spans)) { spans.push(span); found.push(labels[label(match).toLowerCase()]); }
    }
  };
  collect(/(?:优先级|優先級|優先度|\bpriority)\s*[:=]?\s*(urgent|asap|high|medium|normal|low|p[0-3]|紧急|緊急|加急|最高|重要|高|中等|中|普通|低|不急)/gi, m => m[1]);
  collect(/(紧急|緊急|最高|高|中等|中|低)(?:优先级|優先級|優先度)/g, m => m[1]);
  collect(/\b(urgent|asap|high|medium|normal|low|p[0-3])\b/gi, m => m[1]);
  // Preserve words embedded in titles, such as 重要性分析 and 普通话考试.
  collect(/(?<![\p{L}\p{N}])(紧急|緊急|至急|急ぎ|加急|尽快|盡快|重要|普通|不急|低め)(?![\p{L}\p{N}])/gu, m => m[1]);
  return { priority: found.length ? found.reduce((left, right) => weights[left] >= weights[right] ? left : right) : "medium", text: mask(text, spans) };
}

function number(value: string): number {
  if (/^\d+$/.test(value)) return Number(value);
  if (value.toLowerCase() in englishNumbers) return englishNumbers[value.toLowerCase()];
  const digits: Record<string, number> = { 零: 0, 〇: 0, 一: 1, 二: 2, 两: 2, 兩: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9 };
  if (!/[十百廿]/.test(value)) return Number([...value].map(char => digits[char]).join(""));
  let total = 0, current = 0;
  for (const char of value) {
    if (char === "十" || char === "百") { total += (current || 1) * (char === "十" ? 10 : 100); current = 0; }
    else if (char === "廿") { total += 20; current = 0; }
    else current = digits[char];
  }
  return total + current;
}

function makeDate(year: number, month: number, day: number): Date | null {
  if (![year, month, day].every(Number.isInteger) || year < 100 || year > 9999 || month < 1 || month > 12 || day < 1 || day > 31) return null;
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : null;
}
function addDays(date: Date, days: number): Date { const result = new Date(date); result.setDate(result.getDate() + days); return result; }
function atClock(date: Date, clock: Clock): Date { const result = new Date(date); result.setHours(clock.hour, clock.minute, clock.second, 0); return result; }
function matches(text: string, pattern: RegExp): RegExpExecArray[] { return [...text.matchAll(pattern)]; }
function overlaps(span: Span, spans: Span[]): boolean { return spans.some(other => span.start < other.end && span.end > other.start); }
function mask(text: string, spans: Span[]): string {
  const chars = text.split("");
  for (const span of spans) for (let index = span.start; index < span.end; index++) chars[index] = " ";
  return chars.join("");
}
function includeGlue(text: string, span: Span): Span {
  const before = text.slice(0, span.start).match(/(?:截止(?:到|至)?|截至|在|于|於|\b(?:due(?:\s+by)?|by|on|at))\s*$/i);
  const after = text.slice(span.end).match(/^\s*(?:之前|以前|前(?!端|期|景|置|往|沿)|截止|までに|まで|に|的)/);
  return { start: before ? span.start - before[0].length : span.start, end: after ? span.end + after[0].length : span.end };
}
function normalizeTitle(text: string): string {
  return text.replace(/[|,，;；、]/g, " ").replace(/[（(]\s*[）)]/g, " ").replace(/\s+/g, " ")
    .replace(/^[\s:：。.!！?？()（）\[\]【】]+|[\s:：。.!！?？()（）\[\]【】]+$/g, "").trim();
}
function fail(error: QuickAddParseError): QuickAddParseResult { return { ok: false, error }; }
