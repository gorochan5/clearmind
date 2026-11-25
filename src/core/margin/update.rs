use super::models::{MarginConfig, MarginMessage};

pub fn update(config: &mut MarginConfig, message: MarginMessage) {
    match message {
        MarginMessage::ToggleFullWidth => config.full_width = !config.full_width,
        MarginMessage::SetFullWidth(full) => config.full_width = full,
        MarginMessage::SetMaxWidth(width) => {
            config.content_max_width = width.clamp(560.0, 1400.0);
        }
    }
}
