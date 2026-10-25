//! English presentation with compatible INNA and day.month.year input formats.
use chrono::{Datelike, NaiveDate};

pub fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}")
}
pub fn number(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}
pub fn singular(value: usize) -> bool {
    value == 1
}
pub fn counted(value: usize, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        number(value),
        if singular(value) { one } else { many }
    )
}
pub fn weekday(date: NaiveDate) -> &'static str {
    [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][date.weekday().num_days_from_monday() as usize]
}
pub fn month(date: NaiveDate) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][date.month0() as usize]
}
pub fn date(date: NaiveDate) -> String {
    format!("{} {} {}", date.day(), month(date), date.year())
}
pub fn day(date: NaiveDate) -> String {
    format!("{}, {} {}", weekday(date), date.day(), month(date))
}
pub fn short_date(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn sort_key(text: &str) -> Vec<u32> {
    // Keep school course names in their original, consistent alphabetical order.
    let alphabet = "aábdðeéfghiíjklmnoóprstuúvxyýþæö";
    text.to_lowercase()
        .chars()
        .map(|c| {
            alphabet
                .chars()
                .position(|a| a == c)
                .map(|i| i as u32 + 100)
                .unwrap_or(c as u32 + 1000)
        })
        .collect()
}
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    ["%d.%m.%Y", "%Y-%m-%d"]
        .iter()
        .find_map(|f| NaiveDate::parse_from_str(text.trim(), f).ok())
}
pub fn resource(name: &str) -> &str {
    match name {
        "timetable" => "timetable",
        "announcements" => "announcements",
        "assignments" => "assignments",
        "homework" => "homework",
        "todo" => "to-do list",
        "connection" => "connection",
        _ => name,
    }
}
pub fn error(message: &str) -> String {
    match message {
        "network unavailable" => "No network connection is available.".into(),
        "INNA session expired" | "not authenticated" => "Your INNA sign-in has expired.".into(),
        "failed to parse INNA response" | "unexpected INNA response" => {
            "Could not read the response from INNA.".into()
        }
        "invalid XSRF state" => "Please reconnect to INNA.".into(),
        "INNA did not return a student ID" => "INNA did not return a student ID.".into(),
        "Could not forget your sign-in. Close any Anni sign-in windows and try again."
        | "Could not save your INNA sign-in on this computer." => message.into(),
        _ => "Something went wrong. Try again or reconnect to INNA.".into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_presentation_keeps_existing_date_inputs_compatible() {
        assert_eq!(decimal(7.25, 2), "7.25");
        assert_eq!(number(1234567), "1,234,567");
        assert_eq!(counted(1, "course", "courses"), "1 course");
        assert_eq!(counted(11, "course", "courses"), "11 courses");
        assert_eq!(counted(21, "course", "courses"), "21 courses");
        assert_eq!(day(parse_date("18.10.2026").unwrap()), "Sunday, 18 October");
        assert_eq!(parse_date("2026-10-18"), parse_date("18.10.2026"));
        assert!(sort_key("Íslenska") < sort_key("Stærðfræði"));
        assert!(sort_key("Eðlisfræði") < sort_key("Enska"));
    }
}
