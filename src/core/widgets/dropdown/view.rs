use iced::Widget;
use iced::widget::{button, column, container, scrollable, text, text_input};
use iced::{Border, Element, Length};

use crate::core::themes::ThemePalette;

use super::models::{DropdownMessage, DropdownState};

pub fn view<'a>(
    state: &'a DropdownState,
    placeholder: &'a str,
    palette: &'a ThemePalette,
) -> Element<'a, DropdownMessage> {
    let label = state.selected.as_deref().unwrap_or(placeholder);
    let trigger = button(text(label).size(13).color(palette.text))
        .width(Length::Fill)
        .padding([6, 10])
        .style(move |_, _| button::Style {
            background: Some(palette.surface.into()),
            text_color: palette.text,
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .on_press(DropdownMessage::Toggle);

    if !state.is_open {
        return trigger.boxed();
    }

    let filter_input = text_input("Filter...", &state.filter)
        .size(13)
        .padding(6)
        .on_input(DropdownMessage::FilterChanged);

    let mut list = column![].spacing(2);
    for opt in state.filtered_options() {
        let is_selected = state.selected.as_deref() == Some(opt);
        list = list.push(
            button(text(opt).size(13).color(palette.text))
                .width(Length::Fill)
                .padding([5, 8])
                .style(move |_, _| button::Style {
                    background: Some(
                        if is_selected {
                            palette.surface_hover
                        } else {
                            palette.surface
                        }
                        .into(),
                    ),
                    text_color: palette.text,
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: palette.border,
                    },
                    ..Default::default()
                })
                .on_press(DropdownMessage::Select(opt.to_string())),
        );
    }

    let menu = container(
        column![
            filter_input,
            scrollable(list).height(Length::Fixed(180.0)),
        ]
        .spacing(6),
    )
    .padding(8)
    .style(move |_| container::Style {
        background: Some(palette.surface.into()),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    column![trigger, menu].spacing(4).boxed()
}
