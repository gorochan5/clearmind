use iced::Widget;
use iced::widget::{button, column, container, text};
use iced::{Border, Element, Length};

use crate::core::themes::ThemePalette;

#[derive(Debug, Clone)]
pub struct ContextMenuItem<M> {
    pub label: String,
    pub shortcut: Option<String>,
    pub message: M,
    pub enabled: bool,
}

pub fn view_context_menu<'a, M: Clone + 'a>(
    items: &'a [ContextMenuItem<M>],
    palette: &'a ThemePalette,
) -> Element<'a, M> {
    let mut col = column![].spacing(2);
    for item in items {
        let fg = if item.enabled {
            palette.text
        } else {
            palette.text_muted
        };
        let label = match &item.shortcut {
            Some(sc) => format!("{}   {}", item.label, sc),
            None => item.label.clone(),
        };
        let mut btn = button(text(label).size(13).color(fg))
            .width(Length::Fill)
            .padding([5, 10])
            .style(move |_, status| button::Style {
                background: Some(
                    if matches!(status, button::Status::Hovered) && item.enabled {
                        palette.surface_hover
                    } else {
                        palette.surface
                    }
                    .into(),
                ),
                text_color: fg,
                border: Border {
                    radius: 4.0.into(),
                    width: 0.0,
                    color: palette.border,
                },
                ..Default::default()
            });
        if item.enabled {
            btn = btn.on_press(item.message.clone());
        }
        col = col.push(btn);
    }

    container(col)
        .padding(4)
        .width(Length::Fixed(200.0))
        .style(move |_| container::Style {
            background: Some(palette.surface.into()),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .boxed()
}
