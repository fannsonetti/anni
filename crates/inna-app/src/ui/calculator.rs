use crate::calculator::{self, Course, Outcome, Source, Target};
use crate::ui::assignments::{modules, subject};
use crate::ui::theme::*;
use crate::{
    grades,
    locale::{self, t},
    state::AppState,
};
use egui::{RichText, Ui};
use std::collections::HashMap;

pub fn show(ui: &mut Ui, state: &mut AppState) {
    page_title(
        ui,
        t("Final grade calculator", "Lokaeinkunnarreiknir"),
        t(
            "Pick a course and the final grade you want. Your grades from INNA are filled in, and the calculator works out what you need on the rest.",
            "Veldu áfanga og lokaeinkunnina sem þú vilt. Einkunnirnar þínar úr INNU eru settar inn og reiknirinn finnur hvað þú þarft á restinni.",
        ),
    );
    let modules = modules(state);
    inna_status(ui, state);
    if modules.is_empty() {
        card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(t("No courses available yet", "Engir áfangar enn"))
                    .size(20.0)
                    .family(semibold()),
            );
            ui.label(t(
                "Connect INNA and refresh to get your courses and grades.",
                "Tengdu INNU og uppfærðu til að sækja áfangana og einkunnirnar.",
            ));
        });
        return;
    }

    // Start on the first course that has weighted work, and recover if a course disappears.
    let selected = state
        .calculator
        .course
        .clone()
        .filter(|code| modules.iter().any(|m| m.module_str() == code))
        .or_else(|| {
            modules
                .iter()
                .find(|m| !Course::from_module(m).rows.iter().all(|r| r.unlisted()))
                .or(modules.first())
                .map(|m| m.module_str().to_string())
        });
    state.calculator.course = selected.clone();
    let Some(code) = selected else { return };
    let module = modules.iter().find(|m| m.module_str() == code).unwrap();

    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("Course", "Áfangi")).family(semibold()));
        egui::ComboBox::from_id_salt("calculator_course")
            .width(ui.available_width().min(360.0))
            .selected_text(course_label(&code))
            .show_ui(ui, |ui| {
                for m in &modules {
                    let c = m.module_str().to_string();
                    let label = course_label(&c);
                    ui.selectable_value(&mut state.calculator.course, Some(c), label);
                }
            });
        ui.add_space(10.0);
        target_picker(ui, &mut state.calculator);
    });

    let course = Course::from_module(module);
    if course.invalid_total {
        warning_card(ui, t(
            "INNA lists more than 100% of weight for this course, so a final grade cannot be calculated. Check the assessment plan with your teacher.",
            "INNA skráir meira en 100% vægi í þessum áfanga svo ekki er hægt að reikna lokaeinkunn. Athugaðu námsmatið hjá kennaranum.",
        ));
        return;
    }
    if course.non_numeric > 0 {
        warning_card(ui, t(
            "Some weighted items in this course are graded with letters or words (for example S), so a numeric final grade cannot be calculated.",
            "Sum vegin atriði í þessum áfanga eru metin með bókstöfum eða orðum (til dæmis S) svo ekki er hægt að reikna tölulega lokaeinkunn.",
        ));
        return;
    }

    // Parse the student's "what if" grades for this course.
    let texts = state.calculator.entries.entry(code.clone()).or_default();
    let entered: HashMap<String, f64> = texts
        .iter()
        .filter_map(|(key, text)| grades::numeric_grade(text).map(|g| (key.clone(), g)))
        .collect();
    let plan = calculator::plan(&course, &entered, state.calculator.target);

    result_card(ui, &plan);

    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(t("Assignments & exams", "Verkefni og próf"))
                .size(17.0)
                .family(semibold()),
        );
        ui.label(
            RichText::new(t(
                "Grades from INNA are fixed. Type a grade into any open item to try a \"what if\"; the needed grade on the other items updates.",
                "Einkunnir úr INNU eru fastar. Skrifaðu einkunn í opið atriði til að prófa \"hvað ef\"; einkunnin sem þarf á hinum atriðunum uppfærist.",
            ))
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(6.0);
        for (row, value) in course.rows.iter().zip(&plan.values) {
            ui.separator();
            let name = if row.unlisted() {
                t("Rest of the course (not in INNA yet)", "Restin af áfanganum (ekki komið í INNU)").to_string()
            } else {
                row.name.clone()
            };
            ui.label(RichText::new(name).family(semibold()));
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(tr!(
                        "Weight {}%",
                        "Vægi {}%",
                        locale::decimal(row.weight, 1)
                    ))
                    .size(13.0)
                    .color(TEXT_SECONDARY),
                );
                match value {
                    Some((grade, Source::Inna)) => {
                        ui.label(
                            RichText::new(tr!(
                                "From INNA: {}",
                                "Úr INNU: {}",
                                locale::decimal(*grade, 2)
                            ))
                            .family(semibold()),
                        );
                    }
                    Some((grade, Source::Needed)) => {
                        ui.label(
                            RichText::new(tr!(
                                "Needed: {}",
                                "Þarft: {}",
                                locale::decimal(*grade, 1)
                            ))
                            .family(semibold())
                            .color(ACCENT),
                        );
                    }
                    _ => {}
                }
                if row.inna.is_none() {
                    let text = texts.entry(row.key.clone()).or_default();
                    ui.add(
                        egui::TextEdit::singleline(text)
                            .id_salt(("calculator_entry", &code, &row.key))
                            .desired_width(64.0)
                            .hint_text(t("what if", "hvað ef")),
                    );
                    if !text.trim().is_empty() {
                        if grades::numeric_grade(text).is_none() {
                            ui.label(
                                RichText::new(t("Enter 0–10", "Sláðu inn 0–10"))
                                    .size(12.0)
                                    .color(WARNING),
                            );
                        } else if ui.small_button(t("Clear", "Hreinsa")).clicked() {
                            text.clear();
                        }
                    }
                }
            });
        }
    });

    card().fill(NEXT_HIGHLIGHT).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(t("How is this calculated?", "Hvernig er þetta reiknað?")).family(semibold()));
        ui.label(t(
            "Final grade = sum of grade × weight ÷ 100. The needed grade is the same on every open item and is rounded up to one decimal, so reaching it is always enough.",
            "Lokaeinkunn = summa einkunn × vægi ÷ 100. Einkunnin sem þarf er sú sama á öllum opnum atriðum og er námunduð upp að einum aukastaf svo það dugar alltaf að ná henni.",
        ));
        ui.label(t(
            "Final grades are whole numbers, so a 7 needs an average of at least 6.50. If INNA has not listed the whole assessment plan yet, the missing weight is shown as the rest of the course.",
            "Lokaeinkunnir eru heilar tölur svo 7 krefst meðaltals að minnsta kosti 6,50. Ef INNA hefur ekki skráð allt námsmatið enn er vægið sem vantar sýnt sem restin af áfanganum.",
        ));
        if course.missing_weight > 0 {
            ui.label(
                RichText::new(tr!(
                    "{} graded items have no weight in INNA and are left out.",
                    "{} metin atriði hafa ekkert vægi í INNU og eru ekki tekin með.",
                    locale::number(course.missing_weight)
                ))
                .color(WARNING),
            );
        }
        ui.label(t(
            "Individual course requirements, such as a minimum grade on the final exam, may also apply.",
            "Sérstakar kröfur áfanga, til dæmis lágmarkseinkunn á lokaprófi, geta líka gilt.",
        ));
    });
}

