//! Editor view rendering, Focus/Typewriter modes, checklists, code blocks, and status bar.
use std::borrow::Cow;

use iced::Widget;
use iced::advanced::text::Span;
use iced::widget::{
    button, checkbox, column, container, image, mouse_area, rich_text, row, scrollable, svg, text,
};
use iced::{Border, Code, Color, ContentFit, Element, Font, Length, Padding, never};

use super::live_widget::{DisplayMap, LiveMarkdownEditor, parse_inline_spans};
use super::models::{ConflictState, SearchState, Tab};
use super::update::EditorMessage;
use super::widgets::code_block::view_code_block;
use super::widgets::conflict_banner::view_conflict_banner;
use super::widgets::table_grid::view_table_grid;
use crate::core::markdown::block_scanner::{ScannedBlock, scan_blocks};
use crate::core::markdown::image::parse_image_in_line;
use crate::core::themes::ThemePalette;
use crate::core::widgets::button::ghost_icon_style;

pub const EDITOR_SCROLLABLE_ID: &str = "clearmind_editor_scroll";

#[derive(Debug, Clone, Copy)]
pub struct EditorViewConfig {
    pub full_width: bool,
    pub content_max_width: f32,
    pub word_wrap: bool,
    pub font_size: f32,
    pub code_size: f32,
    pub line_height: f32,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
    pub word_target: Option<usize>,
    pub viewport_height: f32,
    pub body_font: iced::Font,
    pub code_font: iced::Font,
    pub slash_menu_open: bool,
}

impl Default for EditorViewConfig {
    fn default() -> Self {
        Self {
            full_width: false,
            content_max_width: 760.0,
            word_wrap: true,
            font_size: 16.0,
            code_size: 14.0,
            line_height: 1.6,
            focus_mode: false,
            typewriter_mode: false,
            word_target: None,
            viewport_height: 700.0,
            body_font: iced::Font::DEFAULT,
            code_font: iced::Font::MONOSPACE,
            slash_menu_open: false,
        }
    }
}

/// Parses a Markdown task/checklist line (`- [ ] `, `- [x] `, `-[ ] `, `-[x] `, `* [ ] `, `+ [ ] `, etc.)
/// and returns `(indent_spaces, is_checked, prefix_char_count)`.
pub fn parse_checklist_line(raw_trimmed: &str) -> Option<(usize, bool, usize)> {
    let indent_spaces = raw_trimmed.chars().take_while(|&c| c == ' ').count();
    let after_indent = &raw_trimmed[indent_spaces..];
    let patterns: &[(&str, bool)] = &[
        ("- [ ]", false),
        ("- [x]", true),
        ("- [X]", true),
        ("* [ ]", false),
        ("* [x]", true),
        ("* [X]", true),
        ("+ [ ]", false),
        ("+ [x]", true),
        ("+ [X]", true),
        ("-[ ]", false),
        ("-[x]", true),
        ("-[X]", true),
    ];
    for &(pat, is_checked) in patterns {
        if let Some(rest) = after_indent.strip_prefix(pat) {
            if rest.is_empty() {
                return Some((indent_spaces, is_checked, indent_spaces + pat.chars().count()));
            } else if rest.starts_with(' ') {
                return Some((
                    indent_spaces,
                    is_checked,
                    indent_spaces + pat.chars().count() + 1,
                ));
            }
        }
    }
    None
}

