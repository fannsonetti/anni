//! A compact launcher window with real download progress and an animated scan bar.
use crate::locale::t;
use crate::{
    ui::{brand, theme::*},
    updates::Progress,
};
use egui::{Align, Align2, FontId, Layout, RichText, Sense, Stroke};
use std::time::Duration;

pub fn show(ctx: &egui::Context, progress: &Progress, elapsed: Duration, applying: bool) -> bool {
    let mut launch = false;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(PANEL_BG)
                .stroke(Stroke::new(1.0, BORDER))
                .rounding(2.0)
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
            ui.horizontal(|ui| {
                let logo = brand::wordmark(ui, true).interact(Sense::click_and_drag());
                if logo.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let preview = ctx.data(|data| {
                        data.get_temp::<bool>(egui::Id::new("update_preview"))
                            .unwrap_or(false)
                    });
                    ui.label(
                        RichText::new(if preview {
                            t("UPDATE PREVIEW", "SÝNISHORN UPPFÆRSLU")
                        } else {
                            t("AUTOMATIC UPDATES", "SJÁLFVIRKAR UPPFÆRSLUR")
                        })
                        .size(10.0)
                        .color(TEXT_DIM),
                    );
                });
            });
            ui.add_space(22.0);
            let (title, description, fraction, status) = match progress {
                Progress::Checking => (
                    t("Checking for updates", "Leita að uppfærslum"),
                    t("Getting the latest version of Anni.", "Sæki nýjustu útgáfu Anni.").into(),
                    None,
                    t("Connecting to release service", "Tengist útgáfuþjónustu").into(),
                ),
                Progress::Downloading {
                    version,
                    received,
                    total,
                } => (
                    t("A fresh Anni is on its way", "Nýtt Anni er á leiðinni"),
                    tr!("Downloading version {version}.", "Sæki útgáfu {version}."),
                    Some((*received as f32 / (*total).max(1) as f32).clamp(0.0, 1.0)),
                    format!(
                        "{:.1} / {:.1} MB",
                        *received as f64 / 1_048_576.0,
                        *total as f64 / 1_048_576.0
                    ),
                ),
                Progress::Verifying { version } => (
                    t("Checking the download", "Athuga niðurhalið"),
                    tr!("Verifying Anni {version} before installation.", "Staðfesti Anni {version} fyrir uppsetningu."),
                    None,
                    t("Verifying SHA-256 checksum", "Staðfesti SHA-256 gátsummu").into(),
                ),
                Progress::Staged(update) => (
                    t("Your update is ready", "Uppfærslan er tilbúin"),
                    tr!("Preparing to install Anni {}.", "Undirbý uppsetningu á Anni {}.", update.version),
                    Some(1.0),
                    t("Download verified", "Niðurhal staðfest").into(),
                ),
                Progress::Installing { version } => (
                    t("Making things a little better", "Gerir hlutina aðeins betri"),
                    tr!("Installing Anni {version}. It will open automatically.", "Set upp Anni {version}. Það opnast sjálfkrafa."),
                    None,
                    t("Installing update", "Set upp uppfærslu").into(),
                ),
                Progress::Current => (
                    t("You're up to date", "Þú ert með nýjustu útgáfu"),
                    t("Everything is ready. Opening Anni…", "Allt er tilbúið. Opna Anni…").into(),
                    Some(1.0),
                    t("Latest version installed", "Nýjasta útgáfan er uppsett").into(),
                ),
                Progress::Complete => (
                    t("Ready to go", "Allt klárt"),
                    t("The update is installed. Welcome back.", "Uppfærslan er uppsett. Gaman að sjá þig aftur.").into(),
                    Some(1.0),
                    t("Opening Anni", "Opna Anni").into(),
                ),
                Progress::Failed(reason) => (
                    if applying {
                        t("The update couldn't finish", "Ekki tókst að ljúka uppfærslunni")
                    } else {
                        t("Starting your current version", "Ræsi núverandi útgáfu")
                    },
                    // The detailed reasons are written in English; Icelandic gets a plain summary.
                    if crate::locale::icelandic() {
                        if applying {
                            "Uppfærslan kláraðist ekki. Uppsetta útgáfan þín er óbreytt.".into()
                        } else {
                            "Ekki tókst að sækja uppfærsluna núna. Uppsetta útgáfan þín er óbreytt.".into()
                        }
                    } else {
                        reason.clone()
                    },
                    Some(0.0),
                    if applying {
                        t("You can retry next time Anni starts", "Þú getur reynt aftur næst þegar Anni ræsist")
                    } else {
                        t("We'll check again next time", "Við athugum aftur næst")
                    }
                    .into(),
                ),
            };
            ui.label(RichText::new(title).size(24.0).strong());
            ui.add_sized(
                [ui.available_width(), 38.0],
                egui::Label::new(RichText::new(description).size(13.0).color(TEXT_SECONDARY))
                    .wrap(),
            );
            ui.add_space(12.0);
            let (bar, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(bar, 1.0, BG_DARK);
            painter.rect_stroke(bar, 1.0, Stroke::new(1.0, BORDER));
            let color = if matches!(progress, Progress::Failed(_)) {
                WARNING
            } else {
                ACCENT
            };
            if let Some(fraction) = fraction {
                if fraction > 0.0 {
                    let fill = egui::Rect::from_min_size(
                        bar.min,
                        egui::vec2(bar.width() * fraction, bar.height()),
                    );
                    painter.rect_filled(fill, 1.0, color);
                }
            } else {
                let segment = bar.width() * 0.28;
                let offset =
                    (elapsed.as_secs_f32() / 1.7).fract() * (bar.width() + segment) - segment;
                let moving = egui::Rect::from_min_size(
                    bar.min + egui::vec2(offset, 1.0),
                    egui::vec2(segment, bar.height() - 2.0),
                );
                painter
                    .with_clip_rect(bar.shrink(1.0))
                    .rect_filled(moving, 1.0, color);
            }
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(status).size(11.0).color(TEXT_DIM));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(fraction) =
                        fraction.filter(|_| !matches!(progress, Progress::Failed(_)))
                    {
                        ui.label(
                            RichText::new(format!("{:.0}%", fraction * 100.0))
                                .size(11.0)
                                .color(color),
                        );
                    } else if !matches!(progress, Progress::Failed(_)) {
                        ui.spinner();
                    }
                });
            });
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("ANNI  {}", env!("CARGO_PKG_VERSION")))
                        .size(10.0)
                        .color(TEXT_DIM),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !applying || matches!(progress, Progress::Failed(_)) {
                        launch = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(
                                        if applying || matches!(progress, Progress::Failed(_)) {
                                            t("Launch Anni", "Ræsa Anni")
                                        } else {
                                            t("Skip for now", "Sleppa í bili")
                                        },
                                    )
                                    .size(12.0),
                                )
                                .frame(false),
                            )
                            .clicked();
                    } else {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(210.0, 28.0), Sense::hover());
                        ui.painter().text(
                            rect.right_center(),
                            Align2::RIGHT_CENTER,
                            t("Anni will open automatically", "Anni opnast sjálfkrafa"),
                            FontId::proportional(11.0),
                            TEXT_DIM,
                        );
                    }
                });
            });
        });
    launch
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_update_phase_fits_the_startup_window() {
        let ctx = egui::Context::default();
        crate::ui::theme::apply_theme(&ctx);
        for progress in [
            Progress::Checking,
            Progress::Downloading {
                version: "0.12.34".into(),
                received: 17_000_000,
                total: 30_000_000,
            },
            Progress::Verifying {
                version: "0.12.34".into(),
            },
            Progress::Installing {
                version: "0.12.34".into(),
            },
            Progress::Current,
            Progress::Complete,
            Progress::Failed(
                "The update did not pass verification. Your installed app has not been changed."
                    .into(),
            ),
        ] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(560.0, 318.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    show(ctx, &progress, Duration::from_secs(1), true);
                },
            );
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    let bounds = text.galley.rect.translate(text.pos.to_vec2());
                    assert!(
                        bounds.right() <= 560.0 && bounds.bottom() <= 318.0,
                        "Update text overflow: {bounds:?}"
                    );
                }
            }
        }
    }
}
