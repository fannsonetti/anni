use crate::{
    grades::{self, GradeSummary, Standing},
    locale::{self, t},
    planner::{self, WorkFilter, WorkItem},
    state::{AppState, Page},
    ui::{assignments, theme::*},
};
use chrono::{Datelike, Duration, Timelike, Utc};
use egui::{vec2, Align, FontId, Layout, RichText, Sense, Stroke, Ui};

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
        0..=11 => t("Good morning", "Góðan daginn"),
        12..=17 => t("Good afternoon", "Góðan dag"),
        _ => t("Good evening", "Gott kvöld"),
    };
    ui.label(
        RichText::new(
            name.map(|n| format!("{greeting}, {n}"))
                .unwrap_or_else(|| t("Your school, at a glance", "Skólinn þinn í hnotskurn").into()),
        )
        .size(28.0)
        .family(crate::ui::theme::semibold()),
    );
    ui.add_space(3.0);
    let summary = if overdue > 0 {
        tr!(
            "{} overdue · {} due today.",
            "{} komin fram yfir · {} á að skila í dag.",
            locale::number(overdue),
            locale::number(due_today)
        )
    } else if due_today > 0 {
        tr!("{} due today.", "{} á að skila í dag.", locale::counted(due_today, ("task", "tasks"), ("verkefni", "verkefni")))
    } else {
        t("No deadlines today. Here's what's coming up.", "Engir skilafrestir í dag. Hér er það sem er fram undan.").into()
    };
    ui.label(RichText::new(summary).size(14.0).color(TEXT_SECONDARY));
    ui.add_space(20.0);
    super::timetable::teacher_absence_notices(ui, state, now.date(), now.date() + Duration::days(13));
    let course_count = assignments::modules(state).len();
    let unread = state
        .announcements
        .iter()
        .filter(|a| a.has_opened == Some(false))
        .count();
    ui.columns(3, |columns| {
        for (index, (label, count)) in [
            (t("Courses & grades", "Áfangar og einkunnir"), course_count),
            (t("Unread notices", "Ólesnar tilkynningar"), unread),
            (t("Due today", "Skil í dag"), due_today),
        ]
        .into_iter()
        .enumerate()
        {
            let response = egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .show(&mut columns[index], |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).size(12.0).color(TEXT_SECONDARY));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(count.to_string())
                                    .size(24.0)
                                    .color(TEXT_PRIMARY),
                            );
                        });
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                if index == 1 {
                    state.announcements_unread_only = true;
                    state.announcement_search.clear();
                    state.navigate(Page::Announcements);
                } else if index == 0 {
                    state.navigate(Page::Assignments);
                } else {
                    open_work(state, WorkFilter::Today);
                    state.work_date = Some(now.date());
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
                schedule(ui, state);
                ui.add_space(17.0);
                priority_queue(ui, state, &items);
                ui.add_space(17.0);
                week(ui, state, &items);
            });
            ui.allocate_ui_with_layout(vec2(right, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.x = 8.0;
                next_lesson(ui, state);
                ui.add_space(15.0);
                courses(ui, state);
                ui.add_space(15.0);
                news(ui, state);
            });
        });
    } else {
        next_lesson(ui, state);
        ui.add_space(15.0);
        schedule(ui, state);
        ui.add_space(17.0);
        courses(ui, state);
        ui.add_space(17.0);
        news(ui, state);
        ui.add_space(17.0);
        priority_queue(ui, state, &items);
        ui.add_space(17.0);
        week(ui, state, &items);
    }
}

