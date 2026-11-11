//! Your own names for classes, e.g. `HAGF1ÞF05 = Hagfræði`, with one name per language.
//!
//! The course code on the left is shared by every language and never changes; only the name on
//! the right is per language. The app language (Settings → Language) decides which names show.
use crate::locale::{t, Language};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassRename {
    /// The class name as INNA writes it, e.g. `HAGF1ÞF05`.
    pub code: String,
    /// What to show instead, per language. A missing or blank name keeps the built-in name.
    pub names: BTreeMap<Language, String>,
}

/// Files saved before the app had one language also hold a `language` here; it is ignored.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassNames {
    pub renames: Vec<ClassRename>,
}

static CURRENT: RwLock<ClassNames> = RwLock::new(ClassNames {
    renames: Vec::new(),
});

impl ClassNames {
    /// Your name for `code` in `language`. An exact code match wins; otherwise the
    /// longest code that `code` starts with, so `ENSK` renames every English course.
    pub fn lookup(&self, language: Language, code: &str) -> Option<&str> {
        let code = code.trim().to_lowercase();
        let mut best: Option<(usize, &str)> = None;
        for rename in &self.renames {
            let key = rename.code.trim().to_lowercase();
            let Some(name) = rename.names.get(&language).map(|n| n.trim()) else {
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
                .map_err(|_| t("Could not load your class names.", "Ekki tókst að hlaða heitum áfanga.").to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(t("Could not load your class names.", "Ekki tókst að hlaða heitum áfanga.").into()),
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
            t("Could not save your class names. Please try again.", "Ekki tókst að vista heiti áfanga. Reyndu aftur.").into()
        })
    }
}

/// Your name for `code` in the app language, if you set one.
pub fn custom_name(code: &str) -> Option<String> {
    CURRENT.read().ok()?.lookup(crate::locale::language(), code).map(str::to_string)
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
    let in_use = crate::locale::language();
    let mut tab: Language = ui.data(|d| d.get_temp(tab_id)).unwrap_or(in_use);

    section(ui, t("Class names", "Heiti áfanga"), |ui| {
        hint(ui, t("Give your classes your own names. The class code on the left stays the same in every language; each language tab has its own name on the right. Anni shows the names for the app language.", "Gefðu áföngunum þín eigin heiti. Áfangaheitið vinstra megin er eins á öllum tungumálum; hvert tungumál hefur sitt eigið heiti hægra megin. Anni sýnir heitin fyrir tungumál forritsins."));
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            for language in Language::ALL {
                let label = if language == in_use {
                    tr!("{} (in use)", "{} (í notkun)", language.native_name())
                } else {
                    language.native_name().to_string()
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
                        .hint_text(t("Class code, e.g. HAGF1ÞF05", "Áfangaheiti, t.d. HAGF1ÞF05"))
                        .desired_width(field_width),
                );
                ui.label(RichText::new("=").size(16.0).family(semibold()));
                // Show the name the class has without your rename, if it has one.
                let code = rename.code.trim().to_uppercase();
                let built_in = crate::ui::assignments::built_in_subject(&code);
                let hint_text = if code.is_empty() || built_in == code {
                    t("New name, e.g. Hagfræði", "Nýtt heiti, t.d. Hagfræði").to_string()
                } else {
                    built_in.to_string()
                };
                ui.add(
                    egui::TextEdit::singleline(rename.names.entry(tab).or_default())
                        .hint_text(hint_text)
                        .desired_width(field_width),
                );
                if ui.small_button("×").on_hover_text(t("Remove this class", "Fjarlægja þennan áfanga")).clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            names.renames.remove(index);
        }
        if names.renames.is_empty() {
            hint(ui, t("No renamed classes yet.", "Engir áfangar með eigin heiti enn."));
        }
        ui.add_space(4.0);
        if ui.button(t("+ Add class", "+ Bæta við áfanga")).clicked() {
            names.renames.push(ClassRename::default());
        }
        if mock {
            ui.label(RichText::new(t("Demo mode · class names are temporary.", "Sýnishorn · heiti áfanga eru tímabundin.")).size(12.0).color(WARNING));
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
        let names = ClassNames {
            renames: vec![rename("Hagf1þf05", "Economics", "Hagfræði"), rename("ENSK", "", "Enska")],
        };
        let (en, is) = (Language::English, Language::Icelandic);
        assert_eq!(names.lookup(is, "HAGF1ÞF05"), Some("Hagfræði"));
        assert_eq!(names.lookup(is, " hagf1þf05 "), Some("Hagfræði"));
        assert_eq!(names.lookup(is, "ENSK2HF05"), Some("Enska"));
        assert_eq!(names.lookup(en, "HAGF1ÞF05"), Some("Economics"));
        // A blank name in this language keeps the built-in name.
        assert_eq!(names.lookup(en, "ENSK2HF05"), None);
        assert_eq!(names.lookup(en, "LÍFF2VL05"), None);
    }

    #[test]
    fn exact_codes_beat_prefixes_and_longer_prefixes_win() {
        let names = ClassNames {
            renames: vec![
                rename("STÆR", "Maths", ""),
                rename("STÆR2", "Maths 2", ""),
                rename("STÆR2A05", "Calculus", ""),
            ],
        };
        let en = Language::English;
        assert_eq!(names.lookup(en, "STÆR2A05"), Some("Calculus"));
        assert_eq!(names.lookup(en, "STÆR2B05"), Some("Maths 2"));
        assert_eq!(names.lookup(en, "STÆR3A05"), Some("Maths"));
    }

    #[test]
    fn class_names_save_and_load() {
        let path = std::env::temp_dir().join(format!("anni-class-names-{}.json", std::process::id()));
        let names = ClassNames {
            renames: vec![rename("Hagf1þf05", "Economics", "Hagfræði")],
        };
        names.save_to(&path).unwrap();
        assert_eq!(ClassNames::load_from(&path).unwrap(), names);
        // Files from before the app-wide language still load.
        std::fs::write(&path, br#"{"language":"Icelandic","renames":[]}"#).unwrap();
        assert!(ClassNames::load_from(&path).unwrap().renames.is_empty());
        std::fs::write(&path, b"broken").unwrap();
        assert!(ClassNames::load_from(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(ClassNames::load_from(&path).unwrap(), ClassNames::default());
    }
}
