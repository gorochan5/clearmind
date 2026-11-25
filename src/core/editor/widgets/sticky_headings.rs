//! Sticky headings breadcrumb bar (`H1 › H2 › H3`) pinned to the top of the viewport.
use iced::Widget;

use iced::widget::{button, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

pub fn view_sticky_headings<'a, Message: Clone + 'a>(
    headings: &'a [(usize, String)],
    surface: Color,
    border_color: Color,
    muted: Color,
    on_jump: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'a, Message>> = Vec::new();
    for (idx, (line, title)) in headings.iter().take(3).enumerate() {
        if idx > 0 {
            items.push(text("›").size(12).color(muted).boxed());
        }
        items.push(
            button(text(title).size(12).color(muted))
                .padding(Padding::from([2, 4]))
                .on_press(on_jump(*line))
                .boxed(),
        );
    }

    container(row![].extend(items).spacing(4).align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .padding(Padding::from([4, 12]))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}