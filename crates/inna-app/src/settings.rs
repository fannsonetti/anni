//! Device preferences are separate from the account's school data.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopSide {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockStyle {
    Reactor,
    #[default]
    Digital,
    Analog,
}

/// Which list the desktop's work panel shows.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkTab {
    /// Assignments, plus exams when they are included.
    #[default]
    Assignments,
    /// Homework, plus INNA reminders when they are included.
    Homework,
}

impl WorkTab {
    pub const ALL: [Self; 2] = [Self::Assignments, Self::Homework];

    pub fn label(self) -> &'static str {
        match self {
            Self::Assignments => "Assignments",
            Self::Homework => "Homework",
        }
    }
}

/// Bump when a default layout changes; see `DesktopSettings::load_from`.
const LAYOUT_VERSION: u8 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopTheme {
    #[default]
    #[serde(alias = "Minimal")]
    Navy,
    Forest,
    #[serde(alias = "Arc")]
    Ocean,
    #[serde(alias = "Lavender")]
    Plum,
    #[serde(alias = "Sunset", alias = "Sand", alias = "IronMan")]
    Ember,
    Graphite,
    HighContrast,
}

/// Overlay text is white with a soft shadow, as in Hermes; themes only change the accents.
pub struct DesktopPalette {
    pub text: egui::Color32,
    pub muted: egui::Color32,
    pub accent: egui::Color32,
    pub background: egui::Color32,
    pub urgent: egui::Color32,
    pub soon: egui::Color32,
}

