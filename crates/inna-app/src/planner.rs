use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalTask {
    pub id: u64,
    pub title: String,
    pub course: String,
    pub due: Option<NaiveDate>,
    pub done: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Planner {
    pub tasks: Vec<PersonalTask>,
    pub completed: HashSet<String>,
    pub next_id: u64,
    pub focus_sessions: Vec<FocusSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSession {
    pub date: NaiveDate,
    pub minutes: u32,
    pub task: String,
}

impl Planner {
    pub fn edit(&mut self, id: u64, title: &str, course: &str, due: &str) -> Result<(), String> {
        // Reuse creation validation before mutating the existing task.
        let mut draft = Planner::default();
        draft.add(title, course, due)?;
        let replacement = draft.tasks.pop().unwrap();
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| "This task could not be found.".to_string())?;
        task.title = replacement.title;
        task.course = replacement.course;
        task.due = replacement.due;
        Ok(())
    }

    pub fn add(&mut self, title: &str, course: &str, due: &str) -> Result<(), String> {
        if title.trim().is_empty() {
            return Err("Enter a task title.".into());
        }
        let due = if due.trim().is_empty() {
            None
        } else {
            Some(
                crate::locale::parse_date(due)
                    .ok_or_else(|| "Enter a date as dd.mm.yyyy, or leave it blank.".to_string())?,
            )
        };
        self.next_id += 1;
        self.tasks.push(PersonalTask {
            id: self.next_id,
            title: title.trim().into(),
            course: course.trim().into(),
            due,
            done: false,
        });
        Ok(())
    }

    pub fn set_done(&mut self, key: &str, done: bool) {
        if let Some(id) = key
            .strip_prefix("personal:")
            .and_then(|s| s.parse::<u64>().ok())
        {
            if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
                task.done = done;
            }
        } else if done {
            self.completed.insert(key.into());
        } else {
            self.completed.remove(key);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkKind {
    Assignment,
    Exam,
    Homework,
    InnaTask,
    Personal,
}

impl WorkKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Assignment => "Assignment",
            Self::Exam => "Exam",
            Self::Homework => "Homework",
            Self::InnaTask => "INNA task",
            Self::Personal => "Personal task",
        }
    }
}

impl WorkKind {
    // Persisted completion keys must not change when presentation is localized.
    fn key_namespace(self) -> &'static str {
        match self {
            Self::Assignment => "Assignment",
            Self::Exam => "Exam",
            Self::Homework => "Homework",
            Self::InnaTask => "INNA task",
            Self::Personal => "Personal task",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    pub at: NaiveDateTime,
    pub has_time: bool,
}

pub fn parse_deadline(text: &str) -> Option<Deadline> {
    let text = text.trim();
    if let Ok(value) = DateTime::parse_from_rfc3339(text) {
        return Some(Deadline {
            at: value.naive_utc(),
            has_time: true,
        });
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%d.%m.%Y %H:%M",
    ] {
        if let Ok(at) = NaiveDateTime::parse_from_str(text, format) {
            return Some(Deadline { at, has_time: true });
        }
    }
    for format in ["%Y-%m-%d", "%d.%m.%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, format) {
            return Some(Deadline {
                at: date.and_hms_opt(23, 59, 59)?,
                has_time: false,
            });
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%a %b %d %Y") {
        return Some(Deadline {
            at: date.and_hms_opt(23, 59, 59)?,
            has_time: false,
        });
    }
    None
}

#[derive(Debug, Clone)]
pub struct WorkItem {
    pub key: String,
    completion_keys: Vec<String>,
    pub title: String,
    pub course: String,
    pub kind: WorkKind,
    pub deadline: Option<Deadline>,
    pub due_text: String,
    pub detail: String,
    pub source_done: bool,
    pub done: bool,
}

impl WorkItem {
    pub fn set_done(&self, planner: &mut Planner, done: bool) {
        planner.set_done(&self.key, done);
        for key in &self.completion_keys {
            planner.set_done(key, done);
        }
    }

    pub fn overdue(&self, now: NaiveDateTime) -> bool {
        !self.done && self.deadline.is_some_and(|d| d.at < now)
    }

