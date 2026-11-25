use iced::Widget;
use iced::widget::{container, mouse_area, opaque, stack};
use iced::{Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::core::themes::ThemePalette;

pub fn popover_overlay<'a, M: Clone + 'a>(
    base: Element<'a, M>,
    content: Element<'a, M>,
    palette: &'a ThemePalette,
    top_offset: f32,
    right_offset: f32,
    on_dismiss: M,
) -> Element<'a, M> {
    let palette_copy = *palette;
    let card = container(content)
        .padding(14)
        .style(move |_| container::Style {
            background: Some(palette_copy.surface.into()),
            border: Border {
                color: palette_copy.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.14,
                    ..palette_copy.text
                },
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
            ..Default::default()
        });

    let positioned = container(opaque(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: top_offset,
            right: right_offset,
            bottom: 0.0,
            left: 0.0,
        })
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top);

    stack![base, mouse_area(positioned).on_press(on_dismiss)].boxed()
}
