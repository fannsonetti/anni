pub mod announcements;
pub mod assignments;
pub mod brand;
pub mod connection;
pub mod desktop;
pub mod focus;
pub mod home;
pub mod navigation;
pub mod planner;
pub mod theme;
pub mod timetable;
pub mod updates;

#[cfg(test)]
mod tests {
    #[test]
    fn redesigned_pages_keep_long_content_inside_the_window() {
        use crate::state::{AppState, Page};
        for page in [Page::Focus, Page::Timetable] {
            for width in [700.0, 764.0, 1004.0, 1440.0] {
                let ctx = egui::Context::default();
                super::theme::apply_theme(&ctx);
                let mut state = AppState::new();
                state.selected_date = crate::locale::parse_date("19.10.2026").unwrap();
                state.timetable = crate::mock::mock_timetable();
                state.focus.task_title = "A very long task title ".repeat(30);
                state
                    .planner
                    .add(&state.focus.task_title, "English", "")
                    .unwrap();
                for _ in 0..2 {
                    let _ = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                let right = ui.max_rect().right();
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    if page == Page::Focus {
                                        super::focus::show(ui, &mut state);
                                    } else {
                                        super::timetable::show(ui, &mut state);
                                    }
                                    assert!(
                                        ui.min_rect().right() <= right + 1.0,
                                        "{page:?} overflow at {width}: {} > {right}",
                                        ui.min_rect().right()
                                    );
                                });
                            });
                        },
                    );
                }
            }
        }
    }
}
