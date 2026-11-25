//! Shields.io and inline badge pill widget.
use iced::Widget;

use iced::widget::{button, container, text};
use iced::{Border, Color, Element, Padding};

pub fn view_badge<'a, Message: Clone + 'a>(
    label: &'a str,
    bg: Color,
    fg: Color,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let pill = container(text(label).size(12).color(fg))
        .padding(Padding::from([2, 8]))
        .style(move |_| container::Style {
            background: Some(bg.into()),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    if let Some(msg) = on_press {
        button(pill)
            .padding(0)
            .style(|_, _| button::Style::default())
            .on_press(msg)
            .boxed()
    } else {
        pill.boxed()
    }
}