pub fn view_editor<'a>(
    tab: &'a Tab,
    search: &'a SearchState,
    cfg: EditorViewConfig,
    palette: &ThemePalette,
) -> Element<'a, EditorMessage> {
    let bg = palette.background;
    let surface = palette.surface;
    let border = palette.border;
    let fg = palette.text;
    let muted = palette.text_muted;
    let accent = palette.accent;
    let selection_bg = palette.selection;
    let placeholder_color = Color { a: 0.55, ..muted };
    let code_bg = Color { a: 0.85, ..surface };
    let wrapping = if cfg.word_wrap {
        iced::advanced::text::Wrapping::WordOrGlyph
    } else {
        iced::advanced::text::Wrapping::None
    };

    let mut top_stack: Vec<Element<'a, EditorMessage>> = Vec::new();

    if let ConflictState::ExternalModified { .. } = tab.conflict {
        top_stack.push(view_conflict_banner(
            "File modified externally on disk.",
            surface,
            accent,
            fg,
            EditorMessage::ReloadFromDisk,
            EditorMessage::KeepLocalChanges,
        ));
    }

    let _ = search;

    let row_h = (cfg.font_size * cfg.line_height * tab.zoom).max(20.0);
    let total_lines = tab.rope.len_lines().max(1);

    let raw_lines_owned: Vec<String> = (0..total_lines)
        .map(|i| tab.rope.line(i).to_string())
        .collect();
    let raw_lines: Vec<&str> = raw_lines_owned.iter().map(|s| s.as_str()).collect();
    let scan = scan_blocks(&raw_lines);

    let mut lines_col: Vec<Element<'a, EditorMessage>> = Vec::with_capacity(total_lines + 2);

    let mut line_idx = 0usize;
    while line_idx < total_lines {
        let active = tab.active_line;
        let block_at_line = scan.blocks.iter().find(|b| match b {
            ScannedBlock::Frontmatter {
                start_line,
                end_line,
                ..
            }
            | ScannedBlock::CodeBlock {
                start_line,
                end_line,
                ..
            }
            | ScannedBlock::Mermaid {
                start_line,
                end_line,
                ..
            }
            | ScannedBlock::DisplayMath {
                start_line,
                end_line,
                ..
            }
            | ScannedBlock::Paragraph {
                start_line,
                end_line,
            } => *start_line == line_idx && active.is_none_or(|a| a < *start_line || a > *end_line),
            ScannedBlock::Table { start_line, .. } => *start_line == line_idx,
            ScannedBlock::Callout(c) => {
                c.alert.is_some()
                    && c.start_line == line_idx
                    && active.is_none_or(|a| a < c.start_line || a > c.end_line)
            }
            ScannedBlock::BadgeBlock { line, .. }
            | ScannedBlock::ThematicBreak { line } => *line == line_idx && active != Some(*line),
            _ => false,
        });

        if let Some(block) = block_at_line {
            match block {
                ScannedBlock::Frontmatter {
                    start_line,
                    end_line,
                    raw,
                } => {
                    lines_col.push(
                        mouse_area(
                            container(text(raw.clone()).size(12).color(muted))
                                .padding(Padding::from([6, 10]))
                                .width(Length::Fill)
                                .clip(!cfg.word_wrap),
                        )
                        .interaction(iced::mouse::Interaction::Text)
                        .on_press(EditorMessage::JumpToLine(*start_line))
                        .boxed(),
                    );
                    line_idx = end_line + 1;
                    continue;
                }
                ScannedBlock::CodeBlock {
                    start_line,
                    end_line,
                    language,
                    unclosed,
                    ..
                } => {
                    let inner_lines = if *end_line > *start_line + 1 && !*unclosed {
                        &raw_lines[*start_line + 1..*end_line]
                    } else if *end_line > *start_line && *unclosed {
                        &raw_lines[*start_line + 1..=*end_line]
                    } else {
                        &[][..]
                    };
                    let body_text = inner_lines
                        .iter()
                        .map(|l| l.trim_end_matches(['\r', '\n']))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let line_count = inner_lines.len().max(1);
                    let lang = language.as_deref().unwrap_or("");
                    let code_font_size =
                        (if cfg.code_size > 0.0 { cfg.code_size } else { 13.0 }) * tab.zoom;
                    let jump_line = (*start_line + 1).min(*end_line);
                    let highlighted_spans = highlight_code_spans(
                        inner_lines,
                        lang,
                        cfg.code_font,
                        code_font_size,
                        palette,
                    );
                    let code_body = mouse_area(
                        container(
                            rich_text(highlighted_spans)
                                .on_link_click(never)
                                .font(cfg.code_font)
                                .size(code_font_size)
                                .line_height(text::LineHeight::Relative(1.45))
                                .wrapping(wrapping)
                                .width(Length::Fill),
                        )
                        .width(Length::Fill)
                        .clip(!cfg.word_wrap),
                    )
                    .interaction(iced::mouse::Interaction::Text)
                    .on_press(EditorMessage::JumpToLine(jump_line))
                    .boxed();

                    lines_col.push(view_code_block(
                        lang,
                        true,
                        line_count,
                        cfg.code_font,
                        code_font_size,
                        code_body,
                        palette,
                        EditorMessage::CopyText(body_text),
                        None,
                    ));
                    line_idx = end_line + 1;
                    continue;
                }
                ScannedBlock::Mermaid {
                    start_line,
                    end_line,
                    source,
                    ..
                } => {
                    let src = source.clone();
                    let jump_line = (*start_line + 1).min(*end_line);
                    let border_c = border;
                    let surf = surface;
                    lines_col.push(
                        mouse_area(
                            container(
                                column![
                                    row![
                                        text("Mermaid diagram").size(12).color(muted),
                                        iced::widget::space::horizontal(),
                                        button(text("Zoom").size(11).color(palette.accent_text))
                                            .padding(Padding::from([2, 8]))
                                            .style(ghost_icon_style(*palette, false))
                                            .on_press(EditorMessage::OpenZoomViewer(src)),
                                    ]
                                    .align_y(iced::Alignment::Center),
                                    text(source.clone())
                                        .font(cfg.code_font)
                                        .size((cfg.code_size * tab.zoom).max(11.0))
                                        .color(fg),
                                ]
                                .spacing(6),
                            )
                            .padding(Padding::from([10, 12]))
                            .width(Length::Fill)
                            .style(move |_| container::Style {
                                background: Some(surf.into()),
                                border: Border {
                                    color: border_c,
                                    width: 1.0,
                                    radius: 8.0.into(),
                                },
                                ..Default::default()
                            }),
                        )
                        .interaction(iced::mouse::Interaction::Text)
                        .on_press(EditorMessage::JumpToLine(jump_line))
                        .boxed(),
                    );
                    line_idx = end_line + 1;
                    continue;
                }
                ScannedBlock::DisplayMath {
                    start_line,
                    end_line,
                    latex,
                    ..
                } => {
                    let src = latex.clone();
                    let jump_line = (*start_line + 1).min(*end_line);
                    let border_c = border;
                    let surf = surface;
                    let fg_hex = format!(
                        "#{:02x}{:02x}{:02x}",
                        (fg.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                        (fg.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                        (fg.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                    );
                    let math_body: Element<'a, EditorMessage> = if let Ok(compiled) =
                        crate::core::markdown::math::compile_math_svg(&src, false, &fg_hex)
                    {
                        container(
                            svg(svg::Handle::from_memory(compiled.svg.into_bytes()))
                                .content_fit(ContentFit::ScaleDown)
                                .width(Length::Fill)
                                .height(Length::Fixed(64.0)),
                        )
                        .width(Length::Fill)
                        .center_x(Length::Fill)
                        .boxed()
                    } else if src.trim().is_empty() {
                        text("Click to enter LaTeX formula...")
                            .size(13)
                            .color(muted)
                            .boxed()
                    } else {
                        text(src.clone())
                            .font(cfg.code_font)
                            .size(13)
                            .color(fg)
                            .boxed()
                    };

                    lines_col.push(
                        mouse_area(
                            container(math_body)
                                .padding(Padding::from([10, 14]))
                                .width(Length::Fill)
                                .style(move |_| container::Style {
                                    background: Some(surf.into()),
                                    border: Border {
                                        color: border_c,
                                        width: 1.0,
                                        radius: 8.0.into(),
                                    },
                                    ..Default::default()
                                }),
                        )
                        .interaction(iced::mouse::Interaction::Text)
                        .on_press(EditorMessage::JumpToLine(jump_line))
                        .boxed(),
                    );
                    line_idx = end_line + 1;
                    continue;
                }
                ScannedBlock::Callout(c) => {
                    use crate::core::markdown::callout::AlertKind;
                    let alert = c.alert.unwrap_or(AlertKind::Note);
                    let alert_color = match alert {
                        AlertKind::Note | AlertKind::Important => palette.accent_text,
                        AlertKind::Tip => palette.success,
                        AlertKind::Warning => palette.warning,
                        AlertKind::Caution => palette.danger,
                    };
                    let body_font_size = cfg.font_size * tab.zoom;
                    let header_font = Font {
                        weight: iced::font::Weight::Bold,
                        ..cfg.body_font
                    };
                    let mut spans: Vec<Span<'static, iced::Never>> = Vec::new();
                    for (idx, line_str) in c.body_lines.iter().enumerate() {
                        if idx > 0 {
                            spans.push(Span::new("\n").font(cfg.body_font).size(body_font_size));
                        }
                        let (line_spans, _, _, _) = parse_inline_spans(
                            line_str,
                            cfg.body_font,
                            cfg.code_font,
                            body_font_size,
                            fg,
                            palette.accent_text,
                            code_bg,
                            false,
                        );
                        for s in line_spans {
                            let mut ns = Span::new(s.text)
                                .size(s.size.unwrap_or(iced::Pixels(body_font_size)))
                                .color(s.color.unwrap_or(fg))
                                .underline(s.underline)
                                .strikethrough(s.strikethrough)
                                .padding(s.padding);
                            if let Some(f) = s.font {
                                ns = ns.font(f);
                            }
                            if let Some(h) = s.highlight {
                                ns = ns.background(h.background).border(h.border);
                            }
                            spans.push(ns);
                        }
                    }
                    let jump_line = c.start_line;
                    let card = container(
                        column![
                            text(alert.label().to_uppercase())
                                .font(header_font)
                                .size(12)
                                .color(alert_color),
                            rich_text(spans)
                                .on_link_click(never)
                                .font(cfg.body_font)
                                .size(body_font_size)
                                .wrapping(wrapping)
                                .width(Length::Fill),
                        ]
                        .spacing(6)
                        .width(Length::Fill),
                    )
                    .padding(Padding::from([10, 14]))
                    .width(Length::Fill)
                    .clip(!cfg.word_wrap)
                    .style(move |_| container::Style {
                        background: Some(surface.into()),
                        border: Border {
                            color: alert_color,
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..container::Style::default()
                    });
                    lines_col.push(
                        mouse_area(card)
                            .interaction(iced::mouse::Interaction::Text)
                            .on_press(EditorMessage::JumpToLine(jump_line))
                            .boxed(),
                    );
                    line_idx = c.end_line + 1;
                    continue;
                }
                ScannedBlock::Table {
                    start_line,
                    end_line,
                    table,
                } => {
                    let is_editing =
                        active.is_some_and(|a| a >= *start_line && a <= *end_line);
                    let active_row_offset = if is_editing {
                        active.and_then(|a| a.checked_sub(*start_line))
                    } else {
                        None
                    };
                    lines_col.push(view_table_grid(
                        *start_line,
                        *end_line,
                        &table.headers,
                        &table.alignments,
                        &table.rows,
                        is_editing,
                        active_row_offset,
                        tab.active_table_col,
                        cfg.body_font,
                        cfg.code_font,
                        cfg.font_size * tab.zoom,
                        palette,
                    ));
                    line_idx = end_line + 1;
                    continue;
                }
                ScannedBlock::ThematicBreak { line } => {
                    let rule_line = *line;
                    let hr = container(
                        container(iced::widget::Space::new().width(Length::Fill).height(1.5))
                            .width(Length::Fill)
                            .height(Length::Fixed(1.5))
                            .style(move |_| container::Style {
                                background: Some(border.into()),
                                ..container::Style::default()
                            }),
                    )
                    .width(Length::Fill)
                    .padding(Padding::from([10, 0]));
                    lines_col.push(
                        mouse_area(hr)
                            .interaction(iced::mouse::Interaction::Text)
                            .on_press(EditorMessage::JumpToLine(rule_line))
                            .boxed(),
                    );
                    line_idx = line + 1;
                    continue;
                }
                ScannedBlock::BadgeBlock { line, badges } => {
                    let pills: Vec<Element<'a, EditorMessage>> = badges
                        .iter()
                        .map(|b| text(format!("[{}]", b.alt)).size(12).color(accent).boxed())
                        .collect();
                    lines_col.push(row![].extend(pills).spacing(6).boxed());
                    line_idx = line + 1;
                    continue;
                }
                ScannedBlock::Paragraph { .. } => {}
                _ => {}
            }
        }

        let line_slice = tab.rope.line(line_idx);
        let raw_line: Cow<'a, str> = match line_slice.as_str() {
            Some(s) => Cow::Borrowed(s),
            None => Cow::Owned(raw_lines_owned[line_idx].clone()),
        };
        let raw_trimmed = raw_line.trim_end_matches(['\r', '\n']);
        let is_active = tab.active_line == Some(line_idx);
        let dimmed = cfg.focus_mode && tab.active_line.is_some() && !is_active;
        let line_fg = if dimmed {
            Color {
                a: fg.a * 0.35,
                ..fg
            }
        } else {
            fg
        };
        let base_font_size = cfg.font_size * tab.zoom;

        // Inline / block Image (`![alt](path)`, `![[path]]`, `<img src="...">`) when line is not active
        if !is_active
            && (raw_trimmed.trim_start().starts_with("![")
                || raw_trimmed.trim_start().to_ascii_lowercase().starts_with("<img"))
            && let Some(img) = parse_image_in_line(raw_trimmed)
        {
            lines_col.push(render_inline_image(
                &img,
                tab.path.as_deref(),
                line_idx,
                palette,
            ));
            line_idx += 1;
            continue;
        }

        // Interactive Checklist Item (`- [ ] `, `- [x] `, `-[ ] `, `-[x] `) when line is not active
        if !is_active
            && let Some((indent_spaces, is_checked, prefix_chars)) =
                parse_checklist_line(raw_trimmed)
        {
            let body_raw = raw_trimmed
                .char_indices()
                .nth(prefix_chars)
                .map(|(b, _)| &raw_trimmed[b..])
                .unwrap_or("");
            let task_fg = if is_checked { muted } else { line_fg };
            let (spans, display_str, inner_map, links) = parse_inline_spans(
                body_raw,
                cfg.body_font,
                cfg.code_font,
                base_font_size,
                task_fg,
                accent,
                code_bg,
                is_checked,
            );
            let display_map = inner_map.with_stripped_prefix(prefix_chars);
            let editor = LiveMarkdownEditor::new(
                line_idx,
                raw_line,
                Cow::Owned(display_str),
                display_map,
                links,
                tab.active_line,
                tab.selection,
                &tab.extra_cursors,
                tab.goal_x,
                cfg.body_font,
                base_font_size,
                cfg.line_height,
                task_fg,
                placeholder_color,
                accent,
                selection_bg,
                EditorMessage::WidgetEvent,
            )
            .with_code_font(cfg.code_font)
            .with_code_bg(code_bg)
            .with_wrapping(wrapping)
            .with_strikethrough_all(is_checked)
            .with_spans(spans)
            .with_slash_menu_open(cfg.slash_menu_open);

            let mut task_items: Vec<Element<'a, EditorMessage>> = Vec::with_capacity(3);
            if indent_spaces > 0 {
                task_items.push(
                    iced::widget::Space::new()
                        .width(Length::Fixed(indent_spaces as f32 * (base_font_size * 0.45)))
                        .height(Length::Shrink)
                        .boxed(),
                );
            }
            let cb = checkbox(is_checked)
                .size(16)
                .on_toggle(move |b| EditorMessage::ToggleTask(line_idx, b))
                .boxed();
            task_items.push(cb);
            task_items.push(editor.into());
            let task_row = row![]
                .extend(task_items)
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill);

            let task_el: Element<'a, EditorMessage> = if cfg.word_wrap {
                task_row.boxed()
            } else {
                container(task_row).width(Length::Fill).clip(true).boxed()
            };
            lines_col.push(task_el);
            line_idx += 1;
            continue;
        }

        // Standard line (active raw line, heading, blockquote, bullet list, or rich paragraph)
        let (display_text, line_font_size, is_blockquote, display_map, spans, links, line_base_font) =
            if is_active {
                (
                    raw_line.clone(),
                    base_font_size,
                    false,
                    DisplayMap::identity(raw_trimmed.chars().count()),
                    Vec::new(),
                    Vec::new(),
                    cfg.body_font,
                )
            } else if let Some((rest, scale, prefix_len)) =
                raw_trimmed
                    .strip_prefix("# ")
                    .map(|r| (r, 1.75f32, 2usize))
                    .or_else(|| raw_trimmed.strip_prefix("## ").map(|r| (r, 1.45, 3)))
                    .or_else(|| raw_trimmed.strip_prefix("### ").map(|r| (r, 1.22, 4)))
                    .or_else(|| raw_trimmed.strip_prefix("#### ").map(|r| (r, 1.1, 5)))
                    .or_else(|| raw_trimmed.strip_prefix("##### ").map(|r| (r, 1.0, 6)))
                    .or_else(|| raw_trimmed.strip_prefix("###### ").map(|r| (r, 1.0, 7)))
            {
                let h_font = iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..cfg.body_font
                };
                let h_size = base_font_size * scale;
                let (spans, disp, map, links) = parse_inline_spans(
                    rest,
                    h_font,
                    cfg.code_font,
                    h_size,
                    line_fg,
                    accent,
                    code_bg,
                    false,
                );
                (
                    Cow::Owned(disp),
                    h_size,
                    false,
                    map.with_stripped_prefix(prefix_len),
                    spans,
                    links,
                    h_font,
                )
            } else if let Some((rest, prefix_len)) = raw_trimmed
                .strip_prefix("> ")
                .map(|r| (r, 2usize))
                .or_else(|| raw_trimmed.strip_prefix('>').map(|r| (r, 1usize)))
            {
                let (spans, disp, map, links) = parse_inline_spans(
                    rest,
                    cfg.body_font,
                    cfg.code_font,
                    base_font_size,
                    line_fg,
                    accent,
                    code_bg,
                    false,
                );
                (
                    Cow::Owned(disp),
                    base_font_size,
                    true,
                    map.with_stripped_prefix(prefix_len),
                    spans,
                    links,
                    cfg.body_font,
                )
            } else {
                let indent_len = raw_trimmed.len() - raw_trimmed.trim_start_matches(' ').len();
                let after_indent = &raw_trimmed[indent_len..];
                if let Some(rest) = after_indent
                    .strip_prefix("- ")
                    .or_else(|| after_indent.strip_prefix("* "))
                    .or_else(|| after_indent.strip_prefix("+ "))
                {
                    let bullet_prefix = format!("{}• ", &raw_trimmed[..indent_len]);
                    let prefix_chars = indent_len + 2;
                    let (mut spans, disp, map, mut links) = parse_inline_spans(
                        rest,
                        cfg.body_font,
                        cfg.code_font,
                        base_font_size,
                        line_fg,
                        accent,
                        code_bg,
                        false,
                    );
                    spans.insert(
                        0,
                        Span::new(bullet_prefix.clone())
                            .font(cfg.body_font)
                            .color(line_fg),
                    );
                    for link in &mut links {
                        link.start_col += prefix_chars;
                        link.end_col += prefix_chars;
                    }
                    (
                        Cow::Owned(format!("{bullet_prefix}{disp}")),
                        base_font_size,
                        false,
                        map.with_1to1_prefix(prefix_chars),
                        spans,
                        links,
                        cfg.body_font,
                    )
                } else {
                    let (spans, disp, map, links) = parse_inline_spans(
                        raw_trimmed,
                        cfg.body_font,
                        cfg.code_font,
                        base_font_size,
                        line_fg,
                        accent,
                        code_bg,
                        false,
                    );
                    (
                        Cow::Owned(disp),
                        base_font_size,
                        false,
                        map,
                        spans,
                        links,
                        cfg.body_font,
                    )
                }
            };

        let editor = LiveMarkdownEditor::new(
            line_idx,
            raw_line,
            display_text,
            display_map,
            links,
            tab.active_line,
            tab.selection,
            &tab.extra_cursors,
            tab.goal_x,
            line_base_font,
            line_font_size,
            cfg.line_height,
            line_fg,
            placeholder_color,
            accent,
            selection_bg,
            EditorMessage::WidgetEvent,
        )
        .with_code_font(cfg.code_font)
        .with_code_bg(code_bg)
        .with_wrapping(wrapping)
        .with_spans(spans)
        .with_slash_menu_open(cfg.slash_menu_open);

        let line_el: Element<'a, EditorMessage> = if is_active {
            container(editor)
                .width(Length::Fill)
                .padding(Padding::from([1, 6]))
                .clip(!cfg.word_wrap)
                .style(move |_| container::Style {
                    background: Some(Color { a: 0.14, ..accent }.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed()
        } else if is_blockquote {
            let bar_h = (line_font_size * cfg.line_height).max(20.0);
            let bq_row = row![
                container(iced::widget::Space::new().width(3).height(bar_h)).style(move |_| {
                    container::Style {
                        background: Some(accent.into()),
                        border: iced::Border {
                            radius: 2.0.into(),
                            ..iced::Border::default()
                        },
                        ..container::Style::default()
                    }
                }),
                editor,
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);

            if cfg.word_wrap {
                bq_row.boxed()
            } else {
                container(bq_row).width(Length::Fill).clip(true).boxed()
            }
        } else if cfg.word_wrap {
            editor.into()
        } else {
            container(editor).width(Length::Fill).clip(true).boxed()
        };

        lines_col.push(line_el);
        line_idx += 1;
    }

    if cfg.typewriter_mode {
        lines_col.push(
            iced::widget::space::vertical()
                .height(Length::Fixed(cfg.viewport_height * 0.5))
                .boxed(),
        );
    }

    let content_h = total_lines as f32 * row_h;
    let bottom_fill_h = (cfg.viewport_height - content_h - 80.0).max(160.0);
    lines_col.push(
        mouse_area(
            iced::widget::space::vertical()
                .width(Length::Fill)
                .height(Length::Fixed(bottom_fill_h)),
        )
        .on_press(EditorMessage::ClickCanvasBottom)
        .interaction(iced::mouse::Interaction::Text)
        .boxed(),
    );

    let doc_col = column![]
        .extend(lines_col)
        .spacing(2)
        .width(Length::Fill);
    let doc_inner: Element<'a, EditorMessage> = if cfg.full_width {
        container(doc_col)
            .width(Length::Fill)
            .clip(!cfg.word_wrap)
            .boxed()
    } else {
        container(
            container(doc_col)
                .width(cfg.content_max_width)
                .clip(!cfg.word_wrap),
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .clip(!cfg.word_wrap)
        .boxed()
    };
    let doc_container = container(doc_inner)
        .width(Length::Fill)
        .padding(Padding::from([24, 32]))
        .clip(!cfg.word_wrap);

    let scroll_view = scrollable(doc_container)
        .id(iced::widget::Id::new(EDITOR_SCROLLABLE_ID))
        .on_scroll(|vp| {
            if matches!(
                vp.source,
                scrollable::Source::Content | scrollable::Source::Resize
            ) || vp.target.is_some()
            {
                scrollable::Action::None
            } else {
                scrollable::Action::Custom(EditorMessage::Scrolled(
                    vp.viewport.absolute_offset().y,
                ))
            }
        })
        .width(Length::Fill)
        .height(Length::Fill);

    let status = view_status_bar(tab, cfg, scan.unclosed_fence.is_some(), palette);

    let top_banners = column![].extend(top_stack).width(Length::Fill);
    let root = column![top_banners, scroll_view, status]
        .width(Length::Fill)
        .height(Length::Fill);

    container(root)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(bg.into()),
            ..container::Style::default()
        })
        .boxed()
}

