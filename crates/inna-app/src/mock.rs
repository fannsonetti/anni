use chrono::{Datelike, NaiveDate, Utc};
use inna_core::models::*;

pub fn mock_user() -> User {
    User {
        student_id: Some("100001".into()),
        user_id: Some(serde_json::json!(999)),
        school_id: Some("SCH01".into()),
        school_short: Some("FLENS".into()),
        school_long: Some("Flensborgarskólinn í Hafnarfirði".into()),
        default_term_id: Some("TERM2026".into()),
        student_name: Some("Jón Jónsson".into()),
        is_student: Some(true),
    }
}

pub fn mock_timetable() -> Vec<TimetableEntry> {
    let today = Utc::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);

    let mut entries = Vec::new();

    let schedule: &[(&str, &str, &str, Option<&str>, Option<&str>)] = &[
        ("08:30", "09:30", "DANS1GR05", Some("M203"), Some("ÁKH")),
        ("09:50", "10:50", "ENSK2HF05", Some("A101"), Some("HSK")),
        ("10:55", "11:55", "STÆR3AF05", Some("B204"), Some("GÞJ")),
        ("12:35", "13:35", "EÐLI2GR05", Some("N302"), Some("ÞÓR")),
        ("13:40", "14:40", "ÍSLE2MM05", Some("A103"), Some("BJÖ")),
    ];

    let schedule_tue: &[(&str, &str, &str, Option<&str>, Option<&str>)] = &[
        ("08:30", "09:30", "STÆR3AF05", Some("B204"), Some("GÞJ")),
        ("09:50", "10:50", "EÐLI2GR05", Some("N302"), Some("ÞÓR")),
        ("10:55", "11:55", "DANS1GR05", Some("M203"), Some("ÁKH")),
        ("12:35", "13:35", "ENSK2HF05", Some("A101"), Some("HSK")),
    ];

    let schedule_wed: &[(&str, &str, &str, Option<&str>, Option<&str>)] = &[
        ("08:30", "09:30", "ÍSLE2MM05", Some("A103"), Some("BJÖ")),
        ("09:50", "10:50", "DANS1GR05", Some("M203"), Some("ÁKH")),
        ("10:55", "11:55", "STÆR3AF05", Some("B204"), Some("GÞJ")),
        ("13:40", "14:40", "EÐLI2GR05", None, Some("ÞÓR")),
    ];

    let schedule_thu: &[(&str, &str, &str, Option<&str>, Option<&str>)] = &[
        ("08:30", "09:30", "ENSK2HF05", Some("A101"), Some("HSK")),
        ("09:50", "10:50", "ÍSLE2MM05", Some("A103"), Some("BJÖ")),
        ("10:55", "11:55", "EÐLI2GR05", Some("N302"), Some("ÞÓR")),
        ("12:35", "13:35", "DANS1GR05", Some("M203"), Some("ÁKH")),
        ("13:40", "14:40", "STÆR3AF05", Some("B204"), None),
    ];

    let schedule_fri: &[(&str, &str, &str, Option<&str>, Option<&str>)] = &[
        ("08:30", "09:30", "STÆR3AF05", Some("B204"), Some("GÞJ")),
        ("09:50", "10:50", "ÍSLE2MM05", Some("A103"), Some("BJÖ")),
    ];

    let days: &[&[(&str, &str, &str, Option<&str>, Option<&str>)]] = &[
        schedule,
        schedule_tue,
        schedule_wed,
        schedule_thu,
        schedule_fri,
    ];

    for (day_offset, day_schedule) in days.iter().enumerate() {
        let date = monday + chrono::Duration::days(day_offset as i64);
        for (start, end, course, room, teacher) in *day_schedule {
            entries.push(make_entry(date, start, end, course, *room, *teacher));
        }
    }

    // Next week too
    let next_monday = monday + chrono::Duration::days(7);
    for (day_offset, day_schedule) in days.iter().enumerate() {
        let date = next_monday + chrono::Duration::days(day_offset as i64);
        for (start, end, course, room, teacher) in *day_schedule {
            entries.push(make_entry(date, start, end, course, *room, *teacher));
        }
    }

    entries
}

fn make_entry(
    date: NaiveDate,
    start_clock: &str,
    end_clock: &str,
    title: &str,
    room: Option<&str>,
    teacher: Option<&str>,
) -> TimetableEntry {
    let start_dt = format!("{}T{}:00.000Z", date, start_clock);
    let end_dt = format!("{}T{}:00.000Z", date, end_clock);

    TimetableEntry {
        module_id: Some(format!("MOD_{title}")),
        group_id: Some("GRP001".into()),
        start: Some(start_dt),
        end: Some(end_dt),
        module_term_id: Some("TERM001".into()),
        title_short: Some(title.into()),
        all_day: Some(false),
        start_clock: Some(start_clock.into()),
        end_clock: Some(end_clock.into()),
        teacher: teacher.map(|s| s.into()),
        classroom: room.map(|s| s.into()),
        group: Some("Group 3".into()),
        timetable_id: Some(0),
        maintable_id: Some(1000),
        student_record_id: Some(2000),
    }
}

