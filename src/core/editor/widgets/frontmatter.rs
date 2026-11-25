//! YAML frontmatter metadata card.
use iced::Widget;

use iced::widget::{column, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

pub fn view_frontmatter<'a, Message: 'a>(
    entries: &'a [(String, String)],
    surface: Color,
    border_color: Color,
    muted: Color,
    fg: Color,
) -> Element<'a, Message> {
    let rows = entries
        .iter()
        .map(|(k, v)| {
            row![
                text(format!("{k}:")).size(12).color(muted),
                text(v).size(12).color(fg),
            ]
            .spacing(8)
            .boxed()
        })
        .collect::<Vec<Element<'a, Message>>>();

    container(column![].extend(rows).width(Length::Fill).spacing(4))
        .width(Length::Fill)
        .padding(Padding::from([8, 12]))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}