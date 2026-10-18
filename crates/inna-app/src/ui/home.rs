use crate::{
    grades::{self, GradeSummary, Standing},
    locale,
    planner::{self, WorkFilter, WorkItem},
    state::{AppState, Page},
    ui::{assignments, theme::*},
};
use chrono::{Datelike, Duration, Timelike, Utc};
use egui::{vec2, Align, Color32, FontId, Layout, RichText, Sense, Stroke, Ui};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum QueueView {
    #[default]
    Upcoming,
    Today,
    Overdue,
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let items = planner::work_items(state);
    let now = Utc::now().naive_utc();
    let due_today = items
        .iter()
        .filter(|i| !i.done && i.deadline.is_some_and(|d| d.at.date() == now.date()))
        .count();
    let overdue = items.iter().filter(|i| i.overdue(now)).count();
    ui.add_space(4.0);
    ui.label(
        RichText::new(locale::day(now.date()).to_uppercase())
            .size(11.0)
            .color(TEXT_DIM),
    );
    ui.add_space(5.0);
    let name = state
        .user
        .as_ref()
        .and_then(|u| u.display_name().split_whitespace().next());
    let greeting = match now.hour() {
        0..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };
    ui.label(
        RichText::new(
            name.map(|n| format!("{greeting}, {n}"))
                .unwrap_or_else(|| "Your day, at a glance".into()),
        )
        .size(32.0)
        .strong(),
    );
    ui.add_space(3.0);
    let summary = if overdue > 0 {
        format!(
            "{} overdue · {} due today.",
            locale::number(overdue),
            locale::number(due_today)
        )
    } else if due_today > 0 {
        format!("{} due today.", locale::counted(due_today, "task", "tasks"))
    } else {
        "No deadlines today. Here's what's coming up.".into()
    };
    ui.label(RichText::new(summary).size(14.0).color(TEXT_SECONDARY));
    ui.add_space(20.0);
    let open_count = items.iter().filter(|item| !item.done).count();
    ui.columns(3, |columns| {
        for (index, (label, count, color, hint)) in [
            ("Due today", due_today, ACCENT, "Your deadlines today"),
            ("Overdue", overdue, WARNING, "Needs your attention"),
            ("Open tasks", open_count, VIOLET, "Everything on your list"),
        ]
        .into_iter()
        .enumerate()
        {
            let response = surface()
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .show(&mut columns[index], |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).size(12.0).color(TEXT_SECONDARY));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(count.to_string()).size(26.0).color(color));
                        });
                    });
                    ui.label(RichText::new(hint).size(11.0).color(TEXT_DIM));
                })
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                if index == 1 {
                    columns[index].ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("home_queue_view"), QueueView::Overdue)
                    });
                } else if index == 0 {
                    open_work(state, WorkFilter::Today);
                    state.work_date = Some(now.date());
                } else {
                    open_work(state, WorkFilter::Open);
                }
            }
        }
    });
    ui.add_space(22.0);
    if ui.available_width() >= 840.0 {
        let left = (ui.available_width() - 22.0) * 0.62;
        let right = ui.available_width() - left - 22.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 22.0;
            ui.allocate_ui_with_layout(vec2(left, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left);
                ui.spacing_mut().item_spacing.x = 8.0;
                priority_queue(ui, state, &items);
                ui.add_space(17.0);
                week(ui, state, &items);
                if !state.today_lessons().is_empty() {
                    ui.add_space(17.0);
                    schedule(ui, state);
                }
                ui.add_space(17.0);
                news(ui, state);
            });
            ui.allocate_ui_with_layout(vec2(right, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.x = 8.0;
                next_lesson(ui, state);
                ui.add_space(15.0);
                focus(ui, state);
                ui.add_space(15.0);
                courses(ui, state);
            });
        });
    } else {
        next_lesson(ui, state);
        ui.add_space(15.0);
        priority_queue(ui, state, &items);
        ui.add_space(17.0);
        week(ui, state, &items);
        ui.add_space(17.0);
        focus(ui, state);
        if !state.today_lessons().is_empty() {
            ui.add_space(17.0);
            schedule(ui, state);
        }
        ui.add_space(17.0);
        courses(ui, state);
        ui.add_space(17.0);
        news(ui, state);
    }
}

