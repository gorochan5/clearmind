use super::models::{CustomCommand, CustomCommandsMessage, CustomCommandsUiState};

pub fn update(
    commands: &mut Vec<CustomCommand>,
    ui: &mut CustomCommandsUiState,
    message: CustomCommandsMessage,
) {
    match message {
        CustomCommandsMessage::NewTriggerChanged(s) => ui.new_trigger = s,
        CustomCommandsMessage::NewLabelChanged(s) => ui.new_label = s,
        CustomCommandsMessage::NewDescriptionChanged(s) => ui.new_description = s,
        CustomCommandsMessage::NewTemplateChanged(s) => ui.new_template = s,
        CustomCommandsMessage::AddCommand => {
            let trigger = ui.new_trigger.trim().trim_start_matches('/').to_string();
            if trigger.is_empty() || ui.new_template.is_empty() {
                return;
            }
            let label = if ui.new_label.trim().is_empty() {
                trigger.clone()
            } else {
                ui.new_label.trim().to_string()
            };
            commands.push(CustomCommand {
                trigger,
                label,
                description: ui.new_description.trim().to_string(),
                template: ui.new_template.clone(),
            });
            *ui = CustomCommandsUiState::default();
        }
        CustomCommandsMessage::RemoveCommand(idx) => {
            if idx < commands.len() {
                commands.remove(idx);
            }
        }
    }
}
