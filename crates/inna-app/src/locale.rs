//! App-wide display language (English or Icelandic) with compatible INNA and
//! day.month.year input formats.
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Icelandic,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Icelandic];

    /// Each language is always named in itself so it can be found in either.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Icelandic => "Íslenska",
        }
    }
}

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

// Tests run in parallel; each test thread keeps its own language so they cannot interfere.
#[cfg(test)]
thread_local! {
    static TEST_LANGUAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub fn set_language(language: Language) {
    #[cfg(test)]
    TEST_LANGUAGE.with(|current| current.set(language as u8));
    #[cfg(not(test))]
    LANGUAGE.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    #[cfg(test)]
    let stored = TEST_LANGUAGE.with(|current| current.get());
    #[cfg(not(test))]
    let stored = LANGUAGE.load(Ordering::Relaxed);
    match stored {
        1 => Language::Icelandic,
        _ => Language::English,
    }
}

pub fn icelandic() -> bool {
    language() == Language::Icelandic
}

/// Picks the text for the current language.
pub fn t(en: &'static str, is: &'static str) -> &'static str {
    if icelandic() {
        is
    } else {
        en
    }
}

/// `tr!("English {x}", "Íslenska {x}", x = ...)` formats the text for the current language.
macro_rules! tr {
    ($en:literal, $is:literal $($args:tt)*) => {
        if $crate::locale::icelandic() {
            format!($is $($args)*)
        } else {
            format!($en $($args)*)
        }
    };
}

pub fn decimal(value: f64, places: usize) -> String {
    decimal_in(language(), value, places)
}
fn decimal_in(language: Language, value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    match language {
        Language::English => text,
        Language::Icelandic => text.replace('.', ","),
    }
}
pub fn number(value: usize) -> String {
    number_in(language(), value)
}
fn number_in(language: Language, value: usize) -> String {
    let separator = match language {
        Language::English => ',',
        Language::Icelandic => '.',
    };
    let digits = value.to_string();
    let mut result = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push(separator);
        }
        result.push(c);
    }
    result
}
pub fn singular(value: usize) -> bool {
    // Icelandic uses the singular for 1, 21, 31, … but not 11, 111, …
    match language() {
        Language::English => value == 1,
        Language::Icelandic => value % 10 == 1 && value % 100 != 11,
    }
}
/// `counted(n, ("course", "courses"), ("áfangi", "áfangar"))`
pub fn counted(value: usize, en: (&str, &str), is: (&str, &str)) -> String {
    let (one, many) = if icelandic() { is } else { en };
    format!(
        "{} {}",
        number(value),
        if singular(value) { one } else { many }
    )
}
const WEEKDAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const WEEKDAYS_IS: [&str; 7] = [
    "mánudagur",
    "þriðjudagur",
    "miðvikudagur",
    "fimmtudagur",
    "föstudagur",
    "laugardagur",
    "sunnudagur",
];
const SHORT_WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const SHORT_WEEKDAYS_IS: [&str; 7] = ["mán", "þri", "mið", "fim", "fös", "lau", "sun"];
const MONTHS_EN: [&str; 12] = [
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
];
const MONTHS_IS: [&str; 12] = [
    "janúar",
    "febrúar",
    "mars",
    "apríl",
    "maí",
    "júní",
    "júlí",
    "ágúst",
    "september",
    "október",
    "nóvember",
    "desember",
];
const SHORT_MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const SHORT_MONTHS_IS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "maí", "jún", "júl", "ágú", "sep", "okt", "nóv", "des",
];

