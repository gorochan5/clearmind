use iced::Widget;
use iced::widget::{column, pick_list, row, slider, text};
use iced::{Alignment, Element, Length, Padding};

use crate::core::themes::ThemePalette;
use crate::core::widgets::styles::{
    rosemary_slider_style, rounded_menu_style, rounded_pick_list_style,
};

use super::models::{FontConfig, FontMessage};

fn styled_font_picker<'a>(
    selected: &'a String,
    available_fonts: &'a [String],
    palette: ThemePalette,
    on_select: impl Fn(String) -> FontMessage + 'a,
) -> Element<'a, FontMessage> {
    pick_list(Some(selected), available_fonts, String::to_string)
        .on_select(on_select)
        .text_size(13)
        .padding(Padding::from([7, 12]))
        .width(Length::Fill)
        .menu_height(Length::Fixed(240.0))
        .style(rounded_pick_list_style(palette))
        .menu_style(rounded_menu_style(palette))
        .boxed()
}

pub fn view_quick_popover<'a>(
    config: &'a FontConfig,
    available_fonts: &'a [String],
    palette: &'a ThemePalette,
) -> Element<'a, FontMessage> {
    let p = *palette;
    column![
        text("Fonts").size(14).color(palette.text),
        column![
            text("Markdown Font").size(12).color(palette.text_muted),
            styled_font_picker(&config.body_family, available_fonts, p, FontMessage::SetBodyFamily),
        ]
        .spacing(5),
        column![
            text("Code Font").size(12).color(palette.text_muted),
            styled_font_picker(&config.code_family, available_fonts, p, FontMessage::SetCodeFamily),
        ]
        .spacing(5),
        column![
            row![
                text("Markdown Size")
                    .size(12)
                    .color(palette.text)
                    .width(Length::Fill),
                text(format!("{:.0}px", config.font_size))
                    .size(12)
                    .color(palette.text_muted),
            ]
            .align_y(Alignment::Center),
            slider(12.0..=28.0, config.font_size, FontMessage::SetFontSize)
                .step(1.0)
                .style(rosemary_slider_style(p)),
        ]
        .spacing(4),
        column![
            row![
                text("Code Size")
                    .size(12)
                    .color(palette.text)
                    .width(Length::Fill),
                text(format!("{:.0}px", config.code_size))
                    .size(12)
                    .color(palette.text_muted),
            ]
            .align_y(Alignment::Center),
            slider(10.0..=24.0, config.code_size, FontMessage::SetCodeSize)
                .step(1.0)
                .style(rosemary_slider_style(p)),
        ]
        .spacing(4),
        column![
            row![
                text("Line Height")
                    .size(12)
                    .color(palette.text)
                    .width(Length::Fill),
                text(format!("{:.2}", config.line_height))
                    .size(12)
                    .color(palette.text_muted),
            ]
            .align_y(Alignment::Center),
            slider(1.2..=2.0, config.line_height, FontMessage::SetLineHeight)
                .step(0.05)
                .style(rosemary_slider_style(p)),
        ]
        .spacing(4),
    ]
    .spacing(12)
    .width(Length::Fixed(276.0))
    .boxed()
}

pub fn view_modal_section<'a>(
    config: &'a FontConfig,
    available_fonts: &'a [String],
    palette: &'a ThemePalette,
) -> Element<'a, FontMessage> {
    let p = *palette;
    column![
        text("Fonts").size(18).color(palette.text),
        row![
            text("Markdown Font")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            styled_font_picker(&config.body_family, available_fonts, p, FontMessage::SetBodyFamily),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            text("Code Font")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            styled_font_picker(&config.code_family, available_fonts, p, FontMessage::SetCodeFamily),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            text("Markdown Size")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            slider(12.0..=28.0, config.font_size, FontMessage::SetFontSize)
                .step(1.0)
                .style(rosemary_slider_style(p))
                .width(Length::Fill),
            text(format!("{:.0}px", config.font_size))
                .size(13)
                .color(palette.text_muted)
                .width(Length::Fixed(48.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            text("Code Size")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            slider(10.0..=24.0, config.code_size, FontMessage::SetCodeSize)
                .step(1.0)
                .style(rosemary_slider_style(p))
                .width(Length::Fill),
            text(format!("{:.0}px", config.code_size))
                .size(13)
                .color(palette.text_muted)
                .width(Length::Fixed(48.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            text("Line Height")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            slider(1.2..=2.0, config.line_height, FontMessage::SetLineHeight)
                .step(0.05)
                .style(rosemary_slider_style(p))
                .width(Length::Fill),
            text(format!("{:.2}", config.line_height))
                .size(13)
                .color(palette.text_muted)
                .width(Length::Fixed(48.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(12)
    .boxed()
}
