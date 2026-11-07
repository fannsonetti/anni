//! Borderless desktop schedule, clock, and homework sharing the live planner.
use crate::{
    day::{self, DayState, Phase, Segment},
    desktop::DesktopAction,
    desktop_platform as platform, locale,
    planner::{self, WorkItem, WorkKind},
    settings::{ClockStyle, DesktopSettings, DesktopSide},
    state::AppState,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Timelike, Utc};
use egui::{
    pos2,
    text::{LayoutJob, TextFormat},
    vec2, Align, Align2, Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Sense, Shape,
    Stroke, UiBuilder, ViewportBuilder, ViewportId,
};
use inna_core::models::TimetableEntry;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

/// Hermes sizes its overlay text in points.
const PT: f32 = 96.0 / 72.0;
const SMALL: f32 = 11.0 * PT;
const ITEM: f32 = 13.0 * PT;
const CURRENT: f32 = 15.0 * PT;

fn tint(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub struct DesktopLayout {
    pub work: Rect,
    pub schedule: Rect,
    pub clock: Rect,
    pub progress: Rect,
}

pub fn layout(screen: Rect, settings: &DesktopSettings) -> DesktopLayout {
    // Hermes' default placement: the middle half of the screen, starting 82% of the way down.
    let progress = Rect::from_min_size(
        pos2(
            screen.left() + screen.width() * 0.25,
            screen.top() + screen.height() * 0.82,
        ),
        vec2(screen.width() * 0.5, (screen.height() * 0.1).max(90.0)),
    );
    // Everything else stays clear of the bar.
    let mut area = screen;
    if settings.show_progress {
        area.max.y = progress.top();
    }
    let margin = (area.width() * 0.07).clamp(20.0, 180.0);
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
        progress,
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
    let progress_shared = shared.clone();
    let work_shared = shared.clone();
    if settings.show_homework {
        ctx.show_viewport_deferred(
            ViewportId::from_hash_of("desktop_work"),
            // Windows reopens clicks over rows; see `platform::set_clickable`.
            viewport(platform::WORK_TITLE, geometry.work).with_mouse_passthrough(cfg!(windows)),
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
            viewport(platform::SCHEDULE_TITLE, geometry.schedule).with_mouse_passthrough(true),
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
    if settings.show_progress {
        let shared = progress_shared;
        ctx.show_viewport_deferred(
            ViewportId::from_hash_of("desktop_progress"),
            viewport(platform::PROGRESS_TITLE, geometry.progress).with_mouse_passthrough(true),
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
                position(ctx, layout(platform::work_area(), &settings).progress);
                let now = Utc::now().naive_utc();
                let state = DayState::compute(&app.state.timetable, now);
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
                        paint_panel(ui.painter(), ui.max_rect().shrink(4.0), &settings);
                        paint_progress(
                            ui.painter(),
                            ui.max_rect().shrink2(vec2(12.0, 8.0)),
                            &state,
                            now,
                            settings.accent_color(),
                            text_scale(&settings),
                            true,
                        );
                    });
                app.finish_update(ctx);
                ctx.request_repaint_after(std::time::Duration::from_secs(15));
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

fn text_scale(settings: &DesktopSettings) -> f32 {
    settings.text_scale as f32 / 100.0
}

fn strong(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("desktop_strong".into()))
}

/// A wrapped run of text, right-aligned around x = 0 when `right` is set.
fn job(width: f32, right: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    if right {
        job.halign = Align::RIGHT;
    }
    job
}

fn plain(font: FontId, color: Color32) -> TextFormat {
    TextFormat::simple(font, color)
}

/// Hermes draws straight on the wallpaper: a soft dark halo keeps white text readable.
fn shadowed(p: &Painter, pos: Pos2, galley: &Arc<Galley>, strength: f32) {
    for (radius, alpha, steps) in [(2.5, 16.0, 8), (1.2, 42.0, 8)] {
        let shadow = Color32::from_black_alpha((alpha * strength) as u8);
        for step in 0..steps {
            let angle = step as f32 / steps as f32 * TAU;
            p.galley_with_override_text_color(
                pos + vec2(angle.cos(), angle.sin()) * radius,
                galley.clone(),
                shadow,
            );
        }
    }
    p.galley(pos, galley.clone(), Color32::WHITE);
}

fn shadowed_text(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: impl ToString,
    font: FontId,
    color: Color32,
) {
    let galley = p.layout_no_wrap(text.to_string(), font, color);
    let rect = anchor.anchor_size(pos, galley.size());
    shadowed(p, rect.min, &galley, 1.0);
}

fn dot(p: &Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    p.circle_filled(
        center,
        radius + 2.0,
        Color32::from_black_alpha((40.0 * strength) as u8),
    );
    p.circle_filled(center, radius, color);
}

/// Hermes' lists sit centered in their part; a long list starts at the top and scrolls.
fn list(ui: &mut egui::Ui, body: Rect, id: &str, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_new_ui(UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(body.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(id)
            .drag_to_scroll(false)
            .auto_shrink([false, false])
            .max_height(body.height())
            .show(ui, |ui| {
                ui.add_space(((body.height() - height) * 0.5).max(0.0));
                add(ui);
            });
    });
}

struct WorkRow {
    title: Arc<Galley>,
    details: Option<Arc<Galley>>,
    color: Color32,
    opacity: f32,
    done: bool,
    height: f32,
}

fn work_row(
    ui: &egui::Ui,
    item: &WorkItem,
    now: NaiveDateTime,
    settings: &DesktopSettings,
    width: f32,
    right: bool,
) -> WorkRow {
    let scale = text_scale(settings);
    let palette = settings.theme.palette();
    let small = FontId::proportional(SMALL * scale);
    // Ticked rows fade to 35% with a strikethrough, as in Hermes.
    let ticked =
        ui.ctx()
            .animate_bool_with_time(egui::Id::new(("ticked", &item.key)), item.done, 0.6);
    let opacity = 1.0 - 0.65 * ticked;
    let days = item.deadline.map(|d| (d.at.date() - now.date()).num_days());
    let color = match days {
        _ if item.overdue(now) => palette.urgent,
        Some(0) => palette.urgent,
        Some(1) => palette.soon,
        _ => palette.muted,
    };
    let color = color.gamma_multiply(opacity);
    let text = palette.text.gamma_multiply(opacity);
    let mut title = job(width, right);
    title.append(
        &item.title,
        0.0,
        plain(FontId::proportional(ITEM * scale), text),
    );
    let mut details = job(width, right);
    let dim = text.gamma_multiply(0.7);
    let mut course = String::new();
    if settings.show_course_codes {
        course.clone_from(&item.course);
    }
    if item.kind == WorkKind::Exam {
        course = [course.as_str(), "exam"]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
    }
    if !course.is_empty() {
        details.append(&course, 0.0, plain(small.clone(), dim));
    }
    let due = if !settings.show_deadlines {
        String::new()
    } else if let Some(deadline) = item.deadline {
        day::countdown(deadline, now)
    } else {
        item.due_text.clone()
    };
    if !due.is_empty() {
        if !course.is_empty() {
            details.append(" · ", 0.0, plain(small.clone(), text.gamma_multiply(0.5)));
        }
        details.append(&due, 0.0, plain(small, color));
    }
    let title = ui.fonts(|fonts| fonts.layout_job(title));
    let details = (!details.text.is_empty()).then(|| ui.fonts(|fonts| fonts.layout_job(details)));
    let height = title.size().y
        + details.as_ref().map_or(0.0, |d| d.size().y)
        + settings.row_spacing as f32 * 0.5;
    WorkRow {
        title,
        details,
        color,
        opacity,
        done: item.done,
        height,
    }
}

fn work_content(
    ctx: &egui::Context,
    state: &mut AppState,
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
            let body = panel_content(ui, settings);
            let scale = text_scale(settings);
            let palette = settings.theme.palette();
            let right = settings.side == DesktopSide::Right;
            let now = Utc::now().naive_utc();
            let items = homework_items(state, settings, now);
            let dot_space = 20.0 * scale;
            let width = (body.width() - dot_space - 16.0).max(40.0);
            let rows: Vec<_> = items
                .iter()
                .map(|item| work_row(ui, item, now, settings, width, right))
                .collect();
            let heading_height = SMALL * scale * 1.4 + 8.0;
            let empty_height = if rows.is_empty() {
                ITEM * scale * 1.4
            } else {
                0.0
            };
            let height =
                heading_height + empty_height + rows.iter().map(|row| row.height).sum::<f32>();
            let mut clickable = Vec::new();
            let mut clicked = None;
            list(ui, body, "desktop_work_list", height, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    vec2(ui.available_width(), heading_height),
                    Sense::hover(),
                );
                let edge = |rect: Rect| if right { rect.right() } else { rect.left() };
                let inward = if right { -1.0 } else { 1.0 };
                let anchor = if right {
                    Align2::RIGHT_TOP
                } else {
                    Align2::LEFT_TOP
                };
                shadowed_text(
                    ui.painter(),
                    pos2(edge(rect) + inward * dot_space, rect.top()),
                    anchor,
                    "Assignments",
                    strong(SMALL * scale),
                    palette.text.gamma_multiply(0.75),
                );
                if rows.is_empty() {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), empty_height),
                        Sense::hover(),
                    );
                    shadowed_text(
                        ui.painter(),
                        pos2(edge(rect) + inward * dot_space, rect.top()),
                        anchor,
                        "Nothing due",
                        FontId::proportional(ITEM * scale),
                        palette.text.gamma_multiply(0.7),
                    );
                }
                for (index, row) in rows.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), row.height),
                        Sense::hover(),
                    );
                    let x = edge(rect) + inward * dot_space;
                    let top = rect.top() + settings.row_spacing as f32 * 0.25;
                    shadowed(ui.painter(), pos2(x, top), &row.title, row.opacity);
                    if row.done {
                        // Drawn here: a formatted strikethrough would get the shadow's passes too.
                        for line in &row.title.rows {
                            let y = top + line.rect.center().y;
                            ui.painter().hline(
                                x + line.rect.left()..=x + line.rect.right(),
                                y,
                                Stroke::new(1.0, Color32::WHITE.gamma_multiply(row.opacity)),
                            );
                        }
                    }
                    if let Some(details) = &row.details {
                        let below = top + row.title.size().y;
                        shadowed(ui.painter(), pos2(x, below), details, row.opacity);
                    }
                    // Only the row's own text and dot catch clicks, as in Hermes.
                    let text_width = row
                        .details
                        .as_ref()
                        .map_or(0.0, |d| d.size().x)
                        .max(row.title.size().x)
                        + dot_space;
                    let mut hit = rect;
                    if right {
                        hit.min.x = hit.max.x - text_width;
                    } else {
                        hit.max.x = hit.min.x + text_width;
                    }
                    let response = ui
                        .interact(hit, ui.id().with(index), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        clicked = Some(index);
                    }
                    clickable.push(hit.intersect(ui.clip_rect()));
                    dot(
                        ui.painter(),
                        pos2(edge(rect) + inward * 6.0, rect.center().y),
                        4.0 * scale,
                        row.color,
                        row.opacity,
                    );
                }
            });
            let pixels = ui.ctx().pixels_per_point();
            platform::set_clickable(
                clickable
                    .into_iter()
                    .map(|rect| Rect::from_min_max(rect.min * pixels, rect.max * pixels))
                    .collect(),
            );
            if let Some(index) = clicked {
                let item = &items[index];
                item.set_done(&mut state.planner, !item.done);
                state.storage_dirty = true;
            }
        });
    action
}

