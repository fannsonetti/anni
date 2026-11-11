//! Final grade calculator: starts from the grades INNA has already given, and
//! works out the grade needed on the remaining assessments for a desired final grade.
//!
//! The course structure comes from the course's Einkunnir tab in INNA (its
//! assessment plan): a tree of parts, each weighing a percentage of its parent,
//! where some groups only count the best few results. Courses without a plan
//! fall back to the weights listed on their assignments.
use crate::grades;
use inna_core::models::{AssignmentModule, GradeRule};
use std::collections::HashMap;

/// Key of the part standing in for weight INNA has not listed yet.
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
    /// course code → part key → what the student typed as a "what if" grade.
    pub entries: HashMap<String, HashMap<String, String>>,
}

/// One part of the course: a graded item, or a group of parts.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub key: String,
    pub name: String,
    /// Percent of the parent part.
    pub weight: f64,
    /// Percent of the final grade.
    pub share: f64,
    /// The grade INNA already has for this part.
    pub inna: Option<f64>,
    /// Only this many of the best sub-parts count.
    pub best: Option<usize>,
    pub children: Vec<usize>,
    /// 0 for the final grade, 1 for its parts, and so on.
    pub depth: usize,
}

impl Part {
    pub fn unlisted(&self) -> bool {
        self.key == UNLISTED
    }
    pub fn is_group(&self) -> bool {
        !self.children.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structure {
    /// The assessment plan from the course's Einkunnir tab.
    Plan,
    /// Only assignment weights were available.
    Assignments,
}

/// A course's parts in display order; `parts[0]` is the final grade.
#[derive(Debug)]
pub struct Course {
    pub parts: Vec<Part>,
    pub structure: Structure,
    /// Items with a grade but no weight, which cannot count.
    pub missing_weight: usize,
    /// Weighted items graded with a letter or word (e.g. "S"), which cannot be averaged.
    pub non_numeric: usize,
    /// Weights add up to more than 100%.
    pub invalid_total: bool,
}

fn root(name: &str) -> Part {
    Part {
        key: "\u{0}final".into(),
        name: name.into(),
        weight: 100.0,
        share: 100.0,
        inna: None,
        best: None,
        children: Vec::new(),
        depth: 0,
    }
}

fn is_letter_grade(text: &str) -> bool {
    let text = text.trim();
    !matches!(text, "" | "-" | "—") && grades::numeric_grade(text).is_none()
}

impl Course {
    pub fn from_module(module: &AssignmentModule) -> Self {
        Self::from_rules(&module.grade_rules).unwrap_or_else(|| Self::from_assignments(module))
    }

    /// Uses INNA's assessment plan when it has at least one part under the final grade.
    pub fn from_rules(rules: &[GradeRule]) -> Option<Self> {
        let top = rules
            .iter()
            .position(|r| r.final_grade)
            .or_else(|| rules.iter().position(|r| r.weighs_up_to.is_none()))?;
        let top_id = rules[top].grade_rule_id?;
        if !rules.iter().any(|r| r.weighs_up_to == Some(top_id)) {
            return None;
        }
        let mut course = Self {
            parts: vec![root(rules[top].title())],
            structure: Structure::Plan,
            missing_weight: 0,
            non_numeric: 0,
            invalid_total: false,
        };
        course.parts[0].inna = rules[top].grade.as_deref().and_then(grades::numeric_grade);
        course.add_rule_children(rules, top_id, 0, 100.0);
        let listed: f64 = course.parts[0]
            .children
            .iter()
            .map(|&c| course.parts[c].weight)
            .sum();
        course.finish(listed);
        Some(course)
    }

    fn add_rule_children(&mut self, rules: &[GradeRule], id: i64, parent: usize, parent_share: f64) {
        let depth = self.parts[parent].depth + 1;
        if depth > 8 {
            return; // A malformed plan must not recurse forever.
        }
        let mut children: Vec<(usize, &GradeRule)> = rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.weighs_up_to == Some(id) && r.grade_rule_id != Some(id))
            .collect();
        children.sort_by_key(|(index, r)| (r.order.unwrap_or(i64::MAX), *index));
        let sibling_total: f64 = children
            .iter()
            .filter_map(|(_, r)| grades::weight(r.weight.as_ref()))
            .sum();
        for (_, rule) in children {
            let Some(weight) = grades::weight(rule.weight.as_ref()).filter(|w| *w > 0.0) else {
                continue;
            };
            // The final grade's parts are out of 100%; a group's parts share the group.
            let out_of = if parent == 0 { 100.0 } else { sibling_total };
            let computed = parent_share * weight / out_of;
            let share = grades::weight(rule.recalculated_weight.as_ref()).unwrap_or(computed);
            let grade_text = rule.grade.as_deref().unwrap_or("");
            let index = self.parts.len();
            self.parts.push(Part {
                key: rule
                    .grade_rule_id
                    .map(|id| format!("rule:{id}"))
                    .unwrap_or_else(|| format!("rule#{index}")),
                name: rule.title().to_string(),
                weight,
                share,
                inna: grades::numeric_grade(grade_text),
                best: rule.nr_best_sub.map(|n| n as usize).filter(|n| *n > 0),
                children: Vec::new(),
                depth,
            });
            self.parts[parent].children.push(index);
            if let Some(child_id) = rule.grade_rule_id {
                self.add_rule_children(rules, child_id, index, share);
            }
            if !self.parts[index].is_group() && is_letter_grade(grade_text) {
                self.non_numeric += 1;
            }
        }
    }

    /// Builds a flat plan from assignment weights, for courses without an INNA plan.
    pub fn from_assignments(module: &AssignmentModule) -> Self {
        let mut course = Self {
            parts: vec![root("")],
            structure: Structure::Assignments,
            missing_weight: 0,
            non_numeric: 0,
            invalid_total: false,
        };
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
            if is_letter_grade(grade_text) {
                course.non_numeric += 1;
                continue;
            }
            let part = course.parts.len();
            course.parts.push(Part {
                key: item
                    .assignment_id
                    .clone()
                    .unwrap_or_else(|| format!("#{index} {}", item.name_str())),
                name: item.name_str().to_string(),
                weight,
                share: weight,
                inna: grade,
                best: None,
                children: Vec::new(),
                depth: 1,
            });
            course.parts[0].children.push(part);
        }
        course.finish(listed);
        course
    }