    pub fn due_label(&self, today: NaiveDate) -> String {
        let Some(deadline) = self.deadline else {
            return if self.due_text.is_empty() {
                "No due date".into()
            } else {
                format!("Original deadline: {}", self.due_text)
            };
        };
        let days = (deadline.at.date() - today).num_days();
        let day = match days {
            0 => "Today".into(),
            1 => "Tomorrow".into(),
            -1 => "Yesterday".into(),
            _ => crate::locale::date(deadline.at.date()),
        };
        if deadline.has_time {
            format!("{day} at {}", deadline.at.format("%H:%M"))
        } else {
            day
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WorkFilter {
    #[default]
    Open,
    Today,
    Week,
    Done,
}

impl WorkFilter {
    pub fn matches(self, item: &WorkItem, today: NaiveDate) -> bool {
        match self {
            Self::Open => !item.done,
            // Include overdue work so a missed deadline never disappears from the daily plan.
            Self::Today => !item.done && item.deadline.is_some_and(|d| d.at.date() <= today),
            Self::Week => {
                !item.done
                    && item
                        .deadline
                        .is_some_and(|d| (d.at.date() - today).num_days() < 7)
            }
            Self::Done => item.done,
        }
    }
}

pub fn work_items(state: &AppState) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for module in &state.assignments {
        for assignment in &module.assignments {
            if assignment.is_evaluation() {
                continue;
            }
            // Some courses expose an empty Heimavinna heading as an assignment.
            if homework_heading(assignment.name_str())
                && assignment.closes_str().trim().is_empty()
                && assignment
                    .comment_by_teacher
                    .as_deref()
                    .map(inna_core::html::html_to_text)
                    .unwrap_or_default()
                    .is_empty()
                && assignment.weight_str().is_empty()
                && assignment.grade_str().trim().is_empty()
            {
                continue;
            }
            let course = module.module_str().to_string();
            let identity = assignment
                .assignment_id
                .as_deref()
                .or(assignment.project_id.as_deref())
                .or(assignment.name.as_deref())
                .unwrap_or("Assignment");
            let key = serde_json::to_string(&("assignment", &module.group_id, &course, identity))
                .unwrap();
            let source_done = assignment.is_handed_in()
                || !matches!(assignment.grade_str().trim(), "" | "-" | "—");
            items.push(WorkItem {
                done: source_done || state.planner.completed.contains(&key),
                key,
                completion_keys: Vec::new(),
                title: assignment.name_str().into(),
                course,
                kind: if assignment.is_exam() {
                    WorkKind::Exam
                } else {
                    WorkKind::Assignment
                },
                deadline: parse_deadline(assignment.closes_str()),
                due_text: assignment.closes_str().into(),
                detail: assignment
                    .comment_by_teacher
                    .as_deref()
                    .map(inna_core::html::html_to_text)
                    .unwrap_or_default(),
                source_done,
            });
        }
    }
    for (kind, rows) in [
        (
            WorkKind::Homework,
            state.homework.iter().map(|h| &h.raw).collect::<Vec<_>>(),
        ),
        (
            WorkKind::InnaTask,
            state.todo_items.iter().map(|t| &t.raw).collect(),
        ),
    ] {
        for raw in rows {
            let title = if kind == WorkKind::Homework {
                // Labels and empty HTML are not tasks. Prefer actual instructions
                // when an endpoint provides a generic title beside its content.
                [
                    "text",
                    "homework",
                    "title",
                    "name",
                    "description",
                    "contentHtml",
                    "content",
                    "descriptionHtml",
                ]
                .iter()
                .map(|key| inna_core::html::html_to_text(&raw_text(raw, &[*key])))
                .find(|text| !text.trim().is_empty() && !homework_heading(text))
                .unwrap_or_default()
            } else {
                inna_core::html::html_to_text(&raw_text(
                    raw,
                    &["text", "homework", "title", "name", "description"],
                ))
            };
            // Do not turn unrecognized metadata or empty teaching-goal rows into tasks.
            if title.trim().is_empty() {
                continue;
            }
            let identity_title = &title;
            let course = raw_text(raw, &["moduleName", "module", "courseName", "course"]);
            let due_text = raw_text(
                raw,
                &[
                    "orderHandInDate",
                    "handInDate",
                    "dueDate",
                    "deadline",
                    "date",
                ],
            );
            let id = raw_text(raw, &["id", "homeworkId", "todoId"]);
            let identity = if id.is_empty() {
                format!("{identity_title}|{due_text}")
            } else {
                id.clone()
            };
            let key = serde_json::to_string(&(kind.key_namespace(), &course, &identity)).unwrap();
            let source_done = ["completed", "done", "handedIn"]
                .iter()
                .any(|k| raw.get(k).is_some_and(truthy));
            let mut completion_keys = Vec::new();
            if kind == WorkKind::Homework {
                let date = parse_deadline(&due_text)
                    .map(|d| d.at.date().to_string())
                    .unwrap_or_else(|| due_text.clone());
                completion_keys.push(
                    serde_json::to_string(&(
                        "homework-content",
                        &course,
                        crate::locale::sort_key(&title),
                        date,
                    ))
                    .unwrap(),
                );
            }
            items.push(WorkItem {
                done: source_done
                    || state.planner.completed.contains(&key)
                    || completion_keys
                        .iter()
                        .any(|key| state.planner.completed.contains(key)),
                key,
                completion_keys,
                title,
                course,
                kind,
                deadline: parse_deadline(&due_text),
                due_text,
                detail: inna_core::html::html_to_text(&raw_text(
                    raw,
                    &[
                        "contentHtml",
                        "content",
                        "descriptionHtml",
                        "description",
                        "text",
                        "homework",
                    ],
                )),
                source_done,
            });
        }
    }
    // A Heimavinna item can appear in the global list and in its course calendar.
    // Keep every identity as an alias so checking either copy survives source changes.
    let mut deduplicated: Vec<WorkItem> = Vec::with_capacity(items.len());
    let mut identities = std::collections::HashMap::new();
    let mut homework_contents = std::collections::HashMap::new();
    for item in items.drain(..) {
        let content = (item.kind == WorkKind::Homework).then(|| {
            serde_json::to_string(&(
                &item.course,
                item.deadline.map(|d| (d.at, d.has_time)),
                crate::locale::sort_key(&item.title),
            ))
            .unwrap()
        });
        let existing = identities.get(&item.key).copied().or_else(|| {
            content
                .as_ref()
                .and_then(|key| homework_contents.get(key).copied())
        });
        let index = existing.unwrap_or(deduplicated.len());
        identities.insert(item.key.clone(), index);
        if let Some(content) = content {
            homework_contents.insert(content, index);
        }
        if let Some(index) = existing {
            let existing = &mut deduplicated[index];
            existing.done |= item.done;
            existing.source_done |= item.source_done;
            existing.completion_keys.push(item.key);
            existing.completion_keys.extend(item.completion_keys);
        } else {
            deduplicated.push(item);
        }
    }
    items = deduplicated;
    for task in &state.planner.tasks {
        items.push(WorkItem {
            key: format!("personal:{}", task.id),
            completion_keys: Vec::new(),
            title: task.title.clone(),
            course: task.course.clone(),
            kind: WorkKind::Personal,
            deadline: task.due.map(|d| Deadline {
                at: d.and_hms_opt(23, 59, 59).unwrap(),
                has_time: false,
            }),
            due_text: String::new(),
            detail: String::new(),
            source_done: false,
            done: task.done,
        });
    }
    items.sort_by_cached_key(|item| {
        (
            item.done,
            item.deadline.is_none(),
            item.deadline.map(|d| d.at),
            crate::locale::sort_key(&item.title),
        )
    });
    items
}

fn raw_text(raw: &serde_json::Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| match raw.get(k) {
            Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
            Some(serde_json::Value::Number(n)) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn homework_heading(text: &str) -> bool {
    matches!(
        inna_core::html::html_to_text(text)
            .trim()
            .trim_end_matches(':')
            .trim()
            .to_lowercase()
            .as_str(),
        "homework" | "heimavinna"
    )
}

fn truthy(value: &serde_json::Value) -> bool {
    value == true
        || value == 1
        || value
            .as_str()
            .is_some_and(|s| s == "1" || s.eq_ignore_ascii_case("true"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn course_homework_headings_are_not_tasks_but_real_instructions_are_kept() {
        let mut state = AppState::new();
        state.homework = serde_json::from_value(serde_json::json!([
            {"id":1,"title":"Homework","moduleName":"MATH"},
            {"id":2,"text":"<p>Heimavinna:</p>","moduleName":"ENSK"},
            {"id":3,"text":"<p>&nbsp;</p>","moduleName":"DANS"},
            {"id":4,"text":"Homework","content":"<p>Read chapter 4</p>","moduleName":"ENSK"},
            {"id":5,"text":"Homework","homework":"Exercises 1–5","date":"19.10.2026"},
            {"id":6,"text":"Prepare homework presentation"}
        ]))
        .unwrap();
        state.assignments = serde_json::from_value(serde_json::json!([{
            "moduleName":"MATH", "assignments":[
                {"assignmentId":"empty", "name":"Homework"},
                {"assignmentId":"real", "name":"Homework", "handInFullDate":"19.10.2026"}
            ]
        }]))
        .unwrap();
        let items = work_items(&state);
        assert_eq!(items.len(), 4);
        for title in [
            "Read chapter 4",
            "Exercises 1–5",
            "Prepare homework presentation",
        ] {
            assert!(items.iter().any(|i| i.title == title));
        }
        assert!(items
            .iter()
            .any(|i| i.title == "Homework" && i.deadline.is_some()));
    }

    #[test]
    fn heimavinna_uses_live_text_and_date_fields_and_completion_survives_a_source_change() {
        let mut state = AppState::new();
        state.homework = serde_json::from_value(serde_json::json!([
            {"id":42,"text":"<p>Read pages 3–5</p>","date":"19.10.2026","moduleName":"ENSK"},
            {"homework":"Read pages 3–5","date":"Mon Oct 19 2026","moduleName":"ENSK","_anniSource":"calendar:g"},
            {"homework":"Read pages 3–5","date":"20.10.2026","moduleName":"ENSK"},
            {"date":"19.10.2026","goal":"Learning goal only"}
        ])).unwrap();
        let items = work_items(&state);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Read pages 3–5");
        assert_eq!(
            items[0].due_label(NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
            "Tomorrow"
        );
        items[0].set_done(&mut state.planner, true);
        state.planner =
            serde_json::from_str(&serde_json::to_string(&state.planner).unwrap()).unwrap();
        state.homework.remove(0); // Global Heimavinna no longer returns the completed row.
        let items = work_items(&state);
        let completed = items.iter().find(|i| i.done).unwrap();
        assert_eq!(
            completed.deadline.unwrap().at.date(),
            NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
        );
        assert_eq!(items.iter().filter(|i| !i.done).count(), 1);
        completed.set_done(&mut state.planner, false);
        assert!(work_items(&state).iter().all(|i| !i.done));
    }

    #[test]
    fn course_page_types_and_full_dates_keep_exams_and_evaluations_out_of_homework() {
        let mut state = AppState::new();
        state.assignments = serde_json::from_value(serde_json::json!([{
            "moduleName":"MATH","groupId":"g","assignments":[
                {"assignmentId":"1","name":"Exercises","type":"0","handInFullDate":"19.10.2026 10:00","handedIn":0},
                {"assignmentId":"2","name":"Test","type":"1","handInFullDate":"20.10.2026 08:30"},
                {"assignmentId":"3","name":"Participation","type":"2"}
            ]
        }])).unwrap();
        let items = work_items(&state);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, WorkKind::Assignment);
        assert_eq!(items[1].kind, WorkKind::Exam);
        assert_eq!(
            items[0].deadline.unwrap().at.to_string(),
            "2026-10-19 10:00:00"
        );
        items[0].set_done(&mut state.planner, true);
        assert!(
            work_items(&state)
                .iter()
                .find(|i| i.title == "Exercises")
                .unwrap()
                .done
        );
    }

    #[test]
    fn editing_validates_before_mutating_and_preserves_identity_and_completion() {
        let mut plan = Planner::default();
        plan.add("Old title", "Math", "2026-10-18").unwrap();
        plan.set_done("personal:1", true);
        assert!(plan.edit(1, "New title", "English", "invalid").is_err());
        assert_eq!(plan.tasks[0].title, "Old title");
        plan.edit(1, " New title ", " English ", "19.10.2026")
            .unwrap();
        let task = &plan.tasks[0];
        assert_eq!(task.title, "New title");
        assert_eq!(task.course, "English");
        assert_eq!(task.due, NaiveDate::from_ymd_opt(2026, 10, 19));
        assert_eq!(task.id, 1);
        assert!(task.done);
        assert_eq!(plan.next_id, 1);
        assert!(plan.edit(99, "Missing", "", "").is_err());
    }

    #[test]
    fn older_plans_restore_without_focus_history() {
        let plan: Planner =
            serde_json::from_str(r#"{"tasks":[],"completed":[],"next_id":4}"#).unwrap();
        assert!(plan.focus_sessions.is_empty());
        assert_eq!(plan.next_id, 4);
    }

    #[test]
    fn localization_preserves_existing_completion_keys_and_accepts_icelandic_dates() {
        let mut state = AppState::new();
        state.homework = serde_json::from_value(
            serde_json::json!([{"id":42,"title":"Read","moduleName":"ENSK"}]),
        )
        .unwrap();
        let key = serde_json::to_string(&("Homework", "ENSK", "42")).unwrap();
        state.planner.completed.insert(key.clone());
        assert_eq!(work_items(&state)[0].key, key);
        assert!(work_items(&state)[0].done);
        state.planner.add("Læra", "English", "18.10.2026").unwrap();
        let local = work_items(&state)
            .into_iter()
            .find(|i| i.kind == WorkKind::Personal)
            .unwrap();
        assert_eq!(
            local.due_label(NaiveDate::from_ymd_opt(2026, 10, 17).unwrap()),
            "Tomorrow"
        );
    }

    #[test]
    fn deadlines_parse_inna_dates_offsets_and_date_only_without_guessing() {
        assert_eq!(
            parse_deadline("2026-10-18 14:30:00")
                .unwrap()
                .at
                .to_string(),
            "2026-10-18 14:30:00"
        );
        assert_eq!(
            parse_deadline("2026-10-18T14:30:00+02:00")
                .unwrap()
                .at
                .hour(),
            12
        );
        let date = parse_deadline("18.10.2026").unwrap();
        assert!(!date.has_time);
        assert_eq!(date.at.time().to_string(), "23:59:59");
        assert!(parse_deadline("later this week").is_none());
        assert!(parse_deadline("2026-02-30").is_none());
    }

    #[test]
    fn plan_sorts_deadlines_keeps_undated_work_and_excludes_submitted_work() {
        let mut state = AppState::new();
        state.assignments = serde_json::from_value(serde_json::json!([{
            "moduleName":"Math", "groupId":"g", "assignments":[
                {"assignmentId":"1", "name":"Tomorrow", "orderHandInDate":"2026-10-19 12:00:00"},
                {"assignmentId":"2", "name":"Submitted", "handedIn":"1"},
                {"assignmentId":"3", "name":"Overdue", "orderHandInDate":"2026-10-17 12:00:00"},
                {"assignmentId":"4", "name":"Undated"},
                {"assignmentId":"5", "name":"Graded", "grade":"0"}
            ]
        }]))
        .unwrap();
        let items = work_items(&state);
        let open: Vec<_> = items
            .iter()
            .filter(|i| !i.done)
            .map(|i| i.title.as_str())
            .collect();
        assert_eq!(open, ["Overdue", "Tomorrow", "Undated"]);
        let today = NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
        assert_eq!(
            items
                .iter()
                .filter(|i| WorkFilter::Today.matches(i, today))
                .count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|i| WorkFilter::Week.matches(i, today))
                .count(),
            2
        );
        state.planner.set_done(&items[0].key, true);
        // A server refresh can reorder assignments without changing local completion.
        state.assignments[0].assignments.reverse();
        assert!(
            work_items(&state)
                .iter()
                .find(|i| i.title == "Overdue")
                .unwrap()
                .done
        );
    }

    #[test]
    fn personal_tasks_validate_and_round_trip_with_completion() {
        let mut plan = Planner::default();
        assert!(plan.add(" ", "", "").is_err());
        assert!(plan.add("Study", "", "invalid").is_err());
        plan.add(" Study ", " Math ", "2026-10-18").unwrap();
        plan.set_done("personal:1", true);
        let restored: Planner =
            serde_json::from_str(&serde_json::to_string(&plan).unwrap()).unwrap();
        assert_eq!(restored.tasks[0].title, "Study");
        assert!(restored.tasks[0].done);
        assert_eq!(restored.next_id, 1);
    }

    #[test]
    fn homework_and_todo_join_the_plan_and_date_only_is_not_overdue_at_noon() {
        let mut state = AppState::new();
        state.homework = serde_json::from_value(
            serde_json::json!([{"id":42,"title":"Read","dueDate":"2026-10-18"}]),
        )
        .unwrap();
        state.todo_items =
            serde_json::from_value(serde_json::json!([{"id":7,"name":"Bring book"}])).unwrap();
        let items = work_items(&state);
        assert_eq!(items.len(), 2);
        assert!(!items[0].overdue(
            NaiveDate::from_ymd_opt(2026, 10, 18)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap()
        ));
        assert_eq!(items[1].title, "Bring book");
    }

    use chrono::Timelike;
}
