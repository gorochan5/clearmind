use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarginConfig {
    pub full_width: bool,
    pub content_max_width: f32,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            full_width: false,
            content_max_width: 820.0,
        }
    }
}

impl MarginConfig {
    pub fn clamped(mut self) -> Self {
        self.content_max_width = self.content_max_width.clamp(560.0, 1400.0);
        self
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarginMessage {
    ToggleFullWidth,
    SetFullWidth(bool),
    SetMaxWidth(f32),
}
