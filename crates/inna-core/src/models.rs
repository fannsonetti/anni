use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub student_id: Option<String>,
    pub user_id: Option<serde_json::Value>,
    pub school_id: Option<String>,
    pub school_short: Option<String>,
    pub school_long: Option<String>,
    pub default_term_id: Option<String>,
    pub student_name: Option<String>,
    pub is_student: Option<bool>,
}

impl User {
    pub fn display_name(&self) -> &str {
        self.student_name.as_deref().unwrap_or("Nemandi")
    }

    pub fn student_id_str(&self) -> Option<&str> {
        self.student_id.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimetableEntry {
    pub module_id: Option<String>,
    pub group_id: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub module_term_id: Option<String>,
    pub title_short: Option<String>,
    pub all_day: Option<bool>,
    pub start_clock: Option<String>,
    pub end_clock: Option<String>,
    pub teacher: Option<String>,
    pub classroom: Option<String>,
    pub group: Option<String>,
    #[serde(rename = "timetable_id")]
    pub timetable_id: Option<i64>,
    #[serde(rename = "maintable_id")]
    pub maintable_id: Option<i64>,
    #[serde(rename = "studentRecordId")]
    pub student_record_id: Option<i64>,
}

impl TimetableEntry {
    /// INNA mixes lessons and school-calendar events in the same response.
    pub fn is_lesson(&self) -> bool {
        if self.all_day == Some(true) {
            return false;
        }
        let title = self.course_code().trim().to_lowercase();
        ![
            "vetrarfrí",
            "haustfrí",
            "jólafrí",
            "páskafrí",
            "sumarfrí",
            "skólafrí",
            "winter break",
            "autumn break",
            "christmas break",
            "easter break",
            "school holiday",
        ]
        .iter()
        .any(|name| {
            title == *name
                || title
                    .strip_prefix(name)
                    .is_some_and(|suffix| suffix.starts_with([' ', '-', ':', '–']))
        })
    }

    pub fn course_code(&self) -> &str {
        self.title_short.as_deref().unwrap_or("—")
    }

    pub fn time_range(&self) -> String {
        let s = self.start_clock.as_deref().unwrap_or("??:??");
        let e = self.end_clock.as_deref().unwrap_or("??:??");
        format!("{s} – {e}")
    }

    pub fn room(&self) -> &str {
        self.classroom.as_deref().unwrap_or("")
    }

    pub fn teacher_str(&self) -> &str {
        self.teacher.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub date: Option<String>,
    pub content_short: Option<String>,
    #[serde(rename = "type")]
    pub announcement_type: Option<String>,
    pub content_html: Option<String>,
    pub title: Option<String>,
    pub user_id: Option<serde_json::Value>,
    pub month: Option<String>,
    pub sender: Option<String>,
    pub has_opened: Option<bool>,
    pub announcement_id: Option<serde_json::Value>,
    pub day: Option<String>,
    pub module_name: Option<String>,
    pub group: Option<String>,
}

impl Announcement {
    pub fn title_str(&self) -> &str {
        self.title.as_deref().unwrap_or("(Untitled)")
    }

    pub fn sender_str(&self) -> &str {
        self.sender.as_deref().unwrap_or("")
    }

    pub fn module_str(&self) -> &str {
        self.module_name.as_deref().unwrap_or("")
    }

    pub fn content_text(&self) -> String {
        if let Some(html) = &self.content_html {
            crate::html::html_to_text(html)
        } else {
            self.content_short.clone().unwrap_or_default()
        }
    }
}

/// Course group from `/api/GetAssignments/GetStudentProjects` (Verkefni/Próf page).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentModule {
    pub module_name: Option<String>,
    pub group_id: Option<String>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    #[serde(default)]
    pub categories: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub module_name: Option<String>,
    pub group_id: Option<String>,
}

impl AssignmentModule {
    pub fn module_str(&self) -> &str {
        self.module_name.as_deref().unwrap_or("—")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub assignment_id: Option<String>,
    pub name: Option<String>,
    pub module: Option<String>,
    pub group_id: Option<String>,
    /// `"0"` = Verkefni, `"1"` = Próf (also seen as numbers).
    pub exam: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub assignment_type: Option<serde_json::Value>,
    pub weight: Option<serde_json::Value>,
    pub grade: Option<String>,
    pub handed_in: Option<serde_json::Value>,
    pub order_assign_date: Option<String>,
    pub order_hand_in_date: Option<String>,
    pub assigned_full_date: Option<String>,
    pub hand_in_full_date: Option<String>,
    pub comment_by_teacher: Option<String>,
    pub has_opened: Option<serde_json::Value>,
    pub project_id: Option<String>,
}

impl Assignment {
    pub fn name_str(&self) -> &str {
        self.name.as_deref().unwrap_or("Assignment")
    }

    pub fn is_exam(&self) -> bool {
        match self.assignment_type.as_ref().or(self.exam.as_ref()) {
            Some(serde_json::Value::String(s)) => s == "1" || s.eq_ignore_ascii_case("true"),
            Some(serde_json::Value::Number(n)) => n.as_u64() == Some(1),
            Some(serde_json::Value::Bool(b)) => *b,
            _ => false,
        }
    }

    pub fn is_evaluation(&self) -> bool {
        self.assignment_type
            .as_ref()
            .is_some_and(|v| v == "2" || v == 2)
    }

    pub fn type_label(&self) -> &'static str {
        if self.is_exam() {
            "Exam"
        } else {
            "Assignment"
        }
    }

    pub fn is_handed_in(&self) -> bool {
        match &self.handed_in {
            Some(serde_json::Value::String(s)) => s == "1" || s.eq_ignore_ascii_case("true"),
            Some(serde_json::Value::Number(n)) => n.as_u64() == Some(1),
            Some(serde_json::Value::Bool(b)) => *b,
            _ => false,
        }
    }

    pub fn weight_str(&self) -> String {
        match &self.weight {
            Some(serde_json::Value::Number(n)) => {
                let f = n.as_f64().unwrap_or(0.0);
                if f == 0.0 {
                    String::new()
                } else if (f - f.round()).abs() < f64::EPSILON {
                    format!("{}%", f as i64)
                } else {
                    format!("{f}%")
                }
            }
            Some(serde_json::Value::String(s)) if !s.is_empty() && s != "0" => {
                if s.contains('%') {
                    s.clone()
                } else {
                    format!("{s}%")
                }
            }
            _ => String::new(),
        }
    }

    pub fn opens_str(&self) -> &str {
        self.order_assign_date
            .as_deref()
            .or(self.assigned_full_date.as_deref())
            .unwrap_or("")
    }

    pub fn closes_str(&self) -> &str {
        self.order_hand_in_date
            .as_deref()
            .or(self.hand_in_full_date.as_deref())
            .unwrap_or("")
    }

    pub fn grade_str(&self) -> &str {
        self.grade.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Homework {
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub answer: Option<String>,
}

impl PingResponse {
    pub fn is_pong(&self) -> bool {
        self.answer.as_deref() == Some("Pong")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_and_all_day_events_are_not_classes() {
        for title in [
            "Vetrarfrí",
            "Vetrarfrí - engin kennsla",
            "Winter break",
            "Jólafrí",
        ] {
            let entry: TimetableEntry =
                serde_json::from_value(serde_json::json!({"titleShort": title})).unwrap();
            assert!(!entry.is_lesson(), "{title}");
        }
        let event: TimetableEntry =
            serde_json::from_value(serde_json::json!({"titleShort":"School event", "allDay":true}))
                .unwrap();
        assert!(!event.is_lesson());
        for title in ["ENSK2HF05", "Winter literature", "FORR2HF05"] {
            let entry: TimetableEntry =
                serde_json::from_value(serde_json::json!({"titleShort":title})).unwrap();
            assert!(entry.is_lesson());
        }
    }

    #[test]
    fn test_parse_user_fixture() {
        let json = include_str!("../../../tests/fixtures/user.json");
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.student_id.as_deref(), Some("100001"));
        assert_eq!(user.display_name(), "Jón Jónsson");
        assert_eq!(user.is_student, Some(true));
        assert_eq!(
            user.school_long.as_deref(),
            Some("Menntaskólinn í Flateyri")
        );
    }

    #[test]
    fn test_parse_timetable_fixture() {
        let json = include_str!("../../../tests/fixtures/timetable.json");
        let entries: Vec<TimetableEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].course_code(), "DANS1GR05");
        assert_eq!(entries[0].time_range(), "08:30 – 09:30");
        assert_eq!(entries[0].room(), "M203");
        assert_eq!(entries[0].teacher_str(), "TST");
        assert_eq!(entries[1].course_code(), "ENSK2HF05");
    }

    #[test]
    fn test_timetable_optional_fields() {
        let json = r#"{"titleShort":"TEST","startClock":"10:00","endClock":"11:00","start":"2026-08-20T10:00:00.000Z","end":"2026-08-20T11:00:00.000Z"}"#;
        let entry: TimetableEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.course_code(), "TEST");
        assert_eq!(entry.room(), "");
        assert_eq!(entry.teacher_str(), "");
        assert!(entry.classroom.is_none());
        assert!(entry.teacher.is_none());
    }

    #[test]
    fn test_parse_announcements_fixture() {
        let json = include_str!("../../../tests/fixtures/announcements.json");
        let items: Vec<Announcement> = serde_json::from_str(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title_str(), "Greetings!");
        assert_eq!(items[0].sender_str(), "Example Teacher");
        assert_eq!(items[0].module_str(), "ENSK2HF05");
        assert_eq!(items[0].has_opened, Some(false));
        let text = items[0].content_text();
        assert!(text.contains("Welcome to"));
        assert!(text.contains("ENSK2HF05"));
    }

    #[test]
    fn test_parse_empty_assignments() {
        let json = include_str!("../../../tests/fixtures/assignments_empty.json");
        let items: Vec<Assignment> = serde_json::from_str(json).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_student_projects() {
        let json = include_str!("../../../tests/fixtures/student_projects.json");
        let modules: Vec<AssignmentModule> = serde_json::from_str(json).unwrap();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].module_str(), "DANS1GR05");
        assert_eq!(modules[0].assignments.len(), 2);
        let a = &modules[0].assignments[0];
        assert_eq!(a.name_str(), "Dagene");
        assert!(a.is_exam());
        assert!(a.is_handed_in());
        assert_eq!(a.grade_str(), "10");
        assert_eq!(a.type_label(), "Exam");
        let b = &modules[0].assignments[1];
        assert!(!b.is_exam());
        assert_eq!(b.type_label(), "Assignment");
        assert_eq!(b.weight_str(), "1%");
    }

