use super::{brand, theme};
use crate::state::{AppState, Page};
use egui::{RichText, Ui};

pub fn shortcuts(ctx: &egui::Context, state: &mut AppState) {
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)) {
        state.go_back();
    }
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight)) {
        state.go_forward();
    }
}

pub fn sidebar(ctx: &egui::Context, state: &mut AppState) {
    egui::SidePanel::left("navigation")
        .exact_width(224.0)
        .resizable(false)
        .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(18.0))
        .show(ctx, |ui| {
            ui.add_space(10.0);
            if brand::wordmark(ui, false).clicked() {
                state.navigate(Page::Home);
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("Your school day, organized")
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(26.0);

            // The footer has its own space; the menu scrolls on short desktop windows.
            let menu_height = (ui.available_height() - 106.0).max(100.0);
            egui::ScrollArea::vertical()
                .id_salt("sidebar_pages")
                .max_height(menu_height)
                .show(ui, |ui| {
                    group(ui, state, "YOUR WORKSPACE", Page::DAILY_PAGES);
                    ui.add_space(18.0);
                    group(ui, state, "SCHOOL", Page::SCHOOL_PAGES);
                });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                if let Some(user) = &state.user {
                    ui.label(
                        RichText::new(user.display_name())
                            .size(13.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                }
                ui.separator();
                if item(
                    ui,
                    Page::Connection,
                    state.selected_page == Page::Connection,
                    false,
                )
                .clicked()
                {
                    state.navigate(Page::Connection);
                }
            });
        });
}

fn group(ui: &mut Ui, state: &mut AppState, label: &str, pages: &[Page]) {
    ui.label(
        RichText::new(label)
            .size(10.0)
            .strong()
            .color(theme::TEXT_DIM),
    );
    ui.add_space(6.0);
    for &page in pages {
        if item(ui, page, state.selected_page == page, true).clicked() {
            state.navigate(page);
        }
        ui.add_space(3.0);
    }
}

pub fn location(ui: &mut Ui, state: &mut AppState) {
    ui.spacing_mut().item_spacing.x = 6.0;
    if ui
        .add_enabled(
            state.can_go_back(),
            egui::Button::new("←")
                .frame(false)
                .min_size(egui::vec2(28.0, 30.0)),
        )
        .on_hover_text("Back · Alt + Left")
        .clicked()
    {
        state.go_back();
    }
    if ui
        .add_enabled(
            state.can_go_forward(),
            egui::Button::new("→")
                .frame(false)
                .min_size(egui::vec2(28.0, 30.0)),
        )
        .on_hover_text("Forward · Alt + Right")
        .clicked()
    {
        state.go_forward();
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new(state.selected_page.label())
            .size(14.0)
            .strong(),
    );
    if state.selected_announcement.is_some() {
        ui.label(
            RichText::new("/ Announcement")
                .size(12.0)
                .color(theme::TEXT_DIM),
        );
    }
}

pub fn compact_bar(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::bottom("nav_bar")
        .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(10.0))
        .show(ctx, |ui| {
            ui.columns(4, |columns| {
                for (column, page) in
                    columns
                        .iter_mut()
                        .zip([Page::Home, Page::Planner, Page::Timetable])
                {
                    let selected = state.selected_page == page;
                    if column
                        .add_sized(
                            [column.available_width(), 46.0],
                            egui::Button::new(RichText::new(page.label()).strong().color(
                                if selected {
                                    theme::ACCENT
                                } else {
                                    theme::TEXT_SECONDARY
                                },
                            ))
                            .fill(if selected {
                                theme::ACCENT_DIM
                            } else {
                                theme::NAV_BG
                            })
                            .stroke(egui::Stroke::NONE),
                        )
                        .on_hover_text(page.description())
                        .clicked()
                    {
                        state.navigate(page);
                    }
                }
                let more_selected =
                    ![Page::Home, Page::Planner, Page::Timetable].contains(&state.selected_page);
                columns[3].with_layout(
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.menu_button(
                            RichText::new("☰  More").size(14.0).color(if more_selected {
                                theme::ACCENT
                            } else {
                                theme::TEXT_SECONDARY
                            }),
                            |ui| {
                                ui.set_min_width(250.0);
                                ui.label(RichText::new("All pages").small().color(theme::TEXT_DIM));
                                egui::ScrollArea::vertical()
                                    .id_salt("compact_pages")
                                    .max_height((ctx.screen_rect().height() - 100.0).max(160.0))
                                    .show(ui, |ui| {
                                        for &page in Page::NAV_PAGES {
                                            if item(ui, page, state.selected_page == page, true)
                                                .clicked()
                                            {
                                                state.navigate(page);
                                                ui.close_menu();
                                            }
                                        }
                                    });
                            },
                        );
                    },
                );
            });
        });
}

