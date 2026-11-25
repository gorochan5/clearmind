use iced::font::Family;
use iced::Task;

use super::models::{FontConfig, FontMessage, FontPreset};

pub fn load_system_fonts() -> Task<FontMessage> {
    iced::font::list().map(|res| {
        let mut names: Vec<String> = res
            .unwrap_or_default()
            .into_iter()
            .filter_map(|family| match family {
                Family::Name(s) => Some(s.to_string()),
                _ => None,
            })
            .collect();
        names.sort();
        names.dedup();
        FontMessage::FontsLoaded(names)
    })
}

pub fn update(
    config: &mut FontConfig,
    available_fonts: &mut Vec<String>,
    message: FontMessage,
) {
    match message {
        FontMessage::SetPreset(preset) => {
            config.preset = preset;
        }
        FontMessage::SetBodyFamily(family) => {
            config.body_family = family;
            config.preset = FontPreset::Custom;
        }
        FontMessage::SetCodeFamily(family) => {
            config.code_family = family;
        }
        FontMessage::SetFontSize(size) => {
            config.font_size = size.clamp(12.0, 28.0);
        }
        FontMessage::SetCodeSize(size) => {
            config.code_size = size.clamp(10.0, 24.0);
        }
        FontMessage::SetLineHeight(lh) => {
            let rounded = (lh * 100.0).round() / 100.0;
            config.line_height = rounded.clamp(1.2, 2.0);
        }
        FontMessage::StepFontSize(delta) => {
            config.font_size = (config.font_size + delta).clamp(12.0, 28.0);
        }
        FontMessage::StepCodeSize(delta) => {
            config.code_size = (config.code_size + delta).clamp(10.0, 24.0);
        }
        FontMessage::StepLineHeight(delta) => {
            let next = ((config.line_height + delta) * 10.0).round() / 10.0;
            config.line_height = next.clamp(1.2, 2.0);
        }
        FontMessage::FontsLoaded(names) => {
            if !names.is_empty() {
                *available_fonts = names;
            }
        }
    }
}
