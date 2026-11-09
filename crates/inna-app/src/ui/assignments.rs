use crate::ui::theme::*;
use crate::{
    grades::{self, GradeSummary, GradeView, Standing},
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
    if code.trim().is_empty() || code == "—" {
        "Unnamed course"
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
        "Courses & grades",
        "Your grades, their weights, and your progress in each course.",
    );
    let modules = modules(state);
    grade_view_picker(ui, &mut state.grade_view);
    let view = state.grade_view;
    let summaries: Vec<_> = modules.iter().map(GradeSummary::for_module).collect();
    ui.horizontal_wrapped(|ui| {
        badge(
            ui,
            &locale::counted(modules.len(), "course", "courses"),
            ACCENT,
        );
        badge(
            ui,
            &format!(
                "{} with numeric grades",
                summaries.iter().filter(|s| s.counted > 0).count()
            ),
            TEXT_SECONDARY,
        );
    });
    ui.add_space(14.0);
    if modules.is_empty() {
        card().show(ui, |ui| {
            ui.label(
                RichText::new("No courses available yet")
                    .size(20.0)
                    .family(crate::ui::theme::semibold()),
            );
            ui.label("Connect INNA and refresh to see your course progress.");
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
        ui.label(RichText::new("How is progress calculated?").family(crate::ui::theme::semibold()));
        ui.label("Graded work = sum of grade × weight ÷ graded weight. Full coursework = sum of grade × weight ÷ 100. For example, an 8 on a test worth 20% gives an 8.00 graded-work average and 1.60 points toward the full course. Zero-weight items are excluded from both.");
        ui.label("INNA's item weights already include their category contribution. Full coursework always uses 100%, even when INNA has not listed all assessments yet.");
        ui.label("Flensborg's general pass mark is a final grade of 5 (45%). The scale marker is at 4.50. Full-course standing stays provisional until all 100% has numeric grades. Individual course requirements may also apply.");
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to("Flensborg curriculum · page 18", grades::RULES_URL);
            ui.hyperlink_to("Course descriptions at flensborg.is", grades::COURSES_URL);
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
            "Course in progress"
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
                "Earned toward full course"
            } else if summary.incomplete() {
                "Average of known weights"
            } else {
                "Current weighted average"
            })
            .size(13.0)
            .color(TEXT_SECONDARY),
        );
        if let Some(average) = view.value(&summary) {
            ui.add_space(10.0);
            grade_scale(ui, average, tint);
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{}% graded · {}% listed · {} items counted",
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
                "Some grades have missing weights or are nonnumeric. Only numeric grades with known positive weights contribute; earned points are incomplete."
            } else { note };
            ui.label(RichText::new(note).size(13.0).color(WARNING));
        }
        if view == GradeView::FullCourse && summary.listed_weight < 100.0 - 1e-6 {
            ui.label(RichText::new("INNA has not supplied a complete 100% assessment plan. Only known earned points are shown.").size(13.0).color(TEXT_SECONDARY));
        }
        ui.add_space(8.0);
        for (name, category) in grades::categories(module) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(name).size(13.0).family(crate::ui::theme::semibold()));
                let average = view.value(&category)
                    .map(grades::average_text)
                    .unwrap_or_else(|| "—".into());
                ui.label(
                    RichText::new(format!("{average} / 10{}", if view == GradeView::FullCourse { " toward course" } else { "" }))
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
                ui.label(RichText::new(assignment.name_str()).family(crate::ui::theme::semibold()));
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
