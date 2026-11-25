use super::engine::find_conflicts;
use super::models::{KeybindingsConfig, KeybindingsMessage, KeybindingsUiState};

pub fn update(
    config: &mut KeybindingsConfig,
    ui: &mut KeybindingsUiState,
    message: KeybindingsMessage,
) {
    match message {
        KeybindingsMessage::SearchChanged(q) => {
            ui.search_query = q;
        }
        KeybindingsMessage::StartRecording(cmd) => {
            ui.recording_command = Some(cmd);
            ui.conflict_warning = None;
        }
        KeybindingsMessage::CancelRecording => {
            ui.recording_command = None;
            ui.conflict_warning = None;
        }
        KeybindingsMessage::CommitBinding { command, shortcut } => {
            let conflicts = find_conflicts(config, command, &shortcut);
            if !conflicts.is_empty() {
                let names: Vec<&str> = conflicts.iter().map(|c| c.as_str()).collect();
                ui.conflict_warning = Some(format!("Conflicts with: {}", names.join(", ")));
            } else {
                ui.conflict_warning = None;
            }
            config.overrides.insert(command.as_str().to_string(), shortcut);
            ui.recording_command = None;
        }
        KeybindingsMessage::Unbind(cmd) => {
            config.overrides.insert(cmd.as_str().to_string(), String::new());
        }
        KeybindingsMessage::ResetOne(cmd) => {
            config.overrides.remove(cmd.as_str());
        }
        KeybindingsMessage::ResetAll => {
            config.overrides.clear();
            ui.conflict_warning = None;
        }
    }
}
