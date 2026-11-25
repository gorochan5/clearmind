//! Sleek floating Find & Replace card with Lucide icon toggles, match counter pill,
//! collapsible Replace row, and rosemary-themed rounded inputs.

use iced::Widget;
use iced::widget::{button, column, container, opaque, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::core::editor::models::SearchState;
use crate::core::icons;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{ghost_icon_style, subtle_button_style};
use crate::core::widgets::styles::rounded_text_input_style;

pub const SEARCH_QUERY_INPUT_ID: &str = "clearmind_search_query_input";
pub const SEARCH_REPLACE_INPUT_ID: &str = "clearmind_search_replace_input";

#[allow(clippy::too_many_arguments)]
pub fn view_search_bar<'a, Message: Clone + 'a>(
    state: &'a SearchState,
    palette: &ThemePalette,
    on_query_change: impl Fn(String) -> Message + 'a,
    on_replace_change: impl Fn(String) -> Message + 'a,
    on_toggle_replace: Message,
    on_toggle_case: Message,
    on_toggle_word: Message,
    on_toggle_regex: Message,
    on_prev: Message,
    on_next: Message,
    on_replace_one: Message,
    on_replace_all: Message,
    on_close: Message,
) -> Element<'a, Message> {
    let p = *palette;

    let toggle_replace_btn = button(
        if state.show_replace {
            icons::chevron_down().size(13)
        } else {
            icons::chevron_right().size(13)
        },
    )
    .padding(Padding::from([5, 5]))
    .style(ghost_icon_style(p, false))
    .on_press(on_toggle_replace);

    let find_input = text_input("Find...", &state.query)
        .id(iced::widget::Id::new(SEARCH_QUERY_INPUT_ID))
        .on_input(on_query_change)
        .on_submit(on_next.clone())
        .size(13)
        .padding(Padding::from([5, 9]))
        .width(Length::Fill)
        .style(rounded_text_input_style(p));

    let case_btn = button(icons::case_sensitive().size(14))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(p, state.case_sensitive))
        .on_press(on_toggle_case);

    let word_btn = button(icons::whole_word().size(14))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(p, state.whole_word))
        .on_press(on_toggle_word);

    let regex_btn = button(icons::regex().size(14))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(p, state.use_regex))
        .on_press(on_toggle_regex);

    let has_matches = !state.matches.is_empty();
    let no_results = !state.query.is_empty() && !has_matches;
    let count_label = if state.query.is_empty() {
        String::new()
    } else if has_matches {
        let cur = state.active_match.map(|i| i + 1).unwrap_or(1);
        format!("{cur}/{}", state.matches.len())
    } else {
        "0/0".to_string()
    };

    let count_color = if no_results {
        p.danger
    } else {
        p.text_muted
    };

    let mut prev_btn = button(icons::chevron_up().size(14))
        .padding(Padding::from([4, 5]))
        .style(ghost_icon_style(p, false));
    if has_matches {
        prev_btn = prev_btn.on_press(on_prev);
    }

    let mut next_btn = button(icons::chevron_down().size(14))
        .padding(Padding::from([4, 5]))
        .style(ghost_icon_style(p, false));
    if has_matches {
        next_btn = next_btn.on_press(on_next);
    }

    let close_btn = button(icons::x().size(14))
        .padding(Padding::from([4, 5]))
        .style(ghost_icon_style(p, false))
        .on_press(on_close);

    let divider = || {
        container(iced::widget::Space::new().width(1).height(16))
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(16.0))
            .style(move |_| container::Style {
                background: Some(p.border.into()),
                ..Default::default()
            })
    };

    let mut find_row_items: Vec<Element<'a, Message>> = vec![
        toggle_replace_btn.boxed(),
        find_input.boxed(),
        row![case_btn, word_btn, regex_btn]
            .spacing(2)
            .align_y(Alignment::Center)
            .boxed(),
    ];

    if !count_label.is_empty() {
        find_row_items.push(
            container(
                text(count_label)
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .color(count_color),
            )
            .padding(Padding::from([2, 6]))
            .style(move |_| container::Style {
                background: Some(
                    if no_results {
                        Color { a: 0.12, ..p.danger }
                    } else {
                        p.background
                    }
                    .into(),
                ),
                border: Border {
                    color: if no_results { p.danger } else { p.border },
                    width: 1.0,
                    radius: 999.0.into(),
                },
                ..Default::default()
            })
            .boxed(),
        );
    }

    find_row_items.push(divider().boxed());
    find_row_items.push(
        row![prev_btn, next_btn, close_btn]
            .spacing(2)
            .align_y(Alignment::Center)
            .boxed(),
    );

    let find_row = row![]
        .extend(find_row_items)
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let mut card_rows: Vec<Element<'a, Message>> = vec![find_row.boxed()];

    if state.show_replace {
        let replace_input = text_input("Replace with...", &state.replace)
            .id(iced::widget::Id::new(SEARCH_REPLACE_INPUT_ID))
            .on_input(on_replace_change)
            .on_submit(on_replace_one.clone())
            .size(13)
            .padding(Padding::from([5, 9]))
            .width(Length::Fill)
            .style(rounded_text_input_style(p));

        let mut replace_one_btn = button(
            row![icons::replace().size(12), text("Replace").size(12)]
                .spacing(4)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([5, 10]))
        .style(subtle_button_style(p, false));
        if has_matches {
            replace_one_btn = replace_one_btn.on_press(on_replace_one);
        }

        let mut replace_all_btn = button(text("All").size(12))
            .padding(Padding::from([5, 10]))
            .style(subtle_button_style(p, has_matches));
        if has_matches {
            replace_all_btn = replace_all_btn.on_press(on_replace_all);
        }

        let replace_row = row![
            iced::widget::Space::new().width(Length::Fixed(23.0)),
            replace_input,
            replace_one_btn,
            replace_all_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill);

        card_rows.push(replace_row.boxed());
    }

    let card = opaque(
        container(column![].extend(card_rows).spacing(8).width(Length::Fill))
            .width(Length::Fixed(468.0))
            .padding(Padding::from([9, 11]))
            .style(move |_| container::Style {
                background: Some(p.surface.into()),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                shadow: Shadow {
                    color: Color {
                        a: 0.16,
                        ..p.text
                    },
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 18.0,
                },
                ..Default::default()
            }),
    );

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_right(Length::Fill)
        .padding(Padding {
            top: 44.0,
            right: 18.0,
            bottom: 0.0,
            left: 0.0,
        })
        .boxed()
}
