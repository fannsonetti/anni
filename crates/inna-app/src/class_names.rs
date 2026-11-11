//! Your own names for classes, e.g. `HAGF1ÞF05 = Hagfræði`, with one name per language.
//!
//! The course code on the left is shared by every language and never changes; only the name on
//! the right is per language. The chosen language decides which names the app shows.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Icelandic,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Icelandic];

    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Icelandic => "Íslenska",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassRename {
    /// The class name as INNA writes it, e.g. `HAGF1ÞF05`.
    pub code: String,
    /// What to show instead, per language. A missing or blank name keeps the built-in name.
    pub names: BTreeMap<Language, String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassNames {
    pub language: Language,
    pub renames: Vec<ClassRename>,
}

static CURRENT: RwLock<ClassNames> = RwLock::new(ClassNames {
    language: Language::English,
    renames: Vec::new(),
});

impl ClassNames {
    /// Your name for `code` in the chosen language. An exact code match wins; otherwise the
    /// longest code that `code` starts with, so `ENSK` renames every English course.
    pub fn lookup(&self, code: &str) -> Option<&str> {
        let code = code.trim().to_lowercase();
        let mut best: Option<(usize, &str)> = None;
        for rename in &self.renames {
            let key = rename.code.trim().to_lowercase();
            let Some(name) = rename.names.get(&self.language).map(|n| n.trim()) else {
                continue;
            };
            if key.is_empty() || name.is_empty() {
                continue;
            }
            if key == code {
                return Some(name);
            }
            if code.starts_with(&key) && best.is_none_or(|(len, _)| key.len() > len) {
                best = Some((key.len(), name));
            }
        }
        best.map(|(_, name)| name)
    }

    pub fn path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("inna-client")
            .join("class_names.json")
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Could not load your class names.".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("Could not load your class names.".into()),
        }
    }

    fn save_to(&self, path: &Path) -> Result<(), String> {
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let temporary = path.with_extension("json.tmp");
            std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
            std::fs::rename(temporary, path)?;
            Ok(())
        };
        save().map_err(|error| {
            tracing::warn!("Could not save class names: {error}");
            "Could not save your class names. Please try again.".into()
        })
    }
}

/// Your name for `code` in the chosen language, if you set one.
pub fn custom_name(code: &str) -> Option<String> {
    CURRENT.read().ok()?.lookup(code).map(str::to_string)
}

/// Loads the saved names; called once at startup (not in demo mode).
pub fn load() -> Result<(), String> {
    let names = ClassNames::load_from(&ClassNames::path())?;
    if let Ok(mut current) = CURRENT.write() {
        *current = names;
    }
    Ok(())
}

fn current() -> ClassNames {
    CURRENT.read().map(|names| names.clone()).unwrap_or_default()
}

fn set_current(names: ClassNames, mock: bool) -> Result<(), String> {
    let result = if mock { Ok(()) } else { names.save_to(&ClassNames::path()) };
    if let Ok(mut current) = CURRENT.write() {
        *current = names;
    }
    result
}