#[cfg(test)]
fn open_work(state: &AppState) -> Vec<WorkItem> {
    planner::work_items(state)
        .into_iter()
        .filter(|item| !item.done)
        .collect()
}

/// Open work, plus work ticked off in the last day so a slip can be undone.
fn homework_items(
    state: &AppState,
    settings: &DesktopSettings,
    now: NaiveDateTime,
) -> Vec<WorkItem> {
    let mut items = planner::work_items(state)
        .into_iter()
        .filter(|item| {
            let ticked_at = state.planner.done_at.get(&item.key);
            !item.done
                || (!item.source_done && ticked_at.is_some_and(|at| now - *at < Duration::days(1)))
        })
        .filter(|item| match item.kind {
            WorkKind::Assignment | WorkKind::Homework => true,
            WorkKind::Exam => settings.include_exams,
            WorkKind::Personal | WorkKind::InnaTask => settings.include_reminders,
        })
        .collect::<Vec<_>>();
    // Ticked rows stay where they are, so a second click undoes the same one.
    items.sort_by_cached_key(|item| {
        (
            item.deadline.is_none(),
            item.deadline.map(|d| d.at),
            crate::locale::sort_key(&item.title),
        )
    });
    items
}

/// Select the current school day while lessons remain, then the next cached day.
/// Invalid dates are ignored; a lesson with no end time still appears before it starts.
fn desktop_lessons(state: &AppState, now: NaiveDateTime) -> (NaiveDate, Vec<&TimetableEntry>) {
    let mut dated: Vec<_> = state
        .timetable
        .iter()
        .filter_map(|entry| {
            if !entry.is_lesson() {
                return None;
            }
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

struct ScheduleRow {
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    subject: String,
    room: String,
    is_break: bool,
}

/// One day's lessons with the listed breaks slotted in between them.
fn schedule_rows(
    state: &AppState,
    date: NaiveDate,
    lessons: &[&TimetableEntry],
) -> Vec<ScheduleRow> {
    let time = |text: &Option<String>| {
        text.as_deref()
            .and_then(planner::parse_deadline)
            .map(|deadline| deadline.at)
    };
    let day: Vec<Segment> = day::lessons(&state.timetable)
        .into_iter()
        .filter(|lesson| lesson.start.date() == date)
        .collect();
    let mut rows: Vec<_> = lessons
        .iter()
        .map(|lesson| ScheduleRow {
            start: time(&lesson.start),
            end: time(&lesson.end),
            subject: crate::ui::assignments::subject(lesson.course_code()).to_string(),
            room: lesson.room().to_string(),
            is_break: false,
        })
        .chain(day::breaks(&day).into_iter().map(|gap| ScheduleRow {
            start: Some(gap.start),
            end: Some(gap.end),
            subject: gap.subject,
            room: String::new(),
            is_break: true,
        }))
        .collect();
    rows.sort_by_key(|row| row.start);
    rows
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
    let rows = schedule_rows(state, date, &lessons);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(0.0))
        .show(ctx, |ui| {
            let body = panel_content(ui, settings);
            let scale = text_scale(settings);
            let palette = settings.theme.palette();
            let accent = settings.accent_color();
            let time_text = |row: &ScheduleRow| {
                let clock = |time: Option<NaiveDateTime>| {
                    time.map_or("—".to_string(), |t| t.format("%H:%M").to_string())
                };
                format!("{} - {}", clock(row.start), clock(row.end))
            };
            let is_current = |row: &ScheduleRow| {
                row.start
                    .zip(row.end)
                    .is_some_and(|(s, e)| s <= now && now < e)
            };
            let time_width = rows
                .iter()
                .map(|row| {
                    let size = if is_current(row) && !row.is_break {
                        CURRENT
                    } else {
                        ITEM
                    };
                    ui.fonts(|fonts| {
                        fonts
                            .layout_no_wrap(
                                time_text(row),
                                FontId::proportional(size * scale),
                                palette.text,
                            )
                            .size()
                            .x
                    })
                })
                .fold(0.0, f32::max);
            let dot_space = 20.0 * scale;
            let time_space = time_width + 12.0 * scale;
            let subject_width = (body.width() - dot_space - time_space - 16.0).max(40.0);

            let lines: Vec<_> = rows
                .iter()
                .map(|row| {
                    let current = is_current(row);
                    let past = row.end.is_some_and(|end| end <= now);
                    let size = match (row.is_break, current) {
                        (true, _) => SMALL,
                        (false, true) => CURRENT,
                        (false, false) => ITEM,
                    } * scale;
                    let color = if current { accent } else { palette.text };
                    let color = match (past, row.is_break && !current) {
                        (true, _) => color.gamma_multiply(0.45),
                        (false, true) => color.gamma_multiply(0.7),
                        _ => color,
                    };
                    let font = if current && !row.is_break {
                        strong(size)
                    } else {
                        FontId::proportional(size)
                    };
                    let mut subject = job(subject_width, false);
                    subject.append(
                        &row.subject,
                        0.0,
                        TextFormat {
                            italics: row.is_break,
                            ..plain(font, color)
                        },
                    );
                    if current && !row.room.is_empty() {
                        subject.append(
                            &row.room,
                            10.0,
                            plain(
                                FontId::proportional(SMALL * scale),
                                palette.text.gamma_multiply(0.7),
                            ),
                        );
                    }
                    let subject = ui.fonts(|fonts| fonts.layout_job(subject));
                    let time = ui.fonts(|fonts| {
                        fonts.layout_no_wrap(time_text(row), FontId::proportional(size), color)
                    });
                    let height =
                        subject.size().y.max(time.size().y) + settings.row_spacing as f32 * 0.5;
                    (row, subject, time, current, past, color, height)
                })
                .collect();

            let heading = if date == now.date() {
                "Today"
            } else {
                "Next school day"
            };
            let header_height = (SMALL * 2.8 + 12.0) * scale;
            let updated = state
                .last_sync
                .get("timetable")
                .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
                .map(|time| format!("Updated {}", time.format("%H:%M")));
            let empty_height = if lines.is_empty() {
                ITEM * scale * 1.4
            } else {
                0.0
            };
            let footer_height = if updated.is_some() {
                (SMALL * 1.4 + 12.0) * scale
            } else {
                0.0
            };
            let height = header_height
                + empty_height
                + footer_height
                + lines.iter().map(|line| line.6).sum::<f32>();
            list(ui, body, "desktop_schedule_list", height, |ui| {
                let (rect, _) = ui
                    .allocate_exact_size(vec2(ui.available_width(), header_height), Sense::hover());
                shadowed_text(
                    ui.painter(),
                    rect.min,
                    Align2::LEFT_TOP,
                    heading,
                    strong(SMALL * scale),
                    palette.text.gamma_multiply(0.75),
                );
                shadowed_text(
                    ui.painter(),
                    rect.min + vec2(0.0, SMALL * 1.4 * scale),
                    Align2::LEFT_TOP,
                    locale::day(date),
                    FontId::proportional(SMALL * scale),
                    palette.text.gamma_multiply(0.75),
                );
                if lines.is_empty() {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), empty_height),
                        Sense::hover(),
                    );
                    shadowed_text(
                        ui.painter(),
                        rect.min,
                        Align2::LEFT_TOP,
                        "No upcoming classes",
                        FontId::proportional(ITEM * scale),
                        palette.text.gamma_multiply(0.75),
                    );
                }
                for (row, subject, time, current, past, color, height) in &lines {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), *height), Sense::hover());
                    let strength = if *past { 0.45 } else { 1.0 };
                    let center = pos2(rect.left() + 5.0, rect.center().y);
                    if row.is_break {
                        ui.painter()
                            .circle_stroke(center, 3.0 * scale, Stroke::new(1.0, *color));
                    } else {
                        let radius = if *current { 4.5 } else { 3.5 } * scale;
                        dot(ui.painter(), center, radius, *color, strength);
                    }
                    let middle = |galley: &Arc<Galley>| rect.center().y - galley.size().y * 0.5;
                    shadowed(
                        ui.painter(),
                        pos2(rect.left() + dot_space, middle(time)),
                        time,
                        strength,
                    );
                    shadowed(
                        ui.painter(),
                        pos2(rect.left() + dot_space + time_space, middle(subject)),
                        subject,
                        strength,
                    );
                }
                if let Some(updated) = &updated {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), footer_height),
                        Sense::hover(),
                    );
                    shadowed_text(
                        ui.painter(),
                        rect.left_bottom(),
                        Align2::LEFT_BOTTOM,
                        updated,
                        FontId::proportional(SMALL * scale),
                        palette.text.gamma_multiply(0.45),
                    );
                }
            });
        });
    action
}

