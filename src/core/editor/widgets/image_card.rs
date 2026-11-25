//! Inline image card with loading/error states and hover zoom button.
use iced::Widget;

use iced::widget::{button, column, container, image, row, text};
use iced::{Border, Color, Element, Length, Padding};

pub fn view_image_card<'a, Message: Clone + 'a>(
    handle: Option<&'a image::Handle>,
    alt: &'a str,
    surface: Color,
    border_color: Color,
    muted: Color,
    on_zoom: Message,
) -> Element<'a, Message> {
    let zoom_btn = button(text("Zoom").size(11).color(muted))
        .padding(Padding::from([2, 6]))
        .on_press(on_zoom);

    let body: Element<'a, Message> = if let Some(h) = handle {
        image(h.clone()).width(Length::Fill).boxed()
    } else {
        text(if alt.is_empty() { "Loading image..." } else { alt })
            .size(12)
            .color(muted)
            .boxed()
    };

    container(column![row![iced::widget::space::horizontal(), zoom_btn], body].spacing(4))
        .width(Length::Fill)
        .padding(Padding::from([8, 10]))
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