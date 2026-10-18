//! Borderless desktop schedule, clock, and homework sharing the live planner.
use crate::{
    desktop::DesktopAction,
    desktop_platform as platform, locale,
    planner::{self, WorkItem, WorkKind},
    settings::{ClockStyle, DesktopSettings, DesktopSide},
    state::AppState,
};
use chrono::{NaiveDate, NaiveDateTime, Timelike, Utc};
use egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke,
    UiBuilder, ViewportBuilder, ViewportId,
};
use inna_core::models::TimetableEntry;
use std::f32::consts::{FRAC_PI_2, TAU};

fn tint(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub struct DesktopLayout {
    pub work: Rect,
    pub schedule: Rect,
    pub clock: Rect,
}

pub fn layout(area: Rect, settings: &DesktopSettings) -> DesktopLayout {
    let margin = (area.width() * 0.035).clamp(20.0, 64.0);
    let size = vec2(
        (settings.panel_width as f32).min(((area.width() - margin * 2.0 - 24.0) / 2.0).max(160.0)),
        (settings.panel_height as f32).min(area.height() - 40.0),
    );
    let mut left = Rect::from_min_size(
        pos2(
            area.left() + margin,
            area.top()
                + 20.0
                + (area.height() - size.y - 40.0).max(0.0) * settings.vertical_position as f32
                    / 100.0,
        ),
        size,
    );
    let mut right = Rect::from_min_size(pos2(area.right() - margin - size.x, left.top()), size);
    let requested_clock = vec2(384.0, 340.0) * settings.clock_scale as f32 / 100.0;
    let fit = ((area.width() - 40.0) / requested_clock.x)
        .min((area.height() - 40.0) / requested_clock.y)
        .min(1.0);
    let clock_size = requested_clock * fit;
    let clock_y = (area.top() + area.height() * 0.43).clamp(
        area.top() + 20.0 + clock_size.y * 0.5,
        area.bottom() - 20.0 - clock_size.y * 0.5,
    );
    let mut clock = Rect::from_center_size(pos2(area.center().x, clock_y), clock_size);
    if settings.show_clock
        && ((settings.show_homework || settings.show_schedule)
            && (clock.expand(20.0).intersects(left) || clock.expand(20.0).intersects(right)))
    {
        let height = (area.height() * 0.32).clamp(160.0, 260.0);
        let width = (384.0 * settings.clock_scale as f32 / 100.0).min(area.width() - 40.0);
        clock = Rect::from_center_size(
            pos2(area.center().x, area.top() + 12.0 + height * 0.5),
            vec2(width, height),
        );
        let top = clock.bottom() + 24.0;
        let height = size.y.min(area.bottom() - top - 20.0);
        left = Rect::from_min_size(pos2(left.left(), top), vec2(size.x, height));
        right = Rect::from_min_size(pos2(right.left(), top), vec2(size.x, height));
    }
    let (work, schedule) = match settings.side {
        DesktopSide::Right => (right, left),
        DesktopSide::Left => (left, right),
    };
    DesktopLayout {
        work,
        schedule,
        clock,
    }
}

fn viewport(title: &str, rect: Rect) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title(title)
        .with_decorations(false)
        .with_close_button(false)
        .with_minimize_button(false)
        .with_maximize_button(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_visible(true)
        .with_window_level(egui::WindowLevel::AlwaysOnBottom)
        .with_position(rect.min)
        .with_inner_size(rect.size())
}

pub fn show(ctx: &egui::Context, shared: std::sync::Arc<std::sync::Mutex<crate::app::AppData>>) {
    let settings = shared.lock().unwrap().desktop.settings.clone();
    if !settings.enabled || shared.lock().unwrap().desktop.quit_requested {
        return;
    }
    let geometry = layout(platform::work_area(), &settings);
    let work_shared = shared.clone();
    if settings.show_homework {
        ctx.show_viewport_deferred(
            ViewportId::from_hash_of("desktop_work"),
            viewport(platform::WORK_TITLE, geometry.work),
            move |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    return;
                }
                let mut app = work_shared.lock().unwrap();
                app.service(ctx);
                if !app.desktop.settings.enabled || app.desktop.quit_requested {
                    return;
                }
                let crate::app::AppData { state, desktop, .. } = &mut *app;
                let action = work_content(ctx, state, &mut desktop.settings);
                desktop.handle(action, ctx, state);
                app.finish_update(ctx);
                ctx.request_repaint_after(std::time::Duration::from_secs(15));
            },
        );
    }
    let schedule_shared = shared.clone();
    if settings.show_schedule {
        ctx.show_viewport_deferred(
            ViewportId::from_hash_of("desktop_schedule"),
            viewport(platform::SCHEDULE_TITLE, geometry.schedule),
            move |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    return;
                }
                let mut app = schedule_shared.lock().unwrap();
                app.service(ctx);
                if !app.desktop.settings.enabled || app.desktop.quit_requested {
                    return;
                }
                let crate::app::AppData { state, desktop, .. } = &mut *app;
                let action = schedule_content(ctx, state, &mut desktop.settings);
                desktop.handle(action, ctx, state);
                app.finish_update(ctx);
                ctx.request_repaint_after(std::time::Duration::from_secs(15));
            },
        );
    }
    if settings.show_clock {
        ctx.show_viewport_deferred(
            ViewportId::from_hash_of("desktop_clock"),
            viewport(platform::CLOCK_TITLE, geometry.clock).with_mouse_passthrough(true),
            move |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    return;
                }
                let mut app = shared.lock().unwrap();
                app.service(ctx);
                if !app.desktop.settings.enabled || app.desktop.quit_requested {
                    return;
                }
                let settings = app.desktop.settings.clone();
                let geometry = layout(platform::work_area(), &settings);
                position(ctx, geometry.clock);
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
                        paint_panel(ui.painter(), ui.max_rect().shrink(8.0), &settings);
                        paint_clock(ui.painter(), ui.max_rect(), Utc::now(), &settings)
                    });
                app.finish_update(ctx);
                let now = Utc::now();
                let delay = if settings.show_seconds {
                    1000 - now.timestamp_subsec_millis() as u64
                } else {
                    (60 - now.second() as u64) * 1000 - now.timestamp_subsec_millis() as u64
                };
                // A clock-only desktop still services focus, sync, and saved data.
                ctx.request_repaint_after(std::time::Duration::from_millis(delay.min(15_000)));
            },
        );
    }
}

