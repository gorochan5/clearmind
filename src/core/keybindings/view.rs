use iced::Widget;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Element, Length, Padding};

use crate::core::icons;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{ghost_icon_style, subtle_button_style};
use crate::core::widgets::styles::{kbd_chip_button_style, rounded_text_input_style};

use super::models::{
    CommandCategory, CommandId, KeybindingsConfig, KeybindingsMessage, KeybindingsUiState,
};

fn build_keybindings_body<'a>(
    config: &'a KeybindingsConfig,
    ui: &'a KeybindingsUiState,
    palette: &'a ThemePalette,
    list_height: f32,
    compact: bool,
) -> Element<'a, KeybindingsMessage> {
    let p = *palette;
    let query = ui.search_query.trim().to_lowercase();

    let header = row![
        column![
            text("Keybindings")
                .size(if compact { 14 } else { 18 })
                .color(palette.text),
            text("Click any shortcut pill to record a new binding")
                .size(11)
                .color(palette.text_muted),
        ]
        .spacing(2)
        .width(Length::Fill),
        button(
            row![icons::rotate_ccw().size(12), text("Reset All").size(12)]
                .spacing(5)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([5, 10]))
        .style(subtle_button_style(p, false))
        .on_press(KeybindingsMessage::ResetAll),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let search = text_input("Search commands or shortcuts...", &ui.search_query)
        .size(13)
        .padding(Padding::from([7, 12]))
        .style(rounded_text_input_style(p))
        .on_input(KeybindingsMessage::SearchChanged);

    let categories = [
        CommandCategory::File,
        CommandCategory::Edit,
        CommandCategory::Format,
        CommandCategory::ViewAndNav,
    ];

    let mut list = column![].spacing(6).padding(Padding {
        top: 2.0,
        right: 12.0,
        bottom: 4.0,
        left: 0.0,
    });

    let mut any_match = false;
    for cat in categories {
        let matching: Vec<CommandId> = CommandId::all()
            .iter()
            .copied()
            .filter(|&cmd| {
                if cmd.category() != cat {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let label = cmd.label().to_lowercase();
                let id = cmd.as_str().to_lowercase();
                let shortcut = config.shortcut_for(cmd).to_lowercase();
                label.contains(&query) || id.contains(&query) || shortcut.contains(&query)
            })
            .collect();

        if matching.is_empty() {
            continue;
        }
        any_match = true;

        list = list.push(
            container(
                text(cat.label().to_uppercase())
                    .size(10)
                    .color(palette.text_muted),
            )
            .padding(Padding {
                top: 6.0,
                right: 0.0,
                bottom: 2.0,
                left: 4.0,
            })
            .boxed(),
        );

        for cmd in matching {
            let shortcut = config.shortcut_for(cmd);
            let is_recording = ui.recording_command == Some(cmd);
            let is_unbound = shortcut.is_empty();
            let is_custom = config.overrides.contains_key(cmd.as_str());

            let chip_label = if is_recording {
                "Press keys...".to_string()
            } else if is_unbound {
                "Unbound".to_string()
            } else {
                shortcut.to_string()
            };

            let shortcut_chip = button(
                text(chip_label)
                    .size(12)
                    .font(iced::Font::MONOSPACE),
            )
            .padding(Padding::from([4, 10]))
            .style(kbd_chip_button_style(p, is_recording, is_unbound))
            .on_press(if is_recording {
                KeybindingsMessage::CancelRecording
            } else {
                KeybindingsMessage::StartRecording(cmd)
            });

            let mut actions = row![shortcut_chip]
                .spacing(4)
                .align_y(Alignment::Center);

            if !is_unbound && !is_recording {
                actions = actions.push(
                    button(icons::x().size(12))
                        .padding(Padding::from([4, 5]))
                        .style(ghost_icon_style(p, false))
                        .on_press(KeybindingsMessage::Unbind(cmd))
                        .boxed(),
                );
            }

            if is_custom {
                actions = actions.push(
                    button(icons::rotate_ccw().size(12))
                        .padding(Padding::from([4, 5]))
                        .style(ghost_icon_style(p, false))
                        .on_press(KeybindingsMessage::ResetOne(cmd))
                        .boxed(),
                );
            }

            let row_card = container(
                row![
                    text(cmd.label())
                        .size(13)
                        .color(palette.text)
                        .width(Length::Fill),
                    actions,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([6, 10]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.background.into()),
                border: Border {
                    color: if is_recording {
                        p.accent_text
                    } else {
                        p.border
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            list = list.push(row_card.boxed());
        }
    }

    if !any_match {
        list = list.push(
            container(
                text("No matching shortcuts found")
                    .size(12)
                    .color(palette.text_muted),
            )
            .padding(Padding::from([16, 12]))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .boxed(),
        );
    }

    let mut col = column![header, search].spacing(10);
    if let Some(warn) = &ui.conflict_warning {
        col = col.push(
            container(text(warn).size(12).color(palette.warning))
                .padding(Padding::from([6, 10]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(p.background.into()),
                    border: Border {
                        color: p.warning,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .boxed(),
        );
    }

    col = col.push(
        scrollable(list)
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fixed(list_height))
            .boxed(),
    );

    if compact {
        col.width(Length::Fixed(390.0)).boxed()
    } else {
        col.width(Length::Fill).boxed()
    }
}

pub fn view_quick_popover<'a>(
    config: &'a KeybindingsConfig,
    ui: &'a KeybindingsUiState,
    palette: &'a ThemePalette,
) -> Element<'a, KeybindingsMessage> {
    build_keybindings_body(config, ui, palette, 340.0, true)
}

pub fn view_modal_section<'a>(
    config: &'a KeybindingsConfig,
    ui: &'a KeybindingsUiState,
    palette: &'a ThemePalette,
) -> Element<'a, KeybindingsMessage> {
    build_keybindings_body(config, ui, palette, 300.0, false)
}
