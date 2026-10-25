use crate::ui::theme::*;
use crate::{locale, planner, state::AppState};
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc};
use egui::{Color32, FontId, RichText, Sense, Stroke, Ui};
use inna_core::models::TimetableEntry;

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        "Timetable",
        "Your week, with a little more breathing room.",
    );
    let today = Utc::now().date_naive();
    let wide = ui.available_width() >= 900.0;
    let view_id = ui.id().with("week_view");
    let mut week = ui.ctx().data_mut(|d| *d.get_temp_mut_or(view_id, true));
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("‹ Previous")
            .on_hover_text("Previous week")
            .clicked()
        {
            state.selected_date -= Duration::days(7);
        }
        if ui.button("Next ›").on_hover_text("Next week").clicked() {
            state.selected_date += Duration::days(7);
        }
        if ui.button("Today").clicked() {
            state.selected_date = today;
        }
        if wide {
            ui.separator();
            ui.selectable_value(&mut week, true, "Week");
            ui.selectable_value(&mut week, false, "Day");
        }
    });
    ui.add_space(14.0);
    let monday = state.selected_date
        - Duration::days(state.selected_date.weekday().num_days_from_monday() as i64);
    let sunday = monday + Duration::days(6);
    let first = if monday.month() == sunday.month() {
        monday.day().to_string()
    } else if monday.year() == sunday.year() {
        format!("{} {}", monday.day(), locale::month(monday))
    } else {
        locale::date(monday)
    };
    ui.label(
        RichText::new(format!("{first} – {}", locale::date(sunday)))
            .size(21.0)
            .strong(),
    );
    ui.add_space(12.0);
    for event in state.timetable.iter().filter(|entry| !entry.is_lesson()) {
        if let Some(date) = event.start.as_deref().and_then(planner::parse_deadline).map(|date| date.at.date()) {
            if date >= monday && date <= sunday {
                ui.label(RichText::new(format!("{} · {} · School calendar", locale::short_date(date), event.course_code())).color(TEXT_SECONDARY));
            }
        }
    }
    if wide && week {
        week_grid(ui, state, monday, &mut week);
    } else {
        day_picker(ui, state, monday);
        ui.add_space(16.0);
        day_schedule(ui, state);
    }
    ui.ctx().data_mut(|d| d.insert_temp(view_id, week));
}

fn day_picker(ui: &mut Ui, state: &mut AppState, monday: NaiveDate) {
    let gap = 6.0;
    let width = (ui.available_width() - gap * 6.0) / 7.0;
    ui.spacing_mut().item_spacing.x = gap;
    ui.horizontal(|ui| {
        for day in 0..7 {
            let date = monday + Duration::days(day);
            day_button(ui, state, date, width, 68.0);
        }
    });
}

fn day_button(ui: &mut Ui, state: &mut AppState, date: NaiveDate, width: f32, height: f32) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let selected = date == state.selected_date;
    let today = date == Utc::now().date_naive();
    let p = ui.painter();
    p.rect_filled(
        rect,
        9.0,
        if selected {
            ACCENT_DIM
        } else if response.hovered() {
            CARD_BG
        } else {
            PANEL_BG
        },
    );
    if selected {
        p.rect_stroke(rect, 9.0, Stroke::new(1.0, ACCENT));
    }
    let weekday = &locale::weekday(date)[..3];
    p.text(
        rect.center_top() + egui::vec2(0.0, 16.0),
        egui::Align2::CENTER_CENTER,
        format!("{weekday} {}", date.day()),
        FontId::proportional(14.0),
        if today || selected {
            ACCENT
        } else {
            TEXT_PRIMARY
        },
    );
    let count = state.lessons_for_date(date).len();
    p.text(
        rect.center_top() + egui::vec2(0.0, 40.0),
        egui::Align2::CENTER_CENTER,
        if count == 0 {
            "Free day".into()
        } else {
            locale::counted(count, "class", "classes")
        },
        FontId::proportional(11.0),
        TEXT_DIM,
    );
    if today {
        p.circle_filled(rect.center_bottom() - egui::vec2(0.0, 7.0), 2.0, ACCENT);
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        state.selected_date = date;
    }
}

fn course_color(code: &str) -> Color32 {
    let colors = [
        ACCENT,
        VIOLET,
        Color32::from_rgb(135, 191, 247),
        WARNING,
        Color32::from_rgb(239, 163, 183),
        Color32::from_rgb(151, 204, 151),
    ];
    let hash = code
        .chars()
        .take(4)
        .fold(0_usize, |h, c| h.wrapping_mul(31).wrapping_add(c as usize));
    colors[hash % colors.len()]
}

