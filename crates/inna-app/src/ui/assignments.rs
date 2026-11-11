use crate::ui::theme::*;
use crate::{
    grades::{self, GradeSummary, GradeView, Standing},
    locale::{self, t},
    state::AppState,
};
use egui::{RichText, Ui};
use inna_core::models::AssignmentModule;

/// The name shown for a class: your own name from Settings → Class names, else the built-in one.
pub fn subject(code: &str) -> String {
    crate::class_names::custom_name(code).unwrap_or_else(|| built_in_subject(code).to_string())
}

pub fn built_in_subject(code: &str) -> &str {
    for (prefix, name) in [
        ("DANS", t("Danish", "Danska")),
        ("ENSK", t("English", "Enska")),
        ("STÆR", t("Mathematics", "Stærðfræði")),
        ("EÐLI", t("Physics", "Eðlisfræði")),
        ("ÍSLE", t("Icelandic", "Íslenska")),
        ("EFNA", t("Chemistry", "Efnafræði")),
        ("SAGA", t("History", "Saga")),
        ("FÉLA", t("Sociology", "Félagsfræði")),
        ("HÁMA", t("Personal development", "Hámarksárangur")),
        ("HLSE", t("Health & fitness", "Heilsa og hreyfing")),
        ("FORR", t("Programming", "Forritun")),
        ("FJÁR", "Hagfræði"),
        ("FJAR", "Hagfræði"),
        ("HMNM", t("Homework", "Heimavinna")),
        ("HMN", t("Homework", "Heimavinna")),
    ] {
        if code.starts_with(prefix) {
            return name;
        }
    }
    if code.trim().is_empty() || code == "—" {
        t("Unnamed course", "Ónefndur áfangi")
    } else {
        code
    }
}

#[cfg(test)]
#[test]
fn unknown_subjects_keep_their_course_code() {
    assert_eq!(subject("LÍFF2VL05"), "LÍFF2VL05");
    assert_eq!(subject("ENSK2HF05"), "English");
    assert_eq!(subject(""), "Unnamed course");
}

pub fn modules(state: &AppState) -> Vec<AssignmentModule> {
    let mut modules = state.assignments.clone();
    for lesson in &state.timetable {
        if !lesson.is_lesson() {
            continue;
        }
        let code = lesson.course_code();
        if code != "—" && !modules.iter().any(|m| m.module_str() == code) {
            modules.push(AssignmentModule {
                module_name: Some(code.into()),
                group_id: None,
                assignments: Vec::new(),
                categories: serde_json::Value::Null,
            });
        }
    }
    modules.sort_by_key(|m| locale::sort_key(m.module_str()));
    modules
}

