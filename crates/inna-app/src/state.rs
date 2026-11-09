use chrono::{NaiveDate, NaiveDateTime, Utc};
use inna_core::models::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Starting,
    Offline,
    Authenticating,
    Connected,
    SessionExpired,
    Error(String),
    MockMode,
}

impl ConnectionState {
    pub fn label(&self) -> &str {
        match self {
            Self::Starting => "Not connected",
            Self::Offline => "Offline",
            Self::Authenticating => "Connecting…",
            Self::Connected => "Connected to INNA",
            Self::SessionExpired => "Sign-in expired",
            Self::Error(_) => "Connection error",
            Self::MockMode => "Demo data",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Connected | Self::MockMode => crate::ui::theme::SUCCESS,
            Self::Offline | Self::SessionExpired => crate::ui::theme::WARNING,
            Self::Error(_) => crate::ui::theme::ERROR_RED,
            _ => crate::ui::theme::TEXT_SECONDARY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Timetable,
    Announcements,
    Assignments,
    Planner,
    Connection,
}

impl Page {
    pub fn label(&self) -> &str {
        match self {
            Self::Home => "Overview",
            Self::Timetable => "Timetable",
            Self::Announcements => "Announcements",
            Self::Assignments => "Courses & grades",
            Self::Planner => "Schoolwork",
            Self::Connection => "Settings",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Home => "Your school portal at a glance",
            Self::Planner => "Assignments, tests, homework, and submissions",
            Self::Timetable => "Lessons, rooms, and teachers",
            Self::Assignments => "Grades and progress in each course",
            Self::Announcements => "News from your school",
            Self::Connection => "Your INNA connection and saved data",
        }
    }

    pub const SCHOOL_PAGES: &'static [Page] = &[
        Page::Home,
        Page::Timetable,
        Page::Assignments,
        Page::Planner,
        Page::Announcements,
    ];

    pub const NAV_PAGES: &'static [Page] = &[
        Page::Home,
        Page::Timetable,
        Page::Assignments,
        Page::Planner,
        Page::Announcements,
        Page::Connection,
    ];
}

#[derive(Clone, Copy)]
struct Destination {
    page: Page,
    announcement: Option<usize>,
}

pub struct AppState {
    pub connection: ConnectionState,
    pub user: Option<User>,
    pub timetable: Vec<TimetableEntry>,
    pub announcements: Vec<Announcement>,
    pub assignments: Vec<AssignmentModule>,
    pub grade_view: crate::grades::GradeView,
    pub homework: Vec<Homework>,
    pub todo_items: Vec<TodoItem>,
    pub selected_page: Page,
    back_history: Vec<Destination>,
    forward_history: Vec<Destination>,
    pub selected_date: NaiveDate,
    pub last_sync: HashMap<String, String>,
    pub refreshing: bool,
    pub selected_announcement: Option<usize>,
    pub planner: crate::planner::Planner,
    pub account_planners: HashMap<String, crate::planner::Planner>,
    pub work_filter: crate::planner::WorkFilter,
    pub work_search: String,
    pub work_date: Option<NaiveDate>,
    pub announcement_search: String,
    pub announcements_unread_only: bool,
    pub storage_dirty: bool,
    pub storage_error: Option<String>,
    pub sync_errors: HashMap<String, String>,
    pub refresh_requested: bool,
}

impl AppState {
    fn destination(&self) -> Destination {
        Destination {
            page: self.selected_page,
            announcement: self.selected_announcement,
        }
    }

    pub fn navigate(&mut self, page: Page) {
        self.visit(Destination {
            page,
            announcement: None,
        });
    }

    pub fn open_announcement(&mut self, index: usize) {
        self.visit(Destination {
            page: Page::Announcements,
            announcement: Some(index),
        });
    }

    pub fn announcements_replaced(&mut self) {
        self.selected_announcement = None;
        // INNA can reorder the list during sync. Never restore a stale list index.
        for destination in self
            .back_history
            .iter_mut()
            .chain(&mut self.forward_history)
        {
            destination.announcement = None;
        }
    }

    fn visit(&mut self, target: Destination) {
        if self.selected_page == target.page && self.selected_announcement == target.announcement {
            return;
        }
        self.back_history.push(self.destination());
        if self.back_history.len() > 50 {
            self.back_history.remove(0);
        }
        self.forward_history.clear();
        self.selected_page = target.page;
        self.selected_announcement = target.announcement;
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_history.is_empty()
    }

