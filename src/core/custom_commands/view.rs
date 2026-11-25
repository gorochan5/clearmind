use iced::Widget;
use iced::widget::{button, column, container, opaque, row, scrollable, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::core::icons;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{ghost_icon_style, primary_button_style, subtle_button_style};
use crate::core::widgets::styles::rounded_text_input_style;

use super::models::{
    CustomCommand, CustomCommandsMessage, CustomCommandsUiState, SlashMenuState, TriggerContext,
};

fn build_commands_body<'a>(
    commands: &'a [CustomCommand],
    ui: &'a CustomCommandsUiState,
    palette: &'a ThemePalette,
    compact: bool,
) -> Element<'a, CustomCommandsMessage> {
    let p = *palette;

    let header = column![
        text("Custom Slash Commands")
            .size(if compact { 14 } else { 18 })
            .color(palette.text),
        text("Type / in the editor to insert reusable Markdown snippets")
            .size(11)
            .color(palette.text_muted),
    ]
    .spacing(2);

    let list_el: Element<'a, CustomCommandsMessage> = if commands.is_empty() {
        container(
            column![
                icons::terminal().size(18).color(palette.text_muted),
                text("No custom commands yet")
                    .size(13)
                    .color(palette.text),
                text("Create your first slash template below to speed up writing.")
                    .size(11)
                    .color(palette.text_muted),
            ]
            .spacing(4)
            .align_x(Alignment::Center),
        )
        .padding(Padding::from([16, 14]))
        .width(Length::Fill)
        .center_x(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.background.into()),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .boxed()
    } else {
        let mut list = column![].spacing(6).padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: 0.0,
        });
        for (idx, cmd) in commands.iter().enumerate() {
            let trigger_pill = container(
                text(format!("/{}", cmd.trigger))
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(palette.accent_text),
            )
            .padding(Padding::from([3, 8]))
            .style(move |_| container::Style {
                background: Some(p.accent.into()),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            let preview = if cmd.template.chars().count() > 48 {
                let s: String = cmd.template.chars().take(48).collect();
                format!("{s}…")
            } else {
                cmd.template.clone()
            };

            let card = container(
                row![
                    trigger_pill,
                    column![
                        text(&cmd.label).size(13).color(palette.text),
                        text(preview)
                            .size(11)
                            .font(iced::Font::MONOSPACE)
                            .color(palette.text_muted),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    button(icons::trash().size(13))
                        .padding(Padding::from([5, 6]))
                        .style(ghost_icon_style(p, false))
                        .on_press(CustomCommandsMessage::RemoveCommand(idx)),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([8, 10]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.background.into()),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });
            list = list.push(card.boxed());
        }

        if commands.len() > 3 {
            scrollable(list)
                .spacing(6)
                .width(Length::Fill)
                .height(Length::Fixed(180.0))
                .boxed()
        } else {
            list.width(Length::Fill).boxed()
        }
    };

    let vars = ["{{cursor}}", "{{date}}", "{{time}}", "{{filename}}"];
    let mut var_chips = row![
        text("Insert:").size(11).color(palette.text_muted),
    ]
    .spacing(5)
    .align_y(Alignment::Center);

    for var in vars {
        let next_val = format!("{}{var}", ui.new_template);
        var_chips = var_chips.push(
            button(text(var).size(11).font(iced::Font::MONOSPACE))
                .padding(Padding::from([2, 7]))
                .style(subtle_button_style(p, false))
                .on_press(CustomCommandsMessage::NewTemplateChanged(next_val))
                .boxed(),
        );
    }

    let can_add = !ui.new_trigger.trim().is_empty() && !ui.new_template.trim().is_empty();
    let mut add_btn = button(
        row![icons::plus().size(13), text("Add Command").size(12)]
            .spacing(5)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([7, 14]))
    .style(primary_button_style(p));
    if can_add {
        add_btn = add_btn.on_press(CustomCommandsMessage::AddCommand);
    }

    let composer = container(
        column![
            text("New Command").size(13).color(palette.text),
            row![
                column![
                    text("Trigger").size(11).color(palette.text_muted),
                    text_input("/meeting", &ui.new_trigger)
                        .size(13)
                        .padding(Padding::from([7, 10]))
                        .style(rounded_text_input_style(p))
                        .on_input(CustomCommandsMessage::NewTriggerChanged),
                ]
                .spacing(4)
                .width(Length::FillPortion(1)),
                column![
                    text("Label").size(11).color(palette.text_muted),
                    text_input("Meeting Notes", &ui.new_label)
                        .size(13)
                        .padding(Padding::from([7, 10]))
                        .style(rounded_text_input_style(p))
                        .on_input(CustomCommandsMessage::NewLabelChanged),
                ]
                .spacing(4)
                .width(Length::FillPortion(1)),
            ]
            .spacing(10),
            column![
                text("Template").size(11).color(palette.text_muted),
                text_input("## Meeting — {{date}}\n- {{cursor}}", &ui.new_template)
                    .size(13)
                    .padding(Padding::from([7, 10]))
                    .style(rounded_text_input_style(p))
                    .on_input(CustomCommandsMessage::NewTemplateChanged),
            ]
            .spacing(4),
            var_chips,
            row![iced::widget::space::horizontal(), add_btn].align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding(14)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(p.background.into()),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    let root = column![header, list_el, composer].spacing(12);
    if compact {
        root.width(Length::Fixed(380.0)).boxed()
    } else {
        root.width(Length::Fill).boxed()
    }
}

pub fn view_quick_popover<'a>(
    commands: &'a [CustomCommand],
    ui: &'a CustomCommandsUiState,
    palette: &'a ThemePalette,
) -> Element<'a, CustomCommandsMessage> {
    build_commands_body(commands, ui, palette, true)
}

pub fn view_modal_section<'a>(
    commands: &'a [CustomCommand],
    ui: &'a CustomCommandsUiState,
    palette: &'a ThemePalette,
) -> Element<'a, CustomCommandsMessage> {
    build_commands_body(commands, ui, palette, false)
}

