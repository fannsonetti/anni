use crate::{desktop_platform as platform, settings::DesktopSettings, ui};
use egui::{RichText, ViewportCommand, ViewportId};

pub fn widget_ids() -> [ViewportId; 4] {
    ["desktop_work", "desktop_schedule", "desktop_clock", "desktop_progress"]
        .map(ViewportId::from_hash_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Page;

    fn run_action(
        action: DesktopAction,
        hidden: bool,
    ) -> (Desktop, crate::state::AppState, egui::FullOutput) {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let mut desktop = Desktop::new(true, hidden);
        desktop.settings.enabled = true;
        let mut state = crate::state::AppState::new();
        state.navigate(Page::Planner);
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            for id in widget_ids() {
                ctx.show_viewport_deferred(id, egui::ViewportBuilder::default(), |_, _| {});
            }
            desktop.handle(action, ctx, &mut state);
        });
        (desktop, state, output)
    }

    #[test]
    fn hide_widgets_keeps_the_current_page_and_does_not_reveal_the_planner() {
        for hidden in [false, true] {
            let (desktop, state, output) = run_action(DesktopAction::Disable, hidden);
            assert!(!desktop.settings.enabled);
            assert_eq!(desktop.planner_hidden, hidden);
            assert_eq!(state.selected_page, Page::Planner);
            assert!(!output.viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| {
                    matches!(
                        command,
                        ViewportCommand::Visible(true) | ViewportCommand::Focus
                    )
                }));
            for id in widget_ids() {
                assert!(output.viewport_output[&id]
                    .commands
                    .contains(&ViewportCommand::Visible(false)));
            }
        }
    }

    #[test]
    fn quitting_hidden_planner_wakes_the_close_event_and_hides_every_widget() {
        let (desktop, _, output) = run_action(DesktopAction::Quit, true);
        assert!(desktop.quit_requested);
        let commands = &output.viewport_output[&ViewportId::ROOT].commands;
        assert!(commands.contains(&ViewportCommand::Close));
        assert!(commands.contains(&ViewportCommand::Visible(true)));
        assert!(!commands.contains(&ViewportCommand::CancelClose));
        assert!(!commands.contains(&ViewportCommand::Focus));
        for id in widget_ids() {
            assert!(output.viewport_output[&id]
                .commands
                .contains(&ViewportCommand::Visible(false)));
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum DesktopAction {
    #[default]
    None,
    HidePlanner,
    Disable,
    Quit,
}

pub struct Desktop {
    pub settings: DesktopSettings,
    pub startup: bool,
    pub error: Option<String>,
    pub quit_requested: bool,
    pub planner_hidden: bool,
    saved: DesktopSettings,
    observed: DesktopSettings,
    save_after: Option<std::time::Instant>,
    hide_on_start: u8,
    last_arrange: Option<std::time::Instant>,
    foreground_watcher: Option<platform::ForegroundWatcher>,
}

impl Desktop {
    pub fn new(mock: bool, desktop_only: bool) -> Self {
        let (mut settings, mut error) = if mock {
            (DesktopSettings::default(), None)
        } else {
            match DesktopSettings::load() {
                Ok(settings) => (settings, None),
                Err(error) => (DesktopSettings::default(), Some(error)),
            }
        };
        if desktop_only {
            settings.enabled = true;
        }
        let startup = if mock {
            false
        } else {
            match platform::startup_enabled() {
                Ok(enabled) => enabled,
                Err(message) => {
                    error = Some(message);
                    false
                }
            }
        };
        Self {
            saved: settings.clone(),
            observed: settings.clone(),
            save_after: None,
            settings,
            startup,
            error,
            quit_requested: false,
            planner_hidden: desktop_only,
            hide_on_start: if desktop_only { 2 } else { 0 },
            last_arrange: None,
            foreground_watcher: None,
        }
    }

    pub fn handle(
        &mut self,
        action: DesktopAction,
        ctx: &egui::Context,
        _state: &mut crate::state::AppState,
    ) {
        match action {
            DesktopAction::None => {}
            DesktopAction::HidePlanner => {
                crate::diagnostics::record(crate::diagnostics::Event::PlannerHidden);
                self.planner_hidden = true;
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
            }
            DesktopAction::Disable => {
                self.hide_on_start = 0;
                self.settings.enabled = false;
                for id in widget_ids() {
                    ctx.send_viewport_cmd_to(id, ViewportCommand::Visible(false));
                }
                ctx.request_repaint_of(ViewportId::ROOT);
            }
            DesktopAction::Quit => {
                crate::diagnostics::record(crate::diagnostics::Event::Quit);
                self.quit_requested = true;
                self.hide_on_start = 0;
                for id in widget_ids() {
                    ctx.send_viewport_cmd_to(id, ViewportCommand::Visible(false));
                }
                // On Windows a hidden window does not receive the redraw that
                // consumes eframe's queued Close event. Wake it outside the
                // desktop so eframe can shut down
                // normally (saving state and releasing the single-instance lock).
                if self.planner_hidden {
                    ctx.send_viewport_cmd_to(
                        ViewportId::ROOT,
                        ViewportCommand::OuterPosition(egui::pos2(-32000.0, -32000.0)),
                    );
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
                }
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
            }
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context, mock: bool) {
        // eframe reveals the root after the first paint. Repeat on the next frame.
        if self.hide_on_start > 0 {
            self.planner_hidden = true;
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
            self.hide_on_start -= 1;
            ctx.request_repaint();
        }
        if self.settings.enabled && !self.quit_requested {
            if self.foreground_watcher.is_none() {
                self.foreground_watcher = Some(platform::ForegroundWatcher::new());
            }
            if self
                .last_arrange
                .is_none_or(|last| last.elapsed().as_secs() >= 2)
            {
                platform::arrange_widgets();
                self.last_arrange = Some(std::time::Instant::now());
            }
        } else {
            self.foreground_watcher = None;
        }
        let now = std::time::Instant::now();
        if self.observed != self.settings {
            self.observed = self.settings.clone();
            self.save_after = Some(now + std::time::Duration::from_millis(350));
            for id in widget_ids() {
                ctx.request_repaint_of(id);
            }
            ctx.request_repaint_of(ViewportId::ROOT);
        }
        if self.saved != self.settings {
            let wait = self
                .save_after
                .unwrap_or(now)
                .saturating_duration_since(now);
            if !mock && !self.quit_requested && self.settings.enabled && !wait.is_zero() {
                ctx.request_repaint_after(wait);
                return;
            }
            if mock {
                self.saved = self.settings.clone();
            } else {
                match self.settings.save() {
                    Ok(()) => {
                        self.saved = self.settings.clone();
                        self.error = None;
                    }
                    Err(error) => {
                        self.error = Some(error);
                        // A storage failure must not turn into another hot render loop.
                        self.save_after = Some(now + std::time::Duration::from_secs(5));
                        ctx.request_repaint_after(std::time::Duration::from_secs(5));
                    }
                }
            }
        }
    }

    pub fn settings_ui(&mut self, ui: &mut egui::Ui, mock: bool) -> DesktopAction {
        use crate::settings::{ClockStyle, DesktopAccent, DesktopSide, DesktopTheme};
        use ui::theme::*;
        let mut action = DesktopAction::None;

        section(ui, "General", |ui| {
            if cfg!(windows) {
                let mut startup = self.startup;
                setting_row(ui, "Start with Windows", |ui| {
                    if ui.add_enabled(!mock, egui::Checkbox::new(&mut startup, "")).changed() {
                        match platform::set_startup(startup) {
                            Ok(()) => {
                                self.startup = startup;
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                });
            }
            let has_widgets = self.settings.enabled
                && (self.settings.show_schedule
                    || self.settings.show_homework
                    || self.settings.show_clock
                    || self.settings.show_progress);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(has_widgets, primary_button("Show desktop widgets")).clicked() {
                    action = DesktopAction::HidePlanner;
                }
                if ui.button("Quit Anni").clicked() {
                    action = DesktopAction::Quit;
                }
            });
            hint(ui, "Closing the window quits Anni and its widgets. Use Show desktop widgets to leave them running, then open Anni from its shortcut to return.");
            if mock {
                ui.label(RichText::new("Demo mode · preferences are temporary and startup is disabled.").size(12.0).color(WARNING));
            }
            if let Some(error) = &self.error {
                ui.colored_label(WARNING, error);
            }
        });

        let settings = &mut self.settings;
        section(ui, "Appearance", |ui| {
            ui.label("Theme");
            ui.add_space(2.0);
            ui.columns(DesktopTheme::ALL.len(), |columns| {
                for (column, theme) in columns.iter_mut().zip(DesktopTheme::ALL) {
                    if swatch(column, theme, settings.theme == theme) {
                        settings.select_theme(theme);
                    }
                }
            });
            ui.add_space(6.0);
            setting_row(ui, "Accent", |ui| {
                egui::ComboBox::from_id_salt("accent")
                    .width(200.0)
                    .selected_text(accent_label(settings.accent))
                    .show_ui(ui, |ui| {
                        for accent in [DesktopAccent::Theme, DesktopAccent::Ice, DesktopAccent::Mint, DesktopAccent::Lavender, DesktopAccent::White, DesktopAccent::Custom] {
                            ui.selectable_value(&mut settings.accent, accent, accent_label(accent));
                        }
                    });
            });
            if settings.accent == DesktopAccent::Custom {
                setting_row(ui, "Custom accent", |ui| {
                    ui.color_edit_button_srgb(&mut settings.custom_accent);
                });
            }
            setting_row(ui, "Text size", |ui| {
                ui.add(egui::Slider::new(&mut settings.text_scale, 85..=140).suffix(" %"));
            });
            setting_row(ui, "Background opacity", |ui| {
                ui.add(egui::Slider::new(&mut settings.panel_opacity, 0..=80).suffix(" %"));
            });
            setting_row(ui, "Corner rounding", |ui| {
                ui.add(egui::Slider::new(&mut settings.corner_radius, 0..=24).suffix(" px"));
            });
            setting_check(ui, "Widget borders", &mut settings.show_borders);
        });

        section(ui, "Desktop widgets", |ui| {
            setting_check(ui, "Show desktop widgets", &mut settings.enabled);
            setting_check(ui, "Schedule", &mut settings.show_schedule);
            setting_check(ui, "Assignments", &mut settings.show_homework);
            setting_check(ui, "Day progress bar", &mut settings.show_progress);
            setting_check(ui, "Clock", &mut settings.show_clock);
            setting_row(ui, "Assignments side", |ui| {
                egui::ComboBox::from_id_salt("side")
                    .width(200.0)
                    .selected_text(if settings.side == DesktopSide::Left { "Left" } else { "Right" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.side, DesktopSide::Left, "Left");
                        ui.selectable_value(&mut settings.side, DesktopSide::Right, "Right");
                    });
            });
            setting_row(ui, "Width", |ui| {
                ui.add(egui::Slider::new(&mut settings.panel_width, 280..=480).suffix(" px"));
            });
            setting_row(ui, "Height", |ui| {
                ui.add(egui::Slider::new(&mut settings.panel_height, 360..=800).suffix(" px"));
            });
            setting_row(ui, "Vertical position", |ui| {
                ui.add(egui::Slider::new(&mut settings.vertical_position, 0..=100).suffix(" %"));
            });
            setting_row(ui, "Row spacing", |ui| {
                ui.add(egui::Slider::new(&mut settings.row_spacing, 4..=24).suffix(" px"));
            });
        });

        section(ui, "Assignments", |ui| {
            setting_check(ui, "Exams (Próf)", &mut settings.include_exams);
            setting_check(ui, "INNA reminders", &mut settings.include_reminders);
            setting_check(ui, "Course codes", &mut settings.show_course_codes);
            setting_check(ui, "Deadlines", &mut settings.show_deadlines);
            hint(ui, "Click an assignment on the desktop to cross it off. It hides a day later; click it again to undo.");
        });

        section(ui, "Clock", |ui| {
            setting_row(ui, "Style", |ui| {
                let label = |style| match style {
                    ClockStyle::Reactor => "Digital with ring",
                    ClockStyle::Digital => "Minimal digital",
                    ClockStyle::Analog => "Analog",
                };
                egui::ComboBox::from_id_salt("clock_style")
                    .width(200.0)
                    .selected_text(label(settings.clock_style))
                    .show_ui(ui, |ui| {
                        for style in [ClockStyle::Digital, ClockStyle::Reactor, ClockStyle::Analog] {
                            ui.selectable_value(&mut settings.clock_style, style, label(style));
                        }
                    });
            });
            setting_row(ui, "Size", |ui| {
                ui.add(egui::Slider::new(&mut settings.clock_scale, 80..=180).suffix(" %"));
            });
            setting_check(ui, "Date", &mut settings.show_date);
            setting_check(ui, "Seconds", &mut settings.show_seconds);
        });
        if ui.button("Reset appearance").clicked() {
            *settings = DesktopSettings { enabled: settings.enabled, ..Default::default() };
        }
        ui.add_space(12.0);
        action
    }
}

fn accent_label(accent: crate::settings::DesktopAccent) -> &'static str {
    use crate::settings::DesktopAccent::*;
    match accent {
        Theme => "Theme",
        Ice => "Ice",
        Mint => "Mint",
        Lavender => "Lavender",
        White => "White",
        Custom => "Custom",
    }
}

/// Hermes' tiny theme preview: its background with the accent and deadline colours.
fn swatch(ui: &mut egui::Ui, theme: crate::settings::DesktopTheme, selected: bool) -> bool {
    use egui::{pos2, vec2, Align2, Color32, FontId, Sense, Stroke};
    let palette = theme.palette();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 66.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 8.0, ui::theme::CONTROL);
    let band = egui::Rect::from_min_size(rect.min, vec2(rect.width(), 40.0));
    p.rect_filled(
        band,
        egui::Rounding { nw: 8.0, ne: 8.0, sw: 0.0, se: 0.0 },
        palette.background,
    );
    let center = band.center();
    p.circle_filled(pos2(center.x - 11.0, center.y), 6.0, palette.accent);
    p.circle_filled(pos2(center.x + 4.0, center.y), 3.5, palette.urgent);
    p.circle_filled(pos2(center.x + 14.0, center.y), 3.5, palette.soon);
    p.text(
        pos2(rect.center().x, rect.bottom() - 13.0),
        Align2::CENTER_CENTER,
        theme.short_label(),
        FontId::proportional(11.0),
        Color32::WHITE,
    );
    let ring = if selected { palette.accent } else { ui::theme::BORDER };
    p.rect_stroke(rect, 8.0, Stroke::new(if selected { 2.0 } else { 1.0 }, ring));
    response
        .on_hover_text(theme.description())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