fn minutes(entry: &TimetableEntry) -> Option<(u32, u32)> {
    let time = |clock: Option<&str>, date: Option<&str>| {
        clock
            .and_then(|s| chrono::NaiveTime::parse_from_str(s, "%H:%M").ok())
            .or_else(|| date.and_then(planner::parse_deadline).map(|d| d.at.time()))
            .map(|t| t.hour() * 60 + t.minute())
    };
    let start = time(entry.start_clock.as_deref(), entry.start.as_deref())?;
    let end = time(entry.end_clock.as_deref(), entry.end.as_deref())?;
    (end > start).then_some((start, end))
}

struct Placed<'a> {
    entry: &'a TimetableEntry,
    start: u32,
    end: u32,
    lane: usize,
    lanes: usize,
}

/// Concurrent lessons share the column, including chains of partial overlaps.
fn place<'a>(lessons: &[&'a TimetableEntry]) -> Vec<Placed<'a>> {
    let mut placed: Vec<_> = lessons
        .iter()
        .filter_map(|entry| {
            let (start, end) = minutes(entry)?;
            Some(Placed {
                entry,
                start,
                end,
                lane: 0,
                lanes: 1,
            })
        })
        .collect();
    placed.sort_by_key(|p| (p.start, p.end));
    let mut first = 0;
    while first < placed.len() {
        let mut last = first;
        let mut group_end = placed[first].end;
        let mut ends = Vec::new();
        while last < placed.len() && (last == first || placed[last].start < group_end) {
            let p = &mut placed[last];
            let lane = ends
                .iter()
                .position(|&end| end <= p.start)
                .unwrap_or(ends.len());
            if lane == ends.len() {
                ends.push(p.end);
            } else {
                ends[lane] = p.end;
            }
            p.lane = lane;
            group_end = group_end.max(p.end);
            last += 1;
        }
        for p in &mut placed[first..last] {
            p.lanes = ends.len();
        }
        first = last;
    }
    placed
}

