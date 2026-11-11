use crate::ui::theme::*;
use crate::{
    locale::{self, t},
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
        ui.label(RichText::new(t("INNA connection", "Tenging við INNU")).size(15.0).family(crate::ui::theme::semibold()));
        badge(ui, state.connection.label(), state.connection.color());
        ui.add_space(10.0);
        if let Some(user) = &state.user {
            ui.label(tr!("Student: {}", "Nemandi: {}", user.display_name()));
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
                            primary_button(t("Connect to INNA", "Tengjast INNU")),
                        )
                        .clicked()
                    {
                        action = ConnectionAction::ReconnectLocal;
                    }
                });
                ui.label(RichText::new(t("Sign in once using Google or your usual INNA method. On startup Anni reuses your session, or tries your remembered Google sign-in in the background. Connect here if your account needs verification.", "Skráðu þig inn einu sinni með Google eða á þinn venjulega hátt í INNU. Við ræsingu notar Anni lotuna aftur eða reynir vistuðu Google-innskráninguna í bakgrunni. Tengstu hér ef staðfesta þarf aðganginn.")).size(13.0).color(TEXT_SECONDARY));
                ui.collapsing(t("Other sign-in options", "Aðrar innskráningarleiðir"), |ui| {
                    if ui
                        .add_enabled(
                            !state.refreshing,
                            egui::Button::new(t("Connect with your phone", "Tengjast með símanum")),
                        )
                        .clicked()
                    {
                        action = ConnectionAction::ReconnectPhone;
                    }
                });
            }
            ConnectionState::Connected | ConnectionState::MockMode => {
                if ui
                    .add_enabled(!state.refreshing, egui::Button::new(t("Disconnect INNA", "Aftengja INNU")))
                    .clicked()
                {
                    action = ConnectionAction::Disconnect;
                }
            }
            _ => {}
        }
        if state.connection != ConnectionState::MockMode {
            ui.collapsing(t("Forget sign-in or change account", "Gleyma innskráningu eða skipta um aðgang"), |ui| {
                ui.label(t("This signs Anni out and removes the login remembered in its sign-in window, including Google. Your schoolwork and checkmarks are kept. Your normal browser is unaffected.", "Þetta skráir Anni út og fjarlægir innskráninguna sem innskráningarglugginn man, líka Google. Skólavinnan og hökin haldast. Venjulegi vafrinn þinn verður ekki fyrir áhrifum."));
                if ui.add_enabled(!state.refreshing && state.connection != ConnectionState::Authenticating, egui::Button::new(t("Forget remembered sign-in", "Gleyma vistaðri innskráningu"))).clicked() { action = ConnectionAction::ForgetSignIn; }
            });
        }
    });
    card().show(ui,|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("Saved data", "Vistuð gögn")).size(15.0).family(crate::ui::theme::semibold()));
        ui.label(t("Anni is an independent, unofficial app. Schoolwork checkmarks are saved on this device, separately for each INNA account.", "Anni er sjálfstætt, óopinbert forrit. Hök við skólavinnu eru vistuð á þessu tæki, sér fyrir hvern INNU-aðgang."));
        ui.add_space(10.0);
        if ui.add_enabled(!state.refreshing,egui::Button::new(RichText::new(t("Clear downloaded INNA data", "Eyða niðurhöluðum INNU-gögnum")).color(WARNING))).clicked(){
            state.timetable.clear();state.announcements.clear();state.assignments.clear();state.homework.clear();state.todo_items.clear();state.last_sync.clear();state.sync_errors.clear();state.storage_dirty=true;
        }
        ui.label(RichText::new(t("Your schoolwork checkmarks are kept. INNA data downloads again on the next refresh.", "Hökin við skólavinnuna haldast. INNU-gögn sækjast aftur við næstu uppfærslu.")).size(13.0).color(TEXT_SECONDARY));
    });
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("Last sync", "Síðasta uppfærsla")).size(15.0).family(crate::ui::theme::semibold()));
        if state.last_sync.is_empty() {
            ui.label(t("No data has been downloaded yet.", "Engin gögn hafa verið sótt enn."));
        }
        let mut resources: Vec<_> = state.last_sync.iter().collect();
        resources.sort_by_key(|(name, _)| *name);
        for (resource, ts) in resources {
            let date = chrono::DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|t| {
                    tr!(
                        "{} at {}",
                        "{} kl. {}",
                        locale::short_date(t.date_naive()),
                        t.format("%H:%M")
                    )
                })
                .unwrap_or_else(|| locale::t("Unknown date", "Óþekkt dagsetning").into());
            ui.horizontal(|ui| {
                ui.label(RichText::new(locale::resource(resource)).family(crate::ui::theme::semibold()));
                ui.label(RichText::new(date).color(TEXT_SECONDARY));
            });
        }
        if !state.sync_errors.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new(t("Refresh problems", "Vandamál við uppfærslu")).family(crate::ui::theme::semibold()).color(WARNING));
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