fn view_status_bar<'a>(
    tab: &'a Tab,
    cfg: EditorViewConfig,
    has_unclosed_fence: bool,
    palette: &ThemePalette,
) -> Element<'a, EditorMessage> {
    let surface = palette.surface;
    let muted = palette.text_muted;
    let accent = palette.accent;
    let palette_copy = *palette;

    let mode_label = if tab.active_line.is_some() {
        "Editing"
    } else {
        "Viewing"
    };
    let path_label = tab
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| tab.title.clone());
    let save_label = if tab.is_dirty {
        "Modified"
    } else if tab.path.is_some() {
        "Saved"
    } else {
        "Unsaved"
    };

    let caret = tab.selection.head;
    let ln_col = format!("Ln {}, Col {}", caret.line + 1, caret.col + 1);
    let words = tab.rope.to_string().split_whitespace().count();
    let mins = (words / 230).max(1);
    let word_stats = format!("{words} words · ~{mins} min read");
    let zoom_label = format!("{}%", (tab.zoom * 100.0).round() as u32);
    let enc_label = if tab.has_bom { "UTF-8 BOM" } else { "UTF-8" };

    let mut items = row![
        text(mode_label).size(11).color(accent),
        text(path_label).size(11).color(muted),
        text(save_label).size(11).color(muted),
        iced::widget::space::horizontal(),
        text(ln_col).size(11).color(muted),
        text(word_stats).size(11).color(muted),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    if let Some(target) = cfg.word_target {
        items = items.push(
            text(format!("{words} / {target}"))
                .size(11)
                .color(accent)
                .boxed(),
        );
    }
    if cfg.focus_mode {
        items = items.push(text("Focus").size(11).color(accent).boxed());
    }
    if cfg.typewriter_mode {
        items = items.push(text("Typewriter").size(11).color(accent).boxed());
    }
    if has_unclosed_fence {
        items = items.push(text("Unclosed fence").size(11).color(accent).boxed());
    }

    items = items
        .push(
            button(text(zoom_label).size(11).color(muted))
                .padding(Padding::from([2, 6]))
                .style(ghost_icon_style(palette_copy, false))
                .on_press(EditorMessage::ResetZoom)
                .boxed(),
        )
        .push(
            button(text(tab.line_ending.label()).size(11).color(muted))
                .padding(Padding::from([2, 6]))
                .style(ghost_icon_style(palette_copy, false))
                .on_press(EditorMessage::ToggleLineEnding)
                .boxed(),
        )
        .push(text(enc_label).size(11).color(muted).boxed());

    container(items)
        .width(Length::Fill)
        .padding(Padding::from([4, 12]))
        .style(move |_| container::Style {
            background: Some(surface.into()),
            ..container::Style::default()
        })
        .boxed()
}