pub fn settings_ui(ui: &mut egui::Ui, mock: bool) {
    use crate::ui::theme::*;
    use egui::RichText;

    let mut names = current();
    let before = names.clone();
    let tab_id = ui.id().with("class_names_tab");
    let error_id = ui.id().with("class_names_error");
    let mut tab: Language = ui.data(|d| d.get_temp(tab_id)).unwrap_or(names.language);

    section(ui, "Class names", |ui| {
        ui.horizontal(|ui| {
            ui.label("Show names in");
            for language in Language::ALL {
                if ui
                    .selectable_value(&mut names.language, language, language.label())
                    .clicked()
                {
                    tab = language;
                }
            }
        });
        hint(ui, "Give your classes your own names. The class code on the left stays the same in every language; each language tab has its own name on the right. The app uses the language picked above.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            for language in Language::ALL {
                let label = if language == names.language {
                    format!("{} (in use)", language.label())
                } else {
                    language.label().to_string()
                };
                ui.selectable_value(&mut tab, language, label);
            }
        });
        ui.separator();

        let mut remove = None;
        let field_width = ((ui.available_width() - 70.0) / 2.0).max(80.0);
        for (index, rename) in names.renames.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut rename.code)
                        .hint_text("Class code, e.g. HAGF1ÞF05")
                        .desired_width(field_width),
                );
                ui.label(RichText::new("=").size(16.0).family(semibold()));
                // Show the name the class has without your rename, if it has one.
                let code = rename.code.trim().to_uppercase();
                let built_in = crate::ui::assignments::built_in_subject(&code);
                let hint_text = if code.is_empty() || built_in == code {
                    "New name, e.g. Hagfræði".to_string()
                } else {
                    built_in.to_string()
                };
                ui.add(
                    egui::TextEdit::singleline(rename.names.entry(tab).or_default())
                        .hint_text(hint_text)
                        .desired_width(field_width),
                );
                if ui.small_button("×").on_hover_text("Remove this class").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            names.renames.remove(index);
        }
        if names.renames.is_empty() {
            hint(ui, "No renamed classes yet.");
        }
        ui.add_space(4.0);
        if ui.button("+ Add class").clicked() {
            names.renames.push(ClassRename::default());
        }
        if mock {
            ui.label(RichText::new("Demo mode · class names are temporary.").size(12.0).color(WARNING));
        }
        if let Some(error) = ui.data(|d| d.get_temp::<String>(error_id)) {
            ui.colored_label(WARNING, error);
        }
    });

    ui.data_mut(|d| d.insert_temp(tab_id, tab));
    // Empty entries are created by the text fields themselves; they don't count as a change.
    let strip = |n: &ClassNames| {
        let mut n = n.clone();
        for r in &mut n.renames {
            r.names.retain(|_, name| !name.is_empty());
        }
        n
    };
    let (names, before) = (strip(&names), strip(&before));
    if names != before {
        match set_current(names, mock) {
            Ok(()) => ui.data_mut(|d| d.remove::<String>(error_id)),
            Err(error) => ui.data_mut(|d| d.insert_temp(error_id, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename(code: &str, en: &str, is: &str) -> ClassRename {
        ClassRename {
            code: code.into(),
            names: [(Language::English, en.into()), (Language::Icelandic, is.into())].into(),
        }
    }

    #[test]
    fn names_follow_the_language_but_codes_stay_the_same() {
        let mut names = ClassNames {
            language: Language::Icelandic,
            renames: vec![rename("Hagf1þf05", "Economics", "Hagfræði"), rename("ENSK", "", "Enska")],
        };
        assert_eq!(names.lookup("HAGF1ÞF05"), Some("Hagfræði"));
        assert_eq!(names.lookup(" hagf1þf05 "), Some("Hagfræði"));
        assert_eq!(names.lookup("ENSK2HF05"), Some("Enska"));
        names.language = Language::English;
        assert_eq!(names.lookup("HAGF1ÞF05"), Some("Economics"));
        // A blank name in this language keeps the built-in name.
        assert_eq!(names.lookup("ENSK2HF05"), None);
        assert_eq!(names.lookup("LÍFF2VL05"), None);
    }

    #[test]
    fn exact_codes_beat_prefixes_and_longer_prefixes_win() {
        let names = ClassNames {
            language: Language::English,
            renames: vec![
                rename("STÆR", "Maths", ""),
                rename("STÆR2", "Maths 2", ""),
                rename("STÆR2A05", "Calculus", ""),
            ],
        };
        assert_eq!(names.lookup("STÆR2A05"), Some("Calculus"));
        assert_eq!(names.lookup("STÆR2B05"), Some("Maths 2"));
        assert_eq!(names.lookup("STÆR3A05"), Some("Maths"));
    }

    #[test]
    fn class_names_save_and_load() {
        let path = std::env::temp_dir().join(format!("anni-class-names-{}.json", std::process::id()));
        let names = ClassNames {
            language: Language::Icelandic,
            renames: vec![rename("Hagf1þf05", "Economics", "Hagfræði")],
        };
        names.save_to(&path).unwrap();
        assert_eq!(ClassNames::load_from(&path).unwrap(), names);
        std::fs::write(&path, b"broken").unwrap();
        assert!(ClassNames::load_from(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(ClassNames::load_from(&path).unwrap(), ClassNames::default());
    }
}
