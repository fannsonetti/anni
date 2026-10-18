use crate::ui::theme::*;
use crate::{locale, state::AppState};
use chrono::{Datelike, Duration, Utc};
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(ui, "Timetable", "Your week ahead, with rooms and teachers.");
    let today = Utc::now().date_naive();
    let now = Utc::now().naive_utc();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Previous week").clicked() {
            state.selected_date -= Duration::days(7);
        }
        let first = state.selected_date
            - Duration::days(state.selected_date.weekday().num_days_from_monday() as i64);
        ui.label(
            RichText::new(format!(
                "{} – {}",
                locale::short_date(first),
                locale::short_date(first + Duration::days(6))
            ))
            .strong(),
        );
        if ui.button("Next week").clicked() {
            state.selected_date += Duration::days(7);
        }
        if ui.button(RichText::new("Today").color(ACCENT)).clicked() {
            state.selected_date = today;
        }
    });
    ui.add_space(12.0);
    let first = state.selected_date
        - Duration::days(state.selected_date.weekday().num_days_from_monday() as i64);
    ui.horizontal_wrapped(|ui| {
        for (i, name) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .enumerate()
        {
            let date = first + Duration::days(i as i64);
            let label = format!("{name} {}.", date.day());
            if ui
                .selectable_label(state.selected_date == date, label)
                .clicked()
            {
                state.selected_date = date;
            }
        }
    });
    ui.add_space(18.0);
    ui.label(
        RichText::new(locale::day(state.selected_date))
            .size(22.0)
            .strong(),
    );
    ui.add_space(8.0);
    let lessons = state.lessons_for_date(state.selected_date);
    if lessons.is_empty() {
        card().show(ui, |ui| {
            ui.label("No lessons are listed for this day.");
        });
        return;
    }
    for entry in lessons {
        let start = entry
            .start
            .as_deref()
            .and_then(crate::planner::parse_deadline)
            .map(|d| d.at);
        let end = entry
            .end
            .as_deref()
            .and_then(crate::planner::parse_deadline)
            .map(|d| d.at);
        let current = start.zip(end).is_some_and(|(s, e)| s <= now && now < e);
        card()
            .fill(if current { NOW_HIGHLIGHT } else { CARD_BG })
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if current {
                    badge(ui, "In class now", SUCCESS);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(entry.time_range())
                            .size(20.0)
                            .color(ACCENT)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(crate::ui::assignments::subject(entry.course_code()))
                            .size(22.0)
                            .strong(),
                    );
                    ui.label(RichText::new(entry.course_code()).color(TEXT_SECONDARY));
                });
                ui.horizontal_wrapped(|ui| {
                    if !entry.room().is_empty() {
                        badge(ui, &format!("Room {}", entry.room()), ACCENT);
                    }
                    if !entry.teacher_str().is_empty() {
                        ui.label(format!("Teacher: {}", entry.teacher_str()));
                    }
                });
            });
    }
}
