use iced::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::presets::default_themes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeDefinition {
    pub name: String,
    pub theme_icon: String,
    pub is_dark: bool,
    pub background: String,
    pub surface: String,
    pub surface_hover: String,
    pub border: String,
    pub text: String,
    pub text_muted: String,
    pub accent: String,
    pub accent_text: String,
    pub danger: String,
    pub warning: String,
    pub success: String,
    pub code_bg: String,
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self {
            name: "Clearmind Light".to_string(),
            theme_icon: "🌿".to_string(),
            is_dark: false,
            background: "#FAFBF9".to_string(),
            surface: "#F0F4EF".to_string(),
            surface_hover: "#E2E9E0".to_string(),
            border: "#D2DCD0".to_string(),
            text: "#1E2621".to_string(),
            text_muted: "#5E6D63".to_string(),
            accent: "#C2CEC0".to_string(),
            accent_text: "#1F3A32".to_string(),
            danger: "#C93B3B".to_string(),
            warning: "#B57C1E".to_string(),
            success: "#2E6F4E".to_string(),
            code_bg: "#F3F6F2".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub active_theme: String,
    #[serde(flatten)]
    pub themes: BTreeMap<String, ThemeDefinition>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            active_theme: "clearmind_light".to_string(),
            themes: default_themes(),
        }
    }
}

impl ThemeConfig {
    pub fn normalized(mut self) -> Self {
        self.active_theme = self.active_theme.replace('-', "_");
        self.themes.remove("custom");
        for (id, def) in default_themes() {
            self.themes.entry(id).or_insert(def);
        }
        if !self.themes.contains_key(&self.active_theme) {
            self.active_theme = "clearmind_light".to_string();
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePalette {
    pub id: &'static str,
    pub name: &'static str,
    pub is_dark: bool,
    pub background: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub selection: Color,
    pub code_bg: Color,
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_comment: Color,
    pub syntax_number: Color,
    pub syntax_function: Color,
    pub syntax_type: Color,
}

impl ThemePalette {
    pub fn to_iced_theme(self) -> iced::Theme {
        iced::Theme::custom(
            self.name.to_string(),
            iced::theme::palette::Seed {
                background: self.background,
                text: self.text,
                primary: self.accent,
                success: self.success,
                warning: self.warning,
                danger: self.danger,
            },
        )
    }
}

pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let trimmed = hex.trim().strip_prefix('#')?;
    match trimmed.len() {
        6 => {
            let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
            let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
            let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
            Some(Color::from_rgb(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
            ))
        }
        8 => {
            let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
            let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
            let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
            let a = u8::from_str_radix(&trimmed[6..8], 16).ok()?;
            Some(Color::from_rgba(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                f32::from(a) / 255.0,
            ))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomColorField {
    Background,
    Surface,
    Text,
    Accent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeMessage {
    SelectTheme(String),
    SetThemeIcon(String),
    SetCustomHex {
        field: CustomColorField,
        value: String,
    },
    ResetActiveTheme,
}
