use crate::ui::theme::*;
use crate::{
    locale,
    state::{AppState, ConnectionState},
};
use egui::{RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionAction {
    None,
    ReconnectBrowser,
    ReconnectLocal,
    ReconnectPhone,
    Disconnect,
}

pub fn show(ui: &mut Ui, state: &mut AppState) -> ConnectionAction {
    let mut action = ConnectionAction::None;
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            super::brand::wordmark(ui, false);
            badge(ui, "Your INNA overview", ACCENT);
        });
        ui.label("Anni brings your timetable, assignments, and grades together. It is an independent, unofficial app.");
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            badge(ui, "Always dark", ACCENT);
        });
        ui.label(
            RichText::new("A consistent dark interface, regardless of your system theme.")
                .color(TEXT_SECONDARY),
        );
    });
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("INNA connection").size(20.0).strong());
        badge(ui, state.connection.label(), state.connection.color());
        ui.add_space(10.0);
        if let Some(user) = &state.user {
            ui.label(format!("Student: {}", user.display_name()));
            if let Some(school) = &user.school_long {
                ui.label(RichText::new(school).color(TEXT_SECONDARY));
            }
        }
        ui.add_space(12.0);
        match &state.connection {
            ConnectionState::Starting
            | ConnectionState::SessionExpired
            | ConnectionState::Offline
            | ConnectionState::Error(_) => {
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!state.refreshing, primary_button("Connect in browser")).clicked() {
                        action = ConnectionAction::ReconnectBrowser;
                    }
                    if ui
                        .add_enabled(
                            !state.refreshing,
                            egui::Button::new("Use separate sign-in window"),
                        )
                        .clicked()
                    {
                        action = ConnectionAction::ReconnectLocal;
                    }
                    if ui
                        .add_enabled(
                            !state.refreshing,
                            egui::Button::new("Connect with your phone"),
                        )
                        .clicked()
                    {
                        action = ConnectionAction::ReconnectPhone;
                    }
                });
                ui.label(RichText::new("Connect in browser uses your usual Chrome or Edge login with a one-time Anni Connect extension setup.").size(13.0).color(TEXT_SECONDARY));
            }
            ConnectionState::Connected | ConnectionState::MockMode => {
                if ui
                    .add_enabled(!state.refreshing, egui::Button::new("Disconnect INNA"))
                    .clicked()
                {
                    action = ConnectionAction::Disconnect;
                }
            }
            _ => {}
        }
    });
    card().show(ui,|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Saved data").size(20.0).strong());
        ui.label("Personal tasks and checkmarks are saved on this device, with a separate plan for each INNA account.");
        ui.add_space(10.0);
        if ui.add_enabled(!state.refreshing,egui::Button::new(RichText::new("Clear downloaded INNA data").color(WARNING))).clicked(){
            state.timetable.clear();state.announcements.clear();state.assignments.clear();state.homework.clear();state.todo_items.clear();state.last_sync.clear();state.sync_errors.clear();state.storage_dirty=true;
        }
        ui.label(RichText::new("Your personal tasks and checkmarks are kept. INNA data downloads again on the next refresh.").size(13.0).color(TEXT_SECONDARY));
    });
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Last sync").size(20.0).strong());
        if state.last_sync.is_empty() {
            ui.label("No data has been downloaded yet.");
        }
        let mut resources: Vec<_> = state.last_sync.iter().collect();
        resources.sort_by_key(|(name, _)| *name);
        for (resource, ts) in resources {
            let date = chrono::DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|t| {
                    format!(
                        "{} at {}",
                        locale::short_date(t.date_naive()),
                        t.format("%H:%M")
                    )
                })
                .unwrap_or_else(|| "Unknown date".into());
            ui.horizontal(|ui| {
                ui.label(RichText::new(locale::resource(resource)).strong());
                ui.label(RichText::new(date).color(TEXT_SECONDARY));
            });
        }
        if !state.sync_errors.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("Refresh problems").strong().color(WARNING));
            for (resource, error) in &state.sync_errors {
                ui.label(format!(
                    "{}: {}",
                    locale::resource(resource),
                    locale::error(error)
                ));
            }
        }
    });
    action
}
