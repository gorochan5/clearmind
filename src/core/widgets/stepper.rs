use iced::Widget;
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length};

use crate::core::themes::ThemePalette;

use super::button::subtle_button_style;

pub fn view_stepper<'a, M: Clone + 'a>(
    label: &'a str,
    value_display: String,
    palette: &'a ThemePalette,
    on_decrement: Option<M>,
    on_increment: Option<M>,
) -> Element<'a, M> {
    let mut dec_btn = button(text("-").size(14))
        .padding([4, 10])
        .style(subtle_button_style(*palette, false));
    if let Some(msg) = on_decrement {
        dec_btn = dec_btn.on_press(msg);
    }

    let mut inc_btn = button(text("+").size(14))
        .padding([4, 10])
        .style(subtle_button_style(*palette, false));
    if let Some(msg) = on_increment {
        inc_btn = inc_btn.on_press(msg);
    }

    row![
        text(label).size(13).color(palette.text).width(Length::Fill),
        dec_btn,
        text(value_display)
            .size(13)
            .color(palette.text)
            .width(Length::Fixed(52.0)),
        inc_btn,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .boxed()
}
