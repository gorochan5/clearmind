use iced::widget::text::Wrapping;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WordWrapConfig {
    pub enabled: bool,
}

impl Default for WordWrapConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl WordWrapConfig {
    pub fn wrapping(&self) -> Wrapping {
        if self.enabled {
            Wrapping::Word
        } else {
            Wrapping::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrapMessage {
    Toggle,
    SetEnabled(bool),
}
