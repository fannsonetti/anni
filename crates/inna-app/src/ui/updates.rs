//! A compact launcher window with real download progress and an animated scan bar.
use crate::{
    ui::{brand, theme::*},
    updates::Progress,
};
use egui::{Align, Align2, Color32, FontId, Layout, RichText, Sense, Stroke};
use std::time::Duration;

pub fn show(ctx: &egui::Context, progress: &Progress, elapsed: Duration, applying: bool) -> bool {
    let mut launch = false;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(PANEL_BG)
                .stroke(Stroke::new(1.0, BORDER))
                .rounding(16.0)
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
                            "UPDATE PREVIEW"
                        } else {
                            "AUTOMATIC UPDATES"
                        })
                        .size(10.0)
                        .color(TEXT_DIM),
                    );
                });
            });
            ui.add_space(22.0);
            let (title, description, fraction, status) = match progress {
                Progress::Checking => (
                    "Checking for updates",
                    "Getting the latest version of Anni.".into(),
                    None,
                    "Connecting to release service".into(),
                ),
                Progress::Downloading {
                    version,
                    received,
                    total,
                } => (
                    "A fresh Anni is on its way",
                    format!("Downloading version {version}."),
                    Some((*received as f32 / (*total).max(1) as f32).clamp(0.0, 1.0)),
                    format!(
                        "{:.1} / {:.1} MB",
                        *received as f64 / 1_048_576.0,
                        *total as f64 / 1_048_576.0
                    ),
                ),
                Progress::Verifying { version } => (
                    "Checking the download",
                    format!("Verifying Anni {version} before installation."),
                    None,
                    "Verifying SHA-256 checksum".into(),
                ),
                Progress::Staged(update) => (
                    "Your update is ready",
                    format!("Preparing to install Anni {}.", update.version),
                    Some(1.0),
                    "Download verified".into(),
                ),
                Progress::Installing { version } => (
                    "Making things a little better",
                    format!("Installing Anni {version}. It will open automatically."),
                    None,
                    "Installing update".into(),
                ),
                Progress::Current => (
                    "You're up to date",
                    "Everything is ready. Opening Anni…".into(),
                    Some(1.0),
                    "Latest version installed".into(),
                ),
                Progress::Complete => (
                    "Ready to go",
                    "The update is installed. Welcome back.".into(),
                    Some(1.0),
                    "Opening Anni".into(),
                ),
                Progress::Failed(reason) => (
                    if applying {
                        "The update couldn't finish"
                    } else {
                        "Starting your current version"
                    },
                    reason.clone(),
                    Some(0.0),
                    if applying {
                        "You can retry next time Anni starts"
                    } else {
                        "We'll check again next time"
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
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 10.0), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(bar, 5.0, BG_DARK);
            painter.rect_stroke(bar, 5.0, Stroke::new(1.0, BORDER));
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
                    painter.rect_filled(fill, 5.0, color);
                    let shine = egui::Rect::from_min_size(
                        fill.min + egui::vec2(1.0, 1.0),
                        egui::vec2((fill.width() - 2.0).max(0.0), 2.0),
                    );
                    painter.rect_filled(shine, 1.0, Color32::from_white_alpha(65));
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
                    .rect_filled(moving, 4.0, color);
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
                                            "Launch Anni"
                                        } else {
                                            "Skip for now"
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
                            "Anni will open automatically",
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
