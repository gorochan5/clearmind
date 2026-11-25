//! External file change conflict banner (`Reload from Disk` / `Keep My Changes`).
use iced::Widget;

use iced::widget::{button, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

pub fn view_conflict_banner<'a, Message: Clone + 'a>(
    message: &'a str,
    surface: Color,
    accent: Color,
    fg: Color,
    on_reload: Message,
    on_keep: Message,
) -> Element<'a, Message> {
    let content = row![
        text(message).size(13).color(fg),
        iced::widget::space::horizontal(),
        button(text("Reload from Disk").size(12))
            .padding(Padding::from([4, 10]))
            .on_press(on_reload),
        button(text("Keep My Changes").size(12))
            .padding(Padding::from([4, 10]))
            .on_press(on_keep),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding(Padding::from([8, 12]))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            border: Border {
                color: accent,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}