fn section(ui: &mut Ui, title: &str, action: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(17.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            clicked = ui
                .add(
                    egui::Button::new(RichText::new(action).size(12.0).color(TEXT_SECONDARY))
                        .frame(false),
                )
                .clicked();
        });
    });
    clicked
}

fn surface() -> egui::Frame {
    card().rounding(12.0).inner_margin(20.0).outer_margin(0.0)
}

fn divider(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [r.left_top(), r.right_top()],
        Stroke::new(1.0, BORDER.linear_multiply(0.65)),
    );
}

fn priority_queue(ui: &mut Ui, state: &mut AppState, items: &[WorkItem]) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, "Up next", "View all →") {
            open_work(state, WorkFilter::Open);
        }
        ui.add_space(10.0);
        let key = egui::Id::new("home_queue_view");
        let mut view = ui
            .ctx()
            .data_mut(|d| d.get_temp::<QueueView>(key).unwrap_or_default());
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = vec2(10.0, 6.0);
            ui.horizontal_wrapped(|ui| {
                for (v, label) in [
                    (QueueView::Upcoming, "Upcoming"),
                    (QueueView::Today, "Due today"),
                    (QueueView::Overdue, "Overdue"),
                ] {
                    ui.selectable_value(&mut view, v, label);
                }
            });
        });
        ui.ctx().data_mut(|d| d.insert_temp(key, view));
        ui.add_space(10.0);
        divider(ui);
        let now = Utc::now().naive_utc();
        let visible: Vec<_> = items
            .iter()
            .filter(|i| {
                !i.done
                    && match view {
                        QueueView::Upcoming => true,
                        QueueView::Today => i.deadline.is_some_and(|d| d.at.date() == now.date()),
                        QueueView::Overdue => i.overdue(now),
                    }
            })
            .collect();
        if visible.is_empty() {
            ui.add_space(23.0);
            ui.label(
                RichText::new(match view {
                    QueueView::Upcoming => "You're all caught up.",
                    QueueView::Today => "Nothing due today.",
                    QueueView::Overdue => "No overdue work.",
                })
                .size(20.0),
            );
            ui.label(
                RichText::new("Add a personal task or look ahead to your next deadline.")
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
            ui.add_space(14.0);
            if ui.add(primary_button("+ Add a task")).clicked() {
                state.new_task();
            }
            ui.add_space(9.0);
        }
        for (index, item) in visible.iter().take(4).enumerate() {
            if index > 0 {
                divider(ui);
            }
            task_row(ui, state, item);
        }
        if visible.len() > 4 {
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Open full task list →")
                            .size(12.0)
                            .color(ACCENT),
                    )
                    .frame(false),
                )
                .clicked()
            {
                open_work(state, WorkFilter::Open);
            }
        }
    });
}

