use crate::planner::{self, WorkFilter, WorkItem, WorkKind};
use crate::state::AppState;
use crate::ui::theme::*;
use chrono::Utc;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        "My work",
        "Assignments from INNA, homework, and your personal plan.",
    );
    let focus_editor = std::mem::take(&mut state.task_editor_requested);
    let editor = card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(if state.editing_task.is_some() {
                "Edit task"
            } else {
                "Add a personal task"
            })
            .strong()
            .size(17.0),
        );
        let title = ui.add(
            egui::TextEdit::singleline(&mut state.task_title)
                .hint_text("What do you need to do?")
                .margin(egui::vec2(10.0, 8.0))
                .desired_width(f32::INFINITY),
        );
        if focus_editor {
            title.request_focus();
        }
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.task_course)
                    .hint_text("Course (optional)")
                    .margin(egui::vec2(10.0, 8.0))
                    .desired_width(180.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.task_due)
                    .hint_text("Due date: dd.mm.yyyy (optional)")
                    .margin(egui::vec2(10.0, 8.0))
                    .desired_width(260.0),
            );
            if ui
                .add(primary_button(if state.editing_task.is_some() {
                    "Save changes"
                } else {
                    "+ Add task"
                }))
                .clicked()
            {
                let result = if let Some(id) = state.editing_task {
                    state
                        .planner
                        .edit(id, &state.task_title, &state.task_course, &state.task_due)
                } else {
                    state
                        .planner
                        .add(&state.task_title, &state.task_course, &state.task_due)
                };
                match result {
                    Ok(()) => {
                        let edited_done = state
                            .editing_task
                            .and_then(|id| state.planner.tasks.iter().find(|t| t.id == id))
                            .is_some_and(|t| t.done);
                        state.task_title.clear();
                        state.task_course.clear();
                        state.task_due.clear();
                        state.task_error = None;
                        state.editing_task = None;
                        state.storage_dirty = true;
                        state.work_filter = if edited_done {
                            WorkFilter::Done
                        } else {
                            WorkFilter::Open
                        };
                        state.work_date = None;
                        state.work_search.clear();
                    }
                    Err(error) => state.task_error = Some(error),
                }
            }
            if state.editing_task.is_some() && ui.button("Cancel").clicked() {
                state.editing_task = None;
                state.task_title.clear();
                state.task_course.clear();
                state.task_due.clear();
                state.task_error = None;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Due date:").size(12.0).color(TEXT_SECONDARY));
            for (days, label) in [(0, "Today"), (1, "Tomorrow"), (7, "In a week")] {
                if ui.small_button(label).clicked() {
                    state.task_due = crate::locale::short_date(
                        Utc::now().date_naive() + chrono::Duration::days(days),
                    );
                }
            }
            if !state.task_due.is_empty() && ui.small_button("None").clicked() {
                state.task_due.clear();
            }
        });
        if let Some(error) = &state.task_error {
            ui.colored_label(WARNING, error);
        }
    });
    if focus_editor {
        editor.response.scroll_to_me(Some(egui::Align::TOP));
    }
    ui.add_space(12.0);
    undo_delete(ui, state);
    let today = Utc::now().date_naive();
    let items = planner::work_items(state);
    ui.horizontal_wrapped(|ui| {
        for (filter, label) in [
            (WorkFilter::Open, "Open"),
            (WorkFilter::Today, "Today & overdue"),
            (WorkFilter::Week, "Next 7 days"),
            (WorkFilter::Done, "Done"),
        ] {
            let count = items.iter().filter(|i| filter.matches(i, today)).count();
            if ui
                .selectable_value(
                    &mut state.work_filter,
                    filter,
                    format!("{label} ({})", crate::locale::number(count)),
                )
                .clicked()
            {
                state.work_date = None;
            }
        }
    });
    if let Some(date) = state.work_date {
        ui.horizontal(|ui| {
            badge(ui, &crate::locale::day(date), ACCENT);
            if ui.small_button("Show all days").clicked() {
                state.work_date = None;
            }
        });
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.work_search)
            .hint_text("Search tasks or courses…")
            .margin(egui::vec2(10.0, 8.0))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let search = state.work_search.trim().to_lowercase();
    let visible: Vec<_> = items
        .iter()
        .filter(|item| {
            state.work_filter.matches(item, today)
                && state
                    .work_date
                    .is_none_or(|date| item.deadline.is_some_and(|d| d.at.date() == date))
                && (search.is_empty()
                    || format!("{} {}", item.title, item.course)
                        .to_lowercase()
                        .contains(&search))
        })
        .collect();
    if visible.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new("No tasks in this view").size(20.0));
        ui.label(
            RichText::new("Choose Open, adjust your search, or add a personal task.")
                .size(15.0)
                .color(TEXT_SECONDARY),
        );
    }
    for item in visible {
        work_card(ui, state, item);
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new("Checkmarks track your local plan. Submit schoolwork in INNA.")
            .size(13.0)
            .color(TEXT_SECONDARY),
    );
    if !state.last_sync.contains_key("assignments") {
        ui.label(
            RichText::new(
                "Connect INNA and refresh to get schoolwork. You can add personal tasks right away.",
            )
            .size(14.0)
            .color(WARNING),
        );
    }
}

