use iced::Widget;
use iced::widget::{column, row, text, Space};
use iced::{Element, Length};

use crate::core::themes::ThemePalette;

use super::button::{text_button, ButtonVariant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmPrompt {
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub secondary_label: Option<String>,
    pub cancel_label: String,
}

pub fn view_confirm_prompt<'a, M: Clone + 'a>(
    prompt: &'a ConfirmPrompt,
    palette: &'a ThemePalette,
    on_confirm: M,
    on_secondary: Option<M>,
    on_cancel: M,
) -> Element<'a, M> {
    let mut actions = row![Space::new().width(Length::Fill)].spacing(8);

    actions = actions.push(text_button(
        &prompt.cancel_label,
        ButtonVariant::Ghost,
        palette,
        Some(on_cancel),
    ));

    if let (Some(sec_label), Some(sec_msg)) = (&prompt.secondary_label, on_secondary) {
        actions = actions.push(text_button(
            sec_label,
            ButtonVariant::Secondary,
            palette,
            Some(sec_msg),
        ));
    }

    actions = actions.push(text_button(
        &prompt.confirm_label,
        ButtonVariant::Primary,
        palette,
        Some(on_confirm),
    ));

    column![
        text(&prompt.title).size(16).color(palette.text),
        text(&prompt.message).size(13).color(palette.text_muted),
        Space::new().height(8),
        actions,
    ]
    .spacing(10)
    .boxed()
}
