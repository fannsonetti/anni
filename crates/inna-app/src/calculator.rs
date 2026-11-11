//! Final grade calculator: starts from the grades INNA has already given, and
//! works out the grade needed on the remaining assessments for a desired final grade.
use crate::grades;
use inna_core::models::AssignmentModule;
use std::collections::HashMap;

/// Key of the row standing in for assessments INNA has not listed yet.
pub const UNLISTED: &str = "\u{0}unlisted";

/// What the student wants to end the course with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A whole-number final grade; final grades are rounded, so 7 needs 6.5.
    Final(u8),
    /// An exact unrounded average.
    Average(f64),
}

impl Target {
    pub fn minimum(self) -> f64 {
        match self {
            Self::Final(grade) => (grade as f64 - 0.5).clamp(0.0, 10.0),
            Self::Average(value) => value.clamp(0.0, 10.0),
        }
    }
}

/// The calculator's choices. Kept for the session so switching pages keeps your work.
#[derive(Debug, Default)]
pub struct Calculator {
    pub course: Option<String>,
    pub target: Option<Target>,
    pub exact_text: String,
    /// course code → row key → what the student typed as a "what if" grade.
    pub entries: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub name: String,
    pub weight: f64,
    /// The grade INNA already has for this item.
    pub inna: Option<f64>,
}

impl Row {
    pub fn unlisted(&self) -> bool {
        self.key == UNLISTED
    }
}

/// The weighted items of a course, ready for calculating.
#[derive(Debug, Default)]
pub struct Course {
    pub rows: Vec<Row>,
    /// Items with a grade but no weight, which cannot count.
    pub missing_weight: usize,
    /// Weighted items graded with a letter or word (e.g. "S"), which cannot be averaged.
    pub non_numeric: usize,
    /// INNA's weights add up to more than 100%.
    pub invalid_total: bool,
}

