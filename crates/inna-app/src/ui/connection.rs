use crate::ui::theme::*;
use crate::{
    locale,
    state::{AppState, ConnectionState},
};
use egui::{RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionAction {
    None,
    ReconnectLocal,
    ReconnectPhone,
    Disconnect,
    ForgetSignIn,
}

pub fn show(ui: &mut Ui, state: &mut AppState) -> ConnectionAction {
    let mut action = ConnectionAction::None;
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("INNA connection").size(15.0).family(crate::ui::theme::semibold()));
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
                    if ui
                        .add_enabled(
                            !state.refreshing,
                            primary_button("Connect to INNA"),
                        )
                        .clicked()
                    {
                        action = ConnectionAction::ReconnectLocal;
                    }
                });
                ui.label(RichText::new("Sign in once using Google or your usual INNA method. On startup Anni reuses your session, or tries your remembered Google sign-in in the background. Connect here if your account needs verification.").size(13.0).color(TEXT_SECONDARY));
                ui.collapsing("Other sign-in options", |ui| {
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
        if state.connection != ConnectionState::MockMode {
            ui.collapsing("Forget sign-in or change account", |ui| {
                ui.label("This signs Anni out and removes the login remembered in its sign-in window, including Google. Your schoolwork and checkmarks are kept. Your normal browser is unaffected.");
                if ui.add_enabled(!state.refreshing && state.connection != ConnectionState::Authenticating, egui::Button::new("Forget remembered sign-in")).clicked() { action = ConnectionAction::ForgetSignIn; }
            });
        }
    });
    card().show(ui,|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Saved data").size(15.0).family(crate::ui::theme::semibold()));
        ui.label("Anni is an independent, unofficial app. Schoolwork checkmarks are saved on this device, separately for each INNA account.");
        ui.add_space(10.0);
        if ui.add_enabled(!state.refreshing,egui::Button::new(RichText::new("Clear downloaded INNA data").color(WARNING))).clicked(){
            state.timetable.clear();state.announcements.clear();state.assignments.clear();state.homework.clear();state.todo_items.clear();state.last_sync.clear();state.sync_errors.clear();state.storage_dirty=true;
        }
        ui.label(RichText::new("Your schoolwork checkmarks are kept. INNA data downloads again on the next refresh.").size(13.0).color(TEXT_SECONDARY));
    });
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Last sync").size(15.0).family(crate::ui::theme::semibold()));
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
                ui.label(RichText::new(locale::resource(resource)).family(crate::ui::theme::semibold()));
                ui.label(RichText::new(date).color(TEXT_SECONDARY));
            });
        }
        if !state.sync_errors.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("Refresh problems").family(crate::ui::theme::semibold()).color(WARNING));
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
