use inna_core::models::AssignmentModule;
use serde_json::Value;

// Flensborg's published curriculum: final grade 5 (45%). This compares the
// unrounded current numeric average with 45%; course-specific conditions remain separate.
pub const PASS_MARK: f64 = 4.5;
pub const RULES_URL: &str = "https://www.flensborg.is/static/files/PDF_allskonar/almennur-hluti_yfirlesinn-2020.pdf#page=18";
pub const COURSES_URL: &str = "https://www.flensborg.is/is/namid/nam-og-namshaettir/afangalysingar";

pub fn average_text(value: f64) -> String {
    if value < PASS_MARK && (value * 100.0).round() / 100.0 >= PASS_MARK {
        "< 4.50".into()
    } else {
        crate::locale::decimal(value, 2)
    }
}

pub fn numeric_grade(text: &str) -> Option<f64> {
    let text = text.trim();
    let percent = text.ends_with('%');
    let value: f64 = text
        .trim_end_matches('%')
        .trim()
        .replace(',', ".")
        .parse()
        .ok()?;
    let value = if percent { value / 10.0 } else { value };
    (value.is_finite() && (0.0..=10.0).contains(&value)).then_some(value)
}

pub fn weight(value: Option<&Value>) -> Option<f64> {
    let value = match value? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s
            .trim()
            .trim_end_matches('%')
            .trim()
            .replace(',', ".")
            .parse()
            .ok()?,
        _ => return None,
    };
    (value.is_finite() && (0.0..=100.0).contains(&value)).then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Above,
    Below,
    Unknown,
}

impl Standing {
    pub fn label(self) -> &'static str {
        match self {
            Self::Above => "Above the general pass mark",
            Self::Below => "Below the general pass mark",
            Self::Unknown => "Not enough information",
        }
    }
}

#[derive(Debug, Default)]
pub struct GradeSummary {
    pub average: Option<f64>,
    pub graded_weight: f64,
    pub listed_weight: f64,
    pub counted: usize,
    pub missing_weights: usize,
    pub non_numeric: usize,
    pub non_numeric_weighted: usize,
    pub invalid_total: bool,
}

impl GradeSummary {
    pub fn for_module(module: &AssignmentModule) -> Self {
        let mut summary = Self::default();
        let mut points = 0.0;
        for item in &module.assignments {
            let grade = numeric_grade(item.grade_str());
            let item_weight = weight(item.weight.as_ref());
            if let Some(w) = item_weight {
                summary.listed_weight += w;
            }
            if grade.is_some() && item_weight.is_none() {
                summary.missing_weights += 1;
            }
            if !matches!(item.grade_str().trim(), "" | "-" | "—") && grade.is_none() {
                summary.non_numeric += 1;
                if item_weight.is_some_and(|w| w > 0.0) {
                    summary.non_numeric_weighted += 1;
                }
            }
            if let (Some(grade), Some(w)) = (grade, item_weight.filter(|w| *w > 0.0)) {
                points += grade * w;
                summary.graded_weight += w;
                summary.counted += 1;
            }
        }
        summary.invalid_total = summary.listed_weight > 100.000001;
        // INNA's assignment weights already describe contributions to the course.
        // Categories group those same items; multiplying by a category again would
        // apply the weight twice (e.g. the 40% / 60% assessment split).
        if summary.graded_weight > 0.0 && !summary.invalid_total {
            summary.average = Some(points / summary.graded_weight);
        }
        summary
    }

    pub fn standing(&self) -> Standing {
        if self.incomplete() {
            return Standing::Unknown;
        }
        match self.average {
            Some(value) if value >= PASS_MARK => Standing::Above,
            Some(_) => Standing::Below,
            None => Standing::Unknown,
        }
    }

    pub fn incomplete(&self) -> bool {
        self.missing_weights > 0 || self.non_numeric_weighted > 0
    }

    pub fn explanation(&self) -> Option<&'static str> {
        if self.invalid_total {
            Some("Listed weights exceed 100%. The assessment structure needs checking before an average can be calculated.")
        } else if self.missing_weights > 0 {
            Some("Some grades have no weight. This average uses only known weights, so the result is uncertain.")
        } else if self.non_numeric_weighted > 0 {
            Some("Some weighted grades are nonnumeric. This average includes only numeric grades, so the result is uncertain.")
        } else if self.average.is_none() {
            Some("No numeric grades with positive weight are available yet. Grades with zero weight are excluded.")
        } else {
            None
        }
    }
}

