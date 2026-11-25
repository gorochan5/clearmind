//! Custom `LiveMarkdownEditor` widget for Iced 0.15.0-dev.
//!
//! Provides click-to-caret via `DisplayMap` & `CosmicParagraph::hit_test`,
//! exact glyph-measured caret positioning (`caret_rect_from_paragraph`),
//! rich inline Markdown spans (`**bold**`, `*italic*`, `_italic_`, `~~strikethrough~~`,
//! `` `inline code` ``, `[link](url)`), direct link following, 2D drag selection,
//! zero-width multi-cursor overlays, 530ms blinking caret, IME preedit/commit support,
//! and strict line-bound clipping when `word_wrap = false`.

use std::borrow::Cow;
use std::time::{Duration, Instant};

use iced::advanced::graphics::text::Paragraph as CosmicParagraph;
use iced::advanced::input_method::{self, InputMethod};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::text::{self, Paragraph as _, Span, Text};
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Shell, Widget};
use iced::keyboard::{self, Key, key::Named};
use iced::mouse;
use iced::window;
use iced::{Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector};

use super::models::{Cursor, DocumentSelection};

pub const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// Bidirectional mapping between rendered display columns and raw Markdown columns on a line.
#[derive(Debug, Clone, Default)]
pub struct DisplayMap {
    pub raw_to_display: Vec<usize>,
    pub display_to_raw: Vec<usize>,
}

impl DisplayMap {
    pub fn identity(len: usize) -> Self {
        let map: Vec<usize> = (0..=len).collect();
        Self {
            raw_to_display: map.clone(),
            display_to_raw: map,
        }
    }

    /// Shifts a `DisplayMap` when `prefix_chars` characters at the start of the raw line
    /// were stripped prior to inline parsing (e.g. `# `, `> `, `- [ ] `).
    pub fn with_stripped_prefix(self, prefix_chars: usize) -> Self {
        if prefix_chars == 0 {
            return self;
        }
        let mut raw_to_display = Vec::with_capacity(prefix_chars + self.raw_to_display.len());
        raw_to_display.extend(std::iter::repeat_n(0, prefix_chars));
        raw_to_display.extend(self.raw_to_display);
        let display_to_raw = self
            .display_to_raw
            .into_iter()
            .map(|r| r + prefix_chars)
            .collect();
        Self {
            raw_to_display,
            display_to_raw,
        }
    }

    /// Shifts a `DisplayMap` when `prefix_chars` characters at the start of the line
    /// were replaced 1:1 in display (e.g. `  - ` -> `  • `).
    pub fn with_1to1_prefix(self, prefix_chars: usize) -> Self {
        if prefix_chars == 0 {
            return self;
        }
        let mut raw_to_display: Vec<usize> = (0..prefix_chars).collect();
        raw_to_display.extend(self.raw_to_display.into_iter().map(|d| d + prefix_chars));
        let mut display_to_raw: Vec<usize> = (0..prefix_chars).collect();
        display_to_raw.extend(self.display_to_raw.into_iter().map(|r| r + prefix_chars));
        Self {
            raw_to_display,
            display_to_raw,
        }
    }

    pub fn to_raw(&self, display_col: usize) -> usize {
        self.display_to_raw
            .get(display_col)
            .copied()
            .unwrap_or_else(|| self.display_to_raw.last().copied().unwrap_or(0))
    }

    pub fn to_display(&self, raw_col: usize) -> usize {
        self.raw_to_display
            .get(raw_col)
            .copied()
            .unwrap_or_else(|| self.raw_to_display.last().copied().unwrap_or(0))
    }
}

/// A clickable span inside a rendered line (link, wiki-link, or local file reference).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    pub start_col: usize,
    pub end_col: usize,
    pub target: String,
}