    /// Checks the total and adds weight that is not listed yet as its own part.
    fn finish(&mut self, listed: f64) {
        self.invalid_total = listed > 100.000001;
        let rest = 100.0 - listed;
        if rest > 0.05 && !self.invalid_total {
            let index = self.parts.len();
            self.parts.push(Part {
                key: UNLISTED.into(),
                name: String::new(),
                weight: rest,
                share: rest,
                inna: None,
                best: None,
                children: Vec::new(),
                depth: 1,
            });
            self.parts[0].children.push(index);
        }
    }

    /// True when there is something to calculate beyond a placeholder for unlisted weight.
    pub fn has_parts(&self) -> bool {
        self.parts.iter().skip(1).any(|p| !p.unlisted())
    }

    pub fn calculable(&self) -> bool {
        !self.invalid_total && self.non_numeric == 0 && self.parts.len() > 1
    }

    fn leaves(&self) -> impl Iterator<Item = usize> + '_ {
        (1..self.parts.len()).filter(|&i| !self.parts[i].is_group())
    }

    /// The value of a part from its leaves' values.
    fn value(&self, index: usize, leaf: &impl Fn(usize) -> Option<f64>) -> Option<f64> {
        let part = &self.parts[index];
        if !part.is_group() {
            return leaf(index);
        }
        // A grade the teacher set on a group stands for the whole group.
        if index != 0 {
            if let Some(grade) = part.inna {
                return Some(grade);
            }
        }
        let mut values = part
            .children
            .iter()
            .map(|&c| Some((self.parts[c].weight, self.value(c, leaf)?)))
            .collect::<Option<Vec<_>>>()?;
        if let Some(best) = part.best {
            values.sort_by(|a, b| b.1.total_cmp(&a.1));
            values.truncate(best);
        }
        let total: f64 = values.iter().map(|(w, _)| w).sum();
        let total = if index == 0 { total.max(100.0) } else { total };
        (total > 0.0).then(|| values.iter().map(|(w, v)| w * v).sum::<f64>() / total)
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
    /// One value per course part (leaves only): its grade and where it came from.
    pub values: Vec<Option<(f64, Source)>>,
    pub outcome: Outcome,
    /// The final average once every item has a value.
    pub projected: Option<f64>,
    /// The final average with 0 on everything still open.
    pub earned: f64,
}

