use crate::ui::theme::*;
use crate::{locale, planner, state::AppState};
use chrono::Utc;
use egui::{Align, Layout, RichText, Ui};
use std::time::Instant;

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(ui, "Focus", "A little space to do your best work.");
    let items = planner::work_items(state);
    if ui.available_width() >= 740.0 {
        let left = (ui.available_width() - 20.0) * 0.58;
        let right = ui.available_width() - left - 20.0;
        ui.spacing_mut().item_spacing.x = 20.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(left, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left);
                timer(ui, state);
                completion(ui, state, &items);
            });
            ui.allocate_ui_with_layout(
                egui::vec2(right, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(right);
                    setup(ui, state, &items);
                    history(ui, state);
                },
            );
        });
    } else {
        timer(ui, state);
        completion(ui, state, &items);
        setup(ui, state, &items);
        history(ui, state);
    }
}

fn timer(ui: &mut Ui, state: &mut AppState) {
    card().fill(PANEL_BG).inner_margin(24.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(if state.focus.is_break {
                    "TAKE A BREATHER"
                } else {
                    "TIME FOR YOURSELF"
                })
                .size(11.0)
                .strong()
                .color(if state.focus.is_break { VIOLET } else { ACCENT }),
            );
            ui.add_space(20.0);
            let size = ui.available_width().min(280.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            let center = rect.center();
            let radius = size * 0.45;
            let now = Instant::now();
            let accent = if state.focus.is_break { VIOLET } else { ACCENT };
            let painter = ui.painter();
            painter.circle_stroke(center, radius, egui::Stroke::new(3.0, BORDER));
            let progress = (1.0
                - state.focus.remaining(now).as_secs_f32() / state.focus.duration().as_secs_f32())
            .clamp(0.0, 1.0);
            if progress > 0.0 {
                let points: Vec<_> = (0..=120)
                    .map(|i| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + std::f32::consts::TAU * progress * i as f32 / 120.0;
                        center + egui::vec2(angle.cos(), angle.sin()) * radius
                    })
                    .collect();
                if let Some(&end) = points.last() {
                    painter.circle_filled(end, 5.0, accent);
                }
                painter.add(egui::Shape::line(points, egui::Stroke::new(4.0, accent)));
            }
            painter.text(
                center - egui::vec2(0.0, 7.0),
                egui::Align2::CENTER_CENTER,
                state.focus.clock(now),
                egui::FontId::monospace(size * 0.19),
                TEXT_PRIMARY,
            );
            let status = if state.focus.completed {
                "SESSION COMPLETE"
            } else if state.focus.running() {
                "ONE THING AT A TIME"
            } else if state.focus.in_progress() {
                "PAUSED · TAKE YOUR TIME"
            } else {
                "READY WHEN YOU ARE"
            };
            painter.text(
                center + egui::vec2(0.0, 34.0),
                egui::Align2::CENTER_CENTER,
                status,
                egui::FontId::proportional(11.0),
                TEXT_SECONDARY,
            );
            ui.add_space(18.0);
            ui.add(
                egui::Label::new(
                    RichText::new(if state.focus.task_title.is_empty() {
                        "Free focus"
                    } else {
                        &state.focus.task_title
                    })
                    .size(17.0)
                    .strong(),
                )
                .wrap(),
            );
            ui.add_space(14.0);
            let label = if state.focus.running() {
                "Pause session"
            } else if state.focus.completed {
                "Start another session"
            } else if state.focus.in_progress() {
                "Resume session"
            } else {
                "Start session"
            };
            if ui
                .add_sized(
                    [ui.available_width().min(260.0), 46.0],
                    primary_button(label),
                )
                .clicked()
            {
                state.focus.toggle(Instant::now());
            }
            if ui
                .add_enabled(
                    state.focus.in_progress() || state.focus.completed,
                    egui::Button::new("Reset timer").frame(false),
                )
                .on_hover_text("Unfinished minutes are not added to your study history.")
                .clicked()
            {
                state.focus.reset();
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new("Keep Anni open. You can visit other pages while the timer runs.")
                    .size(12.0)
                    .color(TEXT_DIM),
            );
            ui.add_space(8.0);
        });
    });
}

