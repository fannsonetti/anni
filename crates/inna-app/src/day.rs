//! Where we are in the school day: drives the progress bar and the schedule's breaks.
//! Ported from Hermes (`Data/DayState.cs`, `Data/Breaks.cs`).
use crate::{locale, planner, ui::assignments::subject};
use chrono::{Duration, NaiveDateTime, Timelike};
use inna_core::models::TimetableEntry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    NoSchool,
    Before,
    InClass,
    Break,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub subject: String,
    pub is_break: bool,
}

impl Segment {
    pub fn fill(&self, now: NaiveDateTime) -> f32 {
        let done = (now - self.start).num_seconds() as f32;
        (done / (self.end - self.start).num_seconds() as f32).clamp(0.0, 1.0)
    }
}

pub struct DayState {
    pub phase: Phase,
    /// Classes and listed breaks: today's, or the next school day's when there is no school today.
    pub segments: Vec<Segment>,
    /// The first part is emphasised.
    pub headline: Vec<String>,
    pub footer: String,
}

/// Every timed lesson, sorted. Lessons without a usable start and end cannot go on a timeline.
pub fn lessons(timetable: &[TimetableEntry]) -> Vec<Segment> {
    let mut lessons: Vec<_> = timetable
        .iter()
        .filter(|entry| entry.is_lesson())
        .filter_map(|entry| {
            let start = planner::parse_deadline(entry.start.as_deref()?)?.at;
            let end = planner::parse_deadline(entry.end.as_deref()?)?.at;
            (start < end).then(|| Segment {
                start,
                end,
                subject: subject(entry.course_code()).to_string(),
                is_break: false,
            })
        })
        .collect();
    lessons.sort_by_key(|lesson| lesson.start);
    lessons
}

/// Gaps of 15 minutes or more in one day's sorted lessons.
pub fn breaks(day: &[Segment]) -> Vec<Segment> {
    let mut breaks = Vec::new();
    let Some(first) = day.first() else {
        return breaks;
    };
    let mut from = first.end;
    for lesson in day {
        if lesson.start - from >= Duration::minutes(15) {
            breaks.push(Segment {
                start: from,
                end: lesson.start,
                subject: break_name(from, lesson.start).into(),
                is_break: true,
            });
        }
        from = from.max(lesson.end);
    }
    breaks
}

/// A long gap around midday is lunch; anything else is a break.
fn break_name(from: NaiveDateTime, to: NaiveDateTime) -> &'static str {
    let lunch = to - from >= Duration::minutes(30)
        && (from.hour(), from.minute()) < (13, 30)
        && (to.hour(), to.minute()) > (11, 30);
    if lunch {
        "Lunch break"
    } else {
        "Break"
    }
}

fn with_breaks(day: Vec<Segment>) -> Vec<Segment> {
    let breaks = breaks(&day);
    let mut segments = day;
    segments.extend(breaks);
    segments.sort_by_key(|segment| segment.start);
    segments
}

fn clock(time: NaiveDateTime) -> String {
    time.format("%H:%M").to_string()
}