/// Hermes' bottom bar: what's on now, one segment per class, and how much of the day is done.
pub fn paint_progress(
    p: &Painter,
    rect: Rect,
    state: &DayState,
    now: NaiveDateTime,
    accent: Color32,
    scale: f32,
    shadow: bool,
) {
    let white = Color32::WHITE;
    let item = FontId::proportional(ITEM * scale);
    let mut headline = job(rect.width(), false);
    headline.wrap.max_rows = 1;
    headline.wrap.break_anywhere = true;
    for (index, part) in state.headline.iter().enumerate() {
        if index > 0 {
            headline.append("  ·  ", 0.0, plain(item.clone(), white));
        }
        let (font, color) = match (index, state.phase) {
            (0, Phase::InClass) => (strong(ITEM * scale), accent),
            (0, _) => (strong(ITEM * scale), white),
            _ => (item.clone(), white),
        };
        headline.append(part, 0.0, plain(font, color));
    }
    let headline = p.fonts(|fonts| fonts.layout_job(headline));
    let footer = p.layout_no_wrap(
        state.footer.clone(),
        FontId::proportional(SMALL * scale),
        white.gamma_multiply(0.85),
    );
    let bar_height = 4.0;
    let footer_height = if state.footer.is_empty() {
        0.0
    } else {
        footer.size().y + 8.0
    };
    let height = headline.size().y + 9.0 + bar_height + footer_height;
    let top = rect.center().y - height * 0.5;
    let strength = if shadow { 1.0 } else { 0.0 };
    let centered = |galley: &Arc<Galley>| rect.center().x - galley.size().x * 0.5;
    shadowed(p, pos2(centered(&headline), top), &headline, strength);
    let bar = Rect::from_min_size(
        pos2(rect.left(), top + headline.size().y + 9.0),
        vec2(rect.width(), bar_height),
    );
    paint_segments(p, bar, state, now, accent, shadow);
    if !state.footer.is_empty() {
        shadowed(
            p,
            pos2(centered(&footer), bar.bottom() + 8.0),
            &footer,
            strength,
        );
    }
}