fn position(ctx: &egui::Context, rect: Rect) {
    // Sending any viewport command requests another frame. Only update geometry
    // when it actually changes, otherwise this becomes an unbounded paint loop.
    let (position, size) = ctx.input(|i| {
        (
            i.viewport().outer_rect.map(|r| r.min),
            i.viewport().inner_rect.map(|r| r.size()),
        )
    });
    if position.is_none_or(|p| p.distance(rect.min) > 1.0) {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(rect.min));
    }
    if size.is_none_or(|s| (s - rect.size()).length() > 1.0) {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(rect.size()));
    }
}

fn panel_style(ui: &mut egui::Ui, settings: &DesktopSettings) {
    ui.style_mut().interaction.selectable_labels = false;
    let palette = settings.theme.palette();
    let accent = settings.accent_color();
    ui.visuals_mut().override_text_color = Some(palette.text);
    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
    ui.visuals_mut().widgets.hovered.weak_bg_fill = tint(accent, 16);
    ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::NONE;
    ui.visuals_mut().widgets.active.weak_bg_fill = tint(accent, 25);
    ui.visuals_mut().selection.bg_fill = tint(accent, 30);
    ui.visuals_mut().selection.stroke = Stroke::new(1.0, accent);
    ui.visuals_mut().window_fill = palette.background;
    ui.spacing_mut().item_spacing = vec2(10.0, 5.0);
    ui.spacing_mut().button_padding = vec2(4.0, 3.0);
    ui.spacing_mut().interact_size = vec2(28.0, 28.0);
}

fn panel_content(ui: &mut egui::Ui, settings: &DesktopSettings) -> Rect {
    panel_style(ui, settings);
    let panel = ui.max_rect().shrink(8.0);
    paint_panel(ui.painter(), panel, settings);
    panel.shrink2(vec2(21.0, 16.0))
}