    #[test]
    fn test_parse_empty_homework() {
        let json = include_str!("../../../tests/fixtures/homework_empty.json");
        let items: Vec<Homework> = serde_json::from_str(json).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_empty_todo() {
        let json = include_str!("../../../tests/fixtures/todo_empty.json");
        let items: Vec<TodoItem> = serde_json::from_str(json).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_ping() {
        let json = include_str!("../../../tests/fixtures/ping.json");
        let ping: PingResponse = serde_json::from_str(json).unwrap();
        assert!(ping.is_pong());
    }

    #[test]
    fn test_ping_not_pong() {
        let ping: PingResponse = serde_json::from_str(r#"{"answer":"Nope"}"#).unwrap();
        assert!(!ping.is_pong());
    }

    #[test]
    fn test_timetable_sorting() {
        let json = include_str!("../../../tests/fixtures/timetable.json");
        let entries: Vec<TimetableEntry> = serde_json::from_str(json).unwrap();
        let times: Vec<&str> = entries
            .iter()
            .map(|e| e.start_clock.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(times, vec!["08:30", "09:50"]);
    }

    #[test]
    fn test_user_id_as_number() {
        let json = r#"{"userId": 12345, "studentName": "Test"}"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.user_id, Some(serde_json::json!(12345)));
    }

    #[test]
    fn test_user_id_as_string() {
        let json = r#"{"userId": "12345", "studentName": "Test"}"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.user_id, Some(serde_json::json!("12345")));
    }
}