fn section(ui: &mut Ui, title: &str, action: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .size(17.0)
                .family(crate::ui::theme::semibold()),
        );
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
    card()
        .inner_margin(egui::Margin::symmetric(18.0, 14.0))
        .outer_margin(0.0)
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
        if section(ui, t("Schoolwork & deadlines", "Skólavinna og skilafrestir"), t("View all →", "Sjá allt →")) {
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
                    (QueueView::Upcoming, t("Upcoming", "Fram undan")),
                    (QueueView::Today, t("Due today", "Skil í dag")),
                    (QueueView::Overdue, t("Overdue", "Komið fram yfir")),
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
                    QueueView::Upcoming => t("You're all caught up.", "Allt er klárt."),
                    QueueView::Today => t("Nothing due today.", "Ekkert á að skila í dag."),
                    QueueView::Overdue => t("No overdue work.", "Ekkert komið fram yfir."),
                })
                .size(20.0),
            );
            ui.label(
                RichText::new(t("Assignments, tests, and homework from INNA appear here.", "Verkefni, próf og heimavinna úr INNU birtast hér."))
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
            ui.add_space(14.0);
            if ui.button(t("Open schoolwork →", "Opna skólavinnu →")).clicked() {
                open_work(state, WorkFilter::Open);
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
                        RichText::new(t("Open all schoolwork →", "Opna alla skólavinnu →"))
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
            .rect_filled(rect.shrink2(vec2(0.0, 3.0)), 2.0, PANEL_BG);
    }
    let mut done = false;
    let check = egui::Rect::from_min_size(rect.min + vec2(0.0, 17.0), vec2(27.0, 29.0));
    if ui
        .put(check, egui::Checkbox::without_text(&mut done))
        .on_hover_text(t("Mark complete in your local plan", "Merkja sem lokið í þínu skipulagi"))
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
        t("Overdue", "Komið fram yfir").into()
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
}