fn color(standing: Standing) -> egui::Color32 {
    match standing {
        Standing::Above => SUCCESS,
        Standing::Below => ERROR_RED,
        Standing::Unknown => TEXT_SECONDARY,
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        t("Courses & grades", "Áfangar og einkunnir"),
        t("Your grades, their weights, and your progress in each course.", "Einkunnirnar þínar, vægi þeirra og framvinda í hverjum áfanga."),
    );
    let modules = modules(state);
    grade_view_picker(ui, &mut state.grade_view);
    let view = state.grade_view;
    let summaries: Vec<_> = modules.iter().map(GradeSummary::for_module).collect();
    ui.horizontal_wrapped(|ui| {
        badge(
            ui,
            &locale::counted(modules.len(), ("course", "courses"), ("áfangi", "áfangar")),
            ACCENT,
        );
        badge(
            ui,
            &tr!(
                "{} with numeric grades",
                "{} með tölulegar einkunnir",
                summaries.iter().filter(|s| s.counted > 0).count()
            ),
            TEXT_SECONDARY,
        );
    });
    ui.add_space(14.0);
    if modules.is_empty() {
        card().show(ui, |ui| {
            ui.label(
                RichText::new(t("No courses available yet", "Engir áfangar enn"))
                    .size(20.0)
                    .family(crate::ui::theme::semibold()),
            );
            ui.label(t("Connect INNA and refresh to see your course progress.", "Tengdu INNU og uppfærðu til að sjá framvinduna í áföngunum."));
        });
    }
    if ui.available_width() >= 780.0 {
        for pair in modules.chunks(2) {
            ui.columns(2, |cols| {
                for (i, module) in pair.iter().enumerate() {
                    course_card(&mut cols[i], module, view);
                }
            });
        }
    } else {
        for module in &modules {
            course_card(ui, module, view);
        }
    }
    ui.add_space(16.0);
    card().fill(NEXT_HIGHLIGHT).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("How is progress calculated?", "Hvernig er framvindan reiknuð?")).family(crate::ui::theme::semibold()));
        ui.label(t("Graded work = sum of grade × weight ÷ graded weight. Full coursework = sum of grade × weight ÷ 100. For example, an 8 on a test worth 20% gives an 8.00 graded-work average and 1.60 points toward the full course. Zero-weight items are excluded from both.", "Metin vinna = summa einkunn × vægi ÷ metið vægi. Allur áfanginn = summa einkunn × vægi ÷ 100. Til dæmis gefur 8 á prófi sem gildir 20% meðaltalið 8,00 í metinni vinnu og 1,60 stig upp í allan áfangann. Atriði með ekkert vægi eru undanskilin í báðum."));
        ui.label(t("INNA's item weights already include their category contribution. Full coursework always uses 100%, even when INNA has not listed all assessments yet.", "Vægi atriða í INNU inniheldur þegar framlag flokksins. Allur áfanginn miðast alltaf við 100%, jafnvel þótt INNA hafi ekki enn skráð allt námsmat."));
        ui.label(t("Flensborg's general pass mark is a final grade of 5 (45%). The scale marker is at 4.50. Full-course standing stays provisional until all 100% has numeric grades. Individual course requirements may also apply.", "Almenn lágmarkseinkunn í Flensborg er lokaeinkunnin 5 (45%). Merkið á kvarðanum er við 4,50. Staða í öllum áfanganum er bráðabirgðastaða þar til öll 100% hafa tölulegar einkunnir. Sérstakar kröfur áfanga geta líka gilt."));
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to(t("Flensborg curriculum · page 18", "Skólanámskrá Flensborgar · bls. 18"), grades::RULES_URL);
            ui.hyperlink_to(t("Course descriptions at flensborg.is", "Áfangalýsingar á flensborg.is"), grades::COURSES_URL);
        });
    });
}

pub fn grade_view_picker(ui: &mut Ui, view: &mut GradeView) {
    ui.horizontal_wrapped(|ui| {
        for mode in [GradeView::GradedWork, GradeView::FullCourse] {
            ui.selectable_value(view, mode, mode.label());
        }
    });
    ui.label(
        RichText::new(view.description())
            .size(12.0)
            .color(TEXT_SECONDARY),
    );
    ui.add_space(12.0);
}

