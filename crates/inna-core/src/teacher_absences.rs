//! Reconcile INNA's dashboard teacher-absence notices with dated lessons.
use crate::models::TimetableEntry;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

/// Fields consumed by INNA's Forföll kennara dashboard panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherAbsence {
    pub name: Option<String>,
    pub module: Option<String>,
    pub time_from: Option<String>,
    pub time_to: Option<String>,
    pub group: Option<String>,
    pub date: Option<String>,
}

fn timestamp(value: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.naive_utc())
        .or_else(|| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").ok())
}

fn date(value: &str) -> Option<NaiveDate> {
    timestamp(value).map(|value| value.date()).or_else(|| {
        ["%Y-%m-%d", "%d.%m.%Y"]
            .iter()
            .find_map(|format| NaiveDate::parse_from_str(value, format).ok())
    })
}

fn clock(value: &str) -> Option<NaiveTime> {
    ["%H:%M", "%H:%M:%S"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(value.trim(), format).ok())
}

/// Undated dashboard notices apply to the nearest matching day from today onward.
/// Never suppress every future lesson of a course because one teacher is absent.
pub fn apply_teacher_absences(
    lessons: &mut [TimetableEntry],
    notices: &[TeacherAbsence],
    now: NaiveDateTime,
) {
    for lesson in lessons.iter_mut() {
        lesson.teacher_absent = false;
    }
    for notice in notices {
        let Some(module) = notice
            .module
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(from) = notice.time_from.as_deref().and_then(clock) else {
            continue;
        };
        let until = match notice.time_to.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(value) => match clock(value) {
                Some(time) => Some(time),
                None => continue,
            },
            None => None,
        };
        let on = match notice.date.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(value) => match date(value) {
                Some(date) => Some(date),
                None => continue,
            },
            None => None,
        };
        let course_matches = |lesson: &TimetableEntry| {
            lesson.is_lesson()
                && lesson.course_code().trim().eq_ignore_ascii_case(module)
                && notice
                    .group
                    .as_deref()
                    .filter(|group| !group.trim().is_empty())
                    .is_none_or(|group| {
                        lesson
                            .group
                            .as_deref()
                            .is_some_and(|value| value.trim().eq_ignore_ascii_case(group.trim()))
                    })
        };
        // Anchor an undated notice before matching clocks. Otherwise a stale 08:30
        // notice could jump over tomorrow's 13:40 class and cancel next week's 08:30.
        // Include finished lessons today so refreshes cannot move a notice mid-day.
        let on = on.or_else(|| {
            lessons
                .iter()
                .filter(|lesson| course_matches(lesson))
                .filter_map(|lesson| timestamp(lesson.start.as_deref()?).map(|start| start.date()))
                .filter(|date| *date >= now.date())
                .min()
        });
        let matches: Vec<_> = lessons
            .iter()
            .enumerate()
            .filter_map(|(index, lesson)| {
                if !course_matches(lesson) {
                    return None;
                }
                let start = timestamp(lesson.start.as_deref()?)?;
                let end = timestamp(lesson.end.as_deref()?)?;
                (start.time() == from
                    && until.is_none_or(|until| end.time() == until)
                    && start < end
                    && Some(start.date()) == on)
                    .then_some((index, start))
            })
            .collect();
        for (index, _) in matches {
            lessons[index].teacher_absent = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danish_absence_only_cancels_the_matching_occurrence() {
        let mut lessons: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"titleShort":"DANS1GR05","group":"Example group","start":"2026-11-10T08:30:00Z","end":"2026-11-10T09:30:00Z"},
            {"titleShort":"ENSK2HF05","start":"2026-11-10T08:30:00Z","end":"2026-11-10T09:30:00Z"},
            {"titleShort":"DANS1GR05","start":"2026-11-10T11:00:00Z","end":"2026-11-10T12:00:00Z"},
            {"titleShort":"DANS1GR05","group":"Example group","start":"2026-11-17T08:30:00Z","end":"2026-11-17T09:30:00Z"},
            {"titleShort":"DANS1GR05","group":"Other group","start":"2026-11-10T08:30:00Z","end":"2026-11-10T09:30:00Z"}
        ])).unwrap();
        let notices: Vec<TeacherAbsence> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/teacher_absences.json"
        ))
        .unwrap();
        let now = timestamp("2026-11-09T18:00:00Z").unwrap();
        apply_teacher_absences(&mut lessons, &notices, now);
        assert!(lessons[0].teacher_absent);
        assert!(lessons[0].is_lesson());
        assert!(!lessons[0].is_active_lesson());
        assert!(lessons[1..].iter().all(TimetableEntry::is_active_lesson));
        apply_teacher_absences(
            &mut lessons,
            &notices,
            timestamp("2026-11-10T18:00:00Z").unwrap(),
        );
        assert!(lessons[0].teacher_absent);
        assert!(
            !lessons[3].teacher_absent,
            "a finished absence must not move to next week"
        );
        apply_teacher_absences(&mut lessons, &[], now);
        assert!(lessons.iter().all(TimetableEntry::is_active_lesson));
    }

    #[test]
    fn undated_notice_does_not_jump_over_a_course_day_with_different_times() {
        let mut lessons: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"titleShort":"DANS1GR05","start":"2026-11-10T13:40:00Z","end":"2026-11-10T14:40:00Z"},
            {"titleShort":"DANS1GR05","start":"2026-11-12T08:30:00Z","end":"2026-11-12T09:30:00Z"}
        ]))
        .unwrap();
        let notice = serde_json::from_value(
            serde_json::json!({"module":"DANS1GR05","timeFrom":"08:30","timeTo":"09:30"}),
        )
        .unwrap();
        apply_teacher_absences(
            &mut lessons,
            &[notice],
            timestamp("2026-11-10T01:00:00Z").unwrap(),
        );
        assert!(lessons.iter().all(TimetableEntry::is_active_lesson));
    }

    #[test]
    fn dated_and_malformed_notices_do_not_cancel_other_days() {
        let mut lessons: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"titleShort":"DANS1GR05","start":"2026-11-10T08:30:00Z","end":"2026-11-10T09:30:00Z"},
            {"titleShort":"DANS1GR05","start":"2026-11-17T08:30:00Z","end":"2026-11-17T09:30:00Z"}
        ]))
        .unwrap();
        let now = timestamp("2026-11-09T18:00:00Z").unwrap();
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"module":"DANS1GR05","timeFrom":"bad"}),
            serde_json::json!({"module":"DANS1GR05","timeFrom":"08:30","date":"bad"}),
        ] {
            apply_teacher_absences(
                &mut lessons,
                &[serde_json::from_value(invalid).unwrap()],
                now,
            );
            assert!(lessons.iter().all(TimetableEntry::is_active_lesson));
        }
        let notice = serde_json::from_value(
            serde_json::json!({"module":"DANS1GR05","timeFrom":"08:30","date":"17.11.2026"}),
        )
        .unwrap();
        apply_teacher_absences(&mut lessons, &[notice], now);
        assert!(!lessons[0].teacher_absent);
        assert!(lessons[1].teacher_absent);
    }
}
