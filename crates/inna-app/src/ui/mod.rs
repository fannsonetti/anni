pub mod announcements;
pub mod assignments;
pub mod brand;
pub mod connection;
pub mod desktop;
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
        for page in [Page::Timetable, Page::Assignments, Page::Planner] {
            for width in [700.0, 764.0, 1004.0, 1440.0] {
                let ctx = egui::Context::default();
                super::theme::apply_theme(&ctx);
                let mut state = AppState::new();
                state.selected_date = crate::locale::parse_date("19.10.2026").unwrap();
                state.timetable = crate::mock::mock_timetable();
                state.assignments = crate::mock::mock_assignments();
                state.assignments[0].assignments[0].name =
                    Some("A very long assignment title ".repeat(30));
                for view in [
                    crate::grades::GradeView::GradedWork,
                    crate::grades::GradeView::FullCourse,
                ] {
                    state.grade_view = view;
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
                                    match page {
                                        Page::Timetable => super::timetable::show(ui, &mut state),
                                        Page::Assignments => {
                                            super::assignments::show(ui, &mut state)
                                        }
                                        Page::Planner => super::planner::show(ui, &mut state),
                                        _ => unreachable!(),
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
