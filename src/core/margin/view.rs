use iced::Widget;
use iced::widget::{column, row, slider, text, toggler};
use iced::{Alignment, Element, Length};

use crate::core::themes::ThemePalette;
use crate::core::widgets::styles::rosemary_slider_style;

use super::models::{MarginConfig, MarginMessage};

pub fn view_modal_section<'a>(
    config: &'a MarginConfig,
    palette: &'a ThemePalette,
) -> Element<'a, MarginMessage> {
    let p = *palette;
    let mut col = column![
        text("Margin & Layout Width").size(18).color(palette.text),
        row![
            text("Full Width Mode")
                .size(13)
                .color(palette.text)
                .width(Length::Fill),
            toggler(config.full_width).on_toggle(MarginMessage::SetFullWidth),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    if !config.full_width {
        col = col.push(
            row![
                text("Content Max Width")
                    .size(13)
                    .color(palette.text)
                    .width(Length::Fixed(128.0)),
                slider(
                    560.0..=1400.0,
                    config.content_max_width,
                    MarginMessage::SetMaxWidth
                )
                .step(10.0)
                .style(rosemary_slider_style(p))
                .width(Length::Fill),
                text(format!("{:.0}px", config.content_max_width))
                    .size(13)
                    .color(palette.text_muted)
                    .width(Length::Fixed(52.0)),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }

    col.boxed()
}