fn course_card(ui: &mut Ui, module: &AssignmentModule, view: GradeView) {
    let summary = GradeSummary::for_module(module);
    let standing = view.standing(&summary);
    let tint = color(standing);
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(subject(module.module_str()))
                .size(14.0)
                .color(TEXT_SECONDARY),
        );
        ui.label(RichText::new(module.module_str()).size(22.0).family(crate::ui::theme::semibold()));
        ui.add_space(4.0);
        badge(ui, if view == GradeView::FullCourse && summary.full_course.is_some() && standing == Standing::Unknown {
            t("Course in progress", "Áfangi í gangi")
        } else { standing.label() }, tint);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    view.value(&summary)
                        .map(grades::average_text)
                        .unwrap_or_else(|| "—".into()),
                )
                .size(44.0)
                .family(crate::ui::theme::semibold())
                .color(tint),
            );
            ui.label(RichText::new("/ 10").size(18.0).color(TEXT_DIM));
        });
        ui.label(
            RichText::new(if view == GradeView::FullCourse {
                t("Earned toward full course", "Áunnið upp í allan áfangann")
            } else if summary.incomplete() {
                t("Average of known weights", "Meðaltal þekkts vægis")
            } else {
                t("Current weighted average", "Núverandi vegið meðaltal")
            })
            .size(13.0)
            .color(TEXT_SECONDARY),
        );
        if let Some(average) = view.value(&summary) {
            ui.add_space(10.0);
            grade_scale(ui, average, tint);
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr!(
                    "{}% graded · {}% listed · {} items counted",
                    "{}% metið · {}% skráð · {} atriði talin",
                    locale::decimal(summary.graded_weight, 1),
                    locale::decimal(summary.listed_weight, 1),
                    locale::number(summary.counted)
                ))
                .size(13.0)
                .color(TEXT_SECONDARY),
            );
        }
        if let Some(note) = summary.explanation() {
            ui.add_space(6.0);
            let note = if view == GradeView::FullCourse && summary.incomplete() {
                t("Some grades have missing weights or are nonnumeric. Only numeric grades with known positive weights contribute; earned points are incomplete.", "Sumar einkunnir vantar vægi eða eru ekki tölulegar. Aðeins tölulegar einkunnir með þekkt jákvætt vægi telja; áunnin stig eru ófullkomin.")
            } else { note };
            ui.label(RichText::new(note).size(13.0).color(WARNING));
        }
        if view == GradeView::FullCourse && summary.listed_weight < 100.0 - 1e-6 {
            ui.label(RichText::new(t("INNA has not supplied a complete 100% assessment plan. Only known earned points are shown.", "INNA hefur ekki gefið upp fullt 100% námsmat. Aðeins þekkt áunnin stig eru sýnd.")).size(13.0).color(TEXT_SECONDARY));
        }
        ui.add_space(8.0);
        for (name, category) in grades::categories(module) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(name).size(13.0).family(crate::ui::theme::semibold()));
                let average = view.value(&category)
                    .map(grades::average_text)
                    .unwrap_or_else(|| "—".into());
                ui.label(
                    RichText::new(format!("{average} / 10{}", if view == GradeView::FullCourse { t(" toward course", " upp í áfangann") } else { "" }))
                        .size(13.0)
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new(tr!(
                        "{}% listed · {}% graded",
                        "{}% skráð · {}% metið",
                        locale::decimal(category.listed_weight, 1),
                        locale::decimal(category.graded_weight, 1)
                    ))
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });
        }
        ui.add_space(4.0);
        egui::CollapsingHeader::new(tr!(
            "Assignments & exams ({})",
            "Verkefni og próf ({})",
            locale::number(module.assignments.len())
        ))
        .id_salt((&module.group_id, module.module_str()))
        .show(ui, |ui| {
            if module.assignments.is_empty() {
                ui.label(t("No assignments or grades have been received from INNA.", "Engin verkefni eða einkunnir hafa borist frá INNU."));
            }
            for assignment in &module.assignments {
                ui.separator();
                ui.label(RichText::new(assignment.name_str()).family(crate::ui::theme::semibold()));
                ui.horizontal_wrapped(|ui| {
                    let grade = grades::numeric_grade(assignment.grade_str())
                        .map(|g| locale::decimal(g, 2))
                        .unwrap_or_else(|| {
                            if assignment.grade_str().is_empty() {
                                t("Ungraded", "Ómetið").into()
                            } else {
                                assignment.grade_str().into()
                            }
                        });
                    ui.label(RichText::new(tr!("Grade: {grade}", "Einkunn: {grade}")).color(ACCENT));
                    let w = grades::weight(assignment.weight.as_ref())
                        .map(|w| format!("{}%", locale::decimal(w, 1)))
                        .unwrap_or_else(|| t("unknown", "óþekkt").into());
                    ui.label(
                        RichText::new(tr!("Weight: {w}", "Vægi: {w}"))
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    if assignment.is_handed_in() {
                        ui.label(RichText::new(t("Submitted", "Skilað")).size(13.0).color(SUCCESS));
                    }
                });
                if let Some(due) = crate::planner::parse_deadline(assignment.closes_str()) {
                    ui.label(
                        RichText::new(tr!("Due: {}", "Skil: {}", locale::date(due.at.date())))
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    );
                }
            }
        });
    });
}

fn grade_scale(ui: &mut Ui, average: f64, color: egui::Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, BORDER);
    let filled = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(
            rect.width() * (average.clamp(0.0, 10.0) / 10.0) as f32,
            rect.height(),
        ),
    );
    ui.painter().rect_filled(filled, 4.0, color);
    let x = rect.left() + rect.width() * (grades::PASS_MARK / 10.0) as f32;
    ui.painter().line_segment(
        [
            egui::pos2(x, rect.top() - 3.0),
            egui::pos2(x, rect.bottom() + 3.0),
        ],
        egui::Stroke::new(2.0, TEXT_PRIMARY),
    );
}
