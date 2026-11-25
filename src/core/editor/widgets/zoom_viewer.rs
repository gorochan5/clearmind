//! Full-window zoom viewer overlay for Mermaid diagrams, math SVGs, and images.
use iced::Widget;

use iced::widget::{button, center, column, container, row, svg, text};
use iced::{Color, Element, Length, Padding};

#[allow(clippy::too_many_arguments)]
pub fn view_zoom_viewer<'a, Message: Clone + 'a>(
    svg_bytes: Option<&'a [u8]>,
    zoom: f32,
    backdrop: Color,
    fg: Color,
    on_zoom_in: Message,
    on_zoom_out: Message,
    on_reset: Message,
    on_close: Message,
) -> Element<'a, Message> {
    let toolbar = row![
        text(format!("{}%", (zoom * 100.0).round() as u32))
            .size(13)
            .color(fg),
        button(text("-").size(13)).on_press(on_zoom_out),
        button(text("0").size(13)).on_press(on_reset),
        button(text("+").size(13)).on_press(on_zoom_in),
        iced::widget::space::horizontal(),
        button(text("Esc / Close").size(13)).on_press(on_close),
    ]
    .spacing(8)
    .padding(Padding::from([8, 12]));

    let content: Element<'a, Message> = if let Some(bytes) = svg_bytes {
        center(svg(svg::Handle::from_memory(bytes.to_vec())).width(Length::Fill)).boxed()
    } else {
        center(text("No preview available").color(fg)).boxed()
    };

    container(column![toolbar, content].width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(backdrop.into()),
            ..container::Style::default()
        })
        .boxed()
}