use crate::ui::theme::*;
use crate::{locale, planner, state::AppState};
use chrono::Utc;
use egui::{RichText, Ui};
use std::time::Instant;

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        "Focus",
        "One task at a time. Make space for focused work.",
    );
    let items = planner::work_items(state);
    let now = Instant::now();
    card()
        .fill(NEXT_HIGHLIGHT)
        .inner_margin(28.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(if state.focus.is_break {
                        "BREAK"
                    } else {
                        "FOCUS SESSION"
                    })
                    .size(12.0)
                    .strong()
                    .color(VIOLET),
                );
                ui.add_space(16.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(300.0, 44.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_enabled_ui(!state.focus.in_progress(), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (minutes, is_break, label) in [
                                    (25, false, "25 min"),
                                    (50, false, "50 min"),
                                    (5, true, "5 min break"),
                                ] {
                                    if ui
                                        .selectable_label(
                                            state.focus.minutes == minutes
                                                && state.focus.is_break == is_break,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        state.focus.preset(minutes, is_break);
                                    }
                                }
                            });
                        });
                    },
                );
                ui.add_space(18.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(240.0, 240.0), egui::Sense::hover());
                let center = rect.center();
                let painter = ui.painter();
                painter.circle_stroke(center, 110.0, egui::Stroke::new(7.0, BORDER));
                let progress = 1.0
                    - (state.focus.remaining(now).as_secs_f32()
                        / state.focus.duration().as_secs_f32())
                    .clamp(0.0, 1.0);
                if progress > 0.0 {
                    let points: Vec<_> = (0..=100)
                        .map(|i| {
                            let angle = -std::f32::consts::FRAC_PI_2
                                + std::f32::consts::TAU * progress * i as f32 / 100.0;
                            center + egui::vec2(angle.cos(), angle.sin()) * 110.0
                        })
                        .collect();
                    painter.add(egui::Shape::line(points, egui::Stroke::new(7.0, ACCENT)));
                }
                painter.text(
                    center - egui::vec2(0.0, 8.0),
                    egui::Align2::CENTER_CENTER,
                    state.focus.clock(now),
                    egui::FontId::monospace(49.0),
                    TEXT_PRIMARY,
                );
                painter.text(
                    center + egui::vec2(0.0, 34.0),
                    egui::Align2::CENTER_CENTER,
                    if state.focus.completed {
                        "COMPLETE"
                    } else if state.focus.running() {
                        "IN PROGRESS"
                    } else if state.focus.in_progress() {
                        "PAUSED"
                    } else {
                        "READY"
                    },
                    egui::FontId::proportional(12.0),
                    TEXT_SECONDARY,
                );
                ui.add_space(16.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width().min(420.0), 44.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_enabled_ui(
                            !state.focus.in_progress() && !state.focus.completed,
                            |ui| {
                                egui::ComboBox::from_id_salt("focus_task")
                                    .width(ui.available_width().min(420.0))
                                    .selected_text(if state.focus.task_title.is_empty() {
                                        "Free focus"
                                    } else {
                                        &state.focus.task_title
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                state.focus.task_key.is_empty(),
                                                "Free focus",
                                            )
                                            .clicked()
                                        {
                                            state.focus.task_key.clear();
                                            state.focus.task_title.clear();
                                        }
                                        for item in items.iter().filter(|i| !i.done) {
                                            if ui
                                                .selectable_label(
                                                    state.focus.task_key == item.key,
                                                    &item.title,
                                                )
                                                .clicked()
                                            {
                                                state.focus.task_key = item.key.clone();
                                                state.focus.task_title = item.title.clone();
                                            }
                                        }
                                    });
                            },
                        );
                    },
                );
                ui.add_space(14.0);
                if ui
                    .add_sized(
                        [210.0, 44.0],
                        primary_button(if state.focus.running() {
                            "Pause"
                        } else if state.focus.completed {
                            "Start another session"
                        } else if state.focus.in_progress() {
                            "Resume"
                        } else {
                            "Start session"
                        }),
                    )
                    .clicked()
                {
                    state.focus.toggle(Instant::now());
                }
                if ui
                    .add_enabled(
                        state.focus.in_progress() || state.focus.completed,
                        egui::Button::new("Reset"),
                    )
                    .on_hover_text("An unfinished session does not count toward study minutes.")
                    .clicked()
                {
                    state.focus.reset();
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Quitting Anni stops the active timer. Completed sessions are saved.",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });
        });
    if state.focus.completed {
        card().fill(NOW_HIGHLIGHT).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(if state.focus.is_break {
                    "Break complete. Ready for another session?"
                } else {
                    "Nice work. Session complete!"
                })
                .strong()
                .color(SUCCESS),
            );
            if !state.focus.is_break {
                ui.label("Your study minutes have been saved. Take a short break.");
                if ui.button("Take a 5-minute break").clicked() {
                    state.focus.preset(5, true);
                }
                if let Some(item) = items
                    .iter()
                    .find(|i| i.key == state.focus.task_key && !i.done)
                {
                    if ui.button("Mark task complete").clicked() {
                        item.set_done(&mut state.planner, true);
                        state.storage_dirty = true;
                    }
                }
            }
        });
    }
    ui.add_space(12.0);
    let today = Utc::now().date_naive();
    let minutes: u32 = state
        .planner
        .focus_sessions
        .iter()
        .filter(|s| s.date == today)
        .map(|s| s.minutes)
        .sum();
    let count = state
        .planner
        .focus_sessions
        .iter()
        .filter(|s| s.date == today)
        .count();
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Today's study").size(19.0).strong());
        ui.label(
            RichText::new(format!(
                "{} minutes  ·  {}",
                locale::number(minutes as usize),
                locale::counted(count, "session", "sessions")
            ))
            .size(24.0)
            .color(ACCENT),
        );
        ui.add_space(8.0);
        if state.planner.focus_sessions.is_empty() {
            ui.label(
                RichText::new("Your first completed session will appear here.")
                    .color(TEXT_SECONDARY),
            );
        }
        for session in state.planner.focus_sessions.iter().rev().take(5) {
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("{} min", session.minutes), VIOLET);
                ui.label(if session.task.is_empty() {
                    "Free focus"
                } else {
                    &session.task
                });
                ui.label(
                    RichText::new(locale::short_date(session.date))
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
            });
        }
    });
}
