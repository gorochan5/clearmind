//! Interactive GFM table grid with thin hairline borders, live in-place cell editing,
//! floating toolbar controls (insert/move/delete rows & columns, column alignment, pipe formatting),
//! and right/bottom `+` quick-add edges.
use iced::Widget;
use iced::advanced::text::Span;
use iced::widget::{button, column, container, mouse_area, rich_text, row, text, text_input, Space};
use iced::{Alignment, Border, Color, Element, Font, Length, Padding, never};

use crate::core::editor::live_widget::parse_inline_spans;
use crate::core::editor::update::{EditorMessage, TableOp};
use crate::core::icons;
use crate::core::markdown::table::TableAlignment;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::{ghost_icon_style, subtle_button_style};
use crate::core::widgets::styles::rounded_text_input_style;

pub const ACTIVE_TABLE_CELL_ID: &str = "clearmind_active_table_cell";

fn render_cell_rich<'a>(
    raw: &str,
    base_font: Font,
    code_font: Font,
    font_size: f32,
    palette: &ThemePalette,
    align: TableAlignment,
) -> Element<'a, EditorMessage> {
    let code_bg = Color {
        a: 0.85,
        ..palette.surface_hover
    };
    let (raw_spans, _, _, _) = parse_inline_spans(
        raw,
        base_font,
        code_font,
        font_size,
        palette.text,
        palette.accent_text,
        code_bg,
        false,
    );
    let spans: Vec<Span<'static, iced::Never>> = if raw_spans.is_empty() {
        vec![Span::new(" ").font(base_font).size(font_size)]
    } else {
        raw_spans
            .into_iter()
            .map(|s| {
                let mut ns = Span::new(s.text)
                    .size(s.size.unwrap_or(iced::Pixels(font_size)))
                    .color(s.color.unwrap_or(palette.text))
                    .underline(s.underline)
                    .strikethrough(s.strikethrough)
                    .padding(s.padding);
                if let Some(f) = s.font {
                    ns = ns.font(f);
                }
                if let Some(h) = s.highlight {
                    ns = ns.background(h.background).border(h.border);
                }
                ns
            })
            .collect()
    };

    let h_align = match align {
        TableAlignment::Center => iced::alignment::Horizontal::Center,
        TableAlignment::Right => iced::alignment::Horizontal::Right,
        TableAlignment::Left | TableAlignment::None => iced::alignment::Horizontal::Left,
    };

    rich_text(spans)
        .on_link_click(never)
        .font(base_font)
        .size(font_size)
        .align_x(h_align)
        .width(Length::Fill)
        .boxed()
}

fn hairline_v<'a>(border_color: Color) -> Element<'a, EditorMessage> {
    container(Space::new().width(1).height(Length::Fill))
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(border_color.into()),
            ..Default::default()
        })
        .boxed()
}

fn hairline_h<'a>(border_color: Color) -> Element<'a, EditorMessage> {
    container(Space::new().width(Length::Fill).height(1))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(border_color.into()),
            ..Default::default()
        })
        .boxed()
}

fn toolbar_divider<'a>(border_color: Color) -> Element<'a, EditorMessage> {
    container(Space::new().width(1).height(16))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(16.0))
        .style(move |_| container::Style {
            background: Some(border_color.into()),
            ..Default::default()
        })
        .boxed()
}

