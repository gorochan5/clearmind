use iced::Widget;
use iced::widget::{column, row, text, toggler};
use iced::{Alignment, Element, Length};

use crate::core::themes::ThemePalette;

use super::models::{WordWrapConfig, WordWrapMessage};

pub fn view_modal_section<'a>(
    config: &'a WordWrapConfig,
    palette: &'a ThemePalette,
) -> Element<'a, WordWrapMessage> {
    column![
        text("Word Wrap").size(18).color(palette.text),
        row![
            text("Wrap long lines in editor and code blocks")
                .size(13)
                .color(palette.text_muted)
                .width(Length::Fill),
            toggler(config.enabled).on_toggle(WordWrapMessage::SetEnabled),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(10)
    .boxed()
}