fn normalize_lang_token(lang: &str) -> &str {
    match lang.trim().to_ascii_lowercase().as_str() {
        "rust" | "rs" => "rs",
        "typescript" | "ts" => "ts",
        "tsx" => "tsx",
        "javascript" | "js" => "js",
        "jsx" => "jsx",
        "python" | "py" => "py",
        "bash" | "sh" | "zsh" | "shell" => "sh",
        "powershell" | "ps1" | "pwsh" => "ps1",
        "markdown" | "md" => "md",
        "yaml" | "yml" => "yaml",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "c" => "c",
        "cpp" | "c++" | "cc" => "cpp",
        "csharp" | "c#" | "cs" => "cs",
        "go" | "golang" => "go",
        "java" => "java",
        "kotlin" | "kt" => "kt",
        "xml" => "xml",
        _ => lang.trim(),
    }
}

fn highlight_code_spans(
    lines: &[&str],
    lang: &str,
    code_font: Font,
    code_size: f32,
    palette: &ThemePalette,
) -> Vec<Span<'static, iced::Never>> {
    if lines.is_empty() {
        return vec![Span::new(" ").font(code_font).size(code_size).color(palette.text)];
    }

    let token = normalize_lang_token(lang);
    let mut parser = iced::highlighter::Parser::new(&iced::highlighter::Settings {
        token: token.to_string(),
    });
    let italic_code = Font {
        style: iced::font::Style::Italic,
        ..code_font
    };
    let bold_code = Font {
        weight: iced::font::Weight::Semibold,
        ..code_font
    };

    let mut spans: Vec<Span<'static, iced::Never>> = Vec::with_capacity(lines.len() * 4);
    for (idx, raw) in lines.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::new("\n").font(code_font).size(code_size));
        }
        let trimmed = raw.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            spans.push(Span::new(" ").font(code_font).size(code_size));
            continue;
        }
        let mut produced = false;
        for (range, code_kind) in parser.parse_line(trimmed) {
            if let Some(slice) = trimmed.get(range)
                && !slice.is_empty()
            {
                produced = true;
                let (color, font) = match code_kind {
                    Code::Comment => (palette.syntax_comment, italic_code),
                    Code::String => (palette.syntax_string, code_font),
                    Code::Keyword => (palette.syntax_keyword, bold_code),
                    Code::Constant => (palette.syntax_number, code_font),
                    Code::Function => (palette.syntax_function, code_font),
                    Code::Type | Code::Support | Code::Path => (palette.syntax_type, code_font),
                    Code::Punctuation => (palette.text_muted, code_font),
                    Code::Invalid => (palette.danger, code_font),
                    Code::Variable | Code::Other => (palette.text, code_font),
                };
                spans.push(
                    Span::new(slice.to_string())
                        .font(font)
                        .size(code_size)
                        .color(color),
                );
            }
        }
        if !produced {
            spans.push(
                Span::new(trimmed.to_string())
                    .font(code_font)
                    .size(code_size)
                    .color(palette.text),
            );
        }
    }
    spans
}