#[allow(clippy::too_many_arguments)]
pub fn view_table_grid<'a>(
    start_line: usize,
    end_line: usize,
    headers: &[String],
    alignments: &[TableAlignment],
    rows: &[Vec<String>],
    is_editing: bool,
    active_row_offset: Option<usize>,
    active_col: usize,
    body_font: Font,
    code_font: Font,
    font_size: f32,
    palette: &ThemePalette,
) -> Element<'a, EditorMessage> {
    let p = *palette;
    let cell_size = (font_size * 0.88).clamp(11.5, 16.0);
    let header_font = Font {
        weight: iced::font::Weight::Semibold,
        ..body_font
    };
    let clamped_col = active_col.min(headers.len().saturating_sub(1));
    let active_row = active_row_offset.unwrap_or(0);
    let is_data_row = active_row >= 2;
    let can_move_row_up = active_row >= 3;
    let can_move_row_down = is_data_row && (active_row - 2 + 1) < rows.len();
    let can_delete_col = headers.len() > 1;
    let can_move_col_left = clamped_col > 0;
    let can_move_col_right = clamped_col + 1 < headers.len();
    let cur_align = alignments.get(clamped_col).copied().unwrap_or_default();

    let mut table_rows: Vec<Element<'a, EditorMessage>> = Vec::with_capacity(rows.len() * 2 + 3);

    // 1. Header row
    let mut header_cells: Vec<Element<'a, EditorMessage>> = Vec::with_capacity(headers.len() * 2);
    for (col_idx, h) in headers.iter().enumerate() {
        if col_idx > 0 {
            header_cells.push(hairline_v(p.border));
        }
        let align = alignments.get(col_idx).copied().unwrap_or_default();
        let is_selected_cell = is_editing && active_row_offset == Some(0) && col_idx == clamped_col;
        let is_selected_col = is_editing && col_idx == clamped_col;

        let cell_content: Element<'a, EditorMessage> = if is_selected_cell {
            text_input("Header", h.clone())
                .id(iced::widget::Id::new(ACTIVE_TABLE_CELL_ID))
                .size(cell_size)
                .font(header_font)
                .padding(Padding::from([4, 6]))
                .width(Length::Fill)
                .style(rounded_text_input_style(p))
                .on_input(move |v| EditorMessage::TableEditCell {
                    start_line,
                    end_line,
                    row_offset: 0,
                    col: col_idx,
                    value: v,
                })
                .boxed()
        } else {
            mouse_area(
                container(render_cell_rich(
                    h,
                    header_font,
                    code_font,
                    cell_size,
                    palette,
                    align,
                ))
                .padding(Padding::from([8, 12]))
                .width(Length::Fill),
            )
            .interaction(iced::mouse::Interaction::Text)
            .on_press(EditorMessage::SelectTableCell {
                table_start: start_line,
                row_offset: 0,
                col: col_idx,
            })
            .boxed()
        };

        let header_bg = if is_selected_col {
            p.surface_hover
        } else {
            p.surface
        };
        header_cells.push(
            container(cell_content)
                .padding(if is_selected_cell {
                    Padding::from([4, 6])
                } else {
                    Padding::ZERO
                })
                .width(Length::Fill)
                .height(Length::Shrink)
                .style(move |_| container::Style {
                    background: Some(header_bg.into()),
                    ..Default::default()
                })
                .boxed(),
        );
    }

    table_rows.push(
        container(
            row![]
                .extend(header_cells)
                .width(Length::Fill)
                .height(Length::Shrink),
        )
        .width(Length::Fill)
        .height(Length::Shrink)
        .boxed(),
    );

    // 2. Data rows
    for (row_idx, r) in rows.iter().enumerate() {
        table_rows.push(hairline_h(p.border));
        let row_offset = row_idx + 2;
        let is_selected_row = is_editing && active_row_offset == Some(row_offset);

        let mut cells: Vec<Element<'a, EditorMessage>> = Vec::with_capacity(r.len() * 2);
        for (col_idx, c) in r.iter().enumerate() {
            if col_idx > 0 {
                cells.push(hairline_v(p.border));
            }
            let align = alignments.get(col_idx).copied().unwrap_or_default();
            let is_selected_cell = is_selected_row && col_idx == clamped_col;

            let cell_content: Element<'a, EditorMessage> = if is_selected_cell {
                text_input("", c.clone())
                    .id(iced::widget::Id::new(ACTIVE_TABLE_CELL_ID))
                    .size(cell_size)
                    .font(body_font)
                    .padding(Padding::from([4, 6]))
                    .width(Length::Fill)
                    .style(rounded_text_input_style(p))
                    .on_input(move |v| EditorMessage::TableEditCell {
                        start_line,
                        end_line,
                        row_offset,
                        col: col_idx,
                        value: v,
                    })
                    .boxed()
            } else {
                mouse_area(
                    container(render_cell_rich(
                        c,
                        body_font,
                        code_font,
                        cell_size,
                        palette,
                        align,
                    ))
                    .padding(Padding::from([7, 12]))
                    .width(Length::Fill),
                )
                .interaction(iced::mouse::Interaction::Text)
                .on_press(EditorMessage::SelectTableCell {
                    table_start: start_line,
                    row_offset,
                    col: col_idx,
                })
                .boxed()
            };

            let cell_bg = if is_selected_cell {
                Color {
                    a: 0.22,
                    ..p.accent
                }
            } else if is_selected_row {
                Color {
                    a: 0.10,
                    ..p.accent
                }
            } else if row_idx % 2 == 1 {
                Color {
                    a: 0.45,
                    ..p.surface
                }
            } else {
                p.background
            };

            cells.push(
                container(cell_content)
                    .padding(if is_selected_cell {
                        Padding::from([3, 6])
                    } else {
                        Padding::ZERO
                    })
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .style(move |_| container::Style {
                        background: Some(cell_bg.into()),
                        ..Default::default()
                    })
                    .boxed(),
            );
        }

        table_rows.push(
            container(
                row![]
                    .extend(cells)
                    .width(Length::Fill)
                    .height(Length::Shrink),
            )
            .width(Length::Fill)
            .height(Length::Shrink)
            .boxed(),
        );
    }

    let table_box = container(
        column![]
            .extend(table_rows)
            .width(Length::Fill)
            .height(Length::Shrink),
    )
    .width(Length::Fill)
    .height(Length::Shrink)
    .clip(true)
    .style(move |_| container::Style {
        background: Some(p.background.into()),
        border: Border {
            color: if is_editing { p.accent_text } else { p.border },
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    });

    if !is_editing {
        return table_box.boxed();
    }

    // 3. Interactive Table Controls (Editing mode only — §6.9 item 5)
    let row_group = row![
        text("Row").size(11).color(p.text_muted),
        button(
            row![icons::plus().size(11), text("Above").size(11)]
                .spacing(3)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([3, 7]))
        .style(subtle_button_style(p, false))
        .on_press(EditorMessage::TableAction {
            start_line,
            end_line,
            op: TableOp::InsertRowAbove(active_row),
        }),
        button(
            row![icons::plus().size(11), text("Below").size(11)]
                .spacing(3)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([3, 7]))
        .style(subtle_button_style(p, false))
        .on_press(EditorMessage::TableAction {
            start_line,
            end_line,
            op: TableOp::InsertRowBelow(active_row),
        }),
        {
            let mut b = button(icons::arrow_up().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if can_move_row_up {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::MoveRowUp(active_row),
                });
            }
            b
        },
        {
            let mut b = button(icons::arrow_down().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if can_move_row_down {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::MoveRowDown(active_row),
                });
            }
            b
        },
        {
            let mut b = button(icons::trash().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if is_data_row {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::DeleteRow(active_row),
                });
            }
            b
        },
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let col_group = row![
        text("Col").size(11).color(p.text_muted),
        button(
            row![icons::plus().size(11), text("Left").size(11)]
                .spacing(3)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([3, 7]))
        .style(subtle_button_style(p, false))
        .on_press(EditorMessage::TableAction {
            start_line,
            end_line,
            op: TableOp::InsertColLeft(clamped_col),
        }),
        button(
            row![icons::plus().size(11), text("Right").size(11)]
                .spacing(3)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([3, 7]))
        .style(subtle_button_style(p, false))
        .on_press(EditorMessage::TableAction {
            start_line,
            end_line,
            op: TableOp::InsertColRight(clamped_col),
        }),
        {
            let mut b = button(icons::arrow_left().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if can_move_col_left {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::MoveColLeft(clamped_col),
                });
            }
            b
        },
        {
            let mut b = button(icons::arrow_right().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if can_move_col_right {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::MoveColRight(clamped_col),
                });
            }
            b
        },
        {
            let mut b = button(icons::trash().size(12))
                .padding(Padding::from([3, 5]))
                .style(ghost_icon_style(p, false));
            if can_delete_col {
                b = b.on_press(EditorMessage::TableAction {
                    start_line,
                    end_line,
                    op: TableOp::DeleteCol(clamped_col),
                });
            }
            b
        },
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let align_group = row![
        button(icons::align_left().size(12))
            .padding(Padding::from([3, 5]))
            .style(ghost_icon_style(
                p,
                matches!(cur_align, TableAlignment::Left | TableAlignment::None)
            ))
            .on_press(EditorMessage::TableAction {
                start_line,
                end_line,
                op: TableOp::SetColAlign(clamped_col, TableAlignment::Left),
            }),
        button(icons::align_center().size(12))
            .padding(Padding::from([3, 5]))
            .style(ghost_icon_style(p, cur_align == TableAlignment::Center))
            .on_press(EditorMessage::TableAction {
                start_line,
                end_line,
                op: TableOp::SetColAlign(clamped_col, TableAlignment::Center),
            }),
        button(icons::align_right().size(12))
            .padding(Padding::from([3, 5]))
            .style(ghost_icon_style(p, cur_align == TableAlignment::Right))
            .on_press(EditorMessage::TableAction {
                start_line,
                end_line,
                op: TableOp::SetColAlign(clamped_col, TableAlignment::Right),
            }),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let delete_table_btn = button(
        row![icons::trash().size(12), text("Delete").size(11)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([3, 8]))
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(
                if hovered {
                    Color {
                        a: 0.14,
                        ..p.danger
                    }
                } else {
                    p.background
                }
                .into(),
            ),
            text_color: if hovered { p.danger } else { p.text_muted },
            border: Border {
                color: if hovered { p.danger } else { p.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    })
    .on_press(EditorMessage::TableAction {
        start_line,
        end_line,
        op: TableOp::DeleteTable,
    });

    let done_btn = button(
        row![icons::check().size(12), text("Done").size(11)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([3, 8]))
    .style(subtle_button_style(p, true))
    .on_press(EditorMessage::ExitTableEdit);

    let toolbar = container(
        row![
            row_group,
            toolbar_divider(p.border),
            col_group,
            toolbar_divider(p.border),
            align_group,
            iced::widget::space::horizontal(),
            delete_table_btn,
            done_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(Padding::from([6, 10]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(p.surface.into()),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    });

    // Right-edge `+` button to append a column
    let add_col_edge = button(icons::plus().size(13))
        .width(Length::Fixed(24.0))
        .height(Length::Fill)
        .padding(Padding::from([4, 4]))
        .style(subtle_button_style(p, false))
        .on_press(EditorMessage::TableAction {
            start_line,
            end_line,
            op: TableOp::InsertColRight(headers.len().saturating_sub(1)),
        });

    // Bottom-edge `+` button to append a row
    let add_row_edge = button(
        row![icons::plus().size(12), text("Add Row").size(11)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::from([4, 10]))
    .style(subtle_button_style(p, false))
    .on_press(EditorMessage::TableAction {
        start_line,
        end_line,
        op: TableOp::InsertRowBelow(rows.len() + 1),
    });

    let grid_with_right_plus = row![table_box, add_col_edge]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Shrink);

    column![toolbar, grid_with_right_plus, add_row_edge]
        .spacing(6)
        .width(Length::Fill)
        .height(Length::Shrink)
        .boxed()
}
