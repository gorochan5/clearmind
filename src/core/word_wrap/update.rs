use super::models::{WordWrapConfig, WordWrapMessage};

pub fn update(config: &mut WordWrapConfig, message: WordWrapMessage) {
    match message {
        WordWrapMessage::Toggle => config.enabled = !config.enabled,
        WordWrapMessage::SetEnabled(enabled) => config.enabled = enabled,
    }
}
