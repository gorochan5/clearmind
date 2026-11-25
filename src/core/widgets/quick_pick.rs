use iced::Widget;
use iced::widget::{button, column, container, scrollable, text, text_input};
use iced::{Border, Element, Length};

use crate::core::themes::ThemePalette;

#[derive(Debug, Clone)]
pub struct QuickPickItem<M> {
    pub title: String,
    pub subtitle: Option<String>,
    pub message: M,
}

pub fn view_quick_pick<'a, M: Clone + 'a>(
    placeholder: &'a str,
    query: &'a str,
    items: &'a [QuickPickItem<M>],
    selected_index: usize,
    palette: &'a ThemePalette,
    on_query_change: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    let input = text_input(placeholder, query)
        .size(14)
        .padding(8)
        .on_input(on_query_change);

    let mut list = column![].spacing(2);
    for (idx, item) in items.iter().enumerate() {
        let is_selected = idx == selected_index;
        let label = match &item.subtitle {
            Some(sub) => format!("{}  —  {}", item.title, sub),
            None => item.title.clone(),
        };
        list = list.push(
            button(text(label).size(13).color(palette.text))
                .width(Length::Fill)
                .padding([6, 10])
                .style(move |_, _| button::Style {
                    background: Some(
                        if is_selected {
                            palette.surface_hover
                        } else {
                            palette.surface
                        }
                        .into(),
                    ),
                    text_color: palette.text,
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: palette.border,
                    },
                    ..Default::default()
                })
                .on_press(item.message.clone()),
        );
    }

    container(
        column![
            input,
            scrollable(list).height(Length::Fixed(260.0)),
        ]
        .spacing(8),
    )
    .padding(12)
    .width(Length::Fixed(480.0))
    .style(move |_| container::Style {
        background: Some(palette.surface.into()),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .boxed()
}
