use iced::Widget;
use iced::widget::{button, container, row, text};
use iced::{Alignment, Border, Element};

use crate::core::themes::ThemePalette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastItem {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
}

pub fn view_toast<'a, M: Clone + 'a>(
    toast: &'a ToastItem,
    palette: &'a ThemePalette,
    on_dismiss: M,
) -> Element<'a, M> {
    let accent = match toast.kind {
        ToastKind::Info => palette.accent,
        ToastKind::Success => palette.success,
        ToastKind::Warning => palette.warning,
        ToastKind::Error => palette.danger,
    };

    container(
        row![
            text(&toast.message).size(13).color(palette.text),
            button(text("×").size(14).color(palette.text_muted))
                .padding([2, 6])
                .on_press(on_dismiss),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .style(move |_| container::Style {
        background: Some(palette.surface.into()),
        border: Border {
            color: accent,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .boxed()
}
