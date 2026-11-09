//! The same monogram is used in the planner, native windows, and Windows resources.
use super::theme::TEXT_PRIMARY;
use egui::{RichText, Sense, Ui};
use std::sync::OnceLock;

pub const NAME: &str = "anni";

pub fn wordmark(ui: &mut Ui, compact: bool) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(NAME)
                .size(if compact { 22.0 } else { 26.0 })
                .family(crate::ui::theme::semibold())
                .color(TEXT_PRIMARY),
        );
    })
    .response
    .interact(Sense::click())
    .on_hover_text("Open Today")
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn icon() -> egui::IconData {
    static ICON: OnceLock<egui::IconData> = OnceLock::new();
    ICON.get_or_init(|| {
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/anni.png"))
            .expect("valid Anni icon")
    })
    .clone()
}
