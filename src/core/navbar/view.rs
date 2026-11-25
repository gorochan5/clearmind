//! 36px borderless navbar layout: Left (Tabs + Drag), Center (Quick Access Pill), Right (Drag + Settings + Window Controls).
use iced::Widget;

use iced::widget::{button, container, mouse_area, row, space};
use iced::{Border, Element, Length, Padding};

use crate::core::icons;
use crate::core::navbar::models::{NavbarState, TabItem};
use crate::core::navbar::update::NavbarMessage;
use crate::core::navbar::widgets::quick_access::view_quick_access;
use crate::core::navbar::widgets::tab_strip::view_tab_strip;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{close_window_button_style, ghost_icon_style};

pub const NAVBAR_HEIGHT: f32 = 36.0;

fn drag_spacer<'a>() -> Element<'a, NavbarMessage> {
    mouse_area(
        container(space::horizontal())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(NavbarMessage::DragWindow)
    .on_double_click(NavbarMessage::ToggleMaximizeWindow)
    .boxed()
}

pub fn view_navbar<'a>(
    state: &'a NavbarState,
    tabs: &[TabItem],
    active_uuid: Option<&str>,
    palette: &ThemePalette,
) -> Element<'a, NavbarMessage> {
    let tab_strip = view_tab_strip(tabs, active_uuid, palette);
    let left_zone = row![tab_strip, drag_spacer()]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(Length::FillPortion(1))
        .height(Length::Fill);

    let center_bg = palette.background;
    let center_border = palette.border;
    let center_zone = container(view_quick_access(state, palette))
        .padding(Padding::from([2, 6]))
        .width(Length::Shrink)
        .style(move |_| container::Style {
            background: Some(center_bg.into()),
            border: Border {
                color: center_border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    let settings_btn = button(icons::settings().size(15))
        .padding(Padding::from([4, 6]))
        .style(ghost_icon_style(*palette, false))
        .on_press(NavbarMessage::OpenSettingsModal);

    let min_btn = button(icons::minus().size(14))
        .padding(Padding::from([4, 8]))
        .style(ghost_icon_style(*palette, false))
        .on_press(NavbarMessage::MinimizeWindow);

    let max_btn = button(icons::square().size(13))
        .padding(Padding::from([4, 8]))
        .style(ghost_icon_style(*palette, false))
        .on_press(NavbarMessage::ToggleMaximizeWindow);

    let close_btn = button(icons::x().size(14))
        .padding(Padding::from([4, 8]))
        .style(close_window_button_style(*palette))
        .on_press(NavbarMessage::CloseWindow);

    let right_zone = row![drag_spacer(), settings_btn, min_btn, max_btn, close_btn]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(Length::FillPortion(1))
        .height(Length::Fill);

    let content = row![left_zone, center_zone, right_zone]
        .spacing(8)
        .padding(Padding::from([0, 8]))
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fixed(NAVBAR_HEIGHT));

    let surface = palette.surface;
    let border_color = palette.border;
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(NAVBAR_HEIGHT))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            border: Border {
                color: border_color,
                width: 0.5,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .boxed()
}