/// Rounds up to one decimal, so reaching the shown grade always reaches the target.
fn round_up(value: f64) -> f64 {
    (value * 10.0 - 1e-9).ceil() / 10.0
}

pub fn plan(course: &Course, entered: &HashMap<String, f64>, target: Option<Target>) -> Plan {
    let mut values: Vec<Option<(f64, Source)>> = course
        .parts
        .iter()
        .map(|part| {
            if part.is_group() {
                return None;
            }
            part.inna
                .map(|g| (g, Source::Inna))
                .or_else(|| entered.get(&part.key).map(|g| (*g, Source::Entered)))
        })
        .collect();
    let open: Vec<usize> = course.leaves().filter(|&i| values[i].is_none()).collect();
    let with = |x: f64| {
        course
            .value(0, &|i| Some(values[i].map(|(g, _)| g).unwrap_or(x)))
            .unwrap_or(0.0)
    };
    let earned = with(0.0);
    if let Some(final_grade) = course.parts.first().and_then(|p| p.inna) {
        return Plan {
            values,
            outcome: Outcome::Complete,
            projected: Some(final_grade),
            earned,
        };
    }
    let outcome = match target {
        _ if open.is_empty() => Outcome::Complete,
        None => Outcome::NoTarget,
        Some(target) => {
            let minimum = target.minimum();
            let best = with(10.0);
            if earned >= minimum - 1e-9 {
                Outcome::Secured { worst: earned }
            } else if best < minimum - 1e-9 {
                Outcome::Unreachable { best }
            } else {
                // The final grade never drops when the open grades rise, so halve the range.
                let (mut low, mut high) = (0.0, 10.0);
                for _ in 0..50 {
                    let middle = (low + high) / 2.0;
                    if with(middle) >= minimum - 1e-9 {
                        high = middle;
                    } else {
                        low = middle;
                    }
                }
                let mut needed = round_up(high).min(10.0);
                while needed < 10.0 && with(needed) < minimum - 1e-9 {
                    needed = round_up(needed + 0.1);
                }
                Outcome::Needed(needed)
            }
        }
    };
    if let Outcome::Needed(grade) = outcome {
        for &i in &open {
            values[i] = Some((grade, Source::Needed));
        }
    }
    let projected = course
        .leaves()
        .all(|i| values[i].is_some())
        .then(|| course.value(0, &|i| values[i].map(|(g, _)| g)))
        .flatten();
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

    fn rules(name: &str) -> Vec<GradeRule> {
        let all: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/grade_rules.json")).unwrap();
        serde_json::from_value(all[name].clone()).unwrap()
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
        assert_eq!(course.structure, Structure::Assignments);
        assert_eq!(course.parts.len(), 5);
        let plan = plan(&course, &HashMap::new(), Some(Target::Final(7)));
        // 6.5 * 100 = 650; INNA gives 150 + 120 = 270; 380 over 65% → 5.846… → 5.9
        assert_eq!(plan.outcome, Outcome::Needed(5.9));
        assert_eq!(plan.values[1], Some((7.5, Source::Inna)));
        assert_eq!(plan.values[3], Some((5.9, Source::Needed)));
        assert_eq!(plan.values[4], Some((5.9, Source::Needed)));
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
        assert_eq!(plan.values[3], Some((9.0, Source::Entered)));
        assert_eq!(plan.outcome, Outcome::Needed(3.2));
        assert_eq!(plan.values[4], Some((3.2, Source::Needed)));
    }

    #[test]
    fn unreachable_secured_and_complete_courses_are_reported() {
        let course = english();
        let plan_10 = plan(&course, &HashMap::new(), Some(Target::Final(10)));
        assert_eq!(plan_10.outcome, Outcome::Unreachable { best: 9.2 });
        assert!(plan_10.values[3].is_none());
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
    fn missing_assessments_become_a_rest_of_course_part() {
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"Test 1", "grade":"8", "weight":20},
            {"name":"Homework", "grade":"9"},
            {"name":"Attendance", "grade":"S", "weight":0}
        ])));
        assert_eq!(course.missing_weight, 1);
        assert_eq!(course.parts.len(), 3);
        assert!(course.parts[2].unlisted());
        assert_eq!(course.parts[2].weight, 80.0);
        let plan = plan(&course, &HashMap::new(), Some(Target::Average(6.0)));
        assert_eq!(plan.outcome, Outcome::Needed(5.5));
        assert!(course.calculable());
    }

    #[test]
    fn letter_grades_and_overweight_courses_are_not_calculated() {
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"Lab", "grade":"S", "weight":30}, {"name":"Exam", "weight":70}
        ])));
        assert_eq!(course.non_numeric, 1);
        assert!(!course.calculable());
        let course = Course::from_module(&module(serde_json::json!([
            {"name":"A", "weight":80}, {"name":"B", "weight":80}
        ])));
        assert!(course.invalid_total);
        assert!(!course.calculable());
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

    #[test]
    fn the_inna_plan_counts_only_the_best_partial_tests() {
        // Partial tests 20% (best 2 of 3: 7 and 8 so far), attendance 10%, final exam 70%.
        let mut m = module(serde_json::json!([{"name":"Ignored", "grade":"1", "weight":100}]));
        m.grade_rules = rules("best_of");
        let course = Course::from_module(&m);
        assert_eq!(course.structure, Structure::Plan);
        let names: Vec<_> = course.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["Final grade", "Partial tests", "Partial test 1", "Partial test 2", "Partial test 3", "Attendance", "Final exam"]
        );
        assert_eq!(course.parts[1].best, Some(2));
        assert_eq!(course.parts[2].share, 10.0);
        let p = plan(&course, &HashMap::new(), Some(Target::Final(7)));
        // With x ≤ 7 the best two stay 7 and 8: 0.2 × 7.5 + 0.8x ≥ 6.5 → x ≥ 6.25 → 6.3.
        assert_eq!(p.outcome, Outcome::Needed(6.3));
        assert_eq!(p.values[4], Some((6.3, Source::Needed)));
        assert_eq!(p.values[6], Some((6.3, Source::Needed)));
        assert!(p.projected.unwrap() >= 6.5);
        assert!((p.earned - 1.5).abs() < 1e-9);
        // A strong third partial test replaces the 7.
        let entered = HashMap::from([("rule:3".to_string(), 10.0)]);
        let p = plan(&course, &entered, Some(Target::Final(7)));
        // 0.2 × 9 + 0.8x ≥ 6.5 → x ≥ 5.875 → 5.9
        assert_eq!(p.outcome, Outcome::Needed(5.9));
    }

    #[test]
    fn nested_groups_use_their_own_weights_in_any_order() {
        let mut m = module(serde_json::json!([]));
        m.grade_rules = rules("nested");
        let course = Course::from_module(&m);
        // Coursework 30% holds three projects weighted 50/30/16 (96 listed); exam 70%.
        assert_eq!(course.parts[1].name, "Coursework");
        assert_eq!(course.parts[1].children.len(), 3);
        assert_eq!(course.parts[0].children.len(), 2);
        let entered = HashMap::from([("rule:22".to_string(), 8.0)]);
        let p = plan(&course, &entered, None);
        assert_eq!(p.outcome, Outcome::NoTarget);
        let all = HashMap::from([("rule:22".to_string(), 8.0), ("rule:12".to_string(), 6.0)]);
        let p = plan(&course, &all, None);
        // Coursework = (50×9 + 30×7 + 16×8) / 96 = 8.1458…; final = 0.3 × that + 0.7 × 6
        let coursework = (50.0 * 9.0 + 30.0 * 7.0 + 16.0 * 8.0) / 96.0;
        assert!((p.projected.unwrap() - (0.3 * coursework + 4.2)).abs() < 1e-9);
    }

    #[test]
    fn plans_without_parts_fall_back_and_published_final_grades_are_final() {
        let mut m = module(serde_json::json!([{"name":"Essay", "weight":100}]));
        m.grade_rules = rules("empty");
        assert_eq!(Course::from_module(&m).structure, Structure::Assignments);
        m.grade_rules = rules("published");
        let course = Course::from_module(&m);
        let p = plan(&course, &HashMap::new(), Some(Target::Final(9)));
        assert_eq!(p.outcome, Outcome::Complete);
        assert_eq!(p.projected, Some(8.0));
    }
}
