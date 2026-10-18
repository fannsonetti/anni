//! Combine the dashboard, Heimavinna, and every enrolled course's work page.
use crate::{client::InnaClient, error::InnaError, models::*};
use chrono::{Duration, NaiveDate};

pub struct Schoolwork {
    pub assignments: Vec<AssignmentModule>,
    pub homework: Vec<Homework>,
    pub warnings: Vec<(String, String)>,
}

impl Schoolwork {
    fn warning(&mut self, resource: &str, source: &str, error: InnaError) -> Result<(), InnaError> {
        if matches!(
            error,
            InnaError::SessionExpired | InnaError::NotAuthenticated | InnaError::Network
        ) {
            return Err(error);
        }
        self.warnings.push((
            resource.into(),
            format!("{source}: {error}. Saved work retained where available."),
        ));
        Ok(())
    }
}

impl InnaClient {
    pub async fn schoolwork(
        &self,
        assignments: Vec<AssignmentModule>,
        homework: Vec<Homework>,
        timetable: &[TimetableEntry],
        today: NaiveDate,
    ) -> Result<Schoolwork, InnaError> {
        let mut result = Schoolwork {
            assignments,
            homework,
            warnings: Vec::new(),
        };
        match self.student_projects().await {
            Ok(rows) => result.assignments = rows,
            Err(error) => result.warning("assignments", "Global projects", error)?,
        }
        let mut courses = match self.courses().await {
            Ok(courses) => courses,
            Err(error) => {
                result.warning("assignments", "Course list", error)?;
                Vec::new()
            }
        };
        for (group, name) in result
            .assignments
            .iter()
            .map(|m| (&m.group_id, &m.module_name))
            .chain(timetable.iter().map(|t| (&t.group_id, &t.title_short)))
        {
            if group.as_deref().is_some_and(|g| !g.is_empty())
                && !courses.iter().any(|c| c.group_id == *group)
            {
                courses.push(Course {
                    group_id: group.clone(),
                    module_name: name.clone(),
                });
            }
        }
        match self.homework().await {
            Ok(rows) => replace_homework(&mut result.homework, "global", rows, None),
            Err(error) => result.warning("homework", "Heimavinna", error)?,
        }
        for course in courses {
            let Some(group) = course.group_id.as_deref().filter(|g| !g.is_empty()) else {
                continue;
            };
            let name = course.module_name.as_deref().unwrap_or(group);
            match self.course_assignments(group).await {
                Ok(rows) => {
                    if let Some(module) = result
                        .assignments
                        .iter_mut()
                        .find(|m| m.group_id.as_deref() == Some(group))
                    {
                        // The course page is authoritative; retain the module's grade categories.
                        let previous = std::mem::take(&mut module.assignments);
                        module.assignments = rows
                            .into_iter()
                            .map(|row| merge_assignment(&previous, row))
                            .collect();
                    } else {
                        result.assignments.push(AssignmentModule {
                            module_name: course.module_name.clone(),
                            group_id: course.group_id.clone(),
                            assignments: rows,
                            categories: serde_json::Value::Null,
                        });
                    }
                }
                Err(error) => {
                    result.warning("assignments", &format!("{name} · Verkefni og próf"), error)?
                }
            }
            for calendar in [false, true] {
                let source = format!("{}:{group}", if calendar { "calendar" } else { "course" });
                let request = if calendar {
                    self.homework_calendar(
                        group,
                        today - Duration::days(90),
                        today + Duration::days(120),
                    )
                    .await
                } else {
                    self.course_homework(group).await
                };
                match request {
                    Ok(rows) => {
                        replace_homework(&mut result.homework, &source, rows, Some(&course))
                    }
                    Err(error) => {
                        result.warning("homework", &format!("{name} · Heimavinna"), error)?
                    }
                }
            }
        }
        Ok(result)
    }
}

fn merge_assignment(previous: &[Assignment], row: Assignment) -> Assignment {
    let Some(old) = previous
        .iter()
        .find(|old| row.assignment_id.is_some() && old.assignment_id == row.assignment_id)
    else {
        return row;
    };
    let mut merged = row;
    // The course and global endpoints expose different date formats. Prefer the refreshed course date.
    if merged.hand_in_full_date.is_none() && merged.order_hand_in_date.is_none() {
        merged.order_hand_in_date = old.order_hand_in_date.clone();
    }
    if merged.assigned_full_date.is_none() && merged.order_assign_date.is_none() {
        merged.order_assign_date = old.order_assign_date.clone();
    }
    merged
}

fn replace_homework(
    saved: &mut Vec<Homework>,
    source: &str,
    rows: Vec<Homework>,
    course: Option<&Course>,
) {
    saved.retain(|h| {
        h.raw
            .get("_anniSource")
            .and_then(|v| v.as_str())
            .unwrap_or("global")
            != source
    });
    for mut row in rows {
        let Some(raw) = row.raw.as_object_mut() else {
            continue;
        };
        // Calendar rows may contain only a teaching goal. Those are not homework.
        if source.starts_with("calendar:")
            && !raw
                .get("homework")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty())
        {
            continue;
        }
        raw.insert("_anniSource".into(), source.into());
        if let Some(course) = course {
            if let Some(name) = &course.module_name {
                raw.entry("moduleName")
                    .or_insert_with(|| name.clone().into());
            }
            if let Some(group) = &course.group_id {
                raw.entry("groupId").or_insert_with(|| group.clone().into());
            }
        }
        saved.push(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn successful_empty_source_clears_only_its_own_rows() {
        let mut rows: Vec<Homework> = serde_json::from_value(json!([
            {"id":1,"text":"Old global"}, {"id":2,"homework":"Cached course","_anniSource":"calendar:g"}
        ])).unwrap();
        replace_homework(&mut rows, "global", vec![], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].raw["id"], 2);
        replace_homework(&mut rows, "calendar:g", vec![], None);
        assert!(rows.is_empty());
    }

    #[test]
    fn calendar_ignores_goals_and_supplies_course_context() {
        let mut saved = vec![];
        let rows = serde_json::from_value(json!([
            {"date":"19.10.2026","goal":"Understand fractions"},
            {"date":"19.10.2026","homework":"Read pages 3–5"}
        ]))
        .unwrap();
        let course = Course {
            module_name: Some("MATH".into()),
            group_id: Some("g".into()),
        };
        replace_homework(&mut saved, "calendar:g", rows, Some(&course));
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].raw["moduleName"], "MATH");
    }
}