fn course_label(code: &str) -> String {
    let name = subject(code);
    if name == code {
        code.to_string()
    } else {
        format!("{name} · {code}")
    }
}

/// Where the grades come from, and a way to check INNA for new ones.
fn inna_status(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal_wrapped(|ui| {
        let updated = state
            .last_sync
            .get("assignments")
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .map(|ts| {
                let local = ts.with_timezone(&chrono::Local);
                tr!(
                    "Grades from INNA, updated {} at {}",
                    "Einkunnir úr INNU, uppfærðar {} kl. {}",
                    locale::short_date(local.date_naive()),
                    local.format("%H:%M")
                )
            })
            .unwrap_or_else(|| {
                t(
                    "Grades have not been downloaded from INNA yet.",
                    "Einkunnir hafa ekki verið sóttar úr INNU enn.",
                )
                .into()
            });
        ui.label(RichText::new(updated).size(13.0).color(TEXT_SECONDARY));
        let label = if state.refreshing {
            t("Checking INNA…", "Athuga INNU…")
        } else {
            t("Check INNA for new grades", "Athuga INNU eftir nýjum einkunnum")
        };
        if ui
            .add_enabled(!state.refreshing, egui::Button::new(label))
            .clicked()
        {
            state.refresh_requested = true;
        }
    });
    if let Some(error) = state.sync_errors.get("assignments") {
        ui.label(
            RichText::new(tr!(
                "Could not update grades: {}",
                "Ekki tókst að uppfæra einkunnir: {}",
                locale::error(error)
            ))
            .size(13.0)
            .color(WARNING),
        );
    }
    ui.add_space(10.0);
}

