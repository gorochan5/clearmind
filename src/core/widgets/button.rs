use iced::Widget;
use iced::widget::{button, text};
use iced::{Border, Color, Element};

use crate::core::themes::ThemePalette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub fn ghost_icon_style(
    palette: ThemePalette,
    is_active: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg) = match status {
            button::Status::Disabled => (Color::TRANSPARENT, palette.text_muted),
            button::Status::Hovered => (
                if is_active {
                    palette.accent
                } else {
                    palette.surface_hover
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
            ),
            button::Status::Pressed => (
                if is_active {
                    palette.accent
                } else {
                    palette.border
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
            ),
            button::Status::Active => (
                if is_active {
                    palette.accent
                } else {
                    Color::TRANSPARENT
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
            ),
        };
        button::Style {
            background: if bg == Color::TRANSPARENT {
                None
            } else {
                Some(bg.into())
            },
            text_color: fg,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn subtle_button_style(
    palette: ThemePalette,
    is_active: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg, border_color) = match status {
            button::Status::Disabled => (palette.surface, palette.text_muted, palette.border),
            button::Status::Hovered => (
                if is_active {
                    palette.accent
                } else {
                    palette.surface_hover
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
                if is_active {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            button::Status::Pressed => (
                if is_active {
                    palette.accent
                } else {
                    palette.border
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
                if is_active {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            button::Status::Active => (
                if is_active {
                    palette.accent
                } else {
                    palette.surface
                },
                if is_active {
                    palette.accent_text
                } else {
                    palette.text
                },
                if is_active {
                    palette.accent
                } else {
                    palette.border
                },
            ),
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn primary_button_style(
    palette: ThemePalette,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg, border_color) = match status {
            button::Status::Disabled => (palette.surface_hover, palette.text_muted, palette.border),
            button::Status::Hovered => (
                Color {
                    a: 0.88,
                    ..palette.accent
                },
                palette.accent_text,
                palette.accent,
            ),
            button::Status::Pressed => (
                Color {
                    a: 0.75,
                    ..palette.accent
                },
                palette.accent_text,
                palette.accent,
            ),
            button::Status::Active => (palette.accent, palette.accent_text, palette.accent),
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn close_window_button_style(
    palette: ThemePalette,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let hover_fg = if palette.is_dark {
            palette.text
        } else {
            palette.background
        };
        let (bg, fg) = match status {
            button::Status::Disabled => (Color::TRANSPARENT, palette.text_muted),
            button::Status::Hovered => (palette.danger, hover_fg),
            button::Status::Pressed => (
                Color {
                    a: 0.85,
                    ..palette.danger
                },
                hover_fg,
            ),
            button::Status::Active => (Color::TRANSPARENT, palette.text_muted),
        };
        button::Style {
            background: if bg == Color::TRANSPARENT {
                None
            } else {
                Some(bg.into())
            },
            text_color: fg,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn styled_button<'a, M: Clone + 'a>(
    label: Element<'a, M>,
    variant: ButtonVariant,
    palette: &'a ThemePalette,
    on_press: Option<M>,
) -> button::Button<'a, M> {
    let mut btn = button(label).padding([6, 12]).style(move |_, status| {
        let (bg, fg, border_color) = match variant {
            ButtonVariant::Primary => (palette.accent, palette.accent_text, palette.accent),
            ButtonVariant::Secondary => (palette.surface_hover, palette.text, palette.border),
            ButtonVariant::Ghost => (Color::TRANSPARENT, palette.text, Color::TRANSPARENT),
            ButtonVariant::Danger => (palette.danger, palette.accent_text, palette.danger),
        };
        let bg = match status {
            button::Status::Hovered if variant == ButtonVariant::Ghost => palette.surface_hover,
            button::Status::Pressed if variant == ButtonVariant::Ghost => palette.border,
            _ => bg,
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: Border {
                color: border_color,
                width: if variant == ButtonVariant::Ghost { 0.0 } else { 1.0 },
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    });
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn
}

pub fn text_button<'a, M: Clone + 'a>(
    label: &'a str,
    variant: ButtonVariant,
    palette: &'a ThemePalette,
    on_press: Option<M>,
) -> Element<'a, M> {
    let fg = match variant {
        ButtonVariant::Primary | ButtonVariant::Danger => palette.accent_text,
        ButtonVariant::Secondary | ButtonVariant::Ghost => palette.text,
    };
    styled_button(text(label).size(13).color(fg).boxed(), variant, palette, on_press).boxed()
}
