use crate::model::{DeadlineSource, NewTaskInput, TaskPriority};
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Weekday};
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickAddParseResult {
    Ok(NewTaskInput),
    Error(&'static str),
}

static TIME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([01]?\d|2[0-3])[:：]([0-5]\d)\b").unwrap());
static ISO_DATE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(20\d{2})[-/](\d{1,2})[-/](\d{1,2})\b").unwrap());
static MONTH_DAY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|\s)(\d{1,2})[-/](\d{1,2})(?:\s|$)").unwrap());

pub fn parse_quick_add(text: &str) -> QuickAddParseResult {
    parse_quick_add_with_now(text, FixedOffset::east_opt(0).unwrap().from_utc_datetime(
        &chrono::Utc::now().naive_utc(),
    ))
}

pub fn parse_quick_add_with_now(text: &str, now: DateTime<FixedOffset>) -> QuickAddParseResult {
    let mut remaining = text.trim().to_string();
    if remaining.is_empty() {
        return QuickAddParseResult::Error("empty");
    }

    let priority = extract_priority(&mut remaining);
    let (hour, minute) = extract_time(&mut remaining);
    let Some(date) = extract_date(&mut remaining, now) else {
        return QuickAddParseResult::Error("missing-date");
    };

    let title = normalize_title(&remaining);
    if title.is_empty() {
        return QuickAddParseResult::Error("missing-title");
    }

    let due_at = now
        .offset()
        .from_local_datetime(&date.and_hms_opt(hour, minute, 0).unwrap())
        .single()
        .unwrap()
        .to_rfc3339();

    QuickAddParseResult::Ok(NewTaskInput {
        title,
        due_at,
        priority,
        notes: None,
        source: Some(DeadlineSource::Manual),
    })
}

fn extract_priority(text: &mut String) -> TaskPriority {
    let aliases: &[(&str, TaskPriority)] = &[
        (r"(?i)\b(urgent|asap)\b|紧急|至急|急ぎ", TaskPriority::Urgent),
        (r"(?i)\bhigh\b|高优先级|高優先度|重要", TaskPriority::High),
        (r"(?i)\bmedium\b|中优先级|中優先度|普通", TaskPriority::Medium),
        (r"(?i)\blow\b|低优先级|低優先度|低め", TaskPriority::Low),
    ];

    for (pattern, priority) in aliases {
        let re = Regex::new(pattern).unwrap();
        if re.is_match(text) {
            *text = re.replace(text, " ").to_string();
            return *priority;
        }
    }

    TaskPriority::Medium
}

fn extract_time(text: &mut String) -> (u32, u32) {
    let Some(captures) = TIME_RE.captures(text) else {
        return (23, 59);
    };
    let matched = captures.get(0).unwrap().as_str().to_string();
    let hour = captures[1].parse::<u32>().unwrap();
    let minute = captures[2].parse::<u32>().unwrap();
    *text = text.replacen(&matched, " ", 1);
    (hour, minute)
}

fn extract_date(text: &mut String, now: DateTime<FixedOffset>) -> Option<NaiveDate> {
    if let Some(captures) = ISO_DATE_RE.captures(text) {
        let matched = captures.get(0).unwrap().as_str().to_string();
        let date = NaiveDate::from_ymd_opt(
            captures[1].parse().ok()?,
            captures[2].parse().ok()?,
            captures[3].parse().ok()?,
        )?;
        *text = text.replacen(&matched, " ", 1);
        return Some(date);
    }

    if let Some(captures) = MONTH_DAY_RE.captures(text) {
        let matched = captures.get(0).unwrap().as_str().to_string();
        let date = NaiveDate::from_ymd_opt(
            now.year(),
            captures[1].parse().ok()?,
            captures[2].parse().ok()?,
        )?;
        *text = text.replacen(&matched, " ", 1);
        return Some(date);
    }

    let relative: &[(&str, i64)] = &[
        (r"(?i)\b(day after tomorrow)\b|后天|後天|あさって", 2),
        (r"(?i)\b(today)\b|今天|今日|きょう", 0),
        (r"(?i)\b(tomorrow|tmr)\b|明天|明日|あした", 1),
    ];
    for (pattern, days) in relative {
        let re = Regex::new(pattern).unwrap();
        if re.is_match(text) {
            *text = re.replace(text, " ").to_string();
            return Some(now.date_naive() + Duration::days(*days));
        }
    }

    let weekdays: &[(&str, Weekday)] = &[
        (r"(?i)\b(mon|monday)\b|周一|星期一|月曜|月曜日", Weekday::Mon),
        (r"(?i)\b(tue|tuesday)\b|周二|星期二|火曜|火曜日", Weekday::Tue),
        (r"(?i)\b(wed|wednesday)\b|周三|星期三|水曜|水曜日", Weekday::Wed),
        (r"(?i)\b(thu|thursday)\b|周四|星期四|木曜|木曜日", Weekday::Thu),
        (r"(?i)\b(fri|friday)\b|周五|星期五|金曜|金曜日", Weekday::Fri),
        (r"(?i)\b(sat|saturday)\b|周六|星期六|土曜|土曜日", Weekday::Sat),
        (r"(?i)\b(sun|sunday)\b|周日|周天|星期日|星期天|日曜|日曜日", Weekday::Sun),
    ];
    for (pattern, weekday) in weekdays {
        let re = Regex::new(pattern).unwrap();
        if re.is_match(text) {
            *text = re.replace(text, " ").to_string();
            return Some(next_weekday(now.date_naive(), *weekday));
        }
    }

    None
}

fn next_weekday(now: NaiveDate, weekday: Weekday) -> NaiveDate {
    let current = now.weekday().num_days_from_sunday() as i64;
    let target = weekday.num_days_from_sunday() as i64;
    let diff = (target - current + 7) % 7;
    now + Duration::days(if diff == 0 { 7 } else { diff })
}

fn normalize_title(text: &str) -> String {
    text.replace(['|', '｜', ',', '，'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(9 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 7, 9, 12, 0, 0)
            .unwrap()
    }

    #[test]
    fn parses_quick_add_defaults_like_typescript() {
        let QuickAddParseResult::Ok(input) =
            parse_quick_add_with_now("课程小测 明天 high", fixed_now())
        else {
            panic!("expected parse success");
        };

        assert_eq!(input.title, "课程小测");
        assert_eq!(input.priority, TaskPriority::High);
        assert_eq!(input.source, Some(DeadlineSource::Manual));
        assert_eq!(input.due_at, "2026-07-10T23:59:00+09:00");
    }

    #[test]
    fn weekday_targets_next_future_day() {
        let QuickAddParseResult::Ok(input) =
            parse_quick_add_with_now("复盘 周四 10:30", fixed_now())
        else {
            panic!("expected parse success");
        };

        assert_eq!(input.due_at, "2026-07-16T10:30:00+09:00");
    }
}
