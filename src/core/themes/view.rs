use iced::Widget;
use iced::widget::{button, column, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::core::widgets::button::subtle_button_style;
use crate::core::widgets::styles::rounded_text_input_style;

use super::models::{CustomColorField, ThemeConfig, ThemeDefinition, ThemeMessage, ThemePalette};

pub fn view_quick_popover<'a>(
    config: &'a ThemeConfig,
    palette: &'a ThemePalette,
) -> Element<'a, ThemeMessage> {
    let mut presets_col = column![].spacing(5);
    for (id, def) in config.themes.iter() {
        let is_selected = config.active_theme == *id;
        presets_col = presets_col.push(theme_button(id, def, is_selected, palette, Length::Fill));
    }

    column![
        text("Themes").size(14).color(palette.text),
        presets_col,
    ]
    .spacing(10)
    .width(Length::Fixed(236.0))
    .boxed()
}

pub fn view_modal_section<'a>(
    config: &'a ThemeConfig,
    palette: &'a ThemePalette,
) -> Element<'a, ThemeMessage> {
    let p = *palette;
    let entries: Vec<(&String, &ThemeDefinition)> = config.themes.iter().collect();
    let mut grid = column![].spacing(8);
    for chunk in entries.chunks(3) {
        let mut r = row![].spacing(8);
        for (id, def) in chunk {
            let is_selected = config.active_theme == **id;
            r = r.push(theme_button(id, def, is_selected, palette, Length::Fill));
        }
        grid = grid.push(r);
    }

    let active_def = config
        .themes
        .get(&config.active_theme)
        .or_else(|| config.themes.values().next());

    let icon_val = active_def.map(|d| d.theme_icon.as_str()).unwrap_or("🌿");
    let bg_val = active_def.map(|d| d.background.as_str()).unwrap_or("");
    let surface_val = active_def.map(|d| d.surface.as_str()).unwrap_or("");
    let text_val = active_def.map(|d| d.text.as_str()).unwrap_or("");
    let accent_val = active_def.map(|d| d.accent.as_str()).unwrap_or("");

    let custom_fields = column![
        row![
            text("Theme Emoji Icon")
                .size(13)
                .color(palette.text)
                .width(Length::Fixed(128.0)),
            text_input("🌿", icon_val)
                .size(13)
                .padding(Padding::from([6, 10]))
                .width(Length::Fixed(140.0))
                .style(rounded_text_input_style(p))
                .on_input(ThemeMessage::SetThemeIcon),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        hex_override_row("Background", bg_val, CustomColorField::Background, palette),
        hex_override_row("Surface", surface_val, CustomColorField::Surface, palette),
        hex_override_row("Text", text_val, CustomColorField::Text, palette),
        hex_override_row("Accent", accent_val, CustomColorField::Accent, palette),
    ]
    .spacing(8);

    column![
        text("Themes").size(18).color(palette.text),
        text("Color Palette").size(13).color(palette.text_muted),
        grid,
        Space::new().height(8),
        text("Active Theme Customization")
            .size(13)
            .color(palette.text_muted),
        custom_fields,
        button(text("Reset Active Theme").size(13))
            .padding([6, 12])
            .style(subtle_button_style(p, false))
            .on_press(ThemeMessage::ResetActiveTheme),
    ]
    .spacing(10)
    .boxed()
}

fn theme_button<'a>(
    id: &'a str,
    def: &'a ThemeDefinition,
    is_selected: bool,
    palette: &'a ThemePalette,
    width: Length,
) -> Element<'a, ThemeMessage> {
    let content = row![
        text(&def.theme_icon)
            .size(14)
            .shaping(iced::widget::text::Shaping::Advanced),
        text(&def.name).size(13),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    button(content)
        .width(width)
        .padding([7, 11])
        .style(subtle_button_style(*palette, is_selected))
        .on_press(ThemeMessage::SelectTheme(id.to_string()))
        .boxed()
}

fn hex_override_row<'a>(
    label: &'a str,
    value: &'a str,
    field: CustomColorField,
    palette: &'a ThemePalette,
) -> Element<'a, ThemeMessage> {
    let p = *palette;
    row![
        text(label)
            .size(13)
            .color(palette.text)
            .width(Length::Fixed(128.0)),
        text_input("#RRGGBB", value)
            .size(13)
            .padding(Padding::from([6, 10]))
            .width(Length::Fixed(140.0))
            .style(rounded_text_input_style(p))
            .on_input(move |v| ThemeMessage::SetCustomHex { field, value: v }),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .boxed()
}
