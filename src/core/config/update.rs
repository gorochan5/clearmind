use crate::core::custom_commands::{self, CustomCommandsUiState};
use crate::core::fonts;
use crate::core::keybindings::{self, KeybindingsUiState};
use crate::core::margin;
use crate::core::themes;
use crate::core::word_wrap;

use super::models::{ClearMindConfig, ConfigMessage, ConfigTab};

#[derive(Debug, Clone)]
pub struct ConfigUiState {
    pub active_tab: ConfigTab,
    pub available_fonts: Vec<String>,
    pub keybindings_ui: KeybindingsUiState,
    pub custom_commands_ui: CustomCommandsUiState,
}

impl Default for ConfigUiState {
    fn default() -> Self {
        Self {
            active_tab: ConfigTab::default(),
            available_fonts: vec![
                "Arial".into(),
                "Calibri".into(),
                "Cambria".into(),
                "Cascadia Code".into(),
                "Cascadia Mono".into(),
                "Consolas".into(),
                "Courier New".into(),
                "Georgia".into(),
                "Segoe UI".into(),
                "Tahoma".into(),
                "Times New Roman".into(),
                "Trebuchet MS".into(),
                "Verdana".into(),
            ],
            keybindings_ui: KeybindingsUiState::default(),
            custom_commands_ui: CustomCommandsUiState::default(),
        }
    }
}

pub fn update(
    config: &mut ClearMindConfig,
    ui: &mut ConfigUiState,
    message: ConfigMessage,
) {
    match message {
        ConfigMessage::SelectTab(tab) => ui.active_tab = tab,
        ConfigMessage::Fonts(msg) => {
            fonts::update::update(&mut config.fonts, &mut ui.available_fonts, msg);
        }
        ConfigMessage::WordWrap(msg) => {
            word_wrap::update::update(&mut config.word_wrap, msg);
        }
        ConfigMessage::Margin(msg) => {
            margin::update::update(&mut config.margin, msg);
        }
        ConfigMessage::Themes(msg) => {
            themes::update::update(&mut config.themes, msg);
        }
        ConfigMessage::Keybindings(msg) => {
            keybindings::update::update(&mut config.keybindings, &mut ui.keybindings_ui, msg);
        }
        ConfigMessage::CustomCommands(msg) => {
            custom_commands::update::update(
                &mut config.commands,
                &mut ui.custom_commands_ui,
                msg,
            );
        }
    }
}
