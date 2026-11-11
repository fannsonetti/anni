use super::{brand, theme};
use crate::locale::t;
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
        .exact_width(200.0)
        .resizable(false)
        .frame(egui::Frame::none().fill(theme::NAV_BG).inner_margin(18.0))
        .show(ctx, |ui| {
            ui.add_space(10.0);
            if brand::wordmark(ui, false).clicked() {
                state.navigate(Page::Home);
            }
            ui.add_space(20.0);

            // The footer has its own space; the menu scrolls on short desktop windows.
            let menu_height = (ui.available_height() - 106.0).max(100.0);
            egui::ScrollArea::vertical()
                .id_salt("sidebar_pages")
                .max_height(menu_height)
                .show(ui, |ui| {
                    group(ui, state, t("School", "Skóli"), Page::SCHOOL_PAGES);
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
            .family(crate::ui::theme::semibold())
            .color(theme::TEXT_DIM),
    );
    ui.add_space(6.0);
    for &page in pages {
        if item(ui, page, state.selected_page == page, false).clicked() {
            state.navigate(page);
        }
        ui.add_space(2.0);
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
        .on_hover_text(t("Back · Alt + Left", "Til baka · Alt + Vinstri"))
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
        .on_hover_text(t("Forward · Alt + Right", "Áfram · Alt + Hægri"))
        .clicked()
    {
        state.go_forward();
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new(state.selected_page.label())
            .size(14.0)
            .family(crate::ui::theme::semibold()),
    );
    if state.selected_announcement.is_some() {
        ui.label(
            RichText::new(t("/ Announcement", "/ Tilkynning"))
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
                        .zip([Page::Home, Page::Timetable, Page::Assignments])
                {
                    let selected = state.selected_page == page;
                    if column
                        .add_sized(
                            [column.available_width(), 46.0],
                            egui::Button::new(
                                RichText::new(page.label())
                                    .family(crate::ui::theme::semibold())
                                    .color(if selected {
                                        theme::ACCENT
                                    } else {
                                        theme::TEXT_SECONDARY
                                    }),
                            )
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
                    ![Page::Home, Page::Timetable, Page::Assignments].contains(&state.selected_page);
                columns[3].with_layout(
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.menu_button(
                            RichText::new(t("More", "Meira")).size(14.0).color(if more_selected {
                                theme::ACCENT
                            } else {
                                theme::TEXT_SECONDARY
                            }),
                            |ui| {
                                ui.set_min_width(250.0);
                                ui.label(RichText::new(t("All pages", "Allar síður")).small().color(theme::TEXT_DIM));
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
        egui::vec2(ui.available_width(), if description { 52.0 } else { 38.0 }),
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
            2.0,
            if selected {
                theme::ACCENT_DIM
            } else {
                theme::CARD_BG
            },
        );
    }
    ui.painter().text(
        egui::pos2(
            rect.left() + 12.0,
            rect.top() + if description { 17.0 } else { 19.0 },
        ),
        egui::Align2::LEFT_CENTER,
        page.label(),
        egui::FontId::proportional(14.0),
        color,
    );
    if description {
        let short = match page {
            Page::Home => t("What matters today", "Það sem skiptir máli í dag"),
            Page::Planner => t("Deadlines & tasks", "Skilafrestir og verkefni"),
            Page::Timetable => t("Lessons & rooms", "Tímar og stofur"),
            Page::Assignments => t("Progress & results", "Framvinda og niðurstöður"),
            Page::Calculator => t("Plan your final grade", "Reiknaðu lokaeinkunnina"),
            Page::Announcements => t("News from your school", "Fréttir frá skólanum"),
            Page::Connection => t("Language & preferences", "Tungumál og stillingar"),
        };
        ui.painter().text(
            egui::pos2(rect.left() + 12.0, rect.top() + 35.0),
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