pub fn mock_announcements() -> Vec<Announcement> {
    vec![
        Announcement {
            date: Some("2026-08-17T15:38:00".into()),
            content_short: Some("Welcome to ENSK2HF05. We will start with an introduction to the course and materials.".into()),
            announcement_type: None,
            content_html: Some("<p>Dear students,</p><p>Welcome to <strong>ENSK2HF05</strong>. We will introduce the course and review the syllabus. Remember to bring your textbook.</p><p>Have a great term!</p>".into()),
            title: Some("Welcome to English".into()),
            user_id: None,
            month: Some("August".into()),
            sender: Some("Helga Sóley Kristjánsdóttir".into()),
            has_opened: Some(false),
            announcement_id: Some(serde_json::json!("ANN001")),
            day: Some("17".into()),
            module_name: Some("ENSK2HF05".into()),
            group: Some("Group 7".into()),
        },
        Announcement {
            date: Some("2026-08-16T10:00:00".into()),
            content_short: Some("Welcome to Flensborg! Here is an overview of this term's assignments…".into()),
            announcement_type: None,
            content_html: Some("<p>Welcome to Flensborg!</p><p>This term includes three major projects and several smaller assignments.</p>".into()),
            title: Some("Welcome to Flensborg".into()),
            user_id: None,
            month: Some("August".into()),
            sender: Some("Hilmir Örn Sigurðsson".into()),
            has_opened: Some(true),
            announcement_id: Some(serde_json::json!("ANN002")),
            day: Some("16".into()),
            module_name: Some("HÁMA1GH02".into()),
            group: None,
        },
        Announcement {
            date: Some("2026-08-15T09:00:00".into()),
            content_short: Some("The autumn timetable has been updated. Please check the changes for Wednesday.".into()),
            announcement_type: None,
            content_html: Some("<p>The autumn timetable has been updated.</p><p>Please check the changes for Wednesday.</p>".into()),
            title: Some("Timetable updated".into()),
            user_id: None,
            month: Some("August".into()),
            sender: Some("Headteacher".into()),
            has_opened: Some(true),
            announcement_id: Some(serde_json::json!("ANN003")),
            day: Some("15".into()),
            module_name: None,
            group: None,
        },
    ]
}

pub fn mock_assignments() -> Vec<AssignmentModule> {
    let today = Utc::now().date_naive();
    let mut modules: Vec<AssignmentModule> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/student_projects.json"
    ))
    .unwrap();
    let upcoming: AssignmentModule = serde_json::from_value(serde_json::json!({
        "moduleName": "STÆR3AF05", "groupId": "sample-math", "assignments": [
            {"assignmentId": "sample-graded-1", "name": "Chapter test 1", "exam": "1", "grade": "3,5", "weight": 20, "handedIn": "1"},
            {"assignmentId": "sample-graded-2", "name": "Chapter test 2", "exam": "1", "grade": "4,3", "weight": 20, "handedIn": "1"},
            {"assignmentId": "sample-1", "name": "Quadratic equations practice", "orderHandInDate": format!("{} 23:59:00", today - chrono::Duration::days(1)), "handedIn": "0"},
            {"assignmentId": "sample-2", "name": "Algebra quiz", "exam": "1", "orderHandInDate": format!("{} 10:55:00", today + chrono::Duration::days(1)), "handedIn": "0"},
            {"assignmentId": "sample-3", "name": "Review notes for the next class", "handedIn": "0"}
        ]
    })).unwrap();
    modules.push(upcoming);
    if let Some(english) = modules.iter_mut().find(|m| m.module_str() == "ENSK2HF05") {
        english.assignments.extend(serde_json::from_value::<Vec<Assignment>>(serde_json::json!([
            {"assignmentId":"sample-english-1","name":"Reading comprehension","grade":"7,5","weight":20,"handedIn":"1"},
            {"assignmentId":"sample-english-2","name":"Writing","grade":"8,0","weight":15,"handedIn":"1"},
            {"assignmentId":"sample-english-3","name":"Final project","weight":30}
        ])).unwrap());
    }
    modules
}

pub fn mock_homework() -> Vec<Homework> {
    serde_json::from_value(serde_json::json!([{
        "id":"sample-reading", "text":"Read chapter 3", "moduleName":"ENSK2HF05",
        "date":Utc::now().date_naive().format("%d.%m.%Y").to_string(), "content":"Read pages 24–36 and prepare three questions for the next class."
    }])).unwrap()
}

pub fn mock_todo() -> Vec<TodoItem> {
    serde_json::from_value(serde_json::json!([{"id":"sample-book", "title":"Bring your physics workbook", "moduleName":"EÐLI2GR05"}])).unwrap()
}
