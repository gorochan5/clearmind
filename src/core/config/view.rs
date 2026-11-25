use iced::widget::{column, container, scrollable, text, Space};
use iced::{Element, Length, Padding, Widget};

use crate::core::custom_commands;
use crate::core::fonts;
use crate::core::keybindings;
use crate::core::margin;
use crate::core::themes::{self, ThemePalette};
use crate::core::word_wrap;

use super::models::{ClearMindConfig, ConfigMessage};
use super::update::ConfigUiState;

pub fn view_modal<'a>(
    config: &'a ClearMindConfig,
    ui: &'a ConfigUiState,
    palette: &'a ThemePalette,
) -> Element<'a, ConfigMessage> {
    let header = text("Settings").size(20).color(palette.text);

    let sections = column![
        themes::view::view_modal_section(&config.themes, palette)
            .map(ConfigMessage::Themes),
        section_divider(palette),
        fonts::view::view_modal_section(&config.fonts, &ui.available_fonts, palette)
            .map(ConfigMessage::Fonts),
        section_divider(palette),
        word_wrap::view::view_modal_section(&config.word_wrap, palette)
            .map(ConfigMessage::WordWrap),
        section_divider(palette),
        margin::view::view_modal_section(&config.margin, palette)
            .map(ConfigMessage::Margin),
        section_divider(palette),
        keybindings::view::view_modal_section(&config.keybindings, &ui.keybindings_ui, palette)
            .map(ConfigMessage::Keybindings),
        section_divider(palette),
        custom_commands::view::view_modal_section(
            &config.commands,
            &ui.custom_commands_ui,
            palette,
        )
        .map(ConfigMessage::CustomCommands),
    ]
    .spacing(20)
    .padding(Padding {
        top: 6.0,
        right: 20.0,
        bottom: 20.0,
        left: 2.0,
    })
    .width(Length::Fill);

    column![
        header,
        section_divider(palette),
        scrollable(sections)
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .spacing(12)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

fn section_divider<'a>(palette: &'a ThemePalette) -> Element<'a, ConfigMessage> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(palette.border.into()),
            ..Default::default()
        })
        .boxed()
}
