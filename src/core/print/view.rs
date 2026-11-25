//! Print Preview modal UI with paper/margin/orientation/theme controls,
//! scrollable paginated vector SVG sheet preview, and Export PDF confirmation footer.

use iced::widget::{
    button, column, container, pick_list, row, scrollable, svg, text, Space,
};
use iced::{Alignment, Border, Color, ContentFit, Element, Length, Padding, Shadow, Vector, Widget};

use crate::core::icons;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{ghost_icon_style, subtle_button_style};
use crate::core::widgets::styles::{rounded_menu_style, rounded_pick_list_style};

use super::models::{
    MarginPreset, PageOrientation, PaperSize, PrintMessage, PrintPreviewState, PrintThemeMode,
};

pub fn view_print_preview_modal<'a>(
    state: &'a PrintPreviewState,
    palette: &'a ThemePalette,
    modal_width: f32,
) -> Element<'a, PrintMessage> {
    let p = *palette;

    // 1. Header bar
    let page_badge = if state.is_compiling {
        "Rendering preview...".to_string()
    } else if state.page_handles.is_empty() {
        "No pages".to_string()
    } else if state.page_handles.len() == 1 {
        "1 page".to_string()
    } else {
        format!("{} pages", state.page_handles.len())
    };

    let header = row![
        icons::printer().size(18).color(p.accent_text),
        text("Print Preview").size(18).color(p.text),
        container(text(page_badge).size(11).color(p.accent_text))
            .padding(Padding::from([2, 8]))
            .style(move |_| container::Style {
                background: Some(p.accent.into()),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 999.0.into(),
                },
                ..Default::default()
            }),
        Space::new().width(Length::Fill),
        button(icons::x().size(15))
            .padding(Padding::from([4, 6]))
            .style(ghost_icon_style(p, false))
            .on_press(PrintMessage::Close),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    // 2. Compact print options bar
    let paper_picker = pick_list(
        Some(state.settings.paper),
        PaperSize::ALL,
        |v| v.to_string(),
    )
    .on_select(PrintMessage::SetPaper)
    .text_size(12)
    .padding(Padding::from([4, 8]))
    .style(rounded_pick_list_style(p))
    .menu_style(rounded_menu_style(p));

    let orient_picker = pick_list(
        Some(state.settings.orientation),
        PageOrientation::ALL,
        |v| v.to_string(),
    )
    .on_select(PrintMessage::SetOrientation)
    .text_size(12)
    .padding(Padding::from([4, 8]))
    .style(rounded_pick_list_style(p))
    .menu_style(rounded_menu_style(p));

    let margin_picker = pick_list(
        Some(state.settings.margin),
        MarginPreset::ALL,
        |v| v.to_string(),
    )
    .on_select(PrintMessage::SetMargin)
    .text_size(12)
    .padding(Padding::from([4, 8]))
    .style(rounded_pick_list_style(p))
    .menu_style(rounded_menu_style(p));

    let theme_picker = pick_list(
        Some(state.settings.theme_mode),
        PrintThemeMode::ALL,
        |v| v.to_string(),
    )
    .on_select(PrintMessage::SetThemeMode)
    .text_size(12)
    .padding(Padding::from([4, 8]))
    .style(rounded_pick_list_style(p))
    .menu_style(rounded_menu_style(p));

    let page_num_btn = button(
        row![
            if state.settings.show_page_numbers {
                icons::check().size(12)
            } else {
                icons::hash().size(12)
            },
            text("Page #").size(12),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([4, 9]))
    .style(subtle_button_style(p, state.settings.show_page_numbers))
    .on_press(PrintMessage::TogglePageNumbers);

    let controls_bar = container(
        row![
            row![text("Paper").size(11).color(p.text_muted), paper_picker]
                .spacing(5)
                .align_y(Alignment::Center),
            row![text("Layout").size(11).color(p.text_muted), orient_picker]
                .spacing(5)
                .align_y(Alignment::Center),
            row![text("Margins").size(11).color(p.text_muted), margin_picker]
                .spacing(5)
                .align_y(Alignment::Center),
            row![text("Style").size(11).color(p.text_muted), theme_picker]
                .spacing(5)
                .align_y(Alignment::Center),
            Space::new().width(Length::Fill),
            page_num_btn,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(Padding::from([8, 12]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(p.background.into()),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    });

    // 3. Preview viewport (scrollable stack of paper sheets)
    let preview_area: Element<'a, PrintMessage> = if let Some(err) = &state.error {
        container(
            column![
                icons::alert_triangle().size(24).color(p.accent_text),
                text("Could not render print preview").size(14).color(p.text),
                text(err).size(12).color(p.text_muted),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(24)
        .boxed()
    } else if state.is_compiling && state.page_handles.is_empty() {
        container(
            column![
                icons::printer().size(24).color(p.text_muted),
                text("Preparing print preview...").size(13).color(p.text_muted),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
    } else {
        let aspect = state.settings.page_aspect_ratio();
        let max_sheet_w = (modal_width - 140.0).clamp(360.0, 620.0);
        let sheet_w = if matches!(state.settings.orientation, PageOrientation::Landscape) {
            max_sheet_w
        } else {
            max_sheet_w.min(540.0)
        };
        let sheet_h = (sheet_w * aspect).round();

        let total = state.page_handles.len();
        let mut pages_col = column![]
            .spacing(18)
            .padding(Padding {
                top: 16.0,
                right: 20.0,
                bottom: 20.0,
                left: 16.0,
            })
            .align_x(Alignment::Center)
            .width(Length::Fill);

        for (idx, handle) in state.page_handles.iter().enumerate() {
            let sheet_svg = svg(handle.clone())
                .width(Length::Fixed(sheet_w))
                .height(Length::Fixed(sheet_h))
                .content_fit(ContentFit::Contain);

            let paper_card = container(sheet_svg)
                .width(Length::Fixed(sheet_w))
                .height(Length::Fixed(sheet_h))
                .style(move |_| container::Style {
                    border: Border {
                        color: p.border,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Shadow {
                        color: Color {
                            a: 0.16,
                            ..p.text
                        },
                        offset: Vector::new(0.0, 4.0),
                        blur_radius: 14.0,
                    },
                    ..Default::default()
                });

            let page_caption = text(format!("Page {} of {total}", idx + 1))
                .size(11)
                .color(p.text_muted);

            pages_col = pages_col.push(
                column![paper_card, page_caption]
                    .spacing(6)
                    .align_x(Alignment::Center),
            );
        }

        scrollable(pages_col)
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    };

    let preview_well = container(preview_area)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(
                Color {
                    a: 0.55,
                    ..p.surface_hover
                }
                .into(),
            ),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

    // 4. Footer with Cancel and Confirm ("Export PDF...")
    let can_export = !state.is_compiling && !state.is_exporting && state.pdf_bytes.is_some();
    let confirm_label = if state.is_exporting {
        "Saving PDF..."
    } else {
        "Export PDF..."
    };
    let mut confirm_btn = button(
        row![
            icons::printer().size(13),
            text(confirm_label).size(13),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([7, 14]))
    .style(subtle_button_style(p, true));

    if can_export {
        confirm_btn = confirm_btn.on_press(PrintMessage::ConfirmExportPdf);
    }

    let cancel_btn = button(text("Cancel").size(13))
        .padding(Padding::from([7, 14]))
        .style(subtle_button_style(p, false))
        .on_press(PrintMessage::Close);

    let footer = row![
        text(&state.doc_title).size(12).color(p.text_muted),
        Space::new().width(Length::Fill),
        cancel_btn,
        confirm_btn,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    column![header, controls_bar, preview_well, footer]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