fn target_picker(ui: &mut Ui, calc: &mut calculator::Calculator) {
    ui.label(RichText::new(t("Desired final grade", "Lokaeinkunn sem þú vilt")).family(semibold()));
    ui.horizontal_wrapped(|ui| {
        for grade in 1..=10u8 {
            let selected = calc.target == Some(Target::Final(grade));
            if ui
                .add_sized(
                    [34.0, 30.0],
                    egui::SelectableLabel::new(selected, grade.to_string()),
                )
                .clicked()
            {
                calc.target = if selected { None } else { Some(Target::Final(grade)) };
                calc.exact_text.clear();
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(t("or an exact average:", "eða nákvæmt meðaltal:"))
                .size(13.0)
                .color(TEXT_SECONDARY),
        );
        let response = ui.add(
            egui::TextEdit::singleline(&mut calc.exact_text)
                .id_salt("calculator_exact")
                .desired_width(64.0)
                .hint_text(locale::decimal(7.25, 2)),
        );
        if response.changed() {
            calc.target = grades::numeric_grade(&calc.exact_text).map(Target::Average);
        }
        if !calc.exact_text.trim().is_empty() && grades::numeric_grade(&calc.exact_text).is_none() {
            ui.label(
                RichText::new(t("Enter 0–10", "Sláðu inn 0–10"))
                    .size(12.0)
                    .color(WARNING),
            );
        }
    });
    if let Some(Target::Final(grade)) = calc.target {
        ui.label(
            RichText::new(tr!(
                "A final grade of {} needs an average of at least {}.",
                "Lokaeinkunnin {} krefst meðaltals að minnsta kosti {}.",
                grade,
                locale::decimal(Target::Final(grade).minimum(), 2)
            ))
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
    }
}

fn result_card(ui: &mut Ui, plan: &calculator::Plan) {
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let (headline, detail): (String, String) = match plan.outcome {
            Outcome::NoTarget => (
                t("Choose a desired final grade", "Veldu lokaeinkunnina sem þú vilt").into(),
                t(
                    "Then the grades you need on the remaining work are filled in below.",
                    "Þá birtast einkunnirnar sem þú þarft á restinni hér fyrir neðan.",
                )
                .into(),
            ),
            Outcome::Needed(grade) => (
                tr!("You need {} on the rest", "Þú þarft {} á restinni", locale::decimal(grade, 1)),
                t(
                    "That grade on every open item below reaches your desired final grade.",
                    "Sú einkunn á öllum opnum atriðum hér fyrir neðan nær lokaeinkunninni sem þú vilt.",
                )
                .into(),
            ),
            Outcome::Secured { worst } => (
                t("You have already reached it", "Þú hefur þegar náð henni").into(),
                tr!(
                    "Even 0 on everything left gives an average of {}.",
                    "Jafnvel 0 á öllu sem eftir er gefur meðaltalið {}.",
                    locale::decimal(worst, 2)
                ),
            ),
            Outcome::Unreachable { best } => (
                t("Not reachable any more", "Ekki lengur hægt að ná henni").into(),
                tr!(
                    "Even 10 on everything left gives an average of {} (final grade {}).",
                    "Jafnvel 10 á öllu sem eftir er gefur meðaltalið {} (lokaeinkunn {}).",
                    locale::decimal(best, 2),
                    calculator::final_grade(best)
                ),
            ),
            Outcome::Complete => (
                t("Every item has a grade", "Öll atriði hafa einkunn").into(),
                t(
                    "Your final grade is shown below.",
                    "Lokaeinkunnin þín er sýnd hér fyrir neðan.",
                )
                .into(),
            ),
        };
        ui.label(RichText::new(headline).size(24.0).family(semibold()).color(ACCENT));
        ui.label(RichText::new(detail).size(13.0).color(TEXT_SECONDARY));
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            badge(
                ui,
                &tr!(
                    "Earned so far: {} / 10",
                    "Áunnið hingað til: {} / 10",
                    locale::decimal(plan.earned, 2)
                ),
                TEXT_SECONDARY,
            );
            if let Some(projected) = plan.projected {
                badge(
                    ui,
                    &tr!(
                        "Final average {} → final grade {}",
                        "Lokameðaltal {} → lokaeinkunn {}",
                        locale::decimal(projected, 2),
                        calculator::final_grade(projected)
                    ),
                    ACCENT,
                );
            }
            if plan.projected.is_some_and(|p| p < grades::PASS_MARK) {
                badge(ui, t("Below the general pass mark", "Undir almennri lágmarkseinkunn"), WARNING);
            }
        });
    });
}

fn warning_card(ui: &mut Ui, text: &str) {
    card().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(text).color(WARNING));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(ctx: &egui::Context, width: f32, state: &mut AppState) -> String {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 2400.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let right = ui.max_rect().right();
                    show(ui, state);
                    assert!(ui.min_rect().right() <= right + 1.0, "overflow at {width}");
                });
            },
        );
        fn collect(shape: &egui::Shape, text: &mut String) {
            match shape {
                egui::Shape::Text(t) => {
                    text.push_str(t.galley.text());
                    text.push('\n');
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, text)),
                _ => {}
            }
        }
        let mut text = String::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut text);
        }
        text
    }

    #[test]
    fn a_desired_grade_fills_in_what_the_open_work_needs_in_both_languages() {
        for (language, inna, needed) in [
            (locale::Language::English, "From INNA: 7.50", "Needed: "),
            (locale::Language::Icelandic, "Úr INNU: 7,50", "Þarft: "),
        ] {
            locale::set_language(language);
            for width in [700.0, 1200.0] {
                let mut state = AppState::new();
                state.assignments = crate::mock::mock_assignments();
                state.calculator.course = Some("ENSK2HF05".into());
                state.calculator.target = Some(Target::Final(7));
                let ctx = egui::Context::default();
                crate::ui::theme::apply_theme(&ctx);
                let text = painted(&ctx, width, &mut state);
                assert!(text.contains(inna), "{language:?}: {text}");
                assert!(text.contains(needed), "{language:?}: {text}");
                assert!(text.contains("ENSK2HF05"));
            }
        }
    }
}