/// Use the refreshed flat rows for grades/weights; category copies may be older.
/// Match by INNA assignment ID so each grade is counted once in its category.
pub fn categories(module: &AssignmentModule) -> Vec<(String, GradeSummary)> {
    module
        .categories
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|category| {
            let name = category.get("name")?.as_str()?.to_owned();
            let rows = category.get("assignments")?.as_array()?;
            let mut group = module.clone();
            group.categories = Value::Null;
            group.assignments.retain(|item| {
                item.assignment_id.as_ref().is_some_and(|id| {
                    rows.iter()
                        .any(|row| row.get("assignmentId").and_then(Value::as_str) == Some(id))
                })
            });
            Some((name, GradeSummary::for_module(&group)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn module(items: Value) -> AssignmentModule {
        serde_json::from_value(serde_json::json!({"moduleName":"TEST", "assignments":items}))
            .unwrap()
    }

    #[test]
    fn weighted_average_uses_only_numeric_graded_work_with_positive_weight() {
        let m = module(serde_json::json!([
            {"grade":"8,0","weight":"20%"}, {"grade":"4","weight":10},
            {"grade":null,"weight":50}, {"grade":"10","weight":0}, {"grade":"S","weight":20}
        ]));
        let s = GradeSummary::for_module(&m);
        assert!((s.average.unwrap() - 20.0 / 3.0).abs() < 1e-9);
        assert_eq!(s.graded_weight, 30.0);
        assert_eq!(s.counted, 2);
        assert_eq!(s.non_numeric, 1);
        assert_eq!(s.standing(), Standing::Unknown);
    }

    #[test]
    fn passing_boundary_uses_unrounded_value_and_zero_is_a_grade() {
        for (grade, expected) in [
            ("4,49", Standing::Below),
            ("4,5", Standing::Above),
            ("0", Standing::Below),
        ] {
            let s = GradeSummary::for_module(&module(
                serde_json::json!([{"grade":grade,"weight":100}]),
            ));
            assert_eq!(s.standing(), expected);
        }
        assert_eq!(average_text(4.49999), "< 4.50");
    }

    #[test]
    fn missing_weights_and_invalid_totals_never_report_a_certain_pass() {
        let s = GradeSummary::for_module(&module(serde_json::json!([{"grade":"8"}])));
        assert_eq!(s.standing(), Standing::Unknown);
        assert!(s.average.is_none());
        let mut m = module(serde_json::json!([{"grade":"8","weight":50}]));
        m.categories = serde_json::json!([{"weight":100}]);
        assert_eq!(GradeSummary::for_module(&m).average, Some(8.0));
        let s = GradeSummary::for_module(&module(
            serde_json::json!([{"grade":"8","weight":80},{"grade":"7","weight":80}]),
        ));
        assert!(s.invalid_total);
        assert!(s.average.is_none());
        assert!(numeric_grade("G").is_none());
        assert!(numeric_grade("NaN").is_none());
        assert!(numeric_grade("11").is_none());
        assert_eq!(numeric_grade("75%"), Some(7.5));
    }

    #[test]
    fn categories_use_effective_weights_once_and_ignore_ungraded_work() {
        let mut m = module(serde_json::json!([
            {"assignmentId":"a", "grade":"8", "weight":20},
            {"assignmentId":"b", "grade":"6", "weight":20},
            {"assignmentId":"c", "grade":"10", "weight":30},
            {"assignmentId":"d", "weight":30}
        ]));
        m.categories = serde_json::json!([
            {"name":"Coursework", "weight":40, "assignments":[{"assignmentId":"a", "grade":"1"},{"assignmentId":"b"}]},
            {"name":"Exams", "weight":60, "assignments":[{"assignmentId":"c"},{"assignmentId":"d"}]}
        ]);
        let s = GradeSummary::for_module(&m);
        assert!((s.average.unwrap() - 580.0 / 70.0).abs() < 1e-9);
        assert_eq!(s.graded_weight, 70.0);
        assert_eq!(s.listed_weight, 100.0);
        assert_eq!(s.counted, 3);
        let groups = categories(&m);
        assert_eq!(groups[0].1.average, Some(7.0));
        assert_eq!(groups[1].1.average, Some(10.0));
        assert_eq!(groups[0].1.listed_weight, 40.0);
        assert_eq!(groups[1].1.listed_weight, 60.0);
    }
}
