use crate::ui::theme::*;
use crate::{
    grades::{self, GradeSummary, Standing},
    locale,
    state::AppState,
};
use egui::{RichText, Ui};
use inna_core::models::AssignmentModule;

pub fn subject(code: &str) -> &str {
    for (prefix, name) in [
        ("DANS", "Danish"),
        ("ENSK", "English"),
        ("STÆR", "Mathematics"),
        ("EÐLI", "Physics"),
        ("ÍSLE", "Icelandic"),
        ("EFNA", "Chemistry"),
        ("SAGA", "History"),
        ("FÉLA", "Sociology"),
        ("HÁMA", "Personal development"),
        ("HLSE", "Health & fitness"),
        ("FORR", "Programming"),
        ("FJÁR", "Hagfræði"),
        ("FJAR", "Hagfræði"),
        ("HMNM", "Homework"),
        ("HMN", "Homework"),
    ] {
        if code.starts_with(prefix) {
            return name;
        }
    }
    if code.trim().is_empty() || code == "—" { "Unnamed course" } else { code }
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
        if !lesson.is_lesson() { continue; }
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
        "Courses & grades",
        "Your grades, their weights, and your progress in each course.",
    );
    let modules = modules(state);
    let summaries: Vec<_> = modules.iter().map(GradeSummary::for_module).collect();
    ui.horizontal_wrapped(|ui| {
        badge(
            ui,
            &locale::counted(modules.len(), "course", "courses"),
            ACCENT,
        );
        let above = summaries
            .iter()
            .filter(|s| s.standing() == Standing::Above)
            .count();
        let below = summaries
            .iter()
            .filter(|s| s.standing() == Standing::Below)
            .count();
        badge(
            ui,
            &format!("{} above the pass mark", locale::number(above)),
            SUCCESS,
        );
        badge(
            ui,
            &format!("{} below the pass mark", locale::number(below)),
            ERROR_RED,
        );
    });
    ui.add_space(14.0);
    if modules.is_empty() {
        card().show(ui, |ui| {
            ui.label(
                RichText::new("No courses available yet")
                    .size(20.0)
                    .strong(),
            );
            ui.label("Connect INNA and refresh to see your course progress.");
        });
    }
    if ui.available_width() >= 780.0 {
        for pair in modules.chunks(2) {
            ui.columns(2, |cols| {
                for (i, module) in pair.iter().enumerate() {
                    course_card(&mut cols[i], module);
                }
            });
        }
    } else {
        for module in &modules {
            course_card(ui, module);
        }
    }
    ui.add_space(16.0);
    card().fill(NEXT_HIGHLIGHT).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("How is progress calculated?").strong());
        ui.label("Average = total grade × weight, divided by the graded weight. Ungraded work does not count as zero; work with zero weight is excluded.");
        ui.label("Flensborg's general pass mark is a final grade of 5 (45%). This view compares the unrounded average with 4.50. Progress is provisional; individual course requirements may also apply.");
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to("Flensborg curriculum · page 18", grades::RULES_URL);
            ui.hyperlink_to("Course descriptions at flensborg.is", grades::COURSES_URL);
        });
    });
}

fn course_card(ui: &mut Ui, module: &AssignmentModule) {
    let summary = GradeSummary::for_module(module);
    let tint = color(summary.standing());
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(subject(module.module_str()))
                .size(14.0)
                .color(TEXT_SECONDARY),
        );
        ui.label(RichText::new(module.module_str()).size(22.0).strong());
        ui.add_space(4.0);
        badge(ui, summary.standing().label(), tint);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    summary
                        .average
                        .map(grades::average_text)
                        .unwrap_or_else(|| "—".into()),
                )
                .size(44.0)
                .strong()
                .color(tint),
            );
            ui.label(RichText::new("/ 10").size(18.0).color(TEXT_DIM));
        });
        ui.label(
            RichText::new(if summary.incomplete() {
                "Average of known weights"
            } else {
                "Current weighted average"
            })
            .size(13.0)
            .color(TEXT_SECONDARY),
        );
        if let Some(average) = summary.average {
            ui.add_space(10.0);
            grade_scale(ui, average, tint);
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Graded weight: {}% · {} items",
                    locale::decimal(summary.graded_weight, 1),
                    locale::number(summary.counted)
                ))
                .size(13.0)
                .color(TEXT_SECONDARY),
            );
        }
        if let Some(note) = summary.explanation() {
            ui.add_space(6.0);
            ui.label(RichText::new(note).size(13.0).color(WARNING));
        }
        ui.add_space(8.0);
        for (name, category) in grades::categories(module) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(name).size(13.0).strong());
                let average = category
                    .average
                    .map(grades::average_text)
                    .unwrap_or_else(|| "—".into());
                ui.label(
                    RichText::new(format!("{average} / 10"))
                        .size(13.0)
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new(format!(
                        "{}% listed · {}% graded",
                        locale::decimal(category.listed_weight, 1),
                        locale::decimal(category.graded_weight, 1)
                    ))
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });
        }
        ui.add_space(4.0);
        egui::CollapsingHeader::new(format!(
            "Assignments & exams ({})",
            locale::number(module.assignments.len())
        ))
        .id_salt((&module.group_id, module.module_str()))
        .show(ui, |ui| {
            if module.assignments.is_empty() {
                ui.label("No assignments or grades have been received from INNA.");
            }
            for assignment in &module.assignments {
                ui.separator();
                ui.label(RichText::new(assignment.name_str()).strong());
                ui.horizontal_wrapped(|ui| {
                    let grade = grades::numeric_grade(assignment.grade_str())
                        .map(|g| locale::decimal(g, 2))
                        .unwrap_or_else(|| {
                            if assignment.grade_str().is_empty() {
                                "Ungraded".into()
                            } else {
                                assignment.grade_str().into()
                            }
                        });
                    ui.label(RichText::new(format!("Grade: {grade}")).color(ACCENT));
                    let w = grades::weight(assignment.weight.as_ref())
                        .map(|w| format!("{}%", locale::decimal(w, 1)))
                        .unwrap_or_else(|| "unknown".into());
                    ui.label(
                        RichText::new(format!("Weight: {w}"))
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    if assignment.is_handed_in() {
                        ui.label(RichText::new("Submitted").size(13.0).color(SUCCESS));
                    }
                });
                if let Some(due) = crate::planner::parse_deadline(assignment.closes_str()) {
                    ui.label(
                        RichText::new(format!("Due: {}", locale::date(due.at.date())))
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