fn heading(ui: &mut egui::Ui, rect: Rect, title: &str, subtitle: &str, settings: &DesktopSettings) {
    let scale = settings.text_scale as f32 / 100.0;
    let palette = settings.theme.palette();
    ui.painter().text(
        rect.min,
        Align2::LEFT_TOP,
        title,
        FontId::proportional(27.0 * scale),
        palette.text,
    );
    ui.painter().text(
        rect.min + vec2(0.0, 44.0),
        Align2::LEFT_TOP,
        subtitle,
        FontId::proportional(12.0 * scale),
        palette.muted,
    );
}

fn work_content(
    ctx: &egui::Context,
    state: &AppState,
    settings: &DesktopSettings,
) -> DesktopAction {
    let mut action = DesktopAction::None;
    position(ctx, layout(platform::work_area(), settings).work);
    if ctx.input(|i| i.viewport().close_requested()) {
        action = DesktopAction::Disable;
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(0.0))
        .show(ctx, |ui| {
            let content = panel_content(ui, settings);
            let items = homework_items(state, settings);
            let summary = if items.is_empty() {
                "All caught up".into()
            } else {
                locale::counted(items.len(), "task", "tasks")
            };
            heading(ui, content, "Homework", &summary, settings);
            let body = Rect::from_min_max(content.min + vec2(0.0, 77.0), content.max);
            ui.allocate_new_ui(UiBuilder::new().max_rect(body), |ui| {
                ui.set_clip_rect(body.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical()
                    .id_salt("desktop_work_list")
                    .drag_to_scroll(false)
                    .auto_shrink([false, false])
                    .max_height(body.height())
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.label(
                                RichText::new("No unfinished tasks.")
                                    .size(14.0)
                                    .color(settings.theme.palette().muted),
                            );
                        }
                        let now = Utc::now().naive_utc();
                        for item in &items {
                            work_row(ui, item, now, settings);
                        }
                    });
            });
        });
    action
}

pub fn open_work(state: &AppState) -> Vec<WorkItem> {
    planner::work_items(state)
        .into_iter()
        .filter(|item| !item.done)
        .collect()
}

fn homework_items(state: &AppState, settings: &DesktopSettings) -> Vec<WorkItem> {
    open_work(state)
        .into_iter()
        .filter(|item| match item.kind {
            WorkKind::Assignment | WorkKind::Homework => true,
            WorkKind::Exam => settings.include_exams,
            WorkKind::Personal | WorkKind::InnaTask => settings.include_reminders,
        })
        .collect()
}

fn work_row(ui: &mut egui::Ui, item: &WorkItem, now: NaiveDateTime, settings: &DesktopSettings) {
    let scale = settings.text_scale as f32 / 100.0;
    let palette = settings.theme.palette();
    let overdue = item.overdue(now);
    let due_today = item.deadline.is_some_and(|d| d.at.date() == now.date());
    let priority = if overdue || due_today {
        palette.urgent
    } else {
        settings.accent_color()
    };
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(0.0, settings.row_spacing as f32))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(23.0, 28.0), Sense::hover());
                let center = pos2(rect.center().x, rect.top() + 10.0);
                ui.painter().circle_stroke(
                    center,
                    6.0,
                    Stroke::new(1.0, priority.linear_multiply(0.6)),
                );
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width().max(40.0));
                    ui.add(
                        egui::Label::new(
                            RichText::new(&item.title)
                                .size(15.0 * scale)
                                .color(palette.text),
                        )
                        .wrap(),
                    );
                    let mut details = Vec::new();
                    if settings.show_course_codes && !item.course.is_empty() {
                        details.push(item.course.clone());
                    }
                    if settings.show_deadlines
                        && (item.deadline.is_some() || !item.due_text.is_empty())
                    {
                        details.push(format!(
                            "{}{}",
                            if overdue { "Overdue · " } else { "" },
                            item.due_label(now.date())
                        ));
                    }
                    if !details.is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(details.join(" · ")).size(11.0 * scale).color(
                                    if overdue || due_today {
                                        palette.urgent
                                    } else {
                                        palette.muted
                                    },
                                ),
                            )
                            .wrap(),
                        );
                    }
                });
            });
        });
}

