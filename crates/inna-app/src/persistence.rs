use crate::{planner::Planner, state::AppState};
use inna_cache::database::Database;
use inna_core::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SNAPSHOT_KEY: &str = "daily_planner_v1";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Snapshot {
    user: Option<User>,
    timetable: Vec<TimetableEntry>,
    announcements: Vec<Announcement>,
    assignments: Vec<AssignmentModule>,
    homework: Vec<Homework>,
    todo_items: Vec<TodoItem>,
    last_sync: HashMap<String, String>,
    planner: Planner,
    account_planners: HashMap<String, Planner>,
}

pub fn restore(db: &Database, state: &mut AppState) -> Result<(), String> {
    let Some(json) = db.get_metadata(SNAPSHOT_KEY).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let saved: Snapshot = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    state.user = saved.user;
    state.timetable = saved.timetable;
    state.announcements = saved.announcements;
    state.assignments = saved.assignments;
    state.homework = saved.homework;
    state.todo_items = saved.todo_items;
    state.last_sync = saved.last_sync;
    state.planner = saved.planner;
    state.account_planners = saved.account_planners;
    Ok(())
}

pub fn save(db: &Database, state: &AppState) -> Result<(), String> {
    // Serialize borrowed data so a checkmark does not clone the entire school cache.
    #[derive(Serialize)]
    struct SnapshotRef<'a> {
        user: &'a Option<User>,
        timetable: &'a [TimetableEntry],
        announcements: &'a [Announcement],
        assignments: &'a [AssignmentModule],
        homework: &'a [Homework],
        todo_items: &'a [TodoItem],
        last_sync: &'a HashMap<String, String>,
        planner: &'a Planner,
        account_planners: &'a HashMap<String, Planner>,
    }
    let saved = SnapshotRef {
        user: &state.user,
        timetable: &state.timetable,
        announcements: &state.announcements,
        assignments: &state.assignments,
        homework: &state.homework,
        todo_items: &state.todo_items,
        last_sync: &state.last_sync,
        planner: &state.planner,
        account_planners: &state.account_planners,
    };
    let json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;
    db.set_metadata(SNAPSHOT_KEY, &json)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_snapshot_survives_reopen_and_persists_empty_updates() {
        let path = std::env::temp_dir().join(format!(
            "inna-planner-test-{}-{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let db = Database::open(&path).unwrap();
            let mut state = AppState::new();
            state.user = Some(crate::mock::mock_user());
            state.assignments = crate::mock::mock_assignments();
            state
                .last_sync
                .insert("assignments".into(), "2026-10-18T12:00:00Z".into());
            state.planner.set_done("remote-id", true);
            let mut other = crate::mock::mock_user();
            other.student_id = Some("other-student".into());
            state.adopt_user(other);
            state.planner.set_done("other-remote-id", true);
            state.adopt_user(crate::mock::mock_user());
            state.assignments = crate::mock::mock_assignments();
            state.timetable = crate::mock::mock_timetable();
            state.timetable[0].teacher_absent = true;
            save(&db, &state).unwrap();
        }
        {
            let db = Database::open(&path).unwrap();
            let mut state = AppState::new();
            restore(&db, &mut state).unwrap();
            assert!(state.user.is_some());
            assert!(!state.assignments.is_empty());
            assert!(state.timetable[0].teacher_absent);
            assert!(!state.timetable[0].is_active_lesson());
            assert!(state.planner.completed.contains("remote-id"));
            assert!(state
                .account_planners
                .values()
                .next()
                .unwrap()
                .completed
                .contains("other-remote-id"));
            assert!(state.planner.done_at.contains_key("remote-id"));
            state.assignments.clear();
            save(&db, &state).unwrap();
            let mut restored = AppState::new();
            restore(&db, &mut restored).unwrap();
            assert!(restored.assignments.is_empty());
            assert!(restored.planner.completed.contains("remote-id"));
        }
        std::fs::remove_file(path).unwrap();
    }
}
