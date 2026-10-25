//! The same monogram is used in the planner, native windows, and Windows resources.
use super::theme::TEXT_PRIMARY;
use egui::{RichText, Sense, Ui, Vec2};
use std::sync::OnceLock;

pub const NAME: &str = "anni";

pub fn wordmark(ui: &mut Ui, compact: bool) -> egui::Response {
    let size = if compact { 32.0 } else { 42.0 };
    ui.horizontal(|ui| {
        let texture_id = ui.id().with("anni_brand_texture");
        let texture = ui
            .ctx()
            .data(|data| data.get_temp::<egui::TextureHandle>(texture_id))
            .unwrap_or_else(|| {
                let icon = icon();
                let texture = ui.ctx().load_texture(
                    "anni",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [icon.width as usize, icon.height as usize],
                        &icon.rgba,
                    ),
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx()
                    .data_mut(|data| data.insert_temp(texture_id, texture.clone()));
                texture
            });
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.label(
            RichText::new(NAME)
                .size(if compact { 26.0 } else { 33.0 })
                .strong()
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
