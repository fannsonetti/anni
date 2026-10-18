use crate::state::AppState;
use crate::ui::theme::*;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Homework").size(24.0).strong());
    ui.add_space(8.0);

    if state.homework.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new("No deadlines listed").size(18.0).color(TEXT_DIM));
    } else {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &state.homework {
                egui::Frame::none()
                    .fill(CARD_BG)
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(10.0))
                    .outer_margin(egui::Margin::symmetric(0.0, 3.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                item.raw
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Homework"),
                            )
                            .size(17.0),
                        );
                    });
            }
        });
    }
}