fn task_row(ui: &mut Ui, state: &mut AppState, item: &WorkItem) {
    let now = Utc::now().naive_utc();
    let overdue = item.overdue(now);
    let due_today = item.deadline.is_some_and(|d| d.at.date() == now.date());
    let width = ui.available_width();
    let due_width = if width > 430.0 { 126.0 } else { 104.0 };
    let title_width = (width - due_width - 47.0).max(80.0);
    let mut job = egui::text::LayoutJob::simple(
        item.title.clone(),
        FontId::proportional(15.0),
        TEXT_PRIMARY,
        title_width,
    );
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let title = ui.painter().layout_job(job);
    let title_height = title.size().y;
    let height = (title_height + 43.0).max(73.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(vec2(0.0, 3.0)), 8.0, PANEL_BG);
    }
    let mut done = false;
    let check = egui::Rect::from_min_size(rect.min + vec2(0.0, 17.0), vec2(27.0, 29.0));
    if ui
        .put(check, egui::Checkbox::without_text(&mut done))
        .on_hover_text("Mark complete in your local plan")
        .changed()
    {
        item.set_done(&mut state.planner, done);
        state.storage_dirty = true;
    }
    let title_at = rect.min + vec2(39.0, 17.0);
    ui.painter().galley(title_at, title, TEXT_PRIMARY);
    let subtitle = if item.course.is_empty() {
        item.kind.label().to_string()
    } else {
        format!("{}  ·  {}", item.course, item.kind.label())
    };
    ui.painter()
        .with_clip_rect(egui::Rect::from_min_size(
            title_at + vec2(0.0, title_height + 5.0),
            vec2(title_width, 15.0),
        ))
        .text(
            title_at + vec2(0.0, title_height + 5.0),
            egui::Align2::LEFT_TOP,
            subtitle,
            FontId::proportional(10.0),
            TEXT_DIM,
        );
    let due = if overdue {
        "Overdue".into()
    } else {
        compact_due(item, now.date())
    };
    ui.painter().text(
        rect.right_top() + vec2(-1.0, 18.0),
        egui::Align2::RIGHT_TOP,
        due,
        FontId::proportional(12.0),
        if overdue {
            WARNING
        } else if due_today {
            ACCENT
        } else {
            TEXT_SECONDARY
        },
    );
    if overdue {
        ui.painter().text(
            rect.right_top() + vec2(-1.0, 34.0),
            egui::Align2::RIGHT_TOP,
            compact_due(item, now.date()),
            FontId::proportional(9.0),
            TEXT_DIM,
        );
    }
    let focus_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 65.0, rect.bottom() - 31.0),
        vec2(65.0, 26.0),
    );
    if ui
        .put(
            focus_rect,
            egui::Button::new(RichText::new("Focus →").size(11.0).color(TEXT_SECONDARY))
                .frame(false),
        )
        .clicked()
    {
        if !state.focus.in_progress() {
            state.focus.reset();
            state.focus.preset(25, false);
            state.focus.task_key = item.key.clone();
            state.focus.task_title = item.title.clone();
        }
        state.navigate(Page::Focus);
    }
}

fn compact_due(item: &WorkItem, today: chrono::NaiveDate) -> String {
    let Some(d) = item.deadline else {
        return if item.due_text.is_empty() {
            "No due date".into()
        } else {
            item.due_text.clone()
        };
    };
    let days = (d.at.date() - today).num_days();
    let date = match days {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        _ => d.at.format("%d %b").to_string(),
    };
    if d.has_time {
        format!("{date} · {}", d.at.format("%H:%M"))
    } else {
        date
    }
}