fn item(ui: &mut Ui, page: Page, selected: bool, description: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), if description { 52.0 } else { 44.0 }),
        egui::Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            page.label(),
        )
    });
    let color = if selected {
        theme::ACCENT
    } else {
        theme::TEXT_SECONDARY
    };
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            9.0,
            if selected {
                theme::ACCENT_DIM
            } else {
                theme::CARD_BG
            },
        );
    }
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 2.0, rect.center().y),
                egui::vec2(3.0, 22.0),
            ),
            1.5,
            theme::ACCENT,
        );
    }
    let center = egui::pos2(rect.left() + 23.0, rect.center().y);
    icon(ui, page, center, color);
    ui.painter().text(
        egui::pos2(
            rect.left() + 44.0,
            rect.top() + if description { 17.0 } else { 22.0 },
        ),
        egui::Align2::LEFT_CENTER,
        page.label(),
        egui::FontId::proportional(14.0),
        color,
    );
    if description {
        let short = match page {
            Page::Home => "What matters today",
            Page::Planner => "Deadlines & tasks",
            Page::Timetable => "Lessons & rooms",
            Page::Focus => "Time to concentrate",
            Page::Assignments => "Progress & results",
            Page::Announcements => "News from your school",
            Page::Connection => "Connection & preferences",
        };
        ui.painter().text(
            egui::pos2(rect.left() + 44.0, rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            short,
            egui::FontId::proportional(11.0),
            theme::TEXT_DIM,
        );
    }
    response
        .on_hover_text(page.description())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn icon(ui: &Ui, page: Page, center: egui::Pos2, color: egui::Color32) {
    let icon = egui::Rect::from_center_size(center, egui::vec2(17.0, 17.0));
    let stroke = egui::Stroke::new(1.5, color);
    let p = ui.painter();
    match page {
        Page::Home => {
            p.line_segment([icon.left_center(), icon.center_top()], stroke);
            p.line_segment([icon.center_top(), icon.right_center()], stroke);
            p.line_segment([icon.left_center(), icon.left_bottom()], stroke);
            p.line_segment([icon.right_center(), icon.right_bottom()], stroke);
            p.line_segment([icon.left_bottom(), icon.right_bottom()], stroke);
        }
        Page::Planner => {
            p.rect_stroke(icon, 3.0, stroke);
            for y in [-4.0, 1.0, 6.0] {
                p.line_segment(
                    [center + egui::vec2(-4.0, y), center + egui::vec2(4.0, y)],
                    stroke,
                );
            }
        }
        Page::Assignments => {
            for (x, height) in [(-6.0, 7.0), (0.0, 12.0), (6.0, 17.0)] {
                p.line_segment(
                    [
                        center + egui::vec2(x, 8.0),
                        center + egui::vec2(x, 8.0 - height),
                    ],
                    egui::Stroke::new(3.0, color),
                );
            }
        }
        Page::Timetable => {
            p.rect_stroke(icon, 3.0, stroke);
            p.line_segment(
                [
                    center + egui::vec2(-8.0, -3.0),
                    center + egui::vec2(8.0, -3.0),
                ],
                stroke,
            );
            for x in [-4.0, 4.0] {
                p.line_segment(
                    [center + egui::vec2(x, -11.0), center + egui::vec2(x, -6.0)],
                    stroke,
                );
            }
        }
        Page::Focus => {
            p.circle_stroke(center, 9.0, stroke);
            p.line_segment([center, center + egui::vec2(0.0, -5.0)], stroke);
            p.line_segment([center, center + egui::vec2(4.0, 2.0)], stroke);
        }
        Page::Announcements => {
            p.rect_stroke(icon, 3.0, stroke);
            p.line_segment([icon.left_top(), center + egui::vec2(0.0, 2.0)], stroke);
            p.line_segment([icon.right_top(), center + egui::vec2(0.0, 2.0)], stroke);
        }
        Page::Connection => {
            p.circle_stroke(center, 7.0, stroke);
            p.circle_stroke(center, 2.0, stroke);
            for angle in 0..8 {
                let a = angle as f32 * std::f32::consts::TAU / 8.0;
                let direction = egui::vec2(a.cos(), a.sin());
                p.line_segment(
                    [center + direction * 7.0, center + direction * 11.0],
                    stroke,
                );
            }
        }
    }
}
