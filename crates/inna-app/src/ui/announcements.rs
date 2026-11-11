use crate::ui::theme::*;
use crate::{locale::{self, t}, state::AppState};
use egui::{RichText, Ui};

fn date_label(date: &str) -> String {
    crate::planner::parse_deadline(date)
        .map(|d| tr!("{} at {}", "{} kl. {}", locale::date(d.at.date()), d.at.format("%H:%M")))
        .unwrap_or_else(|| date.to_string())
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    if let Some(index) = state.selected_announcement {
        if let Some(ann) = state.announcements.get(index) {
            if ui.button(t("Back", "Til baka")).clicked() {
                state.go_back();
                return;
            }
            ui.add_space(12.0);
            card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                page_title(
                    ui,
                    ann.title_str(),
                    &format!("{} · {}", ann.module_str(), ann.sender_str()),
                );
                if let Some(date) = &ann.date {
                    ui.label(
                        RichText::new(date_label(date))
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                }
                ui.add_space(16.0);
                ui.label(ann.content_text());
            });
            return;
        } else {
            state.selected_announcement = None;
        }
    }
    page_title(
        ui,
        t("Announcements", "Tilkynningar"),
        t("Updates from your teachers and school.", "Fréttir frá kennurunum og skólanum."),
    );
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(
            egui::TextEdit::singleline(&mut state.announcement_search)
                .hint_text(t("Search announcements, courses, or teachers…", "Leita í tilkynningum, áföngum eða kennurum…"))
                .margin(egui::vec2(10.0, 8.0))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut state.announcements_unread_only,
                false,
                t("All announcements", "Allar tilkynningar"),
            );
            let unread = state
                .announcements
                .iter()
                .filter(|a| a.has_opened == Some(false))
                .count();
            ui.selectable_value(
                &mut state.announcements_unread_only,
                true,
                tr!("Unread in INNA ({unread})", "Ólesið í INNU ({unread})"),
            );
        });
    });
    let search = state.announcement_search.trim().to_lowercase();
    let mut found = 0;
    if state.announcements.is_empty() {
        card().show(ui, |ui| {
            ui.label(t("No announcements yet.", "Engar tilkynningar enn."));
        });
    }
    for (index, ann) in state.announcements.iter().enumerate() {
        if state.announcements_unread_only && ann.has_opened != Some(false) {
            continue;
        }
        if !search.is_empty()
            && !format!(
                "{} {} {} {}",
                ann.title_str(),
                ann.sender_str(),
                ann.module_str(),
                ann.content_text()
            )
            .to_lowercase()
            .contains(&search)
        {
            continue;
        }
        found += 1;
        let response = card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if ann.has_opened == Some(false) {
                badge(ui, t("Unread", "Ólesið"), ACCENT);
            }
            ui.label(RichText::new(ann.title_str()).size(21.0).family(crate::ui::theme::semibold()));
            ui.label(
                RichText::new(format!("{} · {}", ann.module_str(), ann.sender_str()))
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
            if let Some(date) = &ann.date {
                ui.label(RichText::new(date_label(date)).size(12.0).color(TEXT_DIM));
            }
            if let Some(short) = &ann.content_short {
                let mut preview = short.chars().take(160).collect::<String>();
                if short.chars().count() > 160 {
                    preview.push('…');
                }
                ui.add_space(8.0);
                ui.label(RichText::new(preview).size(15.0));
            }
            ui.add_space(8.0);
            ui.small_button(t("Read announcement", "Lesa tilkynningu")).clicked()
        });
        if response.inner || response.response.interact(egui::Sense::click()).clicked() {
            state.open_announcement(index);
            break;
        }
    }
    if found == 0 && !state.announcements.is_empty() {
        card().show(ui, |ui| {
            ui.label(
                RichText::new(t("No matching announcements", "Engar tilkynningar fundust"))
                    .size(20.0)
                    .family(crate::ui::theme::semibold()),
            );
            ui.label(
                RichText::new(t("Try another search or show all announcements.", "Prófaðu aðra leit eða sýndu allar tilkynningar."))
                    .color(TEXT_SECONDARY),
            );
            if ui.button(t("Clear filters", "Hreinsa síur")).clicked() {
                state.announcement_search.clear();
                state.announcements_unread_only = false;
            }
        });
    }
}