pub fn work_card(ui: &mut Ui, state: &mut AppState, item: &WorkItem) {
    let now = Utc::now().naive_utc();
    let overdue = item.overdue(now);
    let due_today = !item.done && item.deadline.is_some_and(|d| d.at.date() == now.date());
    card().inner_margin(14.0).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let mut done = item.done;
            let checked = ui
                .scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_fill = BG_DARK;
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        egui::Stroke::new(1.0, TEXT_SECONDARY);
                    ui.spacing_mut().icon_width = 22.0;
                    ui.add_enabled_ui(!item.source_done, |ui| {
                        ui.add_sized([36.0, 36.0], egui::Checkbox::new(&mut done, ""))
                    })
                    .inner
                })
                .inner;
            if checked.changed() {
                item.set_done(&mut state.planner, done);
                state.storage_dirty = true;
            }
            ui.vertical(|ui| {
                let title = RichText::new(&item.title).size(17.0).strong();
                ui.label(if item.done {
                    title.color(TEXT_SECONDARY).strikethrough()
                } else {
                    title
                });
                ui.horizontal_wrapped(|ui| {
                    if !item.course.is_empty() {
                        ui.label(RichText::new(&item.course).size(13.0).color(ACCENT));
                    }
                    ui.label(
                        RichText::new(item.kind.label())
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    if item.source_done {
                        ui.label(
                            RichText::new(
                                if matches!(item.kind, WorkKind::Assignment | WorkKind::Exam) {
                                    "Submitted or graded in INNA"
                                } else {
                                    "Completed in INNA"
                                },
                            )
                            .size(13.0)
                            .color(SUCCESS),
                        );
                    } else if item.done {
                        ui.label(RichText::new("Completed by you").size(13.0).color(SUCCESS));
                    } else {
                        let label = format!(
                            "{}{}",
                            if overdue { "Overdue · " } else { "" },
                            item.due_label(now.date())
                        );
                        ui.label(RichText::new(label).size(13.0).color(if overdue {
                            ERROR_RED
                        } else if due_today {
                            WARNING
                        } else {
                            TEXT_SECONDARY
                        }));
                    }
                });
                if !item.detail.is_empty() {
                    egui::CollapsingHeader::new("Details")
                        .id_salt(&item.key)
                        .show(ui, |ui| {
                            ui.label(&item.detail);
                        });
                }
                ui.horizontal_wrapped(|ui| {
                    if !item.done
                        && ui
                            .small_button("Focus on this")
                            .on_hover_text(if state.focus.in_progress() {
                                "Open your current focus session"
                            } else {
                                "Start a focus session for this task"
                            })
                            .clicked()
                    {
                        if !state.focus.in_progress() {
                            state.focus.reset();
                            state.focus.task_key = item.key.clone();
                            state.focus.task_title = item.title.clone();
                            if state.focus.is_break {
                                state.focus.preset(25, false);
                            }
                        }
                        state.navigate(crate::state::Page::Focus);
                    }
                    if item.kind == WorkKind::Personal {
                        if ui.small_button("Edit").clicked() {
                            if let Some(task) = state
                                .planner
                                .tasks
                                .iter()
                                .find(|t| format!("personal:{}", t.id) == item.key)
                            {
                                state.editing_task = Some(task.id);
                                state.task_title = task.title.clone();
                                state.task_course = task.course.clone();
                                state.task_due =
                                    task.due.map(crate::locale::short_date).unwrap_or_default();
                                state.task_error = None;
                                state.task_editor_requested = true;
                                state.navigate(crate::state::Page::Planner);
                            }
                        }
                        if ui
                            .small_button(RichText::new("Delete").color(TEXT_DIM))
                            .clicked()
                        {
                            if let Some(index) = state
                                .planner
                                .tasks
                                .iter()
                                .position(|t| format!("personal:{}", t.id) == item.key)
                            {
                                let task = state.planner.tasks.remove(index);
                                if state.editing_task == Some(task.id) {
                                    state.editing_task = None;
                                    state.task_title.clear();
                                    state.task_course.clear();
                                    state.task_due.clear();
                                }
                                state.deleted_task = Some(task);
                                state.storage_dirty = true;
                            }
                        }
                    }
                });
            });
        });
    });
}

pub fn undo_delete(ui: &mut Ui, state: &mut AppState) {
    if let Some(task) = &state.deleted_task {
        let title = task.title.clone();
        card().fill(ACCENT_DIM).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Deleted: {title}"));
                if ui.button("Undo").clicked() {
                    state.planner.tasks.push(state.deleted_task.take().unwrap());
                    state.storage_dirty = true;
                }
                if ui.small_button("Dismiss").clicked() {
                    state.deleted_task = None;
                }
            });
        });
    }
}
