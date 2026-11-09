use egui::{Color32, FontId, Rounding, Stroke, TextStyle, Visuals};

// A neutral palette: hierarchy comes from spacing, type, and explicit status labels.
pub const BG_DARK: Color32 = Color32::from_gray(16);
pub const PANEL_BG: Color32 = Color32::from_gray(22);
pub const CARD_BG: Color32 = Color32::from_gray(20);
pub const TEXT_PRIMARY: Color32 = Color32::from_gray(240);
pub const TEXT_SECONDARY: Color32 = Color32::from_gray(175);
pub const TEXT_DIM: Color32 = Color32::from_gray(145);
pub const ACCENT: Color32 = Color32::from_gray(240);
pub const ACCENT_TEXT: Color32 = Color32::from_gray(16);
pub const ACCENT_DIM: Color32 = Color32::from_gray(36);
pub const VIOLET: Color32 = TEXT_SECONDARY;
pub const WARNING: Color32 = TEXT_PRIMARY;
pub const ERROR_RED: Color32 = TEXT_PRIMARY;
pub const SUCCESS: Color32 = TEXT_SECONDARY;
pub const NOW_HIGHLIGHT: Color32 = Color32::from_gray(27);
pub const NEXT_HIGHLIGHT: Color32 = Color32::from_gray(24);
pub const BORDER: Color32 = Color32::from_gray(43);
pub const CONTROL: Color32 = Color32::from_gray(28);
pub const CONTROL_BORDER: Color32 = Color32::from_gray(65);
pub const NAV_BG: Color32 = BG_DARK;

pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(
        egui::RichText::new(text)
            .family(semibold())
            .color(ACCENT_TEXT),
    )
    .fill(ACCENT)
    .stroke(Stroke::NONE)
}

/// Quiet surfaces with a fine border and minimal rounding.
pub fn card() -> egui::Frame {
    egui::Frame::none()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(2.0)
        .inner_margin(egui::Margin::symmetric(18.0, 14.0))
        .outer_margin(egui::Margin {
            bottom: 12.0,
            ..Default::default()
        })
}

pub fn semibold() -> egui::FontFamily {
    egui::FontFamily::Name("desktop_strong".into())
}

/// A settings section with a simple title.
pub fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card()
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(15.0)
                    .family(semibold())
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(6.0);
            add(ui)
        })
        .inner
}

/// A label on the left and its control in a fixed column on the right.
pub fn setting_row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_max_width(210.0);
            ui.spacing_mut().slider_width = 130.0;
            add(ui);
        });
    });
    ui.add_space(4.0);
}

pub fn setting_check(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    setting_row(ui, label, |ui| {
        ui.checkbox(value, "");
    });
}

pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(TEXT_SECONDARY));
}

pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(0.0, 3.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(12.0)
                    .family(semibold())
                    .color(color),
            );
        });
}

pub fn page_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(26.0)
            .family(semibold())
            .color(egui::Color32::WHITE),
    );
    ui.label(
        egui::RichText::new(subtitle)
            .size(13.0)
            .color(TEXT_SECONDARY),
    );
    ui.add_space(14.0);
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
    let mut strong_fonts = clock_fonts.clone();
    #[cfg(windows)]
    if let Ok(data) = std::fs::read("C:/Windows/Fonts/seguisb.ttf") {
        fonts
            .font_data
            .insert("segoe_semibold".into(), egui::FontData::from_owned(data));
        strong_fonts.insert(0, "segoe_semibold".into());
    }
    fonts.families.insert(
        egui::FontFamily::Name("desktop_strong".into()),
        strong_fonts,
    );
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
        .insert(TextStyle::Heading, FontId::proportional(26.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
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
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.interact_size = egui::vec2(32.0, 30.0);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_DARK;
    visuals.window_fill = CARD_BG;
    visuals.faint_bg_color = PANEL_BG;
    visuals.extreme_bg_color = BG_DARK;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_rounding = Rounding::same(2.0);
    visuals.menu_rounding = Rounding::same(2.0);
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR_RED;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.slider_trailing_fill = true;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.bg_fill = CONTROL;
    visuals.widgets.inactive.weak_bg_fill = CONTROL;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
    visuals.widgets.hovered.bg_fill = CONTROL_BORDER;
    visuals.widgets.hovered.weak_bg_fill = CONTROL_BORDER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
    visuals.widgets.active.bg_fill = CONTROL_BORDER;
    visuals.widgets.active.weak_bg_fill = CONTROL_BORDER;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.rounding = Rounding::same(2.0);
        widget.expansion = 0.0;
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