fn paint_segments(
    p: &Painter,
    bar: Rect,
    state: &DayState,
    now: NaiveDateTime,
    accent: Color32,
    shadow: bool,
) {
    const GAP: f32 = 4.0;
    let seconds = |segment: &Segment| (segment.end - segment.start).num_seconds() as f32;
    let count = state.segments.len() as f32;
    let total: f32 = state.segments.iter().map(seconds).sum();
    let available = (bar.width() - GAP * (count - 1.0)).max(0.0);
    let mut x = bar.left();
    for (index, segment) in state.segments.iter().enumerate() {
        let width = available * seconds(segment) / total;
        // Breaks are a thinner, softer line between the class segments.
        let height = if segment.is_break {
            bar.height() * 0.5
        } else {
            bar.height()
        };
        let opacity = if segment.is_break { 0.7 } else { 1.0 };
        let track =
            Rect::from_center_size(pos2(x + width * 0.5, bar.center().y), vec2(width, height));
        if shadow {
            p.rect_filled(track.expand(1.5), 3.0, Color32::from_black_alpha(36));
        }
        p.rect_filled(
            track,
            2.0,
            Color32::from_white_alpha(0x40).gamma_multiply(opacity),
        );
        // The fill glides to its new length over 800 ms on every tick, as in Hermes.
        let target = segment.fill(now);
        let fill = p.ctx().animate_value_with_time(
            egui::Id::new(("day_fill", shadow, index)),
            target,
            0.8,
        );
        if fill > 0.0 {
            let color = if target >= 1.0 {
                Color32::from_white_alpha(0xD8)
            } else {
                accent
            };
            let mut done = track;
            done.set_width(width * fill);
            p.rect_filled(done, 2.0, color.gamma_multiply(opacity));
        }
        x += width + GAP;
    }
}