fn resolve_local_image_path(
    target: &str,
    doc_path: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    let trimmed = target.trim();
    if trimmed.is_empty()
        || crate::core::markdown::image::is_allowed_remote_url(trimmed)
        || trimmed.starts_with("data:")
    {
        return None;
    }
    let decoded = trimmed.replace("%20", " ");
    let candidate = std::path::PathBuf::from(&decoded);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate);
    }
    if let Some(doc) = doc_path
        && let Some(dir) = doc.parent()
    {
        let joined = dir.join(&candidate);
        if joined.is_file() {
            return Some(joined);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let joined = cwd.join(&candidate);
        if joined.is_file() {
            return Some(joined);
        }
    }
    None
}

fn render_inline_image<'a>(
    img: &crate::core::markdown::image::ParsedImage,
    doc_path: Option<&std::path::Path>,
    line_idx: usize,
    palette: &ThemePalette,
) -> Element<'a, EditorMessage> {
    let p = *palette;
    let resolved = resolve_local_image_path(&img.target, doc_path);

    let body: Element<'a, EditorMessage> = if let Some(path) = resolved {
        let is_svg = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        if is_svg {
            let mut s = svg(svg::Handle::from_path(path))
                .content_fit(ContentFit::ScaleDown)
                .height(Length::Fixed(320.0));
            if let Some(w) = img.width {
                s = s.width(Length::Fixed(w as f32));
            } else {
                s = s.width(Length::Fill);
            }
            s.boxed()
        } else {
            let mut im = image(image::Handle::from_path(path))
                .content_fit(ContentFit::ScaleDown);
            if let Some(w) = img.width {
                im = im.width(Length::Fixed(w as f32));
            } else {
                im = im.width(Length::Fill);
            }
            if let Some(h) = img.height {
                im = im.height(Length::Fixed(h as f32));
            }
            im.boxed()
        }
    } else {
        let label = if img.alt.is_empty() {
            format!("Image not found: {}", img.target)
        } else {
            format!("Image ({}): {}", img.alt, img.target)
        };
        text(label).size(12).color(p.text_muted).boxed()
    };

    let mut card_col = column![body].spacing(6).align_x(iced::Alignment::Center);
    if !img.alt.is_empty() {
        card_col = card_col.push(
            text(img.alt.clone())
                .size(12)
                .color(p.text_muted)
                .boxed(),
        );
    }

    let card = container(card_col)
        .padding(Padding::from([10, 12]))
        .width(Length::Fill)
        .center_x(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

    mouse_area(card)
        .interaction(iced::mouse::Interaction::Pointer)
        .on_press(EditorMessage::JumpToLine(line_idx))
        .boxed()
}