fn compact_due(item: &WorkItem, today: chrono::NaiveDate) -> String {
    let Some(d) = item.deadline else {
        return if item.due_text.is_empty() {
            t("No due date", "Enginn skilafrestur").into()
        } else {
            item.due_text.clone()
        };
    };
    let days = (d.at.date() - today).num_days();
    let date = match days {
        0 => t("Today", "Í dag").into(),
        1 => t("Tomorrow", "Á morgun").into(),
        -1 => t("Yesterday", "Í gær").into(),
        _ => locale::short_day_month(d.at.date()),
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
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if live {
                    t("IN CLASS NOW", "Í TÍMA NÚNA")
                } else {
                    t("YOUR NEXT CLASS", "NÆSTI TÍMI")
                })
                .size(10.0)
                .family(crate::ui::theme::semibold())
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
                        0 => t("Today", "Í dag").into(),
                        1 => tr!("Tomorrow · {}", "Á morgun · {}", locale::weekday(s.date())),
                        _ => locale::capitalize(&locale::day(s.date())),
                    }
                })
                .unwrap_or_else(|| t("Next scheduled lesson", "Næsti skráði tími").into());
            ui.label(RichText::new(date).size(12.0).color(TEXT_SECONDARY));
            ui.label(RichText::new(lesson.time_range()).size(30.0));
            ui.add_space(11.0);
            ui.label(
                RichText::new(assignments::subject(lesson.course_code()))
                    .size(23.0)
                    .family(crate::ui::theme::semibold()),
            );
            ui.label(
                RichText::new(lesson.course_code())
                    .size(12.0)
                    .color(TEXT_SECONDARY),
            );
            ui.add_space(11.0);
            ui.horizontal_wrapped(|ui| {
                if !lesson.room().is_empty() {
                    badge(ui, &tr!("Room {}", "Stofa {}", lesson.room()), ACCENT);
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
                    egui::Button::new(RichText::new(t("Open timetable →", "Opna stundatöflu →")).size(12.0).color(ACCENT))
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
            ui.label(RichText::new(t("No upcoming classes", "Engir tímar fram undan")).size(24.0));
            ui.add_space(5.0);
            ui.label(
                RichText::new(if state.last_sync.contains_key("timetable") {
                    t("No upcoming lessons in your saved timetable.", "Engir tímar fram undan í vistuðu stundatöflunni.")
                } else {
                    t("Connect INNA to see your next class.", "Tengdu INNU til að sjá næsta tíma.")
                })
                .size(13.0)
                .color(TEXT_SECONDARY),
            );
            ui.add_space(12.0);
            if ui.button(t("Open timetable →", "Opna stundatöflu →")).clicked() {
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
            ui.label(
                RichText::new(t("The next 7 days", "Næstu 7 dagar"))
                    .size(17.0)
                    .family(crate::ui::theme::semibold()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(locale::counted(
                        counts.iter().sum(),
                        ("deadline", "deadlines"),
                        ("skilafrestur", "skilafrestir"),
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
                    2.0,
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
                        2.0,
                        Stroke::new(1.0, ACCENT.linear_multiply(0.4)),
                    );
                }
                let c = if today_tile { ACCENT } else { TEXT_SECONDARY };
                col.painter().text(
                    r.center_top() + vec2(0.0, 17.0),
                    egui::Align2::CENTER_CENTER,
                    if today_tile {
                        t("Today", "Í dag").to_string()
                    } else {
                        locale::short_weekday(date).to_string()
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
                        locale::capitalize(&locale::day(date)),
                        locale::counted(counts[i], ("deadline", "deadlines"), ("skilafrestur", "skilafrestir"))
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

fn courses(ui: &mut Ui, state: &mut AppState) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, t("Course snapshot", "Áfangar í hnotskurn"), t("All →", "Allir →")) {
            state.navigate(Page::Assignments);
        }
        ui.add_space(11.0);
        assignments::grade_view_picker(ui, &mut state.grade_view);
        let view = state.grade_view;
        let modules = assignments::modules(state);
        if modules.is_empty() {
            ui.label(
                RichText::new(t("Connect INNA to see your grades.", "Tengdu INNU til að sjá einkunnirnar."))
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
                            view.value(&summary)
                                .map(grades::average_text)
                                .unwrap_or_else(|| "—".into()),
                        )
                        .size(21.0)
                        .color(match view.standing(&summary) {
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
            RichText::new(tr!("{} · out of 10", "{} · af 10", view.label()))
                .size(10.0)
                .color(TEXT_DIM),
        );
    });
}

fn schedule(ui: &mut Ui, state: &mut AppState) {
    surface().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if section(ui, t("Today's classes", "Tímar dagsins"), t("Timetable →", "Stundatafla →")) {
            state.selected_date = Utc::now().date_naive();
            state.navigate(Page::Timetable);
        }
        ui.add_space(10.0);
        let lessons = state.today_lessons();
        if lessons.is_empty() {
            ui.label(
                RichText::new(if state.last_sync.contains_key("timetable") {
                    t("No classes scheduled today.", "Engir tímar í dag.")
                } else {
                    t("Connect INNA and refresh to see your timetable.", "Tengdu INNU og uppfærðu til að sjá stundatöfluna.")
                })
                .size(13.0)
                .color(TEXT_SECONDARY),
            );
        }
        for lesson in lessons {
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
                        RichText::new(tr!("Room {}", "Stofa {}", lesson.room()))
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
        if section(ui, t("From your school", "Frá skólanum"), t("All updates →", "Allar tilkynningar →")) {
            state.navigate(Page::Announcements);
        }
        ui.add_space(10.0);
        if state.announcements.is_empty() {
            ui.label(
                RichText::new(t("No announcements yet.", "Engar tilkynningar enn."))
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
        for (language, width) in crate::locale::Language::ALL.into_iter().flat_map(|l| [700.0, 1004.0, 1300.0].map(|w| (l, w))) {
            crate::locale::set_language(language);
            let ctx = egui::Context::default();
            crate::ui::theme::apply_theme(&ctx);
            let mut state = AppState::new();
            state.user = Some(crate::mock::mock_user());
            state.timetable = crate::mock::mock_timetable();
            state.timetable[0].teacher_absent = true;
            state.assignments = crate::mock::mock_assignments();
            state.assignments[0].assignments[0].name = Some("A long assignment title ".repeat(20));
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
