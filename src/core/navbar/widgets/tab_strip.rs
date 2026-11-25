//! Scrollable tab strip with ellipsis truncation, middle-click close, `+`, and `folder-open` + recent dropdown.
use iced::Widget;

use iced::widget::{button, container, mouse_area, row, scrollable, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::core::icons;
use crate::core::navbar::models::TabItem;
use crate::core::navbar::update::NavbarMessage;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::ghost_icon_style;

pub const MAX_TAB_WIDTH: f32 = 180.0;

pub fn view_tab_strip<'a>(
    tabs: &[TabItem],
    active_uuid: Option<&str>,
    palette: &ThemePalette,
) -> Element<'a, NavbarMessage> {
    let mut tab_row = row![].spacing(4).align_y(iced::Alignment::Center);
    let palette_copy = *palette;

    for tab in tabs {
        let is_active = active_uuid == Some(tab.uuid.as_str());
        let mut label = tab.title.clone();
        if tab.is_deleted {
            label.push_str(" (deleted)");
        }
        if tab.is_dirty {
            label.push_str(" •");
        }
        if tab.has_conflict {
            label.push_str(" ⚠");
        }

        let close_tab_btn = button(icons::x().size(12))
            .padding(Padding::from([2, 3]))
            .style(ghost_icon_style(palette_copy, false))
            .on_press(NavbarMessage::CloseTab(tab.uuid.clone()));

        let tab_btn = button(
            row![
                text(label)
                    .size(12)
                    .color(if is_active {
                        palette_copy.text
                    } else {
                        palette_copy.text_muted
                    })
                    .wrapping(text::Wrapping::None),
                close_tab_btn,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Shrink)
        .padding(Padding::from([4, 8]))
        .style(move |_, status| {
            let bg = if is_active {
                palette_copy.background
            } else {
                match status {
                    button::Status::Hovered => palette_copy.surface_hover,
                    _ => Color::TRANSPARENT,
                }
            };
            button::Style {
                background: Some(bg.into()),
                text_color: if is_active {
                    palette_copy.text
                } else {
                    palette_copy.text_muted
                },
                border: Border {
                    color: if is_active {
                        palette_copy.accent
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if is_active { 1.0 } else { 0.0 },
                    radius: 6.0.into(),
                },
                ..button::Style::default()
            }
        })
        .on_press(NavbarMessage::SelectTab(tab.uuid.clone()));

        let wrapped = mouse_area(container(tab_btn))
            .on_middle_press(NavbarMessage::CloseTab(tab.uuid.clone()))
            .on_right_press(NavbarMessage::OpenTabContextMenu(tab.uuid.clone()));

        tab_row = tab_row.push(wrapped);
    }

    let scroll_tabs = scrollable(tab_row)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(0).scroller_width(0),
        ))
        .width(Length::Shrink);

    let new_btn = button(icons::plus().size(14))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(palette_copy, false))
        .on_press(NavbarMessage::NewTab);

    let open_btn = button(icons::folder_open().size(14))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(palette_copy, false))
        .on_press(NavbarMessage::OpenFile);

    let recent_btn = button(icons::chevron_down().size(12))
        .padding(Padding::from([4, 4]))
        .style(ghost_icon_style(palette_copy, false))
        .on_press(NavbarMessage::ToggleRecentDropdown);

    row![scroll_tabs, new_btn, open_btn, recent_btn]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .boxed()
}