/// Select the current school day while lessons remain, then the next cached day.
/// Invalid dates are ignored; a lesson with no end time still appears before it starts.
fn desktop_lessons(state: &AppState, now: NaiveDateTime) -> (NaiveDate, Vec<&TimetableEntry>) {
    let mut dated: Vec<_> = state
        .timetable
        .iter()
        .filter_map(|entry| {
            let start = planner::parse_deadline(entry.start.as_deref()?)?.at;
            let end = entry
                .end
                .as_deref()
                .and_then(planner::parse_deadline)
                .map(|d| d.at);
            Some((entry, start, end))
        })
        .collect();
    dated.sort_by_key(|(_, start, _)| *start);
    let date = dated
        .iter()
        .find(|(_, start, end)| end.map_or(*start >= now, |end| end > now))
        .map(|(_, start, _)| start.date())
        .unwrap_or(now.date());
    let lessons = dated
        .into_iter()
        .filter(|(_, start, _)| start.date() == date)
        .map(|(entry, _, _)| entry)
        .collect();
    (date, lessons)
}

fn schedule_content(
    ctx: &egui::Context,
    state: &AppState,
    settings: &DesktopSettings,
) -> DesktopAction {
    let mut action = DesktopAction::None;
    position(ctx, layout(platform::work_area(), settings).schedule);
    if ctx.input(|i| i.viewport().close_requested()) {
        action = DesktopAction::Disable;
    }
    let now = Utc::now().naive_utc();
    let (date, lessons) = desktop_lessons(state, now);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(0.0))
        .show(ctx, |ui| {
            let scale = settings.text_scale as f32 / 100.0;
            let palette = settings.theme.palette();
            let content = panel_content(ui, settings);
            heading(ui, content, "Schedule", &locale::day(date), settings);
            let body = Rect::from_min_max(content.min + vec2(0.0, 77.0), content.max);
            ui.allocate_new_ui(UiBuilder::new().max_rect(body), |ui| {
                ui.set_clip_rect(body.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical()
                    .id_salt("desktop_schedule_list")
                    .drag_to_scroll(false)
                    .auto_shrink([false, false])
                    .max_height(body.height())
                    .show(ui, |ui| {
                        if lessons.is_empty() {
                            ui.label(
                                RichText::new("No upcoming classes.")
                                    .size(14.0)
                                    .color(palette.muted),
                            );
                        }
                        let mut next_shown = false;
                        for lesson in &lessons {
                            let start = lesson
                                .start
                                .as_deref()
                                .and_then(planner::parse_deadline)
                                .map(|d| d.at);
                            let end = lesson
                                .end
                                .as_deref()
                                .and_then(planner::parse_deadline)
                                .map(|d| d.at);
                            let current = start.zip(end).is_some_and(|(s, e)| s <= now && now < e);
                            let past = end.is_some_and(|end| end <= now);
                            let next = !next_shown && start.is_some_and(|start| start > now);
                            if next {
                                next_shown = true;
                            }
                            egui::Frame::none()
                                .inner_margin(egui::Margin::symmetric(
                                    0.0,
                                    settings.row_spacing as f32,
                                ))
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.allocate_ui_with_layout(
                                            vec2(56.0 * scale, 40.0 * scale),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                let start_label = start
                                                    .map(|s| s.format("%H:%M").to_string())
                                                    .unwrap_or_else(|| "—".into());
                                                ui.label(
                                                    RichText::new(start_label)
                                                        .monospace()
                                                        .size(14.0 * scale)
                                                        .color(if past {
                                                            palette.muted
                                                        } else {
                                                            settings.accent_color()
                                                        }),
                                                );
                                                if let Some(end) = end {
                                                    ui.label(
                                                        RichText::new(
                                                            end.format("%H:%M").to_string(),
                                                        )
                                                        .monospace()
                                                        .size(10.0 * scale)
                                                        .color(palette.muted),
                                                    );
                                                }
                                            },
                                        );
                                        ui.vertical(|ui| {
                                            ui.set_width(ui.available_width().max(40.0));
                                            let subject = crate::ui::assignments::subject(
                                                lesson.course_code(),
                                            );
                                            let title = if subject == "Course" {
                                                lesson.course_code()
                                            } else {
                                                subject
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(title).size(15.0 * scale).color(
                                                        if current {
                                                            settings.accent_color()
                                                        } else if past {
                                                            palette.muted
                                                        } else {
                                                            palette.text
                                                        },
                                                    ),
                                                )
                                                .wrap(),
                                            );
                                            let mut details = Vec::new();
                                            if !lesson.room().is_empty() {
                                                details.push(format!("Room {}", lesson.room()));
                                            }
                                            if current {
                                                details.push("Now".into());
                                            } else if next {
                                                details.push("Next".into());
                                            }
                                            if !details.is_empty() {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(details.join(" · "))
                                                            .size(11.0 * scale)
                                                            .color(palette.muted),
                                                    )
                                                    .wrap(),
                                                );
                                            }
                                        });
                                    });
                                });
                        }
                    });
            });
        });
    action
}

