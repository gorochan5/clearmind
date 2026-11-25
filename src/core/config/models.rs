use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::core::custom_commands::{CustomCommand, CustomCommandsMessage};
use crate::core::fonts::{FontConfig, FontMessage};
use crate::core::keybindings::{KeybindingsConfig, KeybindingsMessage};
use crate::core::margin::{MarginConfig, MarginMessage};
use crate::core::themes::{ThemeConfig, ThemeMessage};
use crate::core::word_wrap::{WordWrapConfig, WordWrapMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 960,
            height: 720,
            x: 120,
            y: 80,
            maximized: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub restore_session: bool,
    pub open_files: Vec<String>,
    pub active_file: Option<String>,
    pub recent_files: Vec<String>,
    pub window: WindowConfig,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            restore_session: true,
            open_files: Vec::new(),
            active_file: None,
            recent_files: Vec::new(),
            window: WindowConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewConfig {
    pub zoom: f32,
    pub sticky_headings: bool,
    pub code_line_numbers: bool,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            sticky_headings: true,
            code_line_numbers: false,
            focus_mode: false,
            typewriter_mode: false,
        }
    }
}

impl ViewConfig {
    pub fn clamped(mut self) -> Self {
        self.zoom = self.zoom.clamp(0.5, 2.0);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClearMindConfig {
    pub config_version: u32,
    pub session: SessionConfig,
    pub view: ViewConfig,
    pub writing_targets: BTreeMap<String, u32>,
    pub fonts: FontConfig,
    pub word_wrap: WordWrapConfig,
    pub margin: MarginConfig,
    pub themes: ThemeConfig,
    pub keybindings: KeybindingsConfig,
    pub commands: Vec<CustomCommand>,
}

impl Default for ClearMindConfig {
    fn default() -> Self {
        Self {
            config_version: 1,
            session: SessionConfig::default(),
            view: ViewConfig::default(),
            writing_targets: BTreeMap::new(),
            fonts: FontConfig::default(),
            word_wrap: WordWrapConfig::default(),
            margin: MarginConfig::default(),
            themes: ThemeConfig::default(),
            keybindings: KeybindingsConfig::default(),
            commands: Vec::new(),
        }
    }
}

impl ClearMindConfig {
    pub fn clamped(mut self) -> Self {
        self.view = self.view.clamped();
        self.fonts = self.fonts.clamped();
        self.margin = self.margin.clamped();
        self.themes = self.themes.normalized();
        if self.session.recent_files.len() > 10 {
            self.session.recent_files.truncate(10);
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigTab {
    #[default]
    Fonts,
    WordWrap,
    Margin,
    Themes,
    Keybindings,
    CustomCommands,
}

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    SelectTab(ConfigTab),
    Fonts(FontMessage),
    WordWrap(WordWrapMessage),
    Margin(MarginMessage),
    Themes(ThemeMessage),
    Keybindings(KeybindingsMessage),
    CustomCommands(CustomCommandsMessage),
}
