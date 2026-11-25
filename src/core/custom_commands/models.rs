use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomCommand {
    pub trigger: String,
    pub label: String,
    pub description: String,
    pub template: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerContext {
    Suppressed,
    Prose,
    Math,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashSuggestion {
    pub trigger: String,
    pub label: String,
    pub description: String,
    pub template: String,
    pub takes_args: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashMenuState {
    pub context: TriggerContext,
    pub trigger_line: usize,
    pub trigger_col: usize,
    pub trigger_char: Option<char>,
    pub query: String,
    pub arg: Option<String>,
    pub suggestions: Vec<SlashSuggestion>,
    pub selected_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomCommandsUiState {
    pub new_trigger: String,
    pub new_label: String,
    pub new_description: String,
    pub new_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomCommandsMessage {
    NewTriggerChanged(String),
    NewLabelChanged(String),
    NewDescriptionChanged(String),
    NewTemplateChanged(String),
    AddCommand,
    RemoveCommand(usize),
}
