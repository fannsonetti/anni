use egui::{Color32, FontId, Rounding, Stroke, TextStyle, Visuals};

pub const BG_DARK: Color32 = Color32::from_rgb(15, 18, 25);
pub const PANEL_BG: Color32 = Color32::from_rgb(23, 28, 37);
pub const CARD_BG: Color32 = Color32::from_rgb(26, 31, 41);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(235, 240, 247);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(164, 176, 180);
pub const TEXT_DIM: Color32 = Color32::from_rgb(137, 150, 170);
pub const ACCENT: Color32 = Color32::from_rgb(135, 226, 198);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(12, 39, 33);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(31, 52, 52);
pub const VIOLET: Color32 = Color32::from_rgb(180, 165, 250);
pub const WARNING: Color32 = Color32::from_rgb(245, 195, 113);
pub const ERROR_RED: Color32 = Color32::from_rgb(251, 144, 156);
pub const SUCCESS: Color32 = ACCENT;
pub const NOW_HIGHLIGHT: Color32 = Color32::from_rgb(22, 46, 41);
pub const NEXT_HIGHLIGHT: Color32 = Color32::from_rgb(28, 35, 51);
pub const BORDER: Color32 = Color32::from_rgb(43, 51, 65);
pub const NAV_BG: Color32 = Color32::from_rgb(19, 23, 31);

pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).strong().color(ACCENT_TEXT))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
}

pub fn card() -> egui::Frame {
    egui::Frame::none()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(12.0)
        .inner_margin(18.0)
        .outer_margin(egui::Margin::symmetric(0.0, 5.0))
}

pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.12))
        .rounding(20.0)
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).strong().color(color));
        });
}

pub fn page_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(28.0)
            .strong()
            .color(TEXT_PRIMARY),
    );
    ui.label(
        egui::RichText::new(subtitle)
            .size(15.0)
            .color(TEXT_SECONDARY),
    );
    ui.add_space(16.0);
}

pub fn apply_theme(ctx: &egui::Context) {
    // Explicit preference also covers OS theme changes and restored egui memory.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(windows)]
    if let Ok(data) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts
            .font_data
            .insert("segoe".into(), egui::FontData::from_owned(data));
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "segoe".into());
    }
    let mut clock_fonts = fonts.families[&egui::FontFamily::Proportional].clone();
    #[cfg(windows)]
    if let Ok(data) = std::fs::read("C:/Windows/Fonts/segoeuil.ttf") {
        fonts
            .font_data
            .insert("segoe_light".into(), egui::FontData::from_owned(data));
        clock_fonts.insert(0, "segoe_light".into());
    }
    #[cfg(windows)]
    if let Ok(data) = std::fs::read("C:/Windows/Fonts/bahnschrift.ttf") {
        fonts
            .font_data
            .insert("bahnschrift".into(), egui::FontData::from_owned(data));
        clock_fonts.insert(0, "bahnschrift".into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("desktop_clock".into()), clock_fonts);
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(28.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(14.0));
    style
        .text_styles
        .insert(TextStyle::Name("Large".into()), FontId::proportional(36.0));
    style.text_styles.insert(
        TextStyle::Name("CourseCode".into()),
        FontId::proportional(23.0),
    );
    style.spacing.button_padding = egui::vec2(14.0, 10.0);
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.interact_size = egui::vec2(36.0, 36.0);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_DARK;
    visuals.window_fill = CARD_BG;
    visuals.faint_bg_color = PANEL_BG;
    visuals.extreme_bg_color = BG_DARK;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_rounding = Rounding::same(14.0);
    visuals.menu_rounding = Rounding::same(10.0);
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR_RED;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.bg_fill = CARD_BG;
    visuals.widgets.inactive.weak_bg_fill = CARD_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = ACCENT_DIM;
    visuals.widgets.hovered.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.rounding = Rounding::same(8.0);
        widget.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    }
    style.visuals = visuals;
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    #[test]
    fn dark_preference_overrides_saved_light_and_system_theme() {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::ThemePreference::Light);
        super::apply_theme(&ctx);
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_| {},
        );
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Dark
        );
        assert!(ctx.style().visuals.dark_mode);
        assert_eq!(ctx.style().visuals.panel_fill, super::BG_DARK);
    }
}
