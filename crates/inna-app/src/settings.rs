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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopTheme {
    #[default]
    Minimal,
    Arc,
    IronMan,
    Forest,
    Lavender,
    Sunset,
}

pub struct DesktopPalette {
    pub text: egui::Color32,
    pub muted: egui::Color32,
    pub accent: egui::Color32,
    pub background: egui::Color32,
    pub urgent: egui::Color32,
}

impl DesktopTheme {
    pub const ALL: [Self; 6] = [
        Self::Minimal,
        Self::Arc,
        Self::IronMan,
        Self::Forest,
        Self::Lavender,
        Self::Sunset,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal",
            Self::Arc => "Arc",
            Self::IronMan => "Iron Man",
            Self::Forest => "Forest",
            Self::Lavender => "Lavender",
            Self::Sunset => "Sunset",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Minimal => "Soft white, neutral charcoal, and a simple digital clock.",
            Self::Arc => "Ice blue with a fine, open reactor ring.",
            Self::IronMan => "Warm gold, deep burgundy, and a restrained reactor ring.",
            Self::Forest => "Sage green with warm, natural neutrals.",
            Self::Lavender => "Soft lilac with cool violet neutrals.",
            Self::Sunset => "Muted peach with warm cocoa neutrals.",
        }
    }

    pub fn palette(self) -> DesktopPalette {
        use egui::Color32 as C;
        let (text, muted, accent, background) = match self {
            Self::Minimal => (
                [240, 240, 240],
                [164, 166, 170],
                [225, 227, 232],
                [18, 19, 21],
            ),
            Self::Arc => (
                [226, 242, 246],
                [149, 177, 187],
                [111, 211, 232],
                [12, 24, 32],
            ),
            Self::IronMan => (
                [246, 235, 218],
                [190, 164, 151],
                [225, 184, 104],
                [36, 18, 22],
            ),
            Self::Forest => (
                [233, 241, 231],
                [161, 183, 165],
                [158, 205, 167],
                [17, 28, 24],
            ),
            Self::Lavender => (
                [240, 234, 250],
                [177, 166, 195],
                [193, 175, 237],
                [25, 21, 36],
            ),
            Self::Sunset => (
                [250, 235, 227],
                [197, 170, 155],
                [236, 174, 143],
                [34, 24, 23],
            ),
        };
        let color = |rgb: [u8; 3]| C::from_rgb(rgb[0], rgb[1], rgb[2]);
        DesktopPalette {
            text: color(text),
            muted: color(muted),
            accent: color(accent),
            background: color(background),
            urgent: C::from_rgb(244, 185, 120),
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
            clock_scale: 140,
            clock_style: ClockStyle::Digital,
            show_date: true,
            show_seconds: false,
            panel_width: 368,
            panel_height: 590,
            text_scale: 100,
            vertical_position: 50,
            accent: DesktopAccent::Theme,
            theme: DesktopTheme::Minimal,
            custom_accent: [111, 211, 232],
            show_borders: false,
            corner_radius: 12,
            row_spacing: 12,
            show_course_codes: true,
            show_deadlines: true,
            include_exams: false,
            include_reminders: false,
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
        self.clock_style = match theme {
            DesktopTheme::Arc | DesktopTheme::IronMan => ClockStyle::Reactor,
            _ => ClockStyle::Digital,
        };
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
    fn preferences_restore_with_defaults_and_reject_corrupt_data() {
        let path =
            std::env::temp_dir().join(format!("anni-desktop-test-{}.json", std::process::id()));
        let settings = DesktopSettings {
            enabled: true,
            side: DesktopSide::Left,
            theme: DesktopTheme::IronMan,
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
        assert_eq!(restored.theme, DesktopTheme::Minimal);
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