pub fn weekday(date: NaiveDate) -> &'static str {
    weekday_in(language(), date)
}
fn weekday_in(language: Language, date: NaiveDate) -> &'static str {
    let names = match language {
        Language::English => WEEKDAYS_EN,
        Language::Icelandic => WEEKDAYS_IS,
    };
    names[date.weekday().num_days_from_monday() as usize]
}
pub fn short_weekday(date: NaiveDate) -> &'static str {
    let names = match language() {
        Language::English => SHORT_WEEKDAYS_EN,
        Language::Icelandic => SHORT_WEEKDAYS_IS,
    };
    names[date.weekday().num_days_from_monday() as usize]
}
pub fn month(date: NaiveDate) -> &'static str {
    month_in(language(), date)
}
fn month_in(language: Language, date: NaiveDate) -> &'static str {
    let names = match language {
        Language::English => MONTHS_EN,
        Language::Icelandic => MONTHS_IS,
    };
    names[date.month0() as usize]
}
pub fn short_month(date: NaiveDate) -> &'static str {
    let names = match language() {
        Language::English => SHORT_MONTHS_EN,
        Language::Icelandic => SHORT_MONTHS_IS,
    };
    names[date.month0() as usize]
}
/// "18 October 2026" / "18. október 2026"
pub fn date(date: NaiveDate) -> String {
    date_in(language(), date)
}
fn date_in(language: Language, date: NaiveDate) -> String {
    format!("{} {}", day_month_in(language, date), date.year())
}
/// "18 October" / "18. október"
pub fn day_month(date: NaiveDate) -> String {
    day_month_in(language(), date)
}
fn day_month_in(language: Language, date: NaiveDate) -> String {
    match language {
        Language::English => format!("{} {}", date.day(), month_in(language, date)),
        Language::Icelandic => format!("{}. {}", date.day(), month_in(language, date)),
    }
}
/// "18 Oct" / "18. okt"
pub fn short_day_month(date: NaiveDate) -> String {
    match language() {
        Language::English => format!("{:02} {}", date.day(), short_month(date)),
        Language::Icelandic => format!("{}. {}", date.day(), short_month(date)),
    }
}
/// "Sunday, 18 October" / "sunnudagur, 18. október"
pub fn day(date: NaiveDate) -> String {
    day_in(language(), date)
}
fn day_in(language: Language, date: NaiveDate) -> String {
    format!(
        "{}, {}",
        weekday_in(language, date),
        day_month_in(language, date)
    )
}
pub fn short_date(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}
/// Upper-cases the first letter, for Icelandic names that start a line.
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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
        "timetable" => t("timetable", "stundatafla"),
        "announcements" => t("announcements", "tilkynningar"),
        "assignments" => t("assignments", "verkefni"),
        "homework" => t("homework", "heimavinna"),
        "grades" => t("grades", "einkunnir"),
        "todo" => t("to-do list", "verkefnalisti"),
        "connection" => t("connection", "tenging"),
        _ => name,
    }
}
pub fn error(message: &str) -> String {
    match message {
        "network unavailable" => {
            t("No network connection is available.", "Engin nettenging er tiltæk.").into()
        }
        "INNA session expired" | "not authenticated" => t(
            "Your INNA sign-in has expired.",
            "Innskráningin þín í INNU er útrunnin.",
        )
        .into(),
        "failed to parse INNA response" | "unexpected INNA response" => t(
            "Could not read the response from INNA.",
            "Ekki tókst að lesa svarið frá INNU.",
        )
        .into(),
        "invalid XSRF state" => t("Please reconnect to INNA.", "Vinsamlegast tengstu INNU aftur.").into(),
        "INNA did not return a student ID" => t(
            "INNA did not return a student ID.",
            "INNA skilaði ekki nemendanúmeri.",
        )
        .into(),
        "Could not forget your sign-in. Close any Anni sign-in windows and try again." => t(
            "Could not forget your sign-in. Close any Anni sign-in windows and try again.",
            "Ekki tókst að gleyma innskráningunni. Lokaðu innskráningargluggum Anni og reyndu aftur.",
        )
        .into(),
        "Could not save your INNA sign-in on this computer." => t(
            "Could not save your INNA sign-in on this computer.",
            "Ekki tókst að vista INNU-innskráninguna á þessari tölvu.",
        )
        .into(),
        _ => t(
            "Something went wrong. Try again or reconnect to INNA.",
            "Eitthvað fór úrskeiðis. Reyndu aftur eða tengstu INNU á ný.",
        )
        .into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_presentation_keeps_existing_date_inputs_compatible() {
        let en = Language::English;
        assert_eq!(decimal_in(en, 7.25, 2), "7.25");
        assert_eq!(number_in(en, 1234567), "1,234,567");
        assert_eq!(day_in(en, parse_date("18.10.2026").unwrap()), "Sunday, 18 October");
        assert_eq!(parse_date("2026-10-18"), parse_date("18.10.2026"));
        assert!(sort_key("Íslenska") < sort_key("Stærðfræði"));
        assert!(sort_key("Eðlisfræði") < sort_key("Enska"));
    }

    #[test]
    fn icelandic_presentation_uses_icelandic_names_and_separators() {
        let is = Language::Icelandic;
        let date = parse_date("18.10.2026").unwrap();
        assert_eq!(decimal_in(is, 7.25, 2), "7,25");
        assert_eq!(number_in(is, 1234567), "1.234.567");
        assert_eq!(day_in(is, date), "sunnudagur, 18. október");
        assert_eq!(date_in(is, date), "18. október 2026");
        assert_eq!(capitalize(weekday_in(is, date)), "Sunnudagur");
    }

    #[test]
    fn short_weekdays_are_whole_characters_in_both_languages() {
        // Slicing "miðvikudagur" by bytes would split "ð"; short names are listed instead.
        for (en, is) in SHORT_WEEKDAYS_EN.iter().zip(SHORT_WEEKDAYS_IS) {
            assert_eq!(en.chars().count(), 3);
            assert_eq!(is.chars().count(), 3);
        }
    }
}