fn week_grid(ui: &mut Ui, state: &mut AppState, monday: NaiveDate, week: &mut bool) {
    let weekend = (5..7).any(|day| {
        !state
            .lessons_for_date(monday + Duration::days(day))
            .is_empty()
    });
    let days = if weekend { 7 } else { 5 };
    let gutter = 52.0;
    let gap = 8.0;
    let width = (ui.available_width() - gutter - gap * (days - 1) as f32) / days as f32;
    ui.spacing_mut().item_spacing.x = gap;
    ui.horizontal(|ui| {
        ui.allocate_space(egui::vec2(gutter - gap, 64.0));
        for day in 0..days {
            day_button(ui, state, monday + Duration::days(day), width, 64.0);
        }
    });
    let lessons: Vec<_> = (0..days)
        .map(|day| state.lessons_for_date(monday + Duration::days(day)))
        .collect();
    let timed: Vec<_> = lessons.iter().map(|day| place(day)).collect();
    if timed.iter().all(Vec::is_empty) {
        ui.add_space(12.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label("No lessons are listed for this week.");
            ui.label(
                RichText::new("Try another week or refresh your timetable.").color(TEXT_SECONDARY),
            );
        });
        return;
    }
    let first = timed
        .iter()
        .flatten()
        .map(|p| p.start)
        .min()
        .unwrap_or(8 * 60)
        / 60
        * 60;
    let last = timed
        .iter()
        .flatten()
        .map(|p| p.end)
        .max()
        .unwrap_or(16 * 60)
        .div_ceil(60)
        * 60;
    let scale = 1.4;
    let height = (last - first) as f32 * scale + 16.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    let top = rect.top() + 8.0;
    for day in 0..days as usize {
        let x = rect.left() + gutter + day as f32 * (width + gap);
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, height)),
            8.0,
            PANEL_BG,
        );
    }
    for minute in (first..=last).step_by(30) {
        let y = top + (minute - first) as f32 * scale;
        let color = if minute % 60 == 0 { BORDER } else { CARD_BG };
        p.line_segment(
            [
                egui::pos2(rect.left() + gutter, y),
                egui::pos2(rect.right(), y),
            ],
            Stroke::new(1.0, color),
        );
        if minute % 60 == 0 {
            p.text(
                egui::pos2(rect.left() + 40.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{:02}:00", minute / 60),
                FontId::proportional(11.0),
                TEXT_DIM,
            );
        }
    }
    let now = Utc::now().naive_utc();
    let next_start = state
        .current_and_next_lesson_at(now)
        .1
        .and_then(|e| e.start.clone());
    let mut open_day = None;
    for (day, placements) in timed.iter().enumerate() {
        let x = rect.left() + gutter + day as f32 * (width + gap);
        if placements.is_empty() {
            p.text(
                egui::pos2(x + width / 2.0, top + 28.0),
                egui::Align2::CENTER_CENTER,
                "No lessons",
                FontId::proportional(12.0),
                TEXT_DIM,
            );
        }
        for lesson in placements {
            let lane_width = width / lesson.lanes as f32;
            let r = egui::Rect::from_min_size(
                egui::pos2(
                    x + lesson.lane as f32 * lane_width + 3.0,
                    top + (lesson.start - first) as f32 * scale + 2.0,
                ),
                egui::vec2(
                    lane_width - 6.0,
                    ((lesson.end - lesson.start) as f32 * scale - 4.0).max(12.0),
                ),
            );
            let entry = lesson.entry;
            let color = course_color(entry.course_code());
            let current = entry
                .start
                .as_deref()
                .and_then(planner::parse_deadline)
                .zip(entry.end.as_deref().and_then(planner::parse_deadline))
                .is_some_and(|(s, e)| s.at <= now && now < e.at);
            let next = next_start.is_some() && entry.start == next_start;
            p.rect_filled(
                r,
                6.0,
                color.linear_multiply(if current { 0.22 } else { 0.12 }),
            );
            p.rect_stroke(
                r,
                6.0,
                Stroke::new(
                    if current { 1.5 } else { 0.7 },
                    if current {
                        color
                    } else {
                        color.linear_multiply(0.3)
                    },
                ),
            );
            p.rect_filled(
                egui::Rect::from_min_size(
                    r.min + egui::vec2(0.0, 7.0),
                    egui::vec2(3.0, (r.height() - 14.0).max(1.0)),
                ),
                1.5,
                color,
            );
            let text = p.with_clip_rect(r.shrink2(egui::vec2(8.0, 3.0)));
            let left = r.left() + 10.0;
            text.text(
                egui::pos2(left, r.top() + 8.0),
                egui::Align2::LEFT_TOP,
                entry.time_range(),
                FontId::proportional(11.0),
                color,
            );
            if r.height() > 35.0 {
                text.text(
                    egui::pos2(left, r.top() + 26.0),
                    egui::Align2::LEFT_TOP,
                    super::assignments::subject(entry.course_code()),
                    FontId::proportional(14.0),
                    TEXT_PRIMARY,
                );
            }
            if r.height() > 55.0 {
                text.text(
                    egui::pos2(left, r.top() + 46.0),
                    egui::Align2::LEFT_TOP,
                    entry.course_code(),
                    FontId::proportional(10.0),
                    TEXT_SECONDARY,
                );
            }
            if r.height() > 73.0 && !entry.room().is_empty() {
                text.text(
                    egui::pos2(left, r.bottom() - 8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("Room {}", entry.room()),
                    FontId::proportional(11.0),
                    TEXT_SECONDARY,
                );
            }
            if current || next {
                p.circle_filled(
                    r.right_top() + egui::vec2(-7.0, 7.0),
                    3.0,
                    if current { ACCENT } else { WARNING },
                );
            }
            if ui
                .interact(
                    r,
                    ui.id().with(("lesson", day, lesson.start, lesson.lane)),
                    Sense::click(),
                )
                .on_hover_text(format!(
                    "{} · {}\n{}\nRoom {} · {}{}\nClick to open this day",
                    super::assignments::subject(entry.course_code()),
                    entry.course_code(),
                    entry.time_range(),
                    entry.room(),
                    entry.teacher_str(),
                    if current {
                        " · In class now"
                    } else if next {
                        " · Next class"
                    } else {
                        ""
                    }
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                open_day = Some(monday + Duration::days(day as i64));
            }
        }
    }
    let minute = now.hour() * 60 + now.minute();
    let day = now.date().signed_duration_since(monday).num_days();
    if (0..days).contains(&day) && minute >= first && minute <= last {
        let x = rect.left() + gutter + day as f32 * (width + gap);
        let y = top + (minute - first) as f32 * scale;
        p.line_segment(
            [egui::pos2(x, y), egui::pos2(x + width, y)],
            Stroke::new(2.0, ACCENT),
        );
        p.circle_filled(egui::pos2(x, y), 4.0, ACCENT);
    }
    let untimed = lessons
        .iter()
        .flatten()
        .filter(|e| minutes(e).is_none())
        .count();
    ui.add_space(8.0);
    ui.label(
        RichText::new("Click a class for the day's details. Hover for teacher and room.")
            .size(12.0)
            .color(TEXT_DIM),
    );
    if untimed > 0 {
        ui.label(
            RichText::new(format!(
                "{untimed} lessons have no usable time. Open Day to see them."
            ))
            .size(12.0)
            .color(WARNING),
        );
    }
    if let Some(date) = open_day {
        state.selected_date = date;
        *week = false;
    }
}

fn day_schedule(ui: &mut Ui, state: &mut AppState) {
    ui.label(
        RichText::new(locale::day(state.selected_date))
            .size(21.0)
            .strong(),
    );
    let lessons = state.lessons_for_date(state.selected_date);
    let total: u32 = lessons
        .iter()
        .filter_map(|e| minutes(e))
        .map(|(s, e)| e - s)
        .sum();
    ui.label(
        RichText::new(format!(
            "{} · {}h {}m of lessons",
            locale::counted(lessons.len(), "class", "classes"),
            total / 60,
            total % 60
        ))
        .size(13.0)
        .color(TEXT_SECONDARY),
    );
    ui.add_space(10.0);
    if lessons.is_empty() {
        let next_date = state
            .timetable
            .iter()
            .filter(|entry| entry.is_lesson())
            .filter_map(|e| e.start.as_deref().and_then(planner::parse_deadline))
            .map(|d| d.at.date())
            .filter(|&d| d > state.selected_date)
            .min();
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("A little room in your day.")
                    .size(20.0)
                    .strong(),
            );
            ui.label(RichText::new("No lessons are listed for this date.").color(TEXT_SECONDARY));
            if let Some(date) = next_date {
                if ui
                    .button(format!("Next school day · {}", locale::short_date(date)))
                    .clicked()
                {
                    state.selected_date = date;
                }
            }
        });
        return;
    }
    let now = Utc::now().naive_utc();
    let next = state.current_and_next_lesson_at(now).1;
    let mut previous_end = None;
    for entry in lessons {
        let times = minutes(entry);
        if let Some((start, _)) = times {
            if let Some(end) = previous_end.filter(|&end| start > end) {
                ui.horizontal(|ui| {
                    ui.add_space(108.0);
                    ui.label(
                        RichText::new(format!("{} min break", start - end))
                            .size(12.0)
                            .color(TEXT_DIM),
                    );
                });
            }
        }
        previous_end = times.map(|(_, e)| previous_end.unwrap_or(e).max(e));
        let current = entry
            .start
            .as_deref()
            .and_then(planner::parse_deadline)
            .zip(entry.end.as_deref().and_then(planner::parse_deadline))
            .is_some_and(|(s, e)| s.at <= now && now < e.at);
        let is_next = next.is_some_and(|e| std::ptr::eq(e, entry));
        let color = course_color(entry.course_code());
        card()
            .fill(if current { NOW_HIGHLIGHT } else { PANEL_BG })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(80.0, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(80.0);
                            ui.label(
                                RichText::new(entry.start_clock.as_deref().unwrap_or("—"))
                                    .size(21.0)
                                    .strong()
                                    .color(color),
                            );
                            ui.label(
                                RichText::new(entry.end_clock.as_deref().unwrap_or("—"))
                                    .size(13.0)
                                    .color(TEXT_DIM),
                            );
                        },
                    );
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(super::assignments::subject(entry.course_code()))
                                    .size(20.0)
                                    .strong(),
                            );
                            if current {
                                badge(ui, "In class now", ACCENT);
                            } else if is_next {
                                badge(ui, "Up next", WARNING);
                            }
                        });
                        ui.label(RichText::new(entry.course_code()).size(12.0).color(color));
                        ui.horizontal_wrapped(|ui| {
                            if !entry.room().is_empty() {
                                badge(ui, &format!("Room {}", entry.room()), color);
                            }
                            if !entry.teacher_str().is_empty() {
                                ui.label(
                                    RichText::new(entry.teacher_str())
                                        .size(13.0)
                                        .color(TEXT_SECONDARY),
                                );
                            }
                        });
                    });
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_lessons_get_separate_lanes_and_adjacent_classes_reuse_space() {
        let lessons: Vec<TimetableEntry> = serde_json::from_value(serde_json::json!([
            {"startClock":"08:00","endClock":"09:00"},
            {"startClock":"08:30","endClock":"09:30"},
            {"startClock":"09:00","endClock":"10:00"},
            {"startClock":"10:00","endClock":"11:00"}
        ]))
        .unwrap();
        let placed = place(&lessons.iter().collect::<Vec<_>>());
        assert_eq!(
            placed.iter().map(|p| (p.lane, p.lanes)).collect::<Vec<_>>(),
            [(0, 2), (1, 2), (0, 2), (0, 1)]
        );
    }
}
