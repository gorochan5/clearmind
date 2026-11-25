use iced::Widget;
use iced::widget::{center, container, mouse_area, opaque, stack};
use iced::{Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::core::themes::ThemePalette;

pub fn modal_overlay<'a, M: Clone + 'a>(
    base: Element<'a, M>,
    content: Element<'a, M>,
    palette: &'a ThemePalette,
    on_backdrop_press: M,
    max_width: f32,
) -> Element<'a, M> {
    modal_overlay_sized(base, content, palette, on_backdrop_press, max_width, 620.0)
}

pub fn modal_overlay_sized<'a, M: Clone + 'a>(
    base: Element<'a, M>,
    content: Element<'a, M>,
    palette: &'a ThemePalette,
    on_backdrop_press: M,
    max_width: f32,
    max_height: f32,
) -> Element<'a, M> {
    let palette_copy = *palette;
    let backdrop = Color {
        a: 0.45,
        ..palette_copy.background
    };

    let dialog = container(content)
        .width(Length::Fixed(max_width))
        .height(Length::Fixed(max_height))
        .padding(Padding {
            top: 22.0,
            right: 20.0,
            bottom: 20.0,
            left: 24.0,
        })
        .style(move |_| container::Style {
            background: Some(palette_copy.surface.into()),
            border: Border {
                color: palette_copy.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.18,
                    ..palette_copy.text
                },
                offset: Vector::new(0.0, 10.0),
                blur_radius: 28.0,
            },
            ..Default::default()
        });

    stack![
        base,
        opaque(
            mouse_area(
                center(opaque(dialog))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(backdrop.into()),
                        ..Default::default()
                    })
            )
            .on_press(on_backdrop_press)
        )
    ]
    .boxed()
}
