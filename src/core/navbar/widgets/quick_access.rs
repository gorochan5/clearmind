//! 7 Quick Access buttons (`type`, `text-wrap`, `minimize`/`maximize`, `keyboard`, `terminal`, `palette`, `printer`).
use iced::Widget;

use iced::widget::{button, container, row};
use iced::{Element, Padding};

use crate::core::icons;
use crate::core::navbar::models::{NavbarState, QuickAccessPopover};
use crate::core::navbar::update::NavbarMessage;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::ghost_icon_style;

pub const BTN_TYPE_ID: &str = "qa_btn_type";
pub const BTN_KEYBOARD_ID: &str = "qa_btn_keyboard";
pub const BTN_TERMINAL_ID: &str = "qa_btn_terminal";
pub const BTN_PALETTE_ID: &str = "qa_btn_palette";
pub const BTN_PRINTER_ID: &str = "qa_btn_printer";

pub fn view_quick_access<'a>(
    state: &'a NavbarState,
    palette: &ThemePalette,
) -> Element<'a, NavbarMessage> {
    let palette_copy = *palette;

    let type_active = state.active_popover == Some(QuickAccessPopover::Fonts);
    let type_btn = container(
        button(icons::r#type().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(palette_copy, type_active))
            .on_press(NavbarMessage::TogglePopover(QuickAccessPopover::Fonts)),
    )
    .id(iced::widget::Id::new(BTN_TYPE_ID));

    let wrap_btn = button(icons::text_wrap().size(15))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(palette_copy, state.word_wrap_enabled))
        .on_press(NavbarMessage::ToggleWordWrap);

    let width_icon = if state.full_width_enabled {
        icons::minimize().size(15)
    } else {
        icons::maximize().size(15)
    };
    let width_btn = button(width_icon)
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(palette_copy, state.full_width_enabled))
        .on_press(NavbarMessage::ToggleFullWidth);

    let kb_active = state.active_popover == Some(QuickAccessPopover::Keybindings);
    let kb_btn = container(
        button(icons::keyboard().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(palette_copy, kb_active))
            .on_press(NavbarMessage::TogglePopover(QuickAccessPopover::Keybindings)),
    )
    .id(iced::widget::Id::new(BTN_KEYBOARD_ID));

    let term_active = state.active_popover == Some(QuickAccessPopover::CustomCommands);
    let term_btn = container(
        button(icons::terminal().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(palette_copy, term_active))
            .on_press(NavbarMessage::TogglePopover(
                QuickAccessPopover::CustomCommands,
            )),
    )
    .id(iced::widget::Id::new(BTN_TERMINAL_ID));

    let pal_active = state.active_popover == Some(QuickAccessPopover::Themes);
    let pal_btn = container(
        button(icons::palette().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(palette_copy, pal_active))
            .on_press(NavbarMessage::TogglePopover(QuickAccessPopover::Themes)),
    )
    .id(iced::widget::Id::new(BTN_PALETTE_ID));

    let print_btn = container(
        button(icons::printer().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(palette_copy, state.print_preview_open))
            .on_press(NavbarMessage::OpenPrintPreview),
    )
    .id(iced::widget::Id::new(BTN_PRINTER_ID));

    row![
        type_btn,
        wrap_btn,
        width_btn,
        kb_btn,
        term_btn,
        pal_btn,
        print_btn
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center)
    .boxed()
}