fn paint_panel(p: &Painter, rect: Rect, settings: &DesktopSettings) {
    if settings.panel_opacity > 0 {
        p.rect_filled(
            rect,
            settings.corner_radius as f32,
            tint(
                settings.theme.palette().background,
                (settings.panel_opacity.min(80) as f32 * 2.55).round() as u8,
            ),
        );
    }
    if settings.show_borders {
        p.rect_stroke(
            rect,
            settings.corner_radius as f32,
            Stroke::new(1.0, tint(settings.accent_color(), 90)),
        );
    }
}

fn arc(p: &Painter, center: Pos2, radius: f32, start: f32, sweep: f32, stroke: Stroke) {
    let segments = ((sweep.abs() * radius / 4.0).ceil() as usize).max(2);
    p.add(Shape::line(
        (0..=segments)
            .map(|i| {
                let angle = start + sweep * i as f32 / segments as f32;
                center + vec2(angle.cos(), angle.sin()) * radius
            })
            .collect(),
        stroke,
    ));
}

fn paint_clock(p: &Painter, rect: Rect, now: chrono::DateTime<Utc>, settings: &DesktopSettings) {
    let scale = (rect.width() / 384.0).min(rect.height() / 340.0);
    let center = rect.center();
    let radius = 130.0 * scale;
    let accent = settings.accent_color();
    let palette = settings.theme.palette();
    if settings.clock_style == ClockStyle::Reactor {
        // A single fine broken ring keeps the reactor cue without HUD clutter.
        for start in [0.04, 0.373, 0.706] {
            arc(
                p,
                center,
                radius,
                start * TAU - FRAC_PI_2,
                0.25 * TAU,
                Stroke::new(scale, tint(accent, 140)),
            );
        }
    }
    if settings.clock_style == ClockStyle::Analog {
        for i in 0..12 {
            let angle = i as f32 / 12.0 * TAU - FRAC_PI_2;
            let direction = vec2(angle.cos(), angle.sin());
            p.line_segment(
                [
                    center + direction * (radius - if i % 3 == 0 { 10.0 } else { 5.0 } * scale),
                    center + direction * radius,
                ],
                Stroke::new(scale, tint(accent, if i % 3 == 0 { 160 } else { 70 })),
            );
        }
        let hand = |turn: f32, length: f32, width: f32, color: Color32| {
            let angle = turn * TAU - FRAC_PI_2;
            p.line_segment(
                [
                    center,
                    center + vec2(angle.cos(), angle.sin()) * length * scale,
                ],
                Stroke::new(width * scale, color),
            );
        };
        hand(
            (now.hour() % 12) as f32 / 12.0 + now.minute() as f32 / 720.0,
            65.0,
            3.0,
            palette.text,
        );
        hand(
            now.minute() as f32 / 60.0 + now.second() as f32 / 3600.0,
            100.0,
            2.0,
            palette.text,
        );
        if settings.show_seconds {
            hand(now.second() as f32 / 60.0, 108.0, 1.0, accent);
        }
        p.circle_filled(center, 3.0 * scale, accent);
    }
    if settings.clock_style != ClockStyle::Analog {
        p.text(
            center
                - vec2(
                    0.0,
                    if settings.show_date {
                        18.0 * scale
                    } else {
                        0.0
                    },
                ),
            Align2::CENTER_CENTER,
            now.format(if settings.show_seconds {
                "%H:%M:%S"
            } else {
                "%H:%M"
            }),
            FontId::new(
                (if settings.show_seconds { 53.0 } else { 76.0 }) * scale,
                egui::FontFamily::Name("desktop_clock".into()),
            ),
            palette.text,
        );
    }
    if settings.show_date {
        p.text(
            center
                + vec2(
                    0.0,
                    if settings.clock_style == ClockStyle::Analog {
                        157.0
                    } else {
                        43.0
                    } * scale,
                ),
            Align2::CENTER_CENTER,
            locale::day(now.date_naive()),
            FontId::proportional(13.0 * scale),
            accent.gamma_multiply(0.8),
        );
    }
}