fn setup(ui: &mut Ui, state: &mut AppState, items: &[planner::WorkItem]) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Make it your session").size(19.0).strong());
        ui.add_space(12.0);
        let locked = state.focus.in_progress();
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (is_break, label) in [(false, "Focus"), (true, "Break")] {
                    if ui
                        .selectable_label(state.focus.is_break == is_break, label)
                        .clicked()
                    {
                        state.focus.preset(if is_break { 5 } else { 25 }, is_break);
                    }
                }
            });
            ui.add_space(12.0);
            ui.label(RichText::new("DURATION").size(11.0).color(TEXT_SECONDARY));
            ui.horizontal_wrapped(|ui| {
                let presets: &[u32] = if state.focus.is_break {
                    &[5, 10, 15]
                } else {
                    &[25, 50, 90]
                };
                for &minutes in presets {
                    if ui
                        .selectable_label(state.focus.minutes == minutes, format!("{minutes} min"))
                        .clicked()
                    {
                        state.focus.preset(minutes, state.focus.is_break);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Custom").size(13.0).color(TEXT_SECONDARY));
                let mut minutes = state.focus.minutes;
                if ui
                    .add(
                        egui::DragValue::new(&mut minutes)
                            .range(1..=240)
                            .speed(1.0)
                            .suffix(" min"),
                    )
                    .on_hover_text("Type a duration from 1 to 240 minutes.")
                    .changed()
                {
                    state.focus.preset(minutes, state.focus.is_break);
                }
            });
            ui.add_space(14.0);
            ui.label(
                RichText::new("WHAT ARE YOU WORKING ON?")
                    .size(11.0)
                    .color(TEXT_SECONDARY),
            );
            egui::ComboBox::from_id_salt("focus_task")
                .width(ui.available_width())
                .truncate()
                .selected_text(if state.focus.task_title.is_empty() {
                    "Free focus"
                } else {
                    &state.focus.task_title
                })
                .show_ui(ui, |ui| {
                    ui.set_max_width(360.0);
                    if ui
                        .selectable_label(state.focus.task_key.is_empty(), "Free focus")
                        .clicked()
                    {
                        state.focus.task_key.clear();
                        state.focus.task_title.clear();
                    }
                    for item in items.iter().filter(|i| !i.done) {
                        if ui
                            .add(
                                egui::Button::new(&item.title)
                                    .selected(state.focus.task_key == item.key)
                                    .wrap(),
                            )
                            .clicked()
                        {
                            state.focus.task_key = item.key.clone();
                            state.focus.task_title = item.title.clone();
                        }
                    }
                });
        });
        if locked {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Reset the timer to change your session.")
                    .size(12.0)
                    .color(TEXT_DIM),
            );
        }
    });
}

fn completion(ui: &mut Ui, state: &mut AppState, items: &[planner::WorkItem]) {
    if !state.focus.completed {
        return;
    }
    card().fill(NOW_HIGHLIGHT).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(if state.focus.is_break {
                "Break complete. A fresh start awaits."
            } else {
                "Session complete. Time well spent."
            })
            .strong()
            .color(SUCCESS),
        );
        if !state.focus.is_break {
            ui.label("Your study minutes have been saved.");
            ui.horizontal_wrapped(|ui| {
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
            });
        }
    });
}

fn history(ui: &mut Ui, state: &AppState) {
    let today = Utc::now().date_naive();
    let sessions: Vec<_> = state
        .planner
        .focus_sessions
        .iter()
        .filter(|s| s.date == today)
        .collect();
    let minutes: u32 = sessions.iter().map(|s| s.minutes).sum();
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Today's rhythm").size(19.0).strong());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(locale::number(minutes as usize))
                    .size(36.0)
                    .strong()
                    .color(ACCENT),
            );
            ui.label(
                RichText::new("minutes focused")
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        });
        ui.label(
            RichText::new(locale::counted(
                sessions.len(),
                "completed session",
                "completed sessions",
            ))
            .size(13.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(16.0);
        ui.label(RichText::new("RECENT SESSIONS").size(11.0).color(TEXT_DIM));
        if state.planner.focus_sessions.is_empty() {
            ui.label(
                RichText::new("Start small. Your first completed session will appear here.")
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        }
        for session in state.planner.focus_sessions.iter().rev().take(4) {
            ui.separator();
            ui.add(
                egui::Label::new(
                    RichText::new(if session.task.is_empty() {
                        "Free focus"
                    } else {
                        &session.task
                    })
                    .size(14.0),
                )
                .wrap(),
            );
            ui.label(
                RichText::new(format!(
                    "{} min · {}",
                    session.minutes,
                    locale::short_date(session.date)
                ))
                .size(12.0)
                .color(TEXT_DIM),
            );
        }
    });
}
