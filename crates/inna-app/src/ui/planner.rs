use crate::planner::{self, WorkFilter, WorkItem, WorkKind};
use crate::locale::t;
use crate::state::AppState;
use crate::ui::theme::*;
use chrono::Utc;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        t("Schoolwork", "Skólavinna"),
        t("Assignments, tests, and homework from INNA. Local checkmarks track your progress; submit work in INNA.", "Verkefni, próf og heimavinna úr INNU. Hökin halda utan um framvinduna á tölvunni; skilaðu verkefnum í INNU."),
    );
    ui.hyperlink_to(t("Open INNA for submissions ↗", "Opna INNU til að skila ↗"), "https://nam.inna.is/");
    ui.add_space(12.0);
    let today = Utc::now().date_naive();
    let items = planner::work_items(state);
    ui.horizontal_wrapped(|ui| {
        for (filter, label) in [
            (WorkFilter::Open, t("Open", "Opið")),
            (WorkFilter::Today, t("Today & overdue", "Í dag og komið fram yfir")),
            (WorkFilter::Week, t("Next 7 days", "Næstu 7 dagar")),
            (WorkFilter::Done, t("Done", "Lokið")),
        ] {
            let count = items.iter().filter(|i| filter.matches(i, today)).count();
            if ui
                .selectable_value(
                    &mut state.work_filter,
                    filter,
                    format!("{label} ({})", crate::locale::number(count)),
                )
                .clicked()
            {
                state.work_date = None;
            }
        }
    });
    if let Some(date) = state.work_date {
        ui.horizontal(|ui| {
            badge(ui, &crate::locale::capitalize(&crate::locale::day(date)), ACCENT);
            if ui.small_button(t("Show all days", "Sýna alla daga")).clicked() {
                state.work_date = None;
            }
        });
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.work_search)
            .hint_text(t("Search tasks or courses…", "Leita að verkefnum eða áföngum…"))
            .margin(egui::vec2(10.0, 8.0))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let search = state.work_search.trim().to_lowercase();
    let visible: Vec<_> = items
        .iter()
        .filter(|item| {
            state.work_filter.matches(item, today)
                && state
                    .work_date
                    .is_none_or(|date| item.deadline.is_some_and(|d| d.at.date() == date))
                && (search.is_empty()
                    || format!("{} {}", item.title, item.course)
                        .to_lowercase()
                        .contains(&search))
        })
        .collect();
    if visible.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new(t("No tasks in this view", "Engin verkefni í þessari sýn")).size(20.0));
        ui.label(
            RichText::new(t("Choose Open or adjust your search.", "Veldu Opið eða breyttu leitinni."))
                .size(15.0)
                .color(TEXT_SECONDARY),
        );
    }
    for item in visible {
        work_card(ui, state, item);
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new(t("Checkmarks track your local plan. Submit schoolwork in INNA.", "Hökin halda utan um skipulagið á tölvunni. Skilaðu skólavinnu í INNU."))
            .size(13.0)
            .color(TEXT_SECONDARY),
    );
    if !state.last_sync.contains_key("assignments") {
        ui.label(
            RichText::new(t("Connect INNA and refresh to get schoolwork.", "Tengdu INNU og uppfærðu til að sækja skólavinnu."))
                .size(14.0)
                .color(WARNING),
        );
    }
}

pub fn work_card(ui: &mut Ui, state: &mut AppState, item: &WorkItem) {
    let now = Utc::now().naive_utc();
    let overdue = item.overdue(now);
    let due_today = !item.done && item.deadline.is_some_and(|d| d.at.date() == now.date());
    card().inner_margin(14.0).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let mut done = item.done;
            let checked = ui
                .scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_fill = BG_DARK;
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        egui::Stroke::new(1.0, TEXT_SECONDARY);
                    ui.spacing_mut().icon_width = 22.0;
                    ui.add_enabled_ui(!item.source_done, |ui| {
                        ui.add_sized([36.0, 36.0], egui::Checkbox::new(&mut done, ""))
                    })
                    .inner
                })
                .inner;
            if checked.changed() {
                item.set_done(&mut state.planner, done);
                state.storage_dirty = true;
            }
            ui.vertical(|ui| {
                let title = RichText::new(&item.title)
                    .size(17.0)
                    .family(crate::ui::theme::semibold());
                ui.label(if item.done {
                    title.color(TEXT_SECONDARY).strikethrough()
                } else {
                    title
                });
                ui.horizontal_wrapped(|ui| {
                    if !item.course.is_empty() {
                        ui.label(RichText::new(&item.course).size(13.0).color(ACCENT));
                    }
                    ui.label(
                        RichText::new(item.kind.label())
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    if item.source_done {
                        ui.label(
                            RichText::new(
                                if matches!(item.kind, WorkKind::Assignment | WorkKind::Exam) {
                                    t("Submitted or graded in INNA", "Skilað eða metið í INNU")
                                } else {
                                    t("Completed in INNA", "Lokið í INNU")
                                },
                            )
                            .size(13.0)
                            .color(SUCCESS),
                        );
                    } else if item.done {
                        ui.label(RichText::new(t("Completed by you", "Þú merktir sem lokið")).size(13.0).color(SUCCESS));
                    } else {
                        let label = format!(
                            "{}{}",
                            if overdue { t("Overdue · ", "Komið fram yfir · ") } else { "" },
                            item.due_label(now.date())
                        );
                        ui.label(RichText::new(label).size(13.0).color(if overdue {
                            ERROR_RED
                        } else if due_today {
                            WARNING
                        } else {
                            TEXT_SECONDARY
                        }));
                    }
                });
                if !item.detail.is_empty() {
                    egui::CollapsingHeader::new(t("Details", "Nánar"))
                        .id_salt(&item.key)
                        .show(ui, |ui| {
                            ui.label(&item.detail);
                        });
                }
            });
        });
    });
}