fn next_lesson(ui: &mut Ui, state: &mut AppState) {
    let now = Utc::now().naive_utc();
    let (current, next) = state.current_and_next_lesson();
    let lesson = current.or(next).cloned();
    let live = current.is_some();
    surface()
        .fill(Color32::from_rgb(27, 43, 45))
        .stroke(Stroke::new(1.0, Color32::from_rgb(49, 76, 74)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.0, ACCENT);
                ui.label(
                    RichText::new(if live {
                        "IN CLASS NOW"
                    } else {
                        "YOUR NEXT CLASS"
                    })
                    .size(10.0)
                    .strong()
                    .color(ACCENT),
                );
            });
            ui.add_space(13.0);
            if let Some(lesson) = lesson {
                let start = lesson
                    .start
                    .as_deref()
                    .and_then(planner::parse_deadline)
                    .map(|d| d.at);
                let date = start
                    .map(|s| {
                        let days = (s.date() - now.date()).num_days();
                        match days {
                            0 => "Today".into(),
                            1 => format!("Tomorrow · {}", locale::weekday(s.date())),
                            _ => locale::day(s.date()),
                        }
                    })
                    .unwrap_or_else(|| "Next scheduled lesson".into());
                ui.label(RichText::new(date).size(12.0).color(TEXT_SECONDARY));
                ui.label(RichText::new(lesson.time_range()).size(30.0));
                ui.add_space(11.0);
                ui.label(
                    RichText::new(assignments::subject(lesson.course_code()))
                        .size(23.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(lesson.course_code())
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(11.0);
                ui.horizontal_wrapped(|ui| {
                    if !lesson.room().is_empty() {
                        badge(ui, &format!("Room {}", lesson.room()), ACCENT);
                    }
                    if !lesson.teacher_str().is_empty() {
                        ui.label(
                            RichText::new(lesson.teacher_str())
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                });
                ui.add_space(13.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Open timetable →").size(12.0).color(ACCENT),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    if let Some(start) = start {
                        state.selected_date = start.date();
                    }
                    state.navigate(Page::Timetable);
                }
            } else {
                ui.label(RichText::new("No upcoming classes").size(24.0));
                ui.add_space(5.0);
                ui.label(
                    RichText::new(if state.last_sync.contains_key("timetable") {
                        "No upcoming lessons in your saved timetable."
                    } else {
                        "Connect INNA to see your next class."
                    })
                    .size(13.0)
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(12.0);
                if ui.button("Open timetable →").clicked() {
                    state.navigate(Page::Timetable);
                }
            }
        });
}

fn week(ui: &mut Ui, state: &mut AppState, items: &[WorkItem]) {
    let today = Utc::now().date_naive();
    let counts: Vec<_> = (0..7)
        .map(|i| {
            let date = today + Duration::days(i);
            items
                .iter()
                .filter(|item| !item.done && item.deadline.is_some_and(|d| d.at.date() == date))
                .count()
        })
        .collect();
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("The next 7 days").size(17.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(locale::counted(
                        counts.iter().sum(),
                        "deadline",
                        "deadlines",
                    ))
                    .size(11.0)
                    .color(TEXT_DIM),
                );
            });
        });
        ui.add_space(16.0);
        ui.columns(7, |columns| {
            for (i, col) in columns.iter_mut().enumerate() {
                let date = today + Duration::days(i as i64);
                let (r, response) =
                    col.allocate_exact_size(vec2(col.available_width(), 85.0), Sense::click());
                let today_tile = i == 0;
                col.painter().rect_filled(
                    r,
                    10.0,
                    if today_tile {
                        ACCENT_DIM
                    } else if response.hovered() {
                        PANEL_BG
                    } else {
                        CARD_BG
                    },
                );
                if today_tile {
                    col.painter().rect_stroke(
                        r,
                        10.0,
                        Stroke::new(1.0, ACCENT.linear_multiply(0.4)),
                    );
                }
                let c = if today_tile { ACCENT } else { TEXT_SECONDARY };
                col.painter().text(
                    r.center_top() + vec2(0.0, 17.0),
                    egui::Align2::CENTER_CENTER,
                    if today_tile {
                        "Today".to_string()
                    } else {
                        date.format("%a").to_string()
                    },
                    FontId::proportional(10.0),
                    c,
                );
                col.painter().text(
                    r.center_top() + vec2(0.0, 42.0),
                    egui::Align2::CENTER_CENTER,
                    date.day().to_string(),
                    FontId::proportional(21.0),
                    if today_tile { ACCENT } else { TEXT_PRIMARY },
                );
                if counts[i] > 0 {
                    col.painter().circle_filled(
                        r.center_top() + vec2(0.0, 67.0),
                        3.0,
                        if today_tile { ACCENT } else { VIOLET },
                    );
                } else {
                    col.painter().line_segment(
                        [
                            r.center_top() + vec2(-3.0, 67.0),
                            r.center_top() + vec2(3.0, 67.0),
                        ],
                        Stroke::new(1.0, BORDER),
                    );
                }
                if response
                    .on_hover_text(format!(
                        "{} · {}",
                        locale::day(date),
                        locale::counted(counts[i], "deadline", "deadlines")
                    ))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open_work(state, WorkFilter::Week);
                    state.work_date = Some(date);
                }
            }
        });
    });
}

fn focus(ui: &mut Ui, state: &mut AppState) {
    let today = Utc::now().date_naive();
    let sessions: Vec<_> = state
        .planner
        .focus_sessions
        .iter()
        .filter(|s| s.date == today)
        .collect();
    let minutes: u32 = sessions.iter().map(|s| s.minutes).sum();
    let session_count = sessions.len();
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Focus time").size(17.0).strong());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(minutes.to_string()).size(32.0));
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("minutes today")
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(locale::counted(
                        session_count,
                        "completed session",
                        "completed sessions",
                    ))
                    .size(11.0)
                    .color(TEXT_DIM),
                );
            });
        });
        ui.add_space(10.0);
        let label = if state.focus.in_progress() {
            format!(
                "{} · Return to session",
                state.focus.clock(std::time::Instant::now())
            )
        } else {
            "Start a focus session →".into()
        };
        if ui
            .add_sized([ui.available_width(), 36.0], primary_button(&label))
            .clicked()
        {
            state.navigate(Page::Focus);
        }
    });
}

