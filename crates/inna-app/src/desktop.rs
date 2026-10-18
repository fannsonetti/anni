use crate::{desktop_platform as platform, settings::DesktopSettings, ui};
use egui::{RichText, ViewportCommand, ViewportId};

pub fn widget_ids() -> [ViewportId; 3] {
    ["desktop_work", "desktop_schedule", "desktop_clock"].map(ViewportId::from_hash_of)
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
        use ui::theme::*;
        let mut action = DesktopAction::None;
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Desktop").size(20.0).strong());
            ui.label(RichText::new("Your schedule on one side, homework on the other.").color(TEXT_SECONDARY));
            ui.add_space(10.0);
            ui::desktop::preview(ui, &self.settings);
            ui.add_space(10.0);
            ui.checkbox(&mut self.settings.enabled, "Show desktop widgets");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.settings.show_schedule, "Schedule");
                ui.checkbox(&mut self.settings.show_homework, "Homework");
                ui.checkbox(&mut self.settings.show_clock, "Clock");
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Homework position:");
                ui.selectable_value(&mut self.settings.side, crate::settings::DesktopSide::Left, "Left");
                ui.selectable_value(&mut self.settings.side, crate::settings::DesktopSide::Right, "Right");
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Theme").strong());
            ui.horizontal_wrapped(|ui| {
                for theme in crate::settings::DesktopTheme::ALL {
                    let selected = self.settings.theme == theme;
                    if ui.selectable_label(selected, RichText::new(theme.label()).color(theme.palette().accent)).clicked() {
                        self.settings.select_theme(theme);
                    }
                }
            });
            ui.label(RichText::new(self.settings.theme.description()).size(13.0).color(TEXT_SECONDARY));
            ui.checkbox(&mut self.settings.show_borders, "Show widget borders");
            ui.collapsing("Colors & background", |ui| {
                use crate::settings::DesktopAccent;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Accent:");
                    for (accent, label) in [(DesktopAccent::Theme, "Theme"), (DesktopAccent::Ice, "Ice"), (DesktopAccent::Mint, "Mint"), (DesktopAccent::Lavender, "Lavender"), (DesktopAccent::White, "White"), (DesktopAccent::Custom, "Custom")] {
                        ui.selectable_value(&mut self.settings.accent, accent, label);
                    }
                });
                if self.settings.accent == DesktopAccent::Custom {
                    ui.horizontal(|ui| {
                        ui.label("Custom accent");
                        ui.color_edit_button_srgb(&mut self.settings.custom_accent);
                    });
                }
                ui.add(egui::Slider::new(&mut self.settings.panel_opacity, 0..=80).suffix(" %").text("Background opacity"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings.panel_opacity, 0, "Clear");
                    ui.selectable_value(&mut self.settings.panel_opacity, 12, "Subtle");
                    ui.selectable_value(&mut self.settings.panel_opacity, 35, "Contrast");
                });
                ui.add(egui::Slider::new(&mut self.settings.corner_radius, 0..=24).suffix(" px").text("Corner rounding"));
            });
            ui.collapsing("Clock appearance", |ui| {
                ui.add(egui::Slider::new(&mut self.settings.clock_scale, 80..=180).suffix(" %").text("Clock size"));
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.settings.clock_style, crate::settings::ClockStyle::Reactor, "Minimal reactor");
                    ui.selectable_value(&mut self.settings.clock_style, crate::settings::ClockStyle::Digital, "Minimal digital");
                    ui.selectable_value(&mut self.settings.clock_style, crate::settings::ClockStyle::Analog, "Analog");
                });
                ui.checkbox(&mut self.settings.show_date, "Show date");
                ui.checkbox(&mut self.settings.show_seconds, "Show seconds");
            });
            ui.collapsing("Widget size & layout", |ui| {
                ui.add(egui::Slider::new(&mut self.settings.panel_width, 280..=480).suffix(" px").text("Width"));
                ui.add(egui::Slider::new(&mut self.settings.panel_height, 360..=800).suffix(" px").text("Height"));
                ui.add(egui::Slider::new(&mut self.settings.text_scale, 85..=140).suffix(" %").text("Text size"));
                ui.add(egui::Slider::new(&mut self.settings.vertical_position, 0..=100).suffix(" %").text("Vertical position"));
                ui.add(egui::Slider::new(&mut self.settings.row_spacing, 4..=24).suffix(" px").text("Row spacing"));
                ui.label(RichText::new("Sizes adapt to the space available on your display.").size(12.0).color(TEXT_SECONDARY));
            });
            ui.collapsing("Homework contents", |ui| {
                ui.label("Heimavinna and course assignments (Verkefni) appear by default.");
                ui.checkbox(&mut self.settings.include_exams, "Also show exams (Próf)");
                ui.checkbox(&mut self.settings.include_reminders, "Also show personal tasks and INNA reminders");
                ui.checkbox(&mut self.settings.show_course_codes, "Show course codes under tasks");
                ui.checkbox(&mut self.settings.show_deadlines, "Show task deadlines");
                ui.label(RichText::new("Manage task checkmarks in My work. Desktop widgets only display your plan.").size(12.0).color(TEXT_SECONDARY));
            });
            ui.add_space(6.0);
            if ui.button("Reset appearance").clicked() {
                self.settings = DesktopSettings { enabled: self.settings.enabled, ..Default::default() };
            }
            ui.label(RichText::new("Changes save automatically. Clear removes the background tint; widget borders are optional. Open Anni from its shortcut to change settings.").size(13.0).color(TEXT_SECONDARY));
            if cfg!(windows) {
                let mut startup = self.startup;
                if ui.add_enabled(!mock, egui::Checkbox::new(&mut startup, "Open desktop widgets when I sign in to Windows")).changed() {
                    match platform::set_startup(startup) {
                        Ok(()) => { self.startup = startup; self.error = None; }
                        Err(error) => self.error = Some(error),
                    }
                }
                ui.label(RichText::new("Your saved tasks appear immediately. INNA refreshes when you're connected.").size(13.0).color(TEXT_SECONDARY));
            }
            if mock { ui.label(RichText::new("Demo mode · desktop preferences are temporary and startup is disabled.").size(12.0).color(WARNING)); }
            if self.settings.enabled {
                ui.horizontal_wrapped(|ui| {
                    if ui.add(primary_button("Show desktop widgets")).clicked() { action = DesktopAction::HidePlanner; }
                    if ui.button("Quit Anni").clicked() { action = DesktopAction::Quit; }
                });
                ui.label(RichText::new("Closing the planner leaves your widgets running. Open Anni again from its shortcut to return to the planner.").size(12.0).color(TEXT_SECONDARY));
            }
            if let Some(error) = &self.error { ui.colored_label(WARNING, error); }
        });
        action
    }
}
