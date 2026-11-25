//! Unified code block container with syntax-highlighted body, language badge, Copy button, and line-number gutter.
use iced::Widget;
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Border, Element, Font, Length, Padding};

use crate::core::themes::ThemePalette;
use crate::core::widgets::button::subtle_button_style;

#[allow(clippy::too_many_arguments)]
pub fn view_code_block<'a, Message: Clone + 'a>(
    language: &str,
    show_line_numbers: bool,
    line_count: usize,
    code_font: Font,
    code_size: f32,
    content: Element<'a, Message>,
    palette: &ThemePalette,
    on_copy: Message,
    on_pick_language: Option<Message>,
) -> Element<'a, Message> {
    let palette_copy = *palette;
    let lang_label = if language.is_empty() {
        "plain text".to_string()
    } else {
        language.to_string()
    };

    let lang_badge: Element<'a, Message> = if let Some(msg) = on_pick_language {
        button(text(lang_label).size(11).color(palette.text_muted))
            .padding(Padding::from([3, 8]))
            .style(subtle_button_style(palette_copy, false))
            .on_press(msg)
            .boxed()
    } else {
        container(text(lang_label).size(11).color(palette.text_muted))
            .padding(Padding::from([2, 8]))
            .style(move |_| container::Style {
                background: Some(palette_copy.background.into()),
                border: Border {
                    color: palette_copy.border,
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..Default::default()
            })
            .boxed()
    };

    let copy_btn = button(text("Copy").size(11))
        .padding(Padding::from([3, 9]))
        .style(subtle_button_style(palette_copy, false))
        .on_press(on_copy);

    let header = row![lang_badge, iced::widget::space::horizontal(), copy_btn]
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let divider = container(Space::new().height(1))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(palette_copy.border.into()),
            ..Default::default()
        });

    let body: Element<'a, Message> = if show_line_numbers {
        let gutter_nums: String = (1..=line_count)
            .map(|n| format!("{n:>3}"))
            .collect::<Vec<_>>()
            .join("\n");
        let gutter = text(gutter_nums)
            .font(code_font)
            .size(code_size)
            .line_height(iced::widget::text::LineHeight::Relative(1.45))
            .color(palette.text_muted);
        row![gutter, content].width(Length::Fill).spacing(12).boxed()
    } else {
        content
    };

    container(column![header, divider, body].spacing(8))
        .width(Length::Fill)
        .padding(Padding::from([10, 14]))
        .style(move |_| container::Style {
            background: Some(palette_copy.code_bg.into()),
            border: Border {
                color: palette_copy.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}
