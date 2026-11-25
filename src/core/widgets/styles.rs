//! Shared ClearMind widget styles: rounded pick_list, dropdown menu, text_input, slider, and kbd shortcut chip.
use iced::overlay::menu;
use iced::widget::{button, pick_list, slider, text_input};
use iced::{Border, Color, Shadow, Vector};

use crate::core::themes::ThemePalette;

pub fn rounded_pick_list_style(
    palette: ThemePalette,
) -> impl Fn(&iced::Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let (bg, border_color, handle_color) = match status {
            pick_list::Status::Active => (palette.background, palette.border, palette.text_muted),
            pick_list::Status::Hovered => (palette.surface_hover, palette.accent_text, palette.text),
            pick_list::Status::Opened { .. } => {
                (palette.background, palette.accent_text, palette.accent_text)
            }
            pick_list::Status::Disabled => (palette.surface, palette.border, palette.text_muted),
        };
        pick_list::Style {
            text_color: palette.text,
            placeholder_color: palette.text_muted,
            handle_color,
            background: bg.into(),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 8.0.into(),
            },
        }
    }
}

pub fn rounded_menu_style(palette: ThemePalette) -> impl Fn(&iced::Theme) -> menu::Style {
    move |_| menu::Style {
        background: palette.background.into(),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: palette.text,
        selected_text_color: palette.accent_text,
        selected_background: palette.accent.into(),
        shadow: Shadow {
            color: Color {
                a: 0.14,
                ..palette.text
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
    }
}

pub fn rounded_text_input_style(
    palette: ThemePalette,
) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let (bg, border_color, border_width) = match status {
            text_input::Status::Active => (palette.background, palette.border, 1.0),
            text_input::Status::Hovered => (palette.background, palette.text_muted, 1.0),
            text_input::Status::Focused { .. } => (palette.background, palette.accent_text, 1.5),
            text_input::Status::Disabled => (palette.surface, palette.border, 1.0),
        };
        text_input::Style {
            background: bg.into(),
            border: Border {
                color: border_color,
                width: border_width,
                radius: 8.0.into(),
            },
            placeholder: Color {
                a: 0.65,
                ..palette.text_muted
            },
            value: palette.text,
            selection: palette.selection,
        }
    }
}

pub fn rosemary_slider_style(
    palette: ThemePalette,
) -> impl Fn(&iced::Theme, slider::Status) -> slider::Style {
    move |_, status| {
        let handle_bg = match status {
            slider::Status::Active => palette.background,
            slider::Status::Hovered => palette.accent,
            slider::Status::Dragged => palette.accent_text,
        };
        let handle_border = match status {
            slider::Status::Active => palette.accent_text,
            slider::Status::Hovered | slider::Status::Dragged => palette.accent_text,
        };
        let active_rail = if palette.is_dark {
            palette.accent
        } else {
            palette.accent_text
        };
        slider::Style {
            rail: slider::Rail {
                backgrounds: (active_rail.into(), palette.border.into()),
                width: 4.0,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 2.0.into(),
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 7.0 },
                background: handle_bg.into(),
                border_width: 2.0,
                border_color: handle_border,
            },
        }
    }
}

pub fn kbd_chip_button_style(
    palette: ThemePalette,
    is_recording: bool,
    is_unbound: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg, border_color) = if is_recording {
            (palette.accent, palette.accent_text, palette.accent_text)
        } else {
            match status {
                button::Status::Hovered => (palette.surface_hover, palette.text, palette.accent_text),
                button::Status::Pressed => (palette.accent, palette.accent_text, palette.accent_text),
                button::Status::Active | button::Status::Disabled => (
                    palette.background,
                    if is_unbound {
                        palette.text_muted
                    } else {
                        palette.text
                    },
                    palette.border,
                ),
            }
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.06,
                    ..palette.text
                },
                offset: Vector::new(0.0, 1.0),
                blur_radius: 1.0,
            },
            ..Default::default()
        }
    }
}
