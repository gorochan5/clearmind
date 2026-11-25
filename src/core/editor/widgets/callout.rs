//! GitHub Alert callout card (`> [!NOTE]` .. `> [!CAUTION]`).
use iced::Widget;

use iced::widget::{column, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }
}

pub fn view_callout<'a, Message: 'a>(
    kind: CalloutKind,
    body: Element<'a, Message>,
    accent: Color,
    surface: Color,
) -> Element<'a, Message> {
    let header = row![text(kind.label()).size(13).color(accent)].spacing(6);
    container(column![header, body].spacing(6))
        .width(Length::Fill)
        .padding(Padding::from([10, 14]))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            border: Border {
                color: accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}