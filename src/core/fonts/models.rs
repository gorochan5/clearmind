use iced::font::Family;
use iced::Font;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontPreset {
    #[default]
    Sans,
    Serif,
    Mono,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub preset: FontPreset,
    pub body_family: String,
    pub code_family: String,
    pub font_size: f32,
    pub code_size: f32,
    pub line_height: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            preset: FontPreset::Sans,
            body_family: "Segoe UI".to_string(),
            code_family: "Cascadia Mono".to_string(),
            font_size: 16.0,
            code_size: 14.0,
            line_height: 1.5,
        }
    }
}

fn intern_family_name(name: &str) -> &'static str {
    static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let set = INTERNED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = set.lock().unwrap();
    if let Some(&existing) = guard.get(name) {
        existing
    } else {
        let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
        guard.insert(leaked);
        leaked
    }
}

impl FontConfig {
    pub fn clamped(mut self) -> Self {
        self.font_size = self.font_size.clamp(12.0, 28.0);
        self.code_size = self.code_size.clamp(10.0, 24.0);
        self.line_height = self.line_height.clamp(1.2, 2.0);
        self
    }

    pub fn body_font(&self) -> Font {
        Font::with_family(Family::name(intern_family_name(&self.body_family)))
    }

    pub fn code_font(&self) -> Font {
        Font::with_family(Family::name(intern_family_name(&self.code_family)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FontMessage {
    SetPreset(FontPreset),
    SetBodyFamily(String),
    SetCodeFamily(String),
    SetFontSize(f32),
    SetCodeSize(f32),
    SetLineHeight(f32),
    StepFontSize(f32),
    StepCodeSize(f32),
    StepLineHeight(f32),
    FontsLoaded(Vec<String>),
}