/// Parses inline Markdown in `raw_trimmed` into rich `Span<'static, ()>` items,
/// the rendered display string (with delimiters hidden), an exact character-level
/// `DisplayMap`, and clickable `LinkSpan`s.
#[allow(clippy::too_many_arguments)]
pub fn parse_inline_spans(
    raw_trimmed: &str,
    base_font: iced::Font,
    code_font: iced::Font,
    font_size: f32,
    text_color: Color,
    accent_color: Color,
    code_bg: Color,
    strikethrough_all: bool,
) -> (Vec<Span<'static, ()>>, String, DisplayMap, Vec<LinkSpan>) {
    let raw_chars: Vec<char> = raw_trimmed.chars().collect();
    let n = raw_chars.len();
    let mut raw_to_display = vec![0usize; n + 1];
    let mut display_to_raw = Vec::with_capacity(n + 1);
    let mut display_text = String::with_capacity(raw_trimmed.len());
    let mut spans: Vec<Span<'static, ()>> = Vec::new();
    let mut links: Vec<LinkSpan> = Vec::new();
    let mut plain_buf = String::new();
    let mut display_len = 0usize;

    let flush_plain = |plain_buf: &mut String, spans: &mut Vec<Span<'static, ()>>| {
        if !plain_buf.is_empty() {
            let s = std::mem::take(plain_buf);
            spans.push(
                Span::new(s)
                    .font(base_font)
                    .color(text_color)
                    .strikethrough(strikethrough_all),
            );
        }
    };

    let mut i = 0usize;
    while i < n {
        // 1. Inline code: `code`
        if raw_chars[i] == '`'
            && let Some(offset) = raw_chars[i + 1..].iter().position(|&c| c == '`')
        {
            let close = i + 1 + offset;
            if close > i + 1 {
                flush_plain(&mut plain_buf, &mut spans);
                raw_to_display[i] = display_len;
                let mut code_str = String::with_capacity(close - (i + 1));
                for (idx, &ch) in raw_chars.iter().enumerate().take(close).skip(i + 1) {
                    raw_to_display[idx] = display_len;
                    display_to_raw.push(idx);
                    display_text.push(ch);
                    code_str.push(ch);
                    display_len += 1;
                }
                raw_to_display[close] = display_len;
                spans.push(
                    Span::new(code_str)
                        .font(code_font)
                        .size(font_size * 0.92)
                        .color(text_color)
                        .background(code_bg)
                        .padding([1.0, 4.0])
                        .border(Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        })
                        .strikethrough(strikethrough_all),
                );
                i = close + 1;
                continue;
            }
        }

        // 2. Bold + Italic: ***bold italic*** or ___bold italic___
        if i + 6 <= n
            && ((raw_chars[i] == '*' && raw_chars[i + 1] == '*' && raw_chars[i + 2] == '*')
                || (raw_chars[i] == '_' && raw_chars[i + 1] == '_' && raw_chars[i + 2] == '_'))
        {
            let delim = raw_chars[i];
            let mut found_close = None;
            let mut k = i + 3;
            while k + 3 <= n {
                if raw_chars[k] == delim && raw_chars[k + 1] == delim && raw_chars[k + 2] == delim {
                    found_close = Some(k);
                    break;
                }
                k += 1;
            }
            if let Some(close) = found_close
                && close > i + 3
            {
                flush_plain(&mut plain_buf, &mut spans);
                for slot in &mut raw_to_display[i..i + 3] {
                    *slot = display_len;
                }
                let mut s = String::with_capacity(close - (i + 3));
                for (idx, &ch) in raw_chars.iter().enumerate().take(close).skip(i + 3) {
                    raw_to_display[idx] = display_len;
                    display_to_raw.push(idx);
                    display_text.push(ch);
                    s.push(ch);
                    display_len += 1;
                }
                for slot in &mut raw_to_display[close..close + 3] {
                    *slot = display_len;
                }
                spans.push(
                    Span::new(s)
                        .font(iced::Font {
                            weight: iced::font::Weight::Bold,
                            style: iced::font::Style::Italic,
                            ..base_font
                        })
                        .color(text_color)
                        .strikethrough(strikethrough_all),
                );
                i = close + 3;
                continue;
            }
        }

        // 3. Bold: **bold** or __bold__
        if i + 4 <= n
            && ((raw_chars[i] == '*' && raw_chars[i + 1] == '*')
                || (raw_chars[i] == '_' && raw_chars[i + 1] == '_'))
        {
            let delim = raw_chars[i];
            let mut found_close = None;
            let mut k = i + 2;
            while k + 2 <= n {
                if raw_chars[k] == delim && raw_chars[k + 1] == delim {
                    found_close = Some(k);
                    break;
                }
                k += 1;
            }
            if let Some(close) = found_close
                && close > i + 2
            {
                flush_plain(&mut plain_buf, &mut spans);
                for slot in &mut raw_to_display[i..i + 2] {
                    *slot = display_len;
                }
                let mut s = String::with_capacity(close - (i + 2));
                for (idx, &ch) in raw_chars.iter().enumerate().take(close).skip(i + 2) {
                    raw_to_display[idx] = display_len;
                    display_to_raw.push(idx);
                    display_text.push(ch);
                    s.push(ch);
                    display_len += 1;
                }
                for slot in &mut raw_to_display[close..close + 2] {
                    *slot = display_len;
                }
                spans.push(
                    Span::new(s)
                        .font(iced::Font {
                            weight: iced::font::Weight::Bold,
                            ..base_font
                        })
                        .color(text_color)
                        .strikethrough(strikethrough_all),
                );
                i = close + 2;
                continue;
            }
        }

        // 4. Strikethrough: ~~strike~~
        if i + 4 <= n && raw_chars[i] == '~' && raw_chars[i + 1] == '~' {
            let mut found_close = None;
            let mut k = i + 2;
            while k + 2 <= n {
                if raw_chars[k] == '~' && raw_chars[k + 1] == '~' {
                    found_close = Some(k);
                    break;
                }
                k += 1;
            }
            if let Some(close) = found_close
                && close > i + 2
            {
                flush_plain(&mut plain_buf, &mut spans);
                for slot in &mut raw_to_display[i..i + 2] {
                    *slot = display_len;
                }
                let mut s = String::with_capacity(close - (i + 2));
                for (idx, &ch) in raw_chars.iter().enumerate().take(close).skip(i + 2) {
                    raw_to_display[idx] = display_len;
                    display_to_raw.push(idx);
                    display_text.push(ch);
                    s.push(ch);
                    display_len += 1;
                }
                for slot in &mut raw_to_display[close..close + 2] {
                    *slot = display_len;
                }
                spans.push(
                    Span::new(s)
                        .font(base_font)
                        .color(text_color)
                        .strikethrough(true),
                );
                i = close + 2;
                continue;
            }
        }

        // 5. Inline link: [label](url)
        if raw_chars[i] == '['
            && let Some(cb_offset) = raw_chars[i + 1..].iter().position(|&c| c == ']')
        {
            let close_bracket = i + 1 + cb_offset;
            if close_bracket > i + 1
                && close_bracket + 2 < n
                && raw_chars[close_bracket + 1] == '('
                && let Some(cp_offset) =
                    raw_chars[close_bracket + 2..].iter().position(|&c| c == ')')
            {
                let close_paren = close_bracket + 2 + cp_offset;
                flush_plain(&mut plain_buf, &mut spans);
                raw_to_display[i] = display_len;
                let start_col = display_len;
                let mut label = String::with_capacity(close_bracket - (i + 1));
                for (idx, &ch) in raw_chars.iter().enumerate().take(close_bracket).skip(i + 1) {
                    raw_to_display[idx] = display_len;
                    display_to_raw.push(idx);
                    display_text.push(ch);
                    label.push(ch);
                    display_len += 1;
                }
                let end_col = display_len;
                for slot in &mut raw_to_display[close_bracket..=close_paren] {
                    *slot = display_len;
                }
                let url: String = raw_chars[close_bracket + 2..close_paren].iter().collect();
                links.push(LinkSpan {
                    start_col,
                    end_col,
                    target: url,
                });
                spans.push(
                    Span::new(label)
                        .font(base_font)
                        .color(accent_color)
                        .underline(true)
                        .strikethrough(strikethrough_all),
                );
                i = close_paren + 1;
                continue;
            }
        }

        // 6. Italic: *italic* or _italic_
        if (raw_chars[i] == '*' || raw_chars[i] == '_') && i + 2 < n {
            let delim = raw_chars[i];
            let valid_open = raw_chars[i + 1] != delim
                && raw_chars[i + 1] != ' '
                && (delim != '_' || i == 0 || !raw_chars[i - 1].is_alphanumeric());
            if valid_open {
                let mut found_close = None;
                let mut k = i + 2;
                while k < n {
                    if raw_chars[k] == delim && raw_chars[k - 1] != ' ' {
                        let valid_close =
                            delim != '_' || k + 1 == n || !raw_chars[k + 1].is_alphanumeric();
                        if valid_close {
                            found_close = Some(k);
                            break;
                        }
                    }
                    k += 1;
                }
                if let Some(close) = found_close {
                    flush_plain(&mut plain_buf, &mut spans);
                    raw_to_display[i] = display_len;
                    let mut s = String::with_capacity(close - (i + 1));
                    for (idx, &ch) in raw_chars.iter().enumerate().take(close).skip(i + 1) {
                        raw_to_display[idx] = display_len;
                        display_to_raw.push(idx);
                        display_text.push(ch);
                        s.push(ch);
                        display_len += 1;
                    }
                    raw_to_display[close] = display_len;
                    spans.push(
                        Span::new(s)
                            .font(iced::Font {
                                style: iced::font::Style::Italic,
                                ..base_font
                            })
                            .color(text_color)
                            .strikethrough(strikethrough_all),
                    );
                    i = close + 1;
                    continue;
                }
            }
        }

        // 7. Plain character
        raw_to_display[i] = display_len;
        display_to_raw.push(i);
        display_text.push(raw_chars[i]);
        plain_buf.push(raw_chars[i]);
        display_len += 1;
        i += 1;
    }

    flush_plain(&mut plain_buf, &mut spans);
    raw_to_display[n] = display_len;
    display_to_raw.push(n);

    (
        spans,
        display_text,
        DisplayMap {
            raw_to_display,
            display_to_raw,
        },
        links,
    )
}