fn duration(span: Duration) -> String {
    let minutes = (span.num_seconds().max(0) + 59) / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

fn next_day_text(first: &Segment, now: NaiveDateTime) -> String {
    let day = if (first.start.date() - now.date()).num_days() == 1 {
        "tomorrow".to_string()
    } else {
        first.start.format("%A").to_string()
    };
    format!("{day} {} {}", first.subject, clock(first.start))
}

impl DayState {
    pub fn compute(timetable: &[TimetableEntry], now: NaiveDateTime) -> Self {
        let all = lessons(timetable);
        let today: Vec<_> = all
            .iter()
            .filter(|lesson| lesson.start.date() == now.date())
            .cloned()
            .collect();
        let next_school_day = all.iter().find(|lesson| lesson.start.date() > now.date());
        let next_day = next_school_day.map(|first| next_day_text(first, now));

        let (Some(first), Some(last_end)) =
            (today.first(), today.iter().map(|lesson| lesson.end).max())
        else {
            let segments = next_school_day.map_or_else(Vec::new, |first| {
                with_breaks(
                    all.iter()
                        .filter(|lesson| lesson.start.date() == first.start.date())
                        .cloned()
                        .collect(),
                )
            });
            return Self {
                phase: Phase::NoSchool,
                segments,
                headline: std::iter::once("No school today".to_string())
                    .chain(next_day)
                    .collect(),
                footer: String::new(),
            };
        };

        let first_start = first.start;
        let first_subject = first.subject.clone();
        let current = today
            .iter()
            .find(|lesson| lesson.start <= now && now < lesson.end)
            .cloned();
        let next = today.iter().find(|lesson| lesson.start > now).cloned();
        let previous_end = today
            .iter()
            .map(|lesson| lesson.end)
            .filter(|end| *end <= now)
            .max();
        let segments = with_breaks(today);
        let elapsed = (now - first_start).num_seconds() as f64;
        let total = (last_end - first_start).num_seconds() as f64;
        let percent = (elapsed / total * 100.0).clamp(0.0, 100.0).floor();
        let progress = format!("Day {percent}% done · school ends {}", clock(last_end));
        let next_text =
            |lesson: &Segment| format!("next {} {}", lesson.subject, clock(lesson.start));

        if now < first_start {
            return Self {
                phase: Phase::Before,
                segments,
                headline: vec![
                    format!("First class: {first_subject} at {}", clock(first_start)),
                    format!("in {}", duration(first_start - now)),
                ],
                footer: format!("School ends {}", clock(last_end)),
            };
        }
        if now >= last_end {
            return Self {
                phase: Phase::After,
                segments,
                headline: std::iter::once("School's out".to_string())
                    .chain(next_day)
                    .collect(),
                footer: format!("School ended {}", clock(last_end)),
            };
        }
        if let Some(current) = current {
            return Self {
                phase: Phase::InClass,
                headline: vec![
                    current.subject.clone(),
                    format!("{} left", duration(current.end - now)),
                    next.as_ref()
                        .map_or_else(|| "last class".to_string(), next_text),
                ],
                segments,
                footer: progress,
            };
        }
        // Between classes, so a later lesson exists: `now` is before the last end.
        let next = next.expect("a gap always has a next lesson");
        let name = segments
            .iter()
            .find(|s| s.is_break && s.start <= now && now < s.end)
            .map_or_else(
                || break_name(previous_end.unwrap_or(now), next.start).to_string(),
                |s| s.subject.clone(),
            );
        Self {
            phase: Phase::Break,
            segments,
            headline: vec![
                name,
                format!("{} left", duration(next.start - now)),
                next_text(&next),
            ],
            footer: progress,
        }
    }
}

/// Hermes' countdown: "overdue", "today 22:00", "tomorrow", "4 days".
pub fn countdown(deadline: planner::Deadline, now: NaiveDateTime) -> String {
    let days = (deadline.at.date() - now.date()).num_days();
    let time = if deadline.has_time && (0..=1).contains(&days) {
        format!(" {}", clock(deadline.at))
    } else {
        String::new()
    };
    match days {
        _ if deadline.at < now => "overdue".into(),
        0 => format!("today{time}"),
        1 => format!("tomorrow{time}"),
        days => locale::counted(days as usize, "day", "days"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timetable() -> Vec<TimetableEntry> {
        serde_json::from_value(serde_json::json!([
            {"start":"2026-11-05T13:00:00", "end":"2026-11-05T14:00:00", "titleShort":"Danish"},
            {"start":"2026-11-05T08:30:00", "end":"2026-11-05T09:30:00", "titleShort":"English"},
            {"start":"2026-11-05T09:35:00", "end":"2026-11-05T10:20:00", "titleShort":"Maths"},
            {"start":"2026-11-05T10:40:00", "end":"2026-11-05T12:20:00", "titleShort":"Physics"},
            {"start":"2026-11-06T09:00:00", "end":"2026-11-06T10:00:00", "titleShort":"History"},
            {"start":"2026-11-09T08:30:00", "end":"2026-11-09T09:30:00", "titleShort":"Biology"},
            {"start":"2026-11-05T15:00:00", "end":"2026-11-05T15:00:00", "titleShort":"Zero length"},
            {"start":"invalid", "titleShort":"Bad date"}
        ]))
        .unwrap()
    }

    fn at(text: &str) -> NaiveDateTime {
        planner::parse_deadline(text).unwrap().at
    }

    #[test]
    fn every_phase_of_the_day_reports_progress_and_what_comes_next() {
        let timetable = timetable();
        let day = |time| DayState::compute(&timetable, at(time));

        let before = day("2026-11-05T08:00:00");
        assert_eq!(before.phase, Phase::Before);
        assert_eq!(
            before.headline,
            ["First class: English at 08:30", "in 30 min"]
        );
        assert_eq!(before.footer, "School ends 14:00");

        let class = day("2026-11-05T11:15:00");
        assert_eq!(class.phase, Phase::InClass);
        assert_eq!(
            class.headline,
            ["Physics", "1 h 5 min left", "next Danish 13:00"]
        );
        assert_eq!(class.footer, "Day 50% done · school ends 14:00");

        let last = day("2026-11-05T13:59:30");
        assert_eq!(last.headline, ["Danish", "1 min left", "last class"]);

        // A five-minute gap is unlisted, but the bar still names it.
        let short = day("2026-11-05T09:32:00");
        assert_eq!(short.phase, Phase::Break);
        assert_eq!(short.headline, ["Break", "3 min left", "next Maths 09:35"]);

        let lunch = day("2026-11-05T12:30:00");
        assert_eq!(
            lunch.headline,
            ["Lunch break", "30 min left", "next Danish 13:00"]
        );

        let after = day("2026-11-05T16:00:00");
        assert_eq!(after.phase, Phase::After);
        assert_eq!(after.headline, ["School's out", "tomorrow History 09:00"]);
        assert_eq!(after.footer, "School ended 14:00");
        assert!(after
            .segments
            .iter()
            .all(|s| s.fill(at("2026-11-05T16:00:00")) == 1.0));

        let weekend = day("2026-11-07T12:00:00");
        assert_eq!(weekend.phase, Phase::NoSchool);
        assert_eq!(
            weekend.headline,
            ["No school today", "Monday Biology 08:30"]
        );
        assert_eq!(weekend.segments.len(), 1);
        assert_eq!(weekend.segments[0].fill(at("2026-11-07T12:00:00")), 0.0);

        let empty = DayState::compute(&[], at("2026-11-05T12:00:00"));
        assert_eq!(empty.headline, ["No school today"]);
        assert!(empty.segments.is_empty());
    }

    #[test]
    fn only_gaps_of_fifteen_minutes_or_more_are_listed() {
        let segments = DayState::compute(&timetable(), at("2026-11-05T08:00:00")).segments;
        let breaks: Vec<_> = segments
            .iter()
            .filter(|s| s.is_break)
            .map(|s| (s.subject.as_str(), clock(s.start), clock(s.end)))
            .collect();
        assert_eq!(
            breaks,
            [
                ("Break", "10:20".to_string(), "10:40".to_string()),
                ("Lunch break", "12:20".to_string(), "13:00".to_string())
            ]
        );
    }

    #[test]
    fn overlapping_lessons_do_not_create_a_break_or_end_the_day_early() {
        let timetable: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"start":"2026-11-05T08:00:00", "end":"2026-11-05T12:00:00", "titleShort":"Long"},
            {"start":"2026-11-05T09:00:00", "end":"2026-11-05T10:00:00", "titleShort":"Short"}
        ]))
        .unwrap();
        let day = DayState::compute(&timetable, at("2026-11-05T11:00:00"));
        assert_eq!(day.phase, Phase::InClass);
        assert_eq!(day.footer, "Day 75% done · school ends 12:00");
        assert!(!day.segments.iter().any(|s| s.is_break));
    }

    #[test]
    fn inna_timestamps_with_milliseconds_and_a_zone_work() {
        let timetable: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"start":"2026-11-02T08:30:00.000Z", "end":"2026-11-02T09:30:00.000Z", "titleShort":"DANS1GR05"}
        ]))
        .unwrap();
        let day = DayState::compute(&timetable, at("2026-11-02T09:00:00"));
        assert_eq!(day.phase, Phase::InClass);
        assert_eq!(day.footer, "Day 50% done · school ends 09:30");
    }

    #[test]
    fn countdowns_match_the_deadline_distance() {
        let now = at("2026-11-05T12:00:00");
        let label = |text| countdown(planner::parse_deadline(text).unwrap(), now);
        assert_eq!(label("2026-11-05T10:00:00"), "overdue");
        assert_eq!(label("2026-11-05T22:00:00"), "today 22:00");
        assert_eq!(label("2026-11-06T09:00:00"), "tomorrow 09:00");
        assert_eq!(label("2026-11-09T09:00:00"), "4 days");
    }
}
