import { AppLanguage, getStrings, localeForLanguage } from "../i18n";

export function formatDue(isoDate: string, language: AppLanguage = "zh"): string {
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) {
    return isoDate;
  }

  return new Intl.DateTimeFormat(localeForLanguage(language), {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false
  })
    .format(date)
    .replace(/\//g, "-");
}

export function formatTimeLeft(isoDate: string, now = new Date(), language: AppLanguage = "zh"): string {
  const text = getStrings(language).time;
  const due = new Date(isoDate);
  if (Number.isNaN(due.getTime())) {
    return text.unknown;
  }

  const diffMs = due.getTime() - now.getTime();
  const absMs = Math.abs(diffMs);
  const minuteMs = 60 * 1000;
  const hourMs = 60 * minuteMs;
  const dayMs = 24 * hourMs;

  if (diffMs < 0) {
    if (absMs < hourMs) return text.overdueUnderHour;
    if (absMs < dayMs) return interpolate(text.overdueHours, Math.ceil(absMs / hourMs));
    return interpolate(text.overdueDays, Math.ceil(absMs / dayMs));
  }

  if (diffMs < hourMs) return text.underHour;
  if (diffMs < dayMs) return interpolate(text.hours, Math.ceil(diffMs / hourMs));
  return interpolate(text.days, Math.ceil(diffMs / dayMs));
}

export function toDateTimeLocalValue(isoDate: string): string {
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) {
    return "";
  }

  const offsetMs = date.getTimezoneOffset() * 60 * 1000;
  return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
}

export function fromDateTimeLocalValue(value: string): string {
  const date = new Date(value);
  return date.toISOString();
}

function interpolate(template: string, count: number): string {
  return template.replace("{count}", String(count));
}