/// Floating context-aware `/` & `Ctrl+Space` Intellisense popup (Prose commands & LaTeX functions).
pub fn view_slash_popup<'a, Message: Clone + 'a>(
    menu: &'a SlashMenuState,
    palette: &'a ThemePalette,
    anchor_x: f32,
    anchor_y: f32,
    on_accept: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let p = *palette;
    let is_math = menu.context == TriggerContext::Math;
    let trigger_prefix = if is_math { "\\" } else { "/" };

    let mode_icon = if is_math {
        icons::sigma().size(13).color(p.accent_text)
    } else {
        icons::terminal().size(13).color(p.accent_text)
    };
    let mode_label = if is_math {
        "LaTeX Functions"
    } else {
        "Commands"
    };

    let mut header_items: Vec<Element<'a, Message>> = vec![
        mode_icon.boxed(),
        text(mode_label).size(12).color(p.accent_text).boxed(),
    ];

    if let Some(arg) = &menu.arg
        && !arg.trim().is_empty()
    {
        header_items.push(
            container(
                text(format!("arg: {}", arg.trim()))
                    .size(10)
                    .font(iced::Font::MONOSPACE)
                    .color(p.accent_text),
            )
            .padding(Padding::from([1, 6]))
            .style(move |_| container::Style {
                background: Some(p.accent.into()),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 999.0.into(),
                },
                ..Default::default()
            })
            .boxed(),
        );
    }

    header_items.push(iced::widget::space::horizontal().boxed());
    header_items.push(text("↑↓  Enter / Tab").size(10).color(p.text_muted).boxed());

    let header_row = row![]
        .extend(header_items)
        .spacing(6)
        .align_y(Alignment::Center);

    let header = container(header_row)
        .padding(Padding::from([7, 10]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: Border {
                color: p.border,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    let mut items_col = column![].spacing(2).padding(Padding::from([4, 6]));
    for (idx, item) in menu.suggestions.iter().enumerate() {
        let is_selected = idx == menu.selected_index;
        let badge_bg = if is_selected {
            p.background
        } else {
            p.surface
        };
        let badge_fg = if is_selected {
            p.accent_text
        } else {
            p.text
        };

        let trigger_chip = container(
            text(format!("{trigger_prefix}{}", item.trigger))
                .size(11)
                .font(iced::Font::MONOSPACE)
                .color(badge_fg),
        )
        .padding(Padding::from([2, 6]))
        .style(move |_| container::Style {
            background: Some(badge_bg.into()),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        });

        let row_body = row![
            trigger_chip,
            column![
                text(&item.label)
                    .size(12)
                    .color(if is_selected { p.accent_text } else { p.text }),
                text(&item.description)
                    .size(10)
                    .color(if is_selected {
                        p.accent_text
                    } else {
                        p.text_muted
                    }),
            ]
            .spacing(1)
            .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);

        items_col = items_col.push(
            button(row_body)
                .width(Length::Fill)
                .padding(Padding::from([5, 8]))
                .style(move |_, status| {
                    let bg = if is_selected {
                        p.accent
                    } else if matches!(status, button::Status::Hovered) {
                        p.surface_hover
                    } else {
                        Color::TRANSPARENT
                    };
                    button::Style {
                        background: Some(bg.into()),
                        text_color: if is_selected { p.accent_text } else { p.text },
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    }
                })
                .on_press(on_accept(idx)),
        );
    }

    let list_h = ((menu.suggestions.len() as f32) * 44.0 + 12.0).clamp(52.0, 268.0);
    let list_scroll = scrollable(items_col)
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fixed(list_h));

    let card = opaque(
        container(
            column![
                header,
                container(iced::widget::Space::new().height(1))
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .style(move |_| container::Style {
                        background: Some(p.border.into()),
                        ..Default::default()
                    }),
                list_scroll,
            ]
            .width(Length::Fill),
        )
        .width(Length::Fixed(348.0))
        .clip(true)
        .style(move |_| container::Style {
            background: Some(p.background.into()),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.18,
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
        .padding(Padding {
            top: anchor_y.max(44.0),
            right: 0.0,
            bottom: 0.0,
            left: anchor_x.max(16.0),
        })
        .boxed()
}