/// The same bar along the bottom of the planner window.
pub fn planner_progress(ui: &mut egui::Ui, state: &AppState) {
    let now = Utc::now().naive_utc();
    let day = DayState::compute(&state.timetable, now);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 66.0), Sense::hover());
    paint_progress(
        ui.painter(),
        rect,
        &day,
        now,
        crate::ui::theme::ACCENT,
        0.8,
        false,
    );
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
        shadowed_text(
            p,
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
        shadowed_text(
            p,
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
            palette.text.gamma_multiply(0.85),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_clicks_and_text_drags_do_not_open_pages_menus_or_select_text() {
        for homework in [true, false] {
            let ctx = egui::Context::default();
            crate::ui::theme::apply_theme(&ctx);
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
                            // Row clicks are covered by the tick test below.
                            work_content(ctx, &mut AppState::new(), &settings)
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
        let settings = DesktopSettings::default();
        let now = Utc::now().naive_utc();
        let homework = homework_items(&state, &settings, now);
        assert!(homework
            .iter()
            .all(|i| matches!(i.kind, WorkKind::Assignment | WorkKind::Homework)));
        let first = homework[0].clone();
        first.set_done(&mut state.planner, true);
        let visible = |state: &AppState, now| {
            homework_items(state, &settings, now)
                .iter()
                .find(|i| i.key == first.key)
                .map(|i| i.done)
        };
        // Struck through for a day, then gone; unticking brings it straight back.
        assert_eq!(visible(&state, now), Some(true));
        assert_eq!(visible(&state, now + Duration::hours(25)), None);
        first.set_done(&mut state.planner, false);
        assert_eq!(visible(&state, now), Some(false));
        first.set_done(&mut state.planner, true);
        state.planner.done_at.clear();
        assert_eq!(visible(&state, now), None);
        let mut settings = DesktopSettings::default();
        settings.include_exams = true;
        settings.include_reminders = true;
        assert_eq!(
            homework_items(&state, &settings, now).len(),
            open_work(&state).len()
        );
        assert!(settings.clock_scale > 100);
    }

    #[test]
    fn clicking_an_assignment_ticks_it_and_clicking_again_undoes_it() {
        let ctx = egui::Context::default();
        crate::ui::theme::apply_theme(&ctx);
        let mut state = AppState::new();
        state.assignments = crate::mock::mock_assignments();
        let settings = DesktopSettings::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(368.0, 590.0));
        let key = homework_items(&state, &settings, Utc::now().naive_utc())[0]
            .key
            .clone();
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    work_content(ctx, &mut state, &settings);
                },
            );
            state.planner.completed.contains(&key)
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let click = |frame: &mut dyn FnMut(Vec<egui::Event>) -> bool, pos| {
            frame(vec![egui::Event::PointerMoved(pos)]);
            frame(vec![button(pos, true)]);
            frame(vec![button(pos, false)]);
            frame(vec![])
        };
        frame(vec![]);
        let row = (0..590)
            .step_by(6)
            .map(|y| pos2(300.0, y as f32))
            .find(|pos| click(&mut frame, *pos))
            .expect("a row catches the click");
        assert!(!click(&mut frame, row), "a second click undoes the tick");
        assert!(!click(&mut frame, pos2(40.0, 20.0)), "empty space is inert");
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
                    assert!(!layout.work.intersects(layout.progress));
                    assert!(!layout.clock.intersects(layout.progress));
                    assert!(area.contains_rect(layout.progress));
                    assert_eq!(layout.progress.center().x, area.center().x);
                    assert!(layout.work.height() >= 220.0);
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