    pub fn go_back(&mut self) {
        if let Some(target) = self.back_history.pop() {
            self.forward_history.push(self.destination());
            self.selected_page = target.page;
            self.selected_announcement = target.announcement;
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(target) = self.forward_history.pop() {
            self.back_history.push(self.destination());
            self.selected_page = target.page;
            self.selected_announcement = target.announcement;
        }
    }

    pub fn adopt_user(&mut self, user: User) {
        let changed_account = self
            .user
            .as_ref()
            .is_some_and(|old| account_key(old) != account_key(&user));
        if changed_account {
            if let Some(old) = &self.user {
                self.account_planners
                    .insert(account_key(old), std::mem::take(&mut self.planner));
            }
            self.timetable.clear();
            self.announcements.clear();
            self.assignments.clear();
            self.homework.clear();
            self.todo_items.clear();
            self.last_sync.clear();
            self.planner = self
                .account_planners
                .remove(&account_key(&user))
                .unwrap_or_default();
            self.sync_errors.clear();
            self.selected_announcement = None;
            self.back_history.clear();
            self.forward_history.clear();
        }
        self.user = Some(user);
    }

    pub fn new() -> Self {
        Self {
            connection: ConnectionState::Starting,
            user: None,
            timetable: Vec::new(),
            announcements: Vec::new(),
            assignments: Vec::new(),
            grade_view: Default::default(),
            homework: Vec::new(),
            todo_items: Vec::new(),
            selected_page: Page::Home,
            back_history: Vec::new(),
            forward_history: Vec::new(),
            selected_date: Utc::now().date_naive(),
            last_sync: HashMap::new(),
            refreshing: false,
            selected_announcement: None,
            planner: Default::default(),
            account_planners: HashMap::new(),
            work_filter: Default::default(),
            work_search: String::new(),
            work_date: None,
            announcement_search: String::new(),
            announcements_unread_only: false,
            storage_dirty: false,
            storage_error: None,
            sync_errors: HashMap::new(),
            refresh_requested: false,
        }
    }

    pub fn today_lessons(&self) -> Vec<&TimetableEntry> {
        self.lessons_for_date(Utc::now().date_naive())
    }

    pub fn lessons_for_date(&self, date: NaiveDate) -> Vec<&TimetableEntry> {
        let key = date.format("%Y-%m-%d").to_string();
        let mut lessons: Vec<_> = self
            .timetable
            .iter()
            .filter(|e| {
                e.is_active_lesson()
                    && e.start
                        .as_deref()
                        .map(|s| s.starts_with(&key))
                        .unwrap_or(false)
            })
            .collect();
        lessons.sort_by_key(|e| e.start_clock.as_deref().unwrap_or("99:99"));
        lessons
    }

    pub fn current_and_next_lesson(&self) -> (Option<&TimetableEntry>, Option<&TimetableEntry>) {
        self.current_and_next_lesson_at(Utc::now().naive_utc())
    }

    pub fn current_and_next_lesson_at(
        &self,
        now: NaiveDateTime,
    ) -> (Option<&TimetableEntry>, Option<&TimetableEntry>) {
        let mut dated: Vec<_> = self
            .timetable
            .iter()
            .filter_map(|e| {
                if !e.is_active_lesson() {
                    return None;
                }
                let start = crate::planner::parse_deadline(e.start.as_deref()?)?.at;
                let end = crate::planner::parse_deadline(e.end.as_deref()?)?.at;
                Some((e, start, end))
            })
            .collect();
        dated.sort_by_key(|(_, start, _)| *start);
        let current = dated
            .iter()
            .find(|(_, start, end)| *start <= now && now < *end)
            .map(|(e, _, _)| *e);
        let next = dated
            .iter()
            .find(|(_, start, _)| *start > now)
            .map(|(e, _, _)| *e);
        (current, next)
    }
}

fn account_key(user: &User) -> String {
    let identity = user
        .student_id
        .clone()
        .map(serde_json::Value::String)
        .or(user.user_id.clone());
    serde_json::to_string(&(&user.school_id, identity)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_returns_from_shortcuts_and_new_routes_clear_forward_history() {
        let mut state = AppState::new();
        assert!(!state.can_go_back());
        state.navigate(Page::Timetable);
        state.navigate(Page::Timetable);
        state.navigate(Page::Planner);
        assert_eq!(state.selected_page, Page::Planner);
        state.go_back();
        assert_eq!(state.selected_page, Page::Timetable);
        state.go_back();
        assert_eq!(state.selected_page, Page::Home);
        assert!(!state.can_go_back());
        state.go_forward();
        assert_eq!(state.selected_page, Page::Timetable);
        state.navigate(Page::Assignments);
        assert!(!state.can_go_forward());
    }

    #[test]
    fn announcement_history_restores_details_until_the_list_is_refreshed() {
        let mut state = AppState::new();
        state.navigate(Page::Announcements);
        state.open_announcement(2);
        state.navigate(Page::Planner);
        state.go_back();
        assert_eq!(state.selected_announcement, Some(2));
        state.go_back();
        assert_eq!(state.selected_page, Page::Announcements);
        assert_eq!(state.selected_announcement, None);
        state.go_forward();
        assert_eq!(state.selected_announcement, Some(2));
        state.navigate(Page::Home);
        state.announcements_replaced();
        state.go_back();
        assert_eq!(state.selected_page, Page::Announcements);
        assert_eq!(state.selected_announcement, None);
    }

    #[test]
    fn switching_accounts_keeps_separate_schoolwork_checkmarks() {
        let mut state = AppState::new();
        let first = crate::mock::mock_user();
        state.adopt_user(first.clone());
        state.planner.set_done("first-schoolwork", true);
        let mut other = first.clone();
        other.student_id = Some("another-student".into());
        state.adopt_user(other);
        assert!(state.planner.completed.is_empty());
        state.planner.set_done("other-schoolwork", true);
        state.adopt_user(first);
        assert!(state.planner.completed.contains("first-schoolwork"));
        assert!(!state.planner.completed.contains("other-schoolwork"));
    }

    #[test]
    fn next_lesson_crosses_weekends_and_today_ignores_schedule_navigation() {
        let mut state = AppState::new();
        state.timetable = serde_json::from_value(serde_json::json!([
            {"titleShort":"Vetrarfrí", "start":"2026-10-18T00:00:00Z", "end":"2026-10-20T23:59:00Z", "allDay":true},
            {"titleShort":"Later", "start":"2026-10-19T10:00:00Z", "end":"2026-10-19T11:00:00Z", "startClock":"10:00"},
            {"titleShort":"First", "start":"2026-10-19T08:30:00Z", "end":"2026-10-19T09:30:00Z", "startClock":"08:30"}
        ])).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(14, 0, 0)
            .unwrap();
        assert_eq!(
            state
                .current_and_next_lesson_at(sunday)
                .1
                .unwrap()
                .course_code(),
            "First"
        );
        let monday = sunday + chrono::Duration::days(1) - chrono::Duration::hours(5);
        assert_eq!(
            state
                .current_and_next_lesson_at(monday)
                .0
                .unwrap()
                .course_code(),
            "First"
        );
        assert_eq!(
            state.lessons_for_date(monday.date())[0].course_code(),
            "First"
        );
        assert_eq!(state.lessons_for_date(monday.date()).len(), 2);
        assert!(state.lessons_for_date(sunday.date()).is_empty());
        state.selected_date = Utc::now().date_naive() + chrono::Duration::days(100);
        assert_eq!(
            state.today_lessons().len(),
            state.lessons_for_date(Utc::now().date_naive()).len()
        );
    }
    #[test]
    fn teacher_absences_are_skipped_by_next_class_and_the_day_timer() {
        let mut state = AppState::new();
        state.timetable = serde_json::from_value(serde_json::json!([
            {"titleShort":"DANS1GR05", "group":"Example group", "start":"2026-11-10T08:30:00Z", "end":"2026-11-10T09:30:00Z"},
            {"titleShort":"ENSK2HF05", "start":"2026-11-10T09:50:00Z", "end":"2026-11-10T10:50:00Z"}
        ])).unwrap();
        let now = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap().and_hms_opt(18, 0, 0).unwrap();
        let notices = serde_json::from_str::<Vec<inna_core::teacher_absences::TeacherAbsence>>(
            include_str!("../../../tests/fixtures/teacher_absences.json")).unwrap();
        inna_core::teacher_absences::apply_teacher_absences(&mut state.timetable, &notices, now);
        assert_eq!(state.current_and_next_lesson_at(now).1.unwrap().course_code(), "ENSK2HF05");
        assert_eq!(state.lessons_for_date(now.date() + chrono::Duration::days(1)).len(), 1);
        assert_eq!(crate::day::lessons(&state.timetable).len(), 1);
        let during_cancelled = now.date().succ_opt().unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert!(state.current_and_next_lesson_at(during_cancelled).0.is_none());
        assert_eq!(crate::day::DayState::compute(&state.timetable, during_cancelled).phase, crate::day::Phase::Before);
    }
}