impl DesktopTheme {
    pub const ALL: [Self; 7] = [
        Self::Navy,
        Self::Forest,
        Self::Ocean,
        Self::Plum,
        Self::Ember,
        Self::Graphite,
        Self::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Navy => "Navy",
            Self::Forest => "Forest",
            Self::Ocean => "Ocean",
            Self::Plum => "Plum",
            Self::Ember => "Ember",
            Self::Graphite => "Graphite",
            Self::HighContrast => "High contrast",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::HighContrast => "Contrast",
            _ => self.label(),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Navy => "Sky blue accents on deep navy.",
            Self::Forest => "Fresh green accents with warm deadline colors.",
            Self::Ocean => "Bright teal accents on deep sea blue.",
            Self::Plum => "Soft violet accents with pink deadlines.",
            Self::Ember => "Warm orange accents on dark brown.",
            Self::Graphite => "Muted gold accents on neutral charcoal.",
            Self::HighContrast => "Bright white and yellow on dark panels for easier reading.",
        }
    }

    pub fn palette(self) -> DesktopPalette {
        use egui::Color32 as C;
        let (accent, urgent, soon, muted, background) = match self {
            Self::Navy => (0x7DB8FF, 0xFF6B6B, 0xFFA94D, 0xC9CED8, 0x0E1A2E),
            Self::Forest => (0x7BE0A6, 0xFF7F6E, 0xF2C14E, 0xC9DCCF, 0x0B1C14),
            Self::Ocean => (0x5FD9E6, 0xFF7B7B, 0xFFB86B, 0xC5D9DD, 0x0A1C21),
            Self::Plum => (0xC9A2FF, 0xFF7096, 0xFFB86B, 0xD6CDE3, 0x170F24),
            Self::Ember => (0xFFB177, 0xFF6B6B, 0xFFD166, 0xE2D3CB, 0x1C110D),
            Self::Graphite => (0xE8C872, 0xFF6B6B, 0xFFA94D, 0xC8CCD2, 0x121315),
            Self::HighContrast => (0xFFE15A, 0xFF8A8A, 0xFFC04D, 0xE0E0E0, 0x000000),
        };
        let color = |rgb: u32| C::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        DesktopPalette {
            text: C::WHITE,
            muted: color(muted),
            accent: color(accent),
            background: color(background),
            urgent: color(urgent),
            soon: color(soon),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopAccent {
    #[default]
    Theme,
    Ice,
    Mint,
    Lavender,
    White,
    Custom,
}

impl DesktopAccent {
    pub fn color(self, theme: DesktopTheme, custom: [u8; 3]) -> egui::Color32 {
        match self {
            Self::Theme => theme.palette().accent,
            Self::Ice => egui::Color32::from_rgb(91, 223, 247),
            Self::Mint => egui::Color32::from_rgb(111, 226, 195),
            Self::Lavender => egui::Color32::from_rgb(190, 173, 255),
            Self::White => egui::Color32::from_rgb(232, 239, 247),
            Self::Custom => egui::Color32::from_rgb(custom[0], custom[1], custom[2]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    pub enabled: bool,
    pub side: DesktopSide,
    pub show_clock: bool,
    pub panel_opacity: u8,
    pub show_homework: bool,
    pub show_schedule: bool,
    pub show_progress: bool,
    pub clock_scale: u16,
    pub clock_style: ClockStyle,
    pub show_date: bool,
    pub show_seconds: bool,
    pub panel_width: u16,
    pub panel_height: u16,
    pub text_scale: u16,
    pub vertical_position: u8,
    pub accent: DesktopAccent,
    pub theme: DesktopTheme,
    pub custom_accent: [u8; 3],
    pub show_borders: bool,
    pub corner_radius: u8,
    pub row_spacing: u8,
    pub show_course_codes: bool,
    pub show_deadlines: bool,
    pub include_exams: bool,
    pub include_reminders: bool,
    pub work_tab: WorkTab,
    /// Files saved before this field existed read as 0.
    #[serde(default)]
    pub layout_version: u8,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            side: DesktopSide::Right,
            show_clock: true,
            panel_opacity: 0,
            show_homework: true,
            show_schedule: true,
            show_progress: true,
            clock_scale: 140,
            clock_style: ClockStyle::Digital,
            show_date: true,
            show_seconds: false,
            panel_width: 368,
            panel_height: 496,
            text_scale: 100,
            vertical_position: 62,
            accent: DesktopAccent::Theme,
            theme: DesktopTheme::Navy,
            custom_accent: [111, 211, 232],
            show_borders: false,
            corner_radius: 12,
            row_spacing: 12,
            show_course_codes: true,
            show_deadlines: true,
            include_exams: false,
            include_reminders: false,
            work_tab: WorkTab::Assignments,
            layout_version: LAYOUT_VERSION,
        }
    }
}

impl DesktopSettings {
    pub fn accent_color(&self) -> egui::Color32 {
        self.accent.color(self.theme, self.custom_accent)
    }

    pub fn select_theme(&mut self, theme: DesktopTheme) {
        self.theme = theme;
        self.accent = DesktopAccent::Theme;
        self.clock_style = ClockStyle::Digital;
        if theme == DesktopTheme::HighContrast {
            self.panel_opacity = 80;
            self.text_scale = self.text_scale.max(110);
        }
    }

    pub fn path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("inna-client")
            .join("desktop.json")
    }

    pub fn load() -> Result<Self, String> {
        Self::load_from(&Self::path())
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let mut settings: Self = serde_json::from_slice(&bytes)
                    .map_err(|_| "Could not load desktop preferences.".to_string())?;
                // Replace the old themed clock without resetting layout or content choices.
                let legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                if matches!(legacy["theme"].as_str(), Some("Arc" | "IronMan"))
                    && settings.clock_style == ClockStyle::Reactor
                {
                    settings.clock_style = ClockStyle::Digital;
                }
                // Panels still on the old 590 px / 50% defaults move to the shorter default.
                if settings.layout_version < 1 {
                    if settings.panel_height == 590 && settings.vertical_position == 50 {
                        let defaults = Self::default();
                        settings.panel_height = defaults.panel_height;
                        settings.vertical_position = defaults.vertical_position;
                    }
                    settings.layout_version = LAYOUT_VERSION;
                }
                settings.panel_opacity = settings.panel_opacity.min(80);
                settings.clock_scale = settings.clock_scale.clamp(80, 180);
                settings.panel_width = settings.panel_width.clamp(280, 480);
                settings.panel_height = settings.panel_height.clamp(360, 800);
                settings.text_scale = settings.text_scale.clamp(85, 140);
                settings.vertical_position = settings.vertical_position.min(100);
                settings.corner_radius = settings.corner_radius.min(24);
                settings.row_spacing = settings.row_spacing.clamp(4, 24);
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("Could not load desktop preferences.".into()),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    fn save_to(&self, path: &Path) -> Result<(), String> {
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Keep the previous file intact until the complete replacement is ready.
            let temporary = path.with_extension("json.tmp");
            std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
            std::fs::rename(temporary, path)?;
            Ok(())
        };
        save().map_err(|error| {
            tracing::warn!("Could not save desktop preferences: {error}");
            "Could not save desktop preferences. Please try again.".into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_themes_migrate_without_losing_layout_and_new_presets_use_digital_clocks() {
        let path =
            std::env::temp_dir().join(format!("anni-theme-test-{}.json", std::process::id()));
        for (old, new) in [
            ("Arc", DesktopTheme::Ocean),
            ("IronMan", DesktopTheme::Ember),
        ] {
            std::fs::write(&path, format!(r#"{{"theme":"{old}","clock_style":"Reactor","panel_width":420,"enabled":true}}"#)).unwrap();
            let settings = DesktopSettings::load_from(&path).unwrap();
            assert_eq!(settings.theme, new);
            assert_eq!(settings.clock_style, ClockStyle::Digital);
            assert_eq!(settings.panel_width, 420);
            assert!(settings.enabled);
        }
        let mut settings = DesktopSettings::default();
        for theme in DesktopTheme::ALL {
            settings.select_theme(theme);
            assert_eq!(settings.clock_style, ClockStyle::Digital);
        }
        assert_eq!(settings.panel_opacity, 80);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn old_default_panel_size_moves_to_the_new_default_but_custom_sizes_stay() {
        let path =
            std::env::temp_dir().join(format!("anni-layout-test-{}.json", std::process::id()));
        let defaults = DesktopSettings::default();
        std::fs::write(&path, br#"{"panel_height":590,"vertical_position":50}"#).unwrap();
        let settings = DesktopSettings::load_from(&path).unwrap();
        assert_eq!(settings.panel_height, defaults.panel_height);
        assert_eq!(settings.vertical_position, defaults.vertical_position);
        assert_eq!(settings.layout_version, LAYOUT_VERSION);
        assert_eq!(settings.work_tab, WorkTab::Assignments);
        std::fs::write(&path, br#"{"panel_height":700,"vertical_position":50}"#).unwrap();
        assert_eq!(DesktopSettings::load_from(&path).unwrap().panel_height, 700);
        // Once saved with the current version, 590 px is a deliberate choice.
        std::fs::write(&path, br#"{"panel_height":590,"vertical_position":50,"layout_version":1}"#).unwrap();
        assert_eq!(DesktopSettings::load_from(&path).unwrap().panel_height, 590);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn saved_anni_themes_map_onto_the_hermes_themes() {
        for (old, new) in [
            ("Minimal", DesktopTheme::Navy),
            ("Lavender", DesktopTheme::Plum),
            ("Sunset", DesktopTheme::Ember),
            ("Sand", DesktopTheme::Ember),
            ("Forest", DesktopTheme::Forest),
            ("HighContrast", DesktopTheme::HighContrast),
        ] {
            assert_eq!(serde_json::from_str::<DesktopTheme>(&format!("\"{old}\"")).unwrap(), new);
        }
    }

    #[test]
    fn preferences_restore_with_defaults_and_reject_corrupt_data() {
        let path =
            std::env::temp_dir().join(format!("anni-desktop-test-{}.json", std::process::id()));
        let settings = DesktopSettings {
            enabled: true,
            side: DesktopSide::Left,
            theme: DesktopTheme::Ember,
            accent: DesktopAccent::Custom,
            custom_accent: [235, 181, 102],
            clock_style: ClockStyle::Analog,
            show_borders: true,
            row_spacing: 6,
            corner_radius: 20,
            show_course_codes: false,
            show_deadlines: false,
            ..Default::default()
        };
        settings.save_to(&path).unwrap();
        assert_eq!(DesktopSettings::load_from(&path).unwrap(), settings);
        std::fs::write(&path, br#"{"enabled":true,"panel_opacity":255,"accent":"Mint","clock_style":"Reactor","row_spacing":255,"corner_radius":255}"#).unwrap();
        let restored = DesktopSettings::load_from(&path).unwrap();
        assert!(restored.show_clock);
        assert_eq!(restored.side, DesktopSide::Right);
        assert_eq!(restored.panel_opacity, 80);
        assert_eq!(restored.theme, DesktopTheme::Navy);
        assert!(restored.show_progress);
        assert!(!restored.show_borders);
        assert_eq!(restored.accent, DesktopAccent::Mint);
        assert_eq!(restored.clock_style, ClockStyle::Reactor);
        assert_eq!(restored.row_spacing, 24);
        assert_eq!(restored.corner_radius, 24);
        std::fs::write(&path, b"broken").unwrap();
        assert!(DesktopSettings::load_from(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