pub fn preview(ui: &mut egui::Ui, settings: &DesktopSettings) {
    let palette = settings.theme.palette();
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 200.0), Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 12.0, Color32::from_rgb(20, 26, 31));
    p.add(Shape::convex_polygon(
        vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom() - vec2(100.0, 0.0),
            rect.left_bottom() + vec2(10.0, 0.0),
        ],
        Color32::from_rgb(30, 37, 43),
        Stroke::NONE,
    ));
    // Scale the actual desktop geometry so the preview responds to every layout setting.
    let area = platform::work_area();
    let geometry = layout(area, settings);
    let factor = (rect.width() / area.width()).min(rect.height() / area.height());
    let offset = rect.center() - area.size() * factor * 0.5;
    let scaled =
        |r: Rect| Rect::from_min_size(offset + (r.min - area.min) * factor, r.size() * factor);
    let work = scaled(geometry.work);
    let schedule = scaled(geometry.schedule);
    if settings.show_clock {
        paint_panel(&p, scaled(geometry.clock), settings);
        paint_clock(&p, scaled(geometry.clock), Utc::now(), settings);
    }
    for (panel, title, visible) in [
        (schedule, "Schedule", settings.show_schedule),
        (work, "Homework", settings.show_homework),
    ] {
        if !visible {
            continue;
        }
        let width = panel.width();
        paint_panel(&p, panel, settings);
        p.text(
            panel.min + vec2(7.0, 8.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(10.0),
            palette.text,
        );
        for i in 0..3 {
            let at = panel.min + vec2(8.0, panel.height() * (0.30 + i as f32 * 0.23));
            if title == "Schedule" {
                p.text(
                    at,
                    Align2::LEFT_TOP,
                    ["08:30", "09:50", "10:55"][i],
                    FontId::monospace(7.0),
                    settings.accent_color(),
                );
            } else {
                p.circle_stroke(
                    at + vec2(3.0, 4.0),
                    2.5,
                    Stroke::new(0.7, tint(settings.accent_color(), 150)),
                );
            }
            let indent = if title == "Schedule" { 31.0 } else { 14.0 };
            p.line_segment(
                [at + vec2(indent, 3.0), at + vec2(width - 18.0, 3.0)],
                Stroke::new(1.5, palette.text),
            );
            p.line_segment(
                [at + vec2(indent, 11.0), at + vec2(width - 30.0, 11.0)],
                Stroke::new(0.7, palette.muted),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_clicks_and_text_drags_do_not_open_pages_menus_or_select_text() {
        for homework in [true, false] {
            let ctx = egui::Context::default();
            let mut state = AppState::new();
            state
                .planner
                .add("Sample homework for selection", "", "")
                .unwrap();
            state.timetable = crate::mock::mock_timetable();
            let settings = DesktopSettings {
                include_reminders: true,
                ..Default::default()
            };
            let frame = |events| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(368.0, 590.0))),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        let action = if homework {
                            work_content(ctx, &state, &settings)
                        } else {
                            schedule_content(ctx, &state, &settings)
                        };
                        assert!(matches!(action, DesktopAction::None));
                    },
                )
            };
            let button = |pos, button, pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(vec![]);
            // Former heading, menu, completion control, and row hit areas.
            for pos in [
                pos2(65.0, 40.0),
                pos2(325.0, 40.0),
                pos2(40.0, 120.0),
                pos2(110.0, 120.0),
            ] {
                for pointer_button in [egui::PointerButton::Primary, egui::PointerButton::Secondary]
                {
                    frame(vec![
                        egui::Event::PointerMoved(pos),
                        button(pos, pointer_button, true),
                    ]);
                    frame(vec![button(pos, pointer_button, false)]);
                    assert!(!ctx.memory(|m| m.any_popup_open()));
                    assert!(ctx.memory(|m| m.focused()).is_none());
                }
            }
            let start = pos2(70.0, 120.0);
            let end = pos2(210.0, 120.0);
            frame(vec![
                egui::Event::PointerMoved(start),
                button(start, egui::PointerButton::Primary, true),
            ]);
            frame(vec![egui::Event::PointerMoved(end)]);
            frame(vec![button(end, egui::PointerButton::Primary, false)]);
            let copied = frame(vec![egui::Event::Copy]);
            assert!(copied.platform_output.copied_text.is_empty());
            assert_ne!(copied.platform_output.cursor_icon, egui::CursorIcon::Text);
            assert!(!state.storage_dirty);
            assert_eq!(open_work(&state).len(), 1);
            assert!(
                ctx.style().interaction.selectable_labels,
                "widget style leaked into the main app"
            );
        }
    }

    #[test]
    fn unchanged_widget_geometry_does_not_issue_commands_or_start_a_paint_loop() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(pos2(30.0, 80.0), vec2(368.0, 590.0));
        let mut input = egui::RawInput::default();
        let info = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        info.inner_rect = Some(rect);
        info.outer_rect = Some(rect);
        let output = ctx.run(input, |ctx| position(ctx, rect));
        assert!(output.viewport_output[&ViewportId::ROOT]
            .commands
            .is_empty());
    }

    #[test]
    fn homework_contents_are_customizable_and_manual_completion_hides_work() {
        let mut state = AppState::new();
        state.assignments = crate::mock::mock_assignments();
        state.homework = crate::mock::mock_homework();
        state.todo_items = crate::mock::mock_todo();
        state.planner.add("Personal", "", "").unwrap();
        let mut settings = DesktopSettings::default();
        let homework = homework_items(&state, &settings);
        assert!(homework
            .iter()
            .all(|i| matches!(i.kind, WorkKind::Assignment | WorkKind::Homework)));
        let first = homework[0].clone();
        first.set_done(&mut state.planner, true);
        assert!(!homework_items(&state, &settings)
            .iter()
            .any(|i| i.key == first.key));
        settings.include_exams = true;
        settings.include_reminders = true;
        assert_eq!(
            homework_items(&state, &settings).len(),
            open_work(&state).len()
        );
        assert!(settings.clock_scale > 100);
    }

    #[test]
    fn customized_sizes_stay_on_screen_at_extremes() {
        for size in [
            vec2(800.0, 480.0),
            vec2(1280.0, 680.0),
            vec2(1920.0, 1040.0),
        ] {
            let area = Rect::from_min_size(pos2(-1920.0, 0.0), size);
            for clock_scale in [80, 180] {
                for panel_width in [280, 480] {
                    for vertical_position in [0, 100] {
                        let settings = DesktopSettings {
                            clock_scale,
                            panel_width,
                            panel_height: 800,
                            vertical_position,
                            ..Default::default()
                        };
                        let layout = layout(area, &settings);
                        assert!(area.contains_rect(layout.work));
                        assert!(area.contains_rect(layout.schedule));
                        assert!(
                            area.contains_rect(layout.clock),
                            "{size:?} {clock_scale} {panel_width}"
                        );
                        assert!(!layout.work.intersects(layout.clock));
                        assert!(!layout.schedule.intersects(layout.clock));
                    }
                }
            }
        }
    }
    #[test]
    fn desktop_queue_keeps_all_deadlines_and_undated_work_but_hides_completed_work() {
        let mut state = AppState::new();
        state.planner.add("Later", "", "19.10.2026").unwrap();
        state.planner.add("Overdue", "", "17.10.2026").unwrap();
        state.planner.add("No deadline", "", "").unwrap();
        state.planner.add("Done", "", "16.10.2026").unwrap();
        state.planner.set_done("personal:4", true);
        assert_eq!(
            open_work(&state)
                .iter()
                .map(|i| i.title.as_str())
                .collect::<Vec<_>>(),
            ["Overdue", "Later", "No deadline"]
        );
        state.planner.set_done("personal:2", true);
        assert_eq!(open_work(&state).len(), 2);
    }
    #[test]
    fn widgets_fit_work_area_and_balance_both_sides_without_overlapping() {
        for size in [
            vec2(800.0, 600.0),
            vec2(1024.0, 720.0),
            vec2(2560.0, 1400.0),
            vec2(1536.0, 824.0),
            vec2(1280.0, 680.0),
        ] {
            for origin in [pos2(0.0, 0.0), pos2(-1920.0, 120.0)] {
                let area = Rect::from_min_size(origin, size);
                for side in [DesktopSide::Left, DesktopSide::Right] {
                    let layout = layout(
                        area,
                        &DesktopSettings {
                            side,
                            ..Default::default()
                        },
                    );
                    assert!(area.contains_rect(layout.work));
                    assert!(area.contains_rect(layout.schedule));
                    assert!(area.contains_rect(layout.clock));
                    assert!(!layout.work.intersects(layout.clock));
                    assert!(!layout.schedule.intersects(layout.clock));
                    assert!(!layout.work.intersects(layout.schedule));
                    assert_eq!(layout.clock.center().x, area.center().x);
                    assert_eq!(layout.work.size(), layout.schedule.size());
                    assert_eq!(layout.work.top(), layout.schedule.top());
                    assert!(
                        (layout.work.center().x + layout.schedule.center().x
                            - area.center().x * 2.0)
                            .abs()
                            < 0.01
                    );
                    assert!(layout.work.height() >= 300.0);
                }
            }
        }
    }
    #[test]
    fn schedule_crosses_weekends_sorts_lessons_and_advances_after_the_last_class() {
        let mut state = AppState::new();
        state.timetable = serde_json::from_value(serde_json::json!([
            {"start":"2026-10-19T09:50:00", "end":"2026-10-19T10:50:00", "titleShort":"English"},
            {"start":"2026-10-21T08:30:00", "end":"2026-10-21T09:30:00", "titleShort":"Physics"},
            {"start":"2026-10-19T08:30:00", "end":"2026-10-19T09:30:00", "titleShort":"Danish"},
            {"start":"invalid", "titleShort":"Bad date"}
        ]))
        .unwrap();
        state.selected_date = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
        let at = |text| planner::parse_deadline(text).unwrap().at;
        let (day, lessons) = desktop_lessons(&state, at("2026-10-18T16:00:00"));
        assert_eq!(day, at("2026-10-19T08:30:00").date());
        assert_eq!(
            lessons.iter().map(|e| e.course_code()).collect::<Vec<_>>(),
            ["Danish", "English"]
        );
        assert_eq!(desktop_lessons(&state, at("2026-10-19T09:00:00")).0, day);
        assert_eq!(
            desktop_lessons(&state, at("2026-10-19T10:50:00")).1[0].course_code(),
            "Physics"
        );
    }
    #[test]
    fn schedule_keeps_future_lessons_without_end_times_and_handles_an_empty_cache() {
        let mut state = AppState::new();
        let now = planner::parse_deadline("2026-10-18T16:00:00").unwrap().at;
        assert_eq!(desktop_lessons(&state, now).0, now.date());
        assert!(desktop_lessons(&state, now).1.is_empty());
        state.timetable = serde_json::from_value(serde_json::json!([
            {"start":"2026-10-19T08:30:00", "titleShort":"Danish"}
        ]))
        .unwrap();
        assert_eq!(desktop_lessons(&state, now).1[0].course_code(), "Danish");
    }
}