#[derive(Debug, Clone)]
pub enum EditorWidgetEvent {
    PlaceCaret { cursor: Cursor, extend: bool },
    DragSelect { head: Cursor },
    OpenLink { target: String, reveal_in_explorer: bool },
    InsertText(String),
    KeyCommand(EditorKeyCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKeyCommand {
    Left { extend: bool },
    Right { extend: bool },
    WordLeft { extend: bool },
    WordRight { extend: bool },
    Up { extend: bool },
    Down { extend: bool },
    Home { extend: bool },
    End { extend: bool },
    DocHome { extend: bool },
    DocEnd { extend: bool },
    Backspace,
    Delete,
    DeleteWordBackward,
    DeleteWordForward,
    Enter,
    Tab,
    ShiftTab,
}

#[derive(Debug)]
pub struct EditorWidgetState {
    pub last_blink: Instant,
    pub caret_visible: bool,
    pub is_dragging: bool,
    pub last_click_time: Option<Instant>,
    pub click_count: u8,
    pub preedit: Option<String>,
    pub paragraph: CosmicParagraph,
}

impl Default for EditorWidgetState {
    fn default() -> Self {
        Self {
            last_blink: Instant::now(),
            caret_visible: true,
            is_dragging: false,
            last_click_time: None,
            click_count: 0,
            preedit: None,
            paragraph: CosmicParagraph::default(),
        }
    }
}

pub struct LiveMarkdownEditor<'a, Message> {
    pub line_index: usize,
    pub raw_text: Cow<'a, str>,
    pub display_text: Cow<'a, str>,
    pub display_map: DisplayMap,
    pub links: Vec<LinkSpan>,
    pub spans: Vec<Span<'static, ()>>,
    pub active_line: Option<usize>,
    pub selection: DocumentSelection,
    pub extra_cursors: &'a [Cursor],
    pub goal_x: Option<f32>,
    pub font: iced::Font,
    pub code_font: iced::Font,
    pub font_size: f32,
    pub line_height: f32,
    pub text_color: Color,
    pub code_bg: Color,
    pub placeholder_color: Color,
    pub caret_color: Color,
    pub selection_color: Color,
    pub wrapping: text::Wrapping,
    pub strikethrough_all: bool,
    pub slash_menu_open: bool,
    pub on_event: Box<dyn Fn(EditorWidgetEvent) -> Message + 'a>,
}

impl<'a, Message> LiveMarkdownEditor<'a, Message> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line_index: usize,
        raw_text: impl Into<Cow<'a, str>>,
        display_text: impl Into<Cow<'a, str>>,
        display_map: DisplayMap,
        links: Vec<LinkSpan>,
        active_line: Option<usize>,
        selection: DocumentSelection,
        extra_cursors: &'a [Cursor],
        goal_x: Option<f32>,
        font: iced::Font,
        font_size: f32,
        line_height: f32,
        text_color: Color,
        placeholder_color: Color,
        caret_color: Color,
        selection_color: Color,
        on_event: impl Fn(EditorWidgetEvent) -> Message + 'a,
    ) -> Self {
        Self {
            line_index,
            raw_text: raw_text.into(),
            display_text: display_text.into(),
            display_map,
            links,
            spans: Vec::new(),
            active_line,
            selection,
            extra_cursors,
            goal_x,
            font,
            code_font: iced::Font::MONOSPACE,
            font_size,
            line_height,
            text_color,
            code_bg: Color::from_rgba(0.5, 0.5, 0.5, 0.18),
            placeholder_color,
            caret_color,
            selection_color,
            wrapping: text::Wrapping::WordOrGlyph,
            strikethrough_all: false,
            slash_menu_open: false,
            on_event: Box::new(on_event),
        }
    }

    pub fn with_code_font(mut self, code_font: iced::Font) -> Self {
        self.code_font = code_font;
        self
    }

    pub fn with_code_bg(mut self, code_bg: Color) -> Self {
        self.code_bg = code_bg;
        self
    }

    pub fn with_wrapping(mut self, wrapping: text::Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    pub fn with_strikethrough_all(mut self, strikethrough_all: bool) -> Self {
        self.strikethrough_all = strikethrough_all;
        self
    }

    pub fn with_spans(mut self, spans: Vec<Span<'static, ()>>) -> Self {
        self.spans = spans;
        self
    }

    pub fn with_slash_menu_open(mut self, open: bool) -> Self {
        self.slash_menu_open = open;
        self
    }

    fn is_active(&self) -> bool {
        self.active_line == Some(self.line_index)
    }

    fn shown_trimmed(&self) -> &str {
        let shown: &str = if self.is_active() {
            self.raw_text.as_ref()
        } else {
            self.display_text.as_ref()
        };
        shown.trim_end_matches(['\r', '\n'])
    }

    fn hit_span(&self, col: usize) -> Option<&LinkSpan> {
        self.links
            .iter()
            .find(|span| col >= span.start_col && col < span.end_col)
    }

    /// Computes the exact caret rectangle for `raw_char_col` using `CosmicParagraph`'s
    /// shaped `cosmic_text::Buffer` glyph layout.
    pub fn caret_rect_from_paragraph(
        &self,
        paragraph: &CosmicParagraph,
        bounds: Rectangle,
        raw_char_col: usize,
    ) -> Rectangle {
        let display_char_col = if self.is_active() {
            raw_char_col
        } else {
            self.display_map.to_display(raw_char_col)
        };
        let shown_trimmed = self.shown_trimmed();
        let cursor_byte = shown_trimmed
            .char_indices()
            .nth(display_char_col)
            .map(|(b, _)| b)
            .unwrap_or(shown_trimmed.len());

        let buffer = paragraph.buffer();
        let hint_factor = paragraph.hint_factor().unwrap_or(1.0);
        let line_h = (self.font_size * self.line_height).max(20.0);

        let Some(layout) = buffer.lines.first().and_then(|l| l.layout_opt()) else {
            return Rectangle::new(bounds.position(), Size::new(2.0, line_h));
        };

        if layout.is_empty() || shown_trimmed.is_empty() {
            return Rectangle::new(bounds.position(), Size::new(2.0, line_h));
        }

        let (visual_line, offset_px) = layout
            .iter()
            .enumerate()
            .find_map(|(i, line)| {
                let (start, offset) = line
                    .glyphs
                    .first()
                    .map(|glyph| (glyph.start, glyph.x))
                    .unwrap_or((0, 0.0));
                let end = line.glyphs.last().map(|glyph| glyph.end).unwrap_or(0);

                if start > cursor_byte {
                    let prev_idx = i.saturating_sub(1);
                    let prev = &layout[prev_idx];
                    let prev_offset = prev.glyphs.first().map(|g| g.x).unwrap_or(0.0);
                    Some((prev_idx, prev.w + prev_offset))
                } else if cursor_byte <= end {
                    let mut x = 0.0f32;
                    for glyph in &line.glyphs {
                        if cursor_byte >= glyph.end {
                            x += glyph.w;
                        } else if cursor_byte > glyph.start && glyph.end > glyph.start {
                            let frac =
                                (cursor_byte - glyph.start) as f32 / (glyph.end - glyph.start) as f32;
                            x += glyph.w * frac;
                            break;
                        } else {
                            break;
                        }
                    }
                    Some((i, x + offset))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                (
                    layout.len().saturating_sub(1),
                    layout
                        .last()
                        .map(|line| {
                            line.w + line.glyphs.first().map(|glyph| glyph.x).unwrap_or(0.0)
                        })
                        .unwrap_or(0.0),
                )
            });

        Rectangle::new(
            Point::new(
                bounds.x + (offset_px / hint_factor),
                bounds.y + (visual_line as f32 * line_h),
            ),
            Size::new(2.0, line_h),
        )
    }

    /// Maps a local point inside `bounds` to `(display_col, raw_col)` via `paragraph.hit_test`.
    fn hit_test_columns(&self, paragraph: &CosmicParagraph, local_pos: Point) -> (usize, usize) {
        let shown_trimmed = self.shown_trimmed();
        let display_col = if let Some(hit) = paragraph.hit_test(local_pos) {
            let byte_idx = shown_trimmed.floor_char_boundary(hit.index.min(shown_trimmed.len()));
            shown_trimmed[..byte_idx].chars().count()
        } else if local_pos.x > paragraph.min_bounds().width {
            shown_trimmed.chars().count()
        } else {
            0
        };
        let max_raw_col = self.raw_text.trim_end_matches(['\r', '\n']).chars().count();
        let raw_col = if self.is_active() {
            display_col
        } else {
            self.display_map.to_raw(display_col)
        }
        .min(max_raw_col);

        (display_col, raw_col)
    }
}

impl<'a, Message> iced::advanced::widget::Meta for LiveMarkdownEditor<'a, Message> {}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for LiveMarkdownEditor<'a, Message>
where
    Renderer: renderer::Renderer + text::Renderer<Paragraph = CosmicParagraph>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<EditorWidgetState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(EditorWidgetState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let state = tree.state.downcast_mut::<EditorWidgetState>();
        let max_w = limits.max.width;
        let single_h = (self.font_size * self.line_height).max(20.0);
        let bounds = Size::new(max_w.max(10.0), f32::INFINITY);

        if self.is_active() {
            let raw_trimmed = self.raw_text.trim_end_matches(['\r', '\n']);
            state.paragraph = CosmicParagraph::with_text(Text {
                content: raw_trimmed,
                bounds,
                size: iced::Pixels(self.font_size),
                line_height: text::LineHeight::Relative(self.line_height),
                font: self.font,
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: self.wrapping,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            });
        } else if !self.spans.is_empty() {
            state.paragraph = CosmicParagraph::with_spans(Text {
                content: self.spans.as_slice(),
                bounds,
                size: iced::Pixels(self.font_size),
                line_height: text::LineHeight::Relative(self.line_height),
                font: self.font,
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: self.wrapping,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            });
        } else {
            let shown_trimmed = self.display_text.trim_end_matches(['\r', '\n']);
            state.paragraph = CosmicParagraph::with_text(Text {
                content: shown_trimmed,
                bounds,
                size: iced::Pixels(self.font_size),
                line_height: text::LineHeight::Relative(self.line_height),
                font: self.font,
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: self.wrapping,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            });
        }

        let h = state.paragraph.min_bounds().height.max(single_h);
        tree.size = Size::new(max_w, h);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<EditorWidgetState>();
        let bounds = layout.bounds();

        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                if self.is_active() {
                    if now.duration_since(state.last_blink) >= CARET_BLINK_INTERVAL {
                        state.caret_visible = !state.caret_visible;
                        state.last_blink = *now;
                    }
                    let caret = self.caret_rect_from_paragraph(
                        &state.paragraph,
                        bounds,
                        self.selection.head.col,
                    );
                    shell.request_input_method(&InputMethod::Enabled {
                        cursor: caret,
                        purpose: input_method::Purpose::Normal,
                        preedit: state.preedit.as_ref().map(|content| input_method::Preedit {
                            content: content.clone(),
                            selection: None,
                            text_size: Some(self.font_size.into()),
                        }),
                    });
                }
            }
            Event::InputMethod(ime) if self.is_active() => match ime {
                input_method::Event::Preedit(text, _) => {
                    state.preedit = if text.is_empty() {
                        None
                    } else {
                        Some(text.clone())
                    };
                    shell.capture_event();
                }
                input_method::Event::Commit(text) => {
                    state.preedit = None;
                    state.caret_visible = true;
                    state.last_blink = Instant::now();
                    shell.publish((self.on_event)(EditorWidgetEvent::InsertText(text.clone())));
                    shell.capture_event();
                }
                input_method::Event::Opened | input_method::Event::Closed => {}
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let local_pos = Point::new(pos.x, pos.y);
                    let (display_col, raw_col) =
                        self.hit_test_columns(&state.paragraph, local_pos);

                    if !self.is_active()
                        && let Some(link) = self.hit_span(display_col)
                    {
                        shell.publish((self.on_event)(EditorWidgetEvent::OpenLink {
                            target: link.target.clone(),
                            reveal_in_explorer: false,
                        }));
                        shell.capture_event();
                        return;
                    }

                    state.is_dragging = true;
                    state.caret_visible = true;
                    state.last_blink = Instant::now();
                    shell.publish((self.on_event)(EditorWidgetEvent::PlaceCaret {
                        cursor: Cursor::new(self.line_index, raw_col),
                        extend: false,
                    }));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.is_dragging => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let local_pos = Point::new(pos.x, pos.y);
                    let (_display_col, raw_col) =
                        self.hit_test_columns(&state.paragraph, local_pos);
                    shell.publish((self.on_event)(EditorWidgetEvent::DragSelect {
                        head: Cursor::new(self.line_index, raw_col),
                    }));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.is_dragging = false;
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if self.is_active() => {
                // Never insert Escape or capture it — let the global Escape cascade exit editing cleanly.
                if matches!(key.as_ref(), Key::Named(Named::Escape)) {
                    return;
                }

                // When the slash / LaTeX Intellisense popup is open, let the global router handle
                // Up/Down/Enter/Tab for navigating and accepting completions without mutating the line.
                if self.slash_menu_open
                    && matches!(
                        key.as_ref(),
                        Key::Named(Named::ArrowUp | Named::ArrowDown | Named::Enter | Named::Tab)
                    )
                {
                    return;
                }

                state.caret_visible = true;
                state.last_blink = Instant::now();
                let extend = modifiers.shift();
                let word = modifiers.control();
                let alt = modifiers.alt();
                let cmd = if alt {
                    None
                } else {
                    match key.as_ref() {
                        Key::Named(Named::ArrowLeft) if word => {
                            Some(EditorKeyCommand::WordLeft { extend })
                        }
                        Key::Named(Named::ArrowRight) if word => {
                            Some(EditorKeyCommand::WordRight { extend })
                        }
                        Key::Named(Named::ArrowLeft) => Some(EditorKeyCommand::Left { extend }),
                        Key::Named(Named::ArrowRight) => Some(EditorKeyCommand::Right { extend }),
                        Key::Named(Named::ArrowUp) if !word => Some(EditorKeyCommand::Up { extend }),
                        Key::Named(Named::ArrowDown) if !word => {
                            Some(EditorKeyCommand::Down { extend })
                        }
                        Key::Named(Named::Home) if word => {
                            Some(EditorKeyCommand::DocHome { extend })
                        }
                        Key::Named(Named::End) if word => Some(EditorKeyCommand::DocEnd { extend }),
                        Key::Named(Named::Home) => Some(EditorKeyCommand::Home { extend }),
                        Key::Named(Named::End) => Some(EditorKeyCommand::End { extend }),
                        Key::Named(Named::Backspace) if word => {
                            Some(EditorKeyCommand::DeleteWordBackward)
                        }
                        Key::Named(Named::Delete) if word => {
                            Some(EditorKeyCommand::DeleteWordForward)
                        }
                        Key::Named(Named::Backspace) => Some(EditorKeyCommand::Backspace),
                        Key::Named(Named::Delete) => Some(EditorKeyCommand::Delete),
                        Key::Named(Named::Enter) if !word => Some(EditorKeyCommand::Enter),
                        Key::Named(Named::Tab) if !word => Some(if extend {
                            EditorKeyCommand::ShiftTab
                        } else {
                            EditorKeyCommand::Tab
                        }),
                        _ => None,
                    }
                };
                if let Some(cmd) = cmd {
                    shell.publish((self.on_event)(EditorWidgetEvent::KeyCommand(cmd)));
                    shell.capture_event();
                } else if let Some(txt) = text {
                    let is_altgr = modifiers.control() && modifiers.alt() && !modifiers.shift();
                    if (!modifiers.control() && !modifiers.alt()) || is_altgr {
                        let clean: String = txt.chars().filter(|c| !c.is_control()).collect();
                        if !clean.is_empty() {
                            shell.publish((self.on_event)(EditorWidgetEvent::InsertText(clean)));
                            shell.capture_event();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        _layout: Layout,
        _viewport: &Rectangle,
        _renderer: &Renderer,
        _operation: &mut dyn Operation,
    ) {
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(layout.bounds()) {
            let state = tree.state.downcast_ref::<EditorWidgetState>();
            let local_pos = Point::new(pos.x, pos.y);
            let (display_col, _raw_col) = self.hit_test_columns(&state.paragraph, local_pos);
            if !self.is_active() && self.hit_span(display_col).is_some() {
                return mouse::Interaction::Pointer;
            }
            return mouse::Interaction::Text;
        }
        mouse::Interaction::None
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<EditorWidgetState>();
        let bounds = layout.bounds();
        let Some(clip_bounds) = bounds.intersection(viewport) else {
            return;
        };

        // 1. Selection highlight (measured from shaped paragraph caret positions)
        if !self.selection.is_empty() {
            let (start, end) = self.selection.ordered();
            if self.line_index >= start.line && self.line_index <= end.line {
                let max_raw_col = self.raw_text.trim_end_matches(['\r', '\n']).chars().count();
                let start_col = if self.line_index == start.line {
                    start.col
                } else {
                    0
                };
                let end_col = if self.line_index == end.line {
                    end.col
                } else {
                    max_raw_col
                };
                let start_caret =
                    self.caret_rect_from_paragraph(&state.paragraph, bounds, start_col);
                let end_caret = self.caret_rect_from_paragraph(&state.paragraph, bounds, end_col);

                if (start_caret.y - end_caret.y).abs() < 1.0 {
                    let x = start_caret.x.min(end_caret.x);
                    let w = (end_caret.x - start_caret.x).abs().max(4.0);
                    let sel_rect =
                        Rectangle::new(Point::new(x, start_caret.y), Size::new(w, start_caret.height));
                    if let Some(clipped) = sel_rect.intersection(&clip_bounds) {
                        renderer.fill_quad(
                            Quad {
                                bounds: clipped,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            self.selection_color,
                        );
                    }
                } else {
                    let first_rect = Rectangle::new(
                        Point::new(start_caret.x, start_caret.y),
                        Size::new(
                            (bounds.x + bounds.width - start_caret.x).max(4.0),
                            start_caret.height,
                        ),
                    );
                    if let Some(clipped) = first_rect.intersection(&clip_bounds) {
                        renderer.fill_quad(
                            Quad {
                                bounds: clipped,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            self.selection_color,
                        );
                    }
                    let last_rect = Rectangle::new(
                        Point::new(bounds.x, end_caret.y),
                        Size::new((end_caret.x - bounds.x).max(4.0), end_caret.height),
                    );
                    if let Some(clipped) = last_rect.intersection(&clip_bounds) {
                        renderer.fill_quad(
                            Quad {
                                bounds: clipped,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            self.selection_color,
                        );
                    }
                }
            }
        }

        // 2. Line text (or subtle placeholder on empty line 0)
        let shown_trimmed = self.shown_trimmed();
        let show_placeholder = self.line_index == 0
            && shown_trimmed.is_empty()
            && (self.is_active() || self.raw_text.is_empty());

        if show_placeholder {
            renderer.fill_text(
                Text {
                    content: "Start typing Markdown, or press Ctrl+O to open a file...".to_string(),
                    bounds: bounds.size(),
                    size: iced::Pixels(self.font_size),
                    line_height: text::LineHeight::Relative(self.line_height),
                    font: self.font,
                    align_x: text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: self.wrapping,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: renderer.hint_factor(),
                },
                bounds.position(),
                self.placeholder_color,
                clip_bounds,
            );
        } else if !shown_trimmed.is_empty() {
            let translation = bounds.position() - Point::ORIGIN;

            // Draw inline span background highlights (e.g. `inline code` badges)
            if !self.is_active() && !self.spans.is_empty() {
                for (index, span) in self.spans.iter().enumerate() {
                    if let Some(highlight) = span.highlight {
                        let regions = state.paragraph.span_bounds(index);
                        for (i, region) in regions.iter().enumerate() {
                            let starts = i == 0;
                            let ends = i + 1 == regions.len();
                            let left = if starts { span.padding.left } else { 0.0 };
                            let right = if ends { span.padding.right } else { 0.0 };
                            let quad_bounds = Rectangle::new(
                                region.position() - Vector::new(left, span.padding.top),
                                region.size() + Size::new(left + right, span.padding.y()),
                            ) + translation;
                            if let Some(clipped) = quad_bounds.intersection(&clip_bounds) {
                                renderer.fill_quad(
                                    Quad {
                                        bounds: clipped,
                                        border: highlight.border,
                                        shadow: Shadow::default(),
                                        snap: true,
                                    },
                                    highlight.background,
                                );
                            }
                        }
                    }
                }
            }

            // Draw shaped paragraph glyphs strictly clipped to `clip_bounds`
            renderer.fill_paragraph(
                &state.paragraph,
                bounds.position(),
                self.text_color,
                clip_bounds,
            );

            // Draw inline span decorations (underline for links, strikethrough for ~~strike~~ / checked tasks)
            if !self.is_active() && !self.spans.is_empty() {
                for (index, span) in self.spans.iter().enumerate() {
                    if span.underline || span.strikethrough {
                        let regions = state.paragraph.span_bounds(index);
                        let color = span.color.unwrap_or(self.text_color);
                        for region in &regions {
                            if span.underline {
                                let line_rect = Rectangle::new(
                                    Point::new(
                                        bounds.x + region.x,
                                        bounds.y + region.y + region.height - 1.0,
                                    ),
                                    Size::new(region.width, 1.0),
                                );
                                if let Some(clipped) = line_rect.intersection(&clip_bounds) {
                                    renderer.fill_quad(
                                        Quad {
                                            bounds: clipped,
                                            border: Border::default(),
                                            shadow: Shadow::default(),
                                            snap: true,
                                        },
                                        color,
                                    );
                                }
                            }
                            if span.strikethrough {
                                let line_rect = Rectangle::new(
                                    Point::new(
                                        bounds.x + region.x,
                                        bounds.y + region.y + region.height * 0.5,
                                    ),
                                    Size::new(region.width, 1.0),
                                );
                                if let Some(clipped) = line_rect.intersection(&clip_bounds) {
                                    renderer.fill_quad(
                                        Quad {
                                            bounds: clipped,
                                            border: Border::default(),
                                            shadow: Shadow::default(),
                                            snap: true,
                                        },
                                        color,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Primary caret
        if self.is_active() && state.caret_visible {
            let caret = self.caret_rect_from_paragraph(
                &state.paragraph,
                bounds,
                self.selection.head.col,
            );
            if let Some(clipped) = caret.intersection(&clip_bounds) {
                renderer.fill_quad(
                    Quad {
                        bounds: clipped,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    self.caret_color,
                );
            }
        }

        // 4. Zero-width secondary cursor overlays
        for extra in self.extra_cursors {
            if extra.line == self.line_index {
                let mut caret =
                    self.caret_rect_from_paragraph(&state.paragraph, bounds, extra.col);
                caret.width = 1.5;
                if let Some(clipped) = caret.intersection(&clip_bounds) {
                    renderer.fill_quad(
                        Quad {
                            bounds: clipped,
                            border: Border::default(),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        self.caret_color,
                    );
                }
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        _tree: &'b mut Tree,
        _layout: Layout,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
        _window: Size,
    ) -> Vec<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        Vec::new()
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer> From<LiveMarkdownEditor<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + text::Renderer<Paragraph = CosmicParagraph> + 'a,
{
    fn from(widget: LiveMarkdownEditor<'a, Message>) -> Self {
        iced::advanced::Widget::_boxed(widget)
    }
}