impl Course {
    pub fn from_module(module: &AssignmentModule) -> Self {
        let mut course = Self::default();
        let mut listed = 0.0;
        for (index, item) in module.assignments.iter().enumerate() {
            let grade_text = item.grade_str().trim();
            let grade = grades::numeric_grade(grade_text);
            let weight = grades::weight(item.weight.as_ref());
            let Some(weight) = weight.filter(|w| *w > 0.0) else {
                if grade.is_some() && weight.is_none() {
                    course.missing_weight += 1;
                }
                continue;
            };
            listed += weight;
            if grade.is_none() && !matches!(grade_text, "" | "-" | "—") {
                course.non_numeric += 1;
                continue;
            }
            course.rows.push(Row {
                key: item
                    .assignment_id
                    .clone()
                    .unwrap_or_else(|| format!("#{index} {}", item.name_str())),
                name: item.name_str().to_string(),
                weight,
                inna: grade,
            });
        }
        course.invalid_total = listed > 100.000001;
        let rest = 100.0 - listed;
        if rest > 0.05 && !course.invalid_total {
            course.rows.push(Row {
                key: UNLISTED.into(),
                name: String::new(),
                weight: rest,
                inna: None,
            });
        }
        course
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Inna,
    Entered,
    Needed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// No desired grade has been chosen yet.
    NoTarget,
    /// The same grade on every remaining item reaches the target.
    Needed(f64),
    /// The target is reached even with 0 on everything remaining.
    Secured { worst: f64 },
    /// Even 10 on everything remaining falls short.
    Unreachable { best: f64 },
    /// Every item already has a grade.
    Complete,
}

#[derive(Debug)]
pub struct Plan {
    /// One value per course row: its grade and where it came from (None while still open).
    pub values: Vec<Option<(f64, Source)>>,
    pub outcome: Outcome,
    /// The final average once every row has a value.
    pub projected: Option<f64>,
    /// Points already earned out of 10 (INNA grades and entered grades only).
    pub earned: f64,
}

/// Rounds up to one decimal, so reaching the shown grade always reaches the target.
fn round_up(value: f64) -> f64 {
    (value * 10.0 - 1e-9).ceil() / 10.0
}

pub fn plan(course: &Course, entered: &HashMap<String, f64>, target: Option<Target>) -> Plan {
    let total: f64 = course.rows.iter().map(|r| r.weight).sum();
    let mut values: Vec<Option<(f64, Source)>> = course
        .rows
        .iter()
        .map(|row| {
            row.inna
                .map(|g| (g, Source::Inna))
                .or_else(|| entered.get(&row.key).map(|g| (*g, Source::Entered)))
        })
        .collect();
    let fixed: f64 = course
        .rows
        .iter()
        .zip(&values)
        .filter_map(|(row, value)| value.map(|(g, _)| g * row.weight))
        .sum();
    let open: f64 = course
        .rows
        .iter()
        .zip(&values)
        .filter(|(_, value)| value.is_none())
        .map(|(row, _)| row.weight)
        .sum();
    let earned = if total > 0.0 { fixed / total } else { 0.0 };
    let outcome = match target {
        _ if open <= 0.0 => Outcome::Complete,
        None => Outcome::NoTarget,
        Some(target) => {
            let needed = (target.minimum() * total - fixed) / open;
            if needed <= 1e-9 {
                Outcome::Secured { worst: earned }
            } else if needed > 10.0 + 1e-9 {
                Outcome::Unreachable {
                    best: (fixed + 10.0 * open) / total,
                }
            } else {
                Outcome::Needed(round_up(needed).min(10.0))
            }
        }
    };
    if let Outcome::Needed(grade) = outcome {
        for value in values.iter_mut().filter(|v| v.is_none()) {
            *value = Some((grade, Source::Needed));
        }
    }
    let projected = (total > 0.0 && values.iter().all(Option::is_some)).then(|| {
        course
            .rows
            .iter()
            .zip(&values)
            .map(|(row, value)| value.unwrap().0 * row.weight)
            .sum::<f64>()
            / total
    });
    Plan {
        values,
        outcome,
        projected,
        earned,
    }
}

/// The whole-number final grade an unrounded average gives (6.5 → 7).
pub fn final_grade(average: f64) -> u8 {
    (average + 0.5 + 1e-9).floor().clamp(0.0, 10.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(items: serde_json::Value) -> AssignmentModule {
        serde_json::from_value(serde_json::json!({"moduleName":"TEST", "assignments":items}))
            .unwrap()
    }

    fn english() -> Course {
        Course::from_module(&module(serde_json::json!([
            {"assignmentId":"a", "name":"Reading", "grade":"7,5", "weight":20},
            {"assignmentId":"b", "name":"Writing", "grade":"8,0", "weight":15},
            {"assignmentId":"c", "name":"Final project", "weight":30},
            {"assignmentId":"d", "name":"Exam", "weight":35}
        ])))
    }

    #[test]
    fn inna_grades_are_filled_in_and_the_rest_get_the_needed_grade() {
        let course = english();
        assert_eq!(course.rows.len(), 4);
        let plan = plan(&course, &HashMap::new(), Some(Target::Final(7)));
        // 6.5 * 100 = 650; INNA gives 150 + 120 = 270; 380 over 65% → 5.846… → 5.9
        assert_eq!(plan.outcome, Outcome::Needed(5.9));
        assert_eq!(plan.values[0], Some((7.5, Source::Inna)));
        assert_eq!(plan.values[2], Some((5.9, Source::Needed)));
        assert_eq!(plan.values[3], Some((5.9, Source::Needed)));
        assert!(plan.projected.unwrap() >= 6.5);
        assert_eq!(final_grade(plan.projected.unwrap()), 7);
        assert!((plan.earned - 2.7).abs() < 1e-9);
    }

    #[test]
    fn a_what_if_grade_changes_what_the_other_items_need() {
        let course = english();
        let entered = HashMap::from([("c".to_string(), 9.0)]);
        let plan = plan(&course, &entered, Some(Target::Final(7)));
        // 650 - 270 - 270 = 110 over 35% → 3.14… → 3.2
        assert_eq!(plan.values[2], Some((9.0, Source::Entered)));
        assert_eq!(plan.outcome, Outcome::Needed(3.2));
        assert_eq!(plan.values[3], Some((3.2, Source::Needed)));
    }

    #[test]
    fn unreachable_secured_and_complete_courses_are_reported() {
        let course = english();
        let plan_10 = plan(&course, &HashMap::new(), Some(Target::Final(10)));
        assert_eq!(plan_10.outcome, Outcome::Unreachable { best: 9.2 });
        assert!(plan_10.values[2].is_none());
        assert_eq!(plan_10.projected, None);
        let plan_2 = plan(&course, &HashMap::new(), Some(Target::Final(2)));
        assert!(matches!(plan_2.outcome, Outcome::Secured { .. }));
        let none = plan(&course, &HashMap::new(), None);
        assert_eq!(none.outcome, Outcome::NoTarget);
        let all = HashMap::from([("c".to_string(), 6.0), ("d".to_string(), 6.0)]);
        let done = plan(&course, &all, Some(Target::Final(7)));
        assert_eq!(done.outcome, Outcome::Complete);
        assert!((done.projected.unwrap() - 6.6).abs() < 1e-9);
        assert_eq!(final_grade(done.projected.unwrap()), 7);
    }

    #[test]
    fn missing_assessments_become_a_rest_of_course_row() {
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"Test 1", "grade":"8", "weight":20},
            {"name":"Homework", "grade":"9"},
            {"name":"Attendance", "grade":"S", "weight":0}
        ])));
        assert_eq!(course.missing_weight, 1);
        assert_eq!(course.rows.len(), 2);
        assert!(course.rows[1].unlisted());
        assert_eq!(course.rows[1].weight, 80.0);
        let plan = plan(&course, &HashMap::new(), Some(Target::Average(6.0)));
        assert_eq!(plan.outcome, Outcome::Needed(5.5));
    }

    #[test]
    fn letter_grades_and_overweight_courses_are_not_calculated() {
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"Lab", "grade":"S", "weight":30}, {"name":"Exam", "weight":70}
        ])));
        assert_eq!(course.non_numeric, 1);
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"A", "weight":80}, {"name":"B", "weight":80}
        ])));
        assert!(course.invalid_total);
    }

    #[test]
    fn final_grades_round_half_up() {
        assert_eq!(final_grade(6.49), 6);
        assert_eq!(final_grade(6.5), 7);
        assert_eq!(final_grade(4.5), 5);
        assert_eq!(final_grade(10.0), 10);
        assert_eq!(Target::Final(5).minimum(), grades::PASS_MARK);
        assert_eq!(Target::Final(10).minimum(), 9.5);
    }
}
