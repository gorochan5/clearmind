//! Mermaid & display-math SVG diagram block with hover zoom button.
use iced::Widget;

use iced::widget::{button, container, row, svg, text};
use iced::{Border, Color, Element, Length, Padding};

pub fn view_diagram_block<'a, Message: Clone + 'a>(
    svg_bytes: Option<&'a [u8]>,
    error_message: Option<&'a str>,
    surface: Color,
    border_color: Color,
    muted: Color,
    on_zoom: Message,
) -> Element<'a, Message> {
    let zoom_btn = button(text("Zoom").size(11).color(muted))
        .padding(Padding::from([2, 6]))
        .on_press(on_zoom);

    let body: Element<'a, Message> = if let Some(bytes) = svg_bytes {
        svg(svg::Handle::from_memory(bytes.to_vec()))
            .width(Length::Fill)
            .boxed()
    } else if let Some(err) = error_message {
        text(err).size(12).color(muted).boxed()
    } else {
        text("Rendering diagram...").size(12).color(muted).boxed()
    };

    container(iced::widget::column![
        row![iced::widget::space::horizontal(), zoom_btn],
        body
    ])
    .width(Length::Fill)
    .padding(Padding::from([10, 12]))
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