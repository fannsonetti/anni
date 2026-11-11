These fixtures use example people and placeholder account, course, and record IDs.
Keep real session cookies, account identifiers, private messages, and downloaded
INNA responses out of this directory. Local application data and build artifacts
are excluded from Git.
# Teacher absences

`teacher_absences.json` is a synthetic example using the `name`, `module`, `timeFrom`, `timeTo`, and `group` fields verified against INNA's Forföll kennara response. It contains no real teacher data. The matching tests supply dated timetable entries separately.

# Grade rules

`grade_rules.json` is a synthetic assessment plan in the shape of INNA's course Einkunnir tab (`/api/StudentGrades/GetStudentGradeRuleGrades`): parts linked by `gradeRuleId` / `weighsUpTo`, each `weight` a percent of its parent, `recalculatedWeight` its percent of the final grade, and `nrBestSub` the number of best sub-parts that count. Names, IDs, and grades are made up.