fn courses(ui: &mut Ui, state: &mut AppState) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, "Course snapshot", "All →") {
            state.navigate(Page::Assignments);
        }
        ui.add_space(11.0);
        let modules = assignments::modules(state);
        if modules.is_empty() {
            ui.label(
                RichText::new("Connect INNA to see your grades.")
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        }
        for (i, module) in modules.iter().take(3).enumerate() {
            if i > 0 {
                ui.add_space(9.0);
                divider(ui);
                ui.add_space(9.0);
            }
            let summary = GradeSummary::for_module(module);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(assignments::subject(module.module_str())).size(13.0));
                    ui.label(
                        RichText::new(module.module_str())
                            .size(10.0)
                            .color(TEXT_DIM),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(
                            summary
                                .average
                                .map(grades::average_text)
                                .unwrap_or_else(|| "—".into()),
                        )
                        .size(21.0)
                        .color(match summary.standing() {
                            Standing::Above => ACCENT,
                            Standing::Below => WARNING,
                            Standing::Unknown => TEXT_SECONDARY,
                        }),
                    );
                });
            });
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new("Weighted averages · out of 10")
                .size(10.0)
                .color(TEXT_DIM),
        );
    });
}

fn schedule(ui: &mut Ui, state: &mut AppState) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, "Today's classes", "Timetable →") {
            state.selected_date = Utc::now().date_naive();
            state.navigate(Page::Timetable);
        }
        ui.add_space(10.0);
        for lesson in state.today_lessons() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lesson.start_clock.as_deref().unwrap_or("—"))
                        .size(13.0)
                        .color(ACCENT),
                );
                ui.add_space(9.0);
                ui.label(RichText::new(assignments::subject(lesson.course_code())).size(13.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("Room {}", lesson.room()))
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                });
            });
            ui.add_space(7.0);
        }
    });
}

fn news(ui: &mut Ui, state: &mut AppState) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, "From your school", "All updates →") {
            state.navigate(Page::Announcements);
        }
        ui.add_space(10.0);
        if state.announcements.is_empty() {
            ui.label(
                RichText::new("No announcements yet.")
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        }
        let mut selected = None;
        for (index, ann) in state.announcements.iter().take(2).enumerate() {
            if index > 0 {
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
            }
            ui.horizontal_top(|ui| {
                if ann.has_opened == Some(false) {
                    let (r, _) = ui.allocate_exact_size(vec2(7.0, 19.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 2.5, ACCENT);
                }
                ui.vertical(|ui| {
                    if ui
                        .add(
                            egui::Label::new(RichText::new(ann.title_str()).size(13.0))
                                .wrap()
                                .sense(Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        selected = Some(index);
                    }
                    ui.label(
                        RichText::new(format!("{} · {}", ann.module_str(), ann.sender_str()))
                            .size(10.0)
                            .color(TEXT_DIM),
                    );
                });
            });
        }
        if let Some(index) = selected {
            state.open_announcement(index);
        }
    });
}

fn open_work(state: &mut AppState, filter: WorkFilter) {
    state.navigate(Page::Planner);
    state.work_filter = filter;
    state.work_search.clear();
    state.work_date = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_keeps_long_titles_within_narrow_and_wide_windows() {
        for width in [700.0, 1004.0, 1300.0] {
            let ctx = egui::Context::default();
            crate::ui::theme::apply_theme(&ctx);
            let mut state = AppState::new();
            state.user = Some(crate::mock::mock_user());
            state.timetable = crate::mock::mock_timetable();
            state.assignments = crate::mock::mock_assignments();
            state
                .planner
                .add(&"A long assignment title ".repeat(20), "English", "")
                .unwrap();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(width, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let right = ui.max_rect().right();
                        show(ui, &mut state);
                        assert!(
                            ui.min_rect().right() <= right + 1.0,
                            "Dashboard overflow at width {width}: {} > {right}",
                            ui.min_rect().right()
                        );
                    });
                },
            );
        }
    }
}
