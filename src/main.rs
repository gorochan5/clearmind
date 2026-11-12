mod icons;

use std::path::PathBuf;
use std::sync::Arc;

use iced::alignment;
use iced::keyboard;
use iced::mouse;
use iced::widget::{
    self, Widget, button, column, container, markdown, mouse_area, row, scrollable, space, text,
    text_editor,
};
use iced::{
    Background, Border, Color, Font, Length, Padding, Size, Subscription, Task, Theme, window,
};

const ACTIVE_EDITOR_ID: &str = "clearmind-active-line-editor";

/// Generates the GitHub Light mode custom theme.
fn custom_theme() -> Theme {
    let seed = iced::theme::palette::Seed {
        background: Color::from_rgb8(255, 255, 255), // Main canvas (#ffffff)
        text: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
        primary: Color::from_rgb8(9, 105, 218),      // Links and accents (#0969da)
        success: Color::from_rgb8(26, 127, 55),      // Green (#1a7f37)
        warning: Color::from_rgb8(154, 103, 0),      // Yellow/Orange (#9a6700)
        danger: Color::from_rgb8(207, 34, 46),       // Red (#cf222e)
    };

    Theme::custom("GitHub Light", seed)
}

/// Converts a character index into a valid UTF-8 byte index in `s`.
fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

/// Converts an editor position (line, index) to an absolute byte offset in `text`.
fn editor_pos_to_byte_offset(text: &str, pos: text::Position) -> usize {
    let mut current_offset = 0;
    for (line_idx, line_str) in text.split('\n').enumerate() {
        if line_idx == pos.line {
            let clamped = pos.index.min(line_str.len());
            return current_offset + clamped;
        }
        current_offset += line_str.len() + 1; // +1 for '\n'
    }
    text.len()
}

/// Represents a contiguous range of lines that form a closed fenced code block [start..=end].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeBlockRange {
    /// Zero-based line index of the opening fence.
    pub start: usize,
    /// Zero-based line index of the closing fence.
    pub end: usize,
}

/// Parses an opening code fence from a line.
/// Returns (fence_len, fence_char) if the line is an opening fence with up to 3 leading spaces.
pub fn parse_opening_fence(raw: &str) -> Option<(usize, char)> {
    let indent = raw.chars().take_while(|c| *c == ' ').count();
    if indent > 3 {
        return None;
    }
    let trimmed = raw.trim_start();
    if trimmed.starts_with("```") {
        let len = trimmed.chars().take_while(|&c| c == '`').count();
        if len >= 3 {
            let rest = &trimmed[len..];
            if !rest.contains('`') {
                return Some((len, '`'));
            }
        }
    } else if trimmed.starts_with("~~~") {
        let len = trimmed.chars().take_while(|&c| c == '~').count();
        if len >= 3 {
            return Some((len, '~'));
        }
    }
    None
}

/// Checks if a line is a valid closing fence matching `required_char` and `min_len`.
pub fn parse_closing_fence(raw: &str, required_char: char, min_len: usize) -> bool {
    let indent = raw.chars().take_while(|c| *c == ' ').count();
    if indent > 3 {
        return false;
    }
    let trimmed = raw.trim_start();
    let count = trimmed.chars().take_while(|&c| c == required_char).count();
    if count >= min_len {
        let rest = &trimmed[count..];
        if rest.trim().is_empty() {
            return true;
        }
    }
    false
}

/// Scans document lines and identifies all complete fenced code block ranges.
pub fn find_code_blocks(lines: &[Line]) -> Vec<CodeBlockRange> {
    let mut ranges = Vec::new();
    let mut current: Option<(usize, usize, char)> = None;

    for (idx, line) in lines.iter().enumerate() {
        if let Some((start_idx, min_len, fence_char)) = current {
            if parse_closing_fence(&line.raw, fence_char, min_len) {
                ranges.push(CodeBlockRange {
                    start: start_idx,
                    end: idx,
                });
                current = None;
            }
        } else if let Some((len, fence_char)) = parse_opening_fence(&line.raw) {
            current = Some((idx, len, fence_char));
        }
    }

    ranges
}

/// Represents a contiguous range of lines that form a Markdown table [start..=end].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRange {
    /// Zero-based line index of the table header.
    pub start: usize,
    /// Zero-based line index of the last row of the table.
    pub end: usize,
}

/// Checks if a line matches the GFM table delimiter syntax (e.g. `|---|---|` or `|:---|---:|`).
pub fn is_table_delimiter(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.contains('-') || !trimmed.contains('|') {
        return false;
    }
    // Must only contain |, -, :, and whitespace
    if !trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
    {
        return false;
    }
    let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
    let filtered: Vec<&str> = parts.into_iter().filter(|s| !s.is_empty()).collect();
    if filtered.is_empty() {
        return false;
    }
    filtered.iter().all(|cell| {
        let trimmed_cell = cell.trim();
        let has_hyphen = trimmed_cell.contains('-');
        let valid_chars = trimmed_cell.chars().all(|c| c == '-' || c == ':');
        has_hyphen && valid_chars
    })
}

/// Checks if a line can be a candidate row of a Markdown table.
pub fn is_potential_table_row(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") || trimmed.starts_with('#') {
        return false;
    }
    trimmed.contains('|')
}

/// Scans document lines and identifies all complete Markdown tables [start..=end].
pub fn find_tables(lines: &[Line], code_blocks: &[CodeBlockRange]) -> Vec<TableRange> {
    let mut ranges = Vec::new();
    let mut i = 0;

    while i + 1 < lines.len() {
        if code_blocks.iter().any(|cb| i >= cb.start && i <= cb.end) {
            i += 1;
            continue;
        }

        if is_potential_table_row(&lines[i].raw)
            && is_table_delimiter(&lines[i + 1].raw)
            && !is_table_delimiter(&lines[i].raw)
        {
            let start = i;
            let mut end = i + 1;

            let mut j = i + 2;
            while j < lines.len() {
                if code_blocks.iter().any(|cb| j >= cb.start && j <= cb.end) {
                    break;
                }
                if is_potential_table_row(&lines[j].raw) {
                    end = j;
                    j += 1;
                } else {
                    break;
                }
            }

            ranges.push(TableRange { start, end });
            i = end + 1;
        } else {
            i += 1;
        }
    }

    ranges
}

/// Parsed metadata for a shields.io badge.
#[derive(Debug, Clone, PartialEq)]
pub struct BadgeInfo {
    pub label: String,
    pub message: String,
    pub color: Color,
    pub logo: Option<String>,
    pub link: Option<String>,
}

/// Decodes percent-encoded and shields.io escaped strings (`%20` -> space, `--` -> `-`, `_` -> space).
pub fn decode_badge_str(s: &str) -> String {
    let replaced = s.replace("--", "\u{0000}").replace('_', " ");
    let mut bytes = Vec::new();
    let mut chars = replaced.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(c1), Some(c2)) = (h1, h2) {
                let hex_str = format!("{c1}{c2}");
                if let Ok(b) = u8::from_str_radix(&hex_str, 16) {
                    bytes.push(b);
                    continue;
                }
            }
        }
        for b in c.to_string().as_bytes() {
            bytes.push(*b);
        }
    }
    let decoded = String::from_utf8_lossy(&bytes).to_string();
    decoded.replace('\u{0000}', "-")
}

/// Parses a shields.io color string into an iced `Color`.
pub fn parse_badge_color(color_str: &str) -> Color {
    let clean = color_str.trim().trim_start_matches('#').to_lowercase();
    match clean.as_str() {
        "brightgreen" => Color::from_rgb8(76, 201, 71), // #4c1
        "green" => Color::from_rgb8(68, 204, 17),       // #44cc11
        "yellowgreen" => Color::from_rgb8(151, 202, 0), // #97ca00
        "yellow" => Color::from_rgb8(223, 179, 23),     // #dfb317
        "orange" => Color::from_rgb8(254, 125, 55),     // #fe7d37
        "red" => Color::from_rgb8(224, 93, 68),         // #e05d44
        "blue" => Color::from_rgb8(0, 126, 198),        // #007ec6
        "lightgrey" | "lightgray" => Color::from_rgb8(159, 159, 159),
        "purple" | "blueviolet" => Color::from_rgb8(138, 43, 226), // #8a2be2
        "success" => Color::from_rgb8(46, 164, 79),
        "important" => Color::from_rgb8(254, 125, 55),
        "critical" => Color::from_rgb8(224, 93, 68),
        "informational" => Color::from_rgb8(0, 126, 198),
        "inactive" => Color::from_rgb8(159, 159, 159),
        hex if hex.len() == 6 => {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                Color::from_rgb8(r, g, b)
            } else {
                Color::from_rgb8(101, 109, 118)
            }
        }
        hex if hex.len() == 3 => {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..1], 16),
                u8::from_str_radix(&hex[1..2], 16),
                u8::from_str_radix(&hex[2..3], 16),
            ) {
                Color::from_rgb8(r * 17, g * 17, b * 17)
            } else {
                Color::from_rgb8(101, 109, 118)
            }
        }
        _ => Color::from_rgb8(101, 109, 118),
    }
}

/// Parses a shields.io badge URL into a `BadgeInfo`.
pub fn parse_shield_badge_url(url: &str, link: Option<String>) -> Option<BadgeInfo> {
    if !url.contains("shields.io/") {
        return None;
    }

    let (path_part, query_part) = match url.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (url, None),
    };

    let mut logo = None;
    if let Some(query) = query_part {
        for param in query.split('&') {
            if let Some((k, v)) = param.split_once('=')
                && k == "logo"
            {
                logo = Some(v.to_lowercase());
            }
        }
    }

    if let Some(badge_idx) = path_part.find("/badge/") {
        let slug = &path_part[badge_idx + "/badge/".len()..];
        let protected = slug.replace("--", "\u{1}");
        let parts: Vec<&str> = protected.split('-').collect();
        if parts.len() >= 2 {
            let color_str = parts.last().unwrap().replace('\u{1}', "-");
            let label_str = parts[0].replace('\u{1}', "-");
            let message_str = if parts.len() == 2 {
                color_str.clone()
            } else {
                parts[1..parts.len() - 1].join("-").replace('\u{1}', "-")
            };

            let color = parse_badge_color(&color_str);
            let label = decode_badge_str(&label_str);
            let message = decode_badge_str(&message_str);

            return Some(BadgeInfo {
                label,
                message,
                color,
                logo,
                link,
            });
        }
    } else if path_part.contains("/static/v1") && query_part.is_some() {
        let mut label = String::new();
        let mut message = String::new();
        let mut color_str = String::from("gray");

        if let Some(query) = query_part {
            for param in query.split('&') {
                if let Some((k, v)) = param.split_once('=') {
                    match k {
                        "label" => label = decode_badge_str(v),
                        "message" => message = decode_badge_str(v),
                        "color" => color_str = v.to_string(),
                        _ => {}
                    }
                }
            }
        }

        let color = parse_badge_color(&color_str);
        return Some(BadgeInfo {
            label,
            message,
            color,
            logo,
            link,
        });
    }

    None
}

/// Extracts all shields.io badges from a line of text.
pub fn extract_badges_from_line(raw: &str) -> Vec<BadgeInfo> {
    let mut badges = Vec::new();
    let mut cur = raw;

    while let Some(shields_idx) = cur.find("shields.io/") {
        let prefix = &cur[..shields_idx];
        let url_start = prefix
            .rfind(|c: char| c == '(' || c == '"' || c == '\'' || c == '<' || c.is_whitespace())
            .map_or(0, |pos| pos + 1);

        let after_start = &cur[url_start..];
        let url_end = after_start
            .find(|c: char| c == ')' || c == '"' || c == '\'' || c == '>' || c.is_whitespace())
            .unwrap_or(after_start.len());

        let badge_url = &after_start[..url_end];
        let after_url = &after_start[url_end..];
        let mut link = None;
        let mut advance_idx = url_start + url_end;

        if let Some(link_part) = after_url.strip_prefix("](")
            && let Some(link_end) = link_part.find(')')
        {
            let candidate_link = link_part[..link_end].trim();
            if !candidate_link.is_empty() {
                link = Some(candidate_link.to_string());
            }
            advance_idx += 2 + link_end + 1;
        }

        if let Some(badge) = parse_shield_badge_url(badge_url, link) {
            badges.push(badge);
        }

        if advance_idx <= shields_idx {
            cur = &cur[shields_idx + "shields.io/".len()..];
        } else {
            cur = &cur[advance_idx..];
        }
    }

    badges
}

/// Checks if a line contains only shields.io badge links and whitespace/tags.
pub fn is_pure_badge_line(raw: &str) -> bool {
    let trimmed = raw.trim();
    if !trimmed.contains("shields.io/") {
        return false;
    }
    let mut cleaned = String::new();
    let mut in_link_target = false;
    let mut in_tag = false;
    for c in trimmed.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if in_tag {
            // inside html tag
        } else if c == '(' {
            in_link_target = true;
        } else if c == ')' {
            in_link_target = false;
        } else if in_link_target {
            // inside URL target
        } else if c == '[' || c == ']' || c == '!' {
            // markdown formatting characters
        } else if c.is_whitespace() {
            // whitespace
        } else {
            cleaned.push(c);
        }
    }
    let without_nbsp = cleaned.replace("&nbsp;", "");
    without_nbsp.is_empty()
}

/// Represents a contiguous range of lines that are pure shields.io badges [start..=end].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeBlockRange {
    pub start: usize,
    pub end: usize,
}

/// Scans document lines and identifies contiguous groups of badge-only lines.
pub fn find_badge_blocks(
    lines: &[Line],
    code_blocks: &[CodeBlockRange],
    tables: &[TableRange],
) -> Vec<BadgeBlockRange> {
    let mut ranges = Vec::new();
    let mut current_start: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        let is_in_cb = code_blocks
            .iter()
            .any(|cb| idx >= cb.start && idx <= cb.end);
        let is_in_tbl = tables.iter().any(|tb| idx >= tb.start && idx <= tb.end);

        if !is_in_cb && !is_in_tbl && is_pure_badge_line(&line.raw) {
            if current_start.is_none() {
                current_start = Some(idx);
            }
        } else if let Some(start) = current_start {
            ranges.push(BadgeBlockRange {
                start,
                end: idx - 1,
            });
            current_start = None;
        }
    }

    if let Some(start) = current_start {
        ranges.push(BadgeBlockRange {
            start,
            end: lines.len() - 1,
        });
    }

    ranges
}

/// Parses an image reference from a line (Obsidian wikilink `![[...]]`, Markdown `![...](...)`, or `<img ... />`).
pub fn parse_image_reference(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    // 1. Obsidian wikilink image: ![[filename.png]] or ![[filename.png|300]]
    if trimmed.starts_with("![[") && trimmed.ends_with("]]") {
        let inner = &trimmed[3..trimmed.len() - 2];
        let file_part = inner.split('|').next().unwrap_or(inner).trim();
        return Some((file_part.to_string(), format!("Attachment • {file_part}")));
    }

    // 2. Standard markdown image: ![alt](url)
    if trimmed.starts_with("![")
        && trimmed.ends_with(')')
        && let Some(close_bracket) = trimmed.find("](")
    {
        let alt = &trimmed[2..close_bracket];
        let url = &trimmed[close_bracket + 2..trimmed.len() - 1];
        if !url.contains("shields.io") {
            let display_title = if alt.trim().is_empty() {
                url.split('/').next_back().unwrap_or("Image")
            } else {
                alt
            };
            return Some((display_title.to_string(), url.to_string()));
        }
    }

    // 3. HTML img: <img ... />
    if trimmed.starts_with("<img ")
        && trimmed.ends_with('>')
        && let Some(src_start) = trimmed.find("src=\"")
    {
        let after_src = &trimmed[src_start + 5..];
        if let Some(src_end) = after_src.find('"') {
            let src = &after_src[..src_end];
            if !src.contains("shields.io") {
                return Some(("Embedded Image".to_string(), src.to_string()));
            }
        }
    }

    None
}

/// Renders a single shields.io badge as a two-tone colored vector pill widget.
pub fn render_badge(badge: &BadgeInfo) -> iced::Element<'static, Message> {
    let logo_text = if let Some(ref l) = badge.logo {
        match l.as_str() {
            "rust" => "🦀 ",
            "github" => "🐙 ",
            _ => "",
        }
    } else if badge.label.to_lowercase() == "linguagem" && badge.message.to_lowercase() == "rust" {
        "⚙ "
    } else {
        ""
    };

    let label_widget = container(
        row![
            if !logo_text.is_empty() {
                text(logo_text).size(10).color(Color::WHITE)
            } else {
                text("").size(0)
            },
            text(badge.label.clone())
                .size(11)
                .color(Color::WHITE)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                }),
        ]
        .spacing(3)
        .align_y(alignment::Vertical::Center),
    )
    .padding([3, 7])
    .style(|_theme| container::Style {
        background: Some(Background::Color(Color::from_rgb8(85, 85, 85))), // #555555
        ..Default::default()
    });

    let bg_color = badge.color;
    let message_widget = container(
        text(badge.message.clone())
            .size(11)
            .color(Color::WHITE)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::DEFAULT
            }),
    )
    .padding([3, 7])
    .style(move |_theme| container::Style {
        background: Some(Background::Color(bg_color)),
        ..Default::default()
    });

    let badge_pill = container(
        row![label_widget, message_widget]
            .spacing(0)
            .align_y(alignment::Vertical::Center),
    )
    .style(|_theme| container::Style {
        border: Border::default().rounded(3.5),
        ..Default::default()
    });

    if let Some(ref link) = badge.link {
        let uri = markdown::Uri::from(link.clone());
        mouse_area(badge_pill)
            .interaction(mouse::Interaction::Pointer)
            .on_press(Message::LinkClicked(uri))
            .boxed()
    } else {
        badge_pill.boxed()
    }
}

/// Renders a clean GitHub Light card for local or remote image attachments.
pub fn render_image_card(title: &str, target: &str) -> iced::Element<'static, Message> {
    container(
        row![
            icons::image().size(20).color(Color::from_rgb8(9, 105, 218)),
            column![
                text(title.to_string())
                    .size(13)
                    .color(Color::from_rgb8(31, 35, 40))
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Font::DEFAULT
                    }),
                text(target.to_string())
                    .size(11)
                    .color(Color::from_rgb8(101, 109, 118)),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([8, 14])
    .style(|_theme| container::Style {
        background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
        border: Border::default()
            .rounded(6.0)
            .color(Color::from_rgb8(208, 215, 222))
            .width(1.0),
        ..Default::default()
    })
    .boxed()
}

/// A discrete line in the document with cached parsed Markdown items.
#[derive(Debug, Clone)]
pub struct Line {
    /// Raw text of the line.
    pub raw: String,
    /// Pre-parsed Markdown items for zero-cost view rendering.
    pub items: Vec<markdown::Item>,
    /// Pre-parsed code block Markdown items if this line starts a closed code block.
    pub code_block_items: Option<Vec<markdown::Item>>,
    /// Pre-parsed table Markdown items if this line starts a Markdown table.
    pub table_items: Option<Vec<markdown::Item>>,
}

impl Line {
    /// Creates a new line and parses its Markdown items.
    pub fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let items = markdown::parse(&raw).collect();
        Self {
            raw,
            items,
            code_block_items: None,
            table_items: None,
        }
    }

    /// Updates the line text and regenerates pre-parsed items.
    pub fn update(&mut self, new_raw: String) {
        self.items = markdown::parse(&new_raw).collect();
        self.raw = new_raw;
        self.code_block_items = None;
        self.table_items = None;
    }
}

/// A document tab in the editor.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Unique tab identifier.
    pub id: usize,
    /// Tab display title.
    pub title: String,
    /// Optional backing file path on disk.
    pub file_path: Option<PathBuf>,
    /// Lines of the document.
    pub lines: Vec<Line>,
    /// Index of the currently active line showing raw Markdown.
    pub active_line: Option<usize>,
    /// Buffer for the active line editor.
    pub active_editor: text_editor::Content,
    /// Additional multi-line cursors `(line_index, char_column)` created via Ctrl+Alt+Up/Down.
    pub extra_cursors: Vec<(usize, usize)>,
}

impl Tab {
    /// Creates a new document tab from title, Markdown content, and optional file path.
    pub fn new(id: usize, title: String, content: &str, file_path: Option<PathBuf>) -> Self {
        let lines: Vec<Line> = if content.is_empty() {
            vec![Line::new("")]
        } else {
            content
                .split('\n')
                .map(|s| Line::new(s.trim_end_matches('\r')))
                .collect()
        };

        let mut tab = Self {
            id,
            title,
            file_path,
            lines,
            active_line: None,
            active_editor: text_editor::Content::new(),
            extra_cursors: Vec::new(),
        };
        tab.refresh_code_blocks();
        tab
    }

    /// Scans document lines for fenced code blocks and tables, parses them, and caches items at their start lines.
    pub fn refresh_document_blocks(&mut self) {
        let code_blocks = find_code_blocks(&self.lines);
        for line in &mut self.lines {
            line.code_block_items = None;
            line.table_items = None;
        }
        for range in &code_blocks {
            let block_text = self.lines[range.start..=range.end]
                .iter()
                .map(|l| l.raw.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let items: Vec<markdown::Item> = markdown::parse(&block_text).collect();
            if let Some(first_line) = self.lines.get_mut(range.start) {
                first_line.code_block_items = Some(items);
            }
        }

        let tables = find_tables(&self.lines, &code_blocks);
        for range in tables {
            let table_text = self.lines[range.start..=range.end]
                .iter()
                .map(|l| l.raw.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let items: Vec<markdown::Item> = markdown::parse(&table_text).collect();
            if let Some(first_line) = self.lines.get_mut(range.start) {
                first_line.table_items = Some(items);
            }
        }
    }

    /// Legacy alias forwarding to `refresh_document_blocks`.
    #[inline]
    pub fn refresh_code_blocks(&mut self) {
        self.refresh_document_blocks();
    }

    /// Selects a line to edit, committing any previously active line and clearing extra cursors.
    pub fn select_line(&mut self, idx: usize) {
        self.extra_cursors.clear();
        if self.active_line == Some(idx) {
            return;
        }
        self.commit_active_line();
        if idx < self.lines.len() {
            self.active_line = Some(idx);
            self.active_editor = text_editor::Content::with_text(&self.lines[idx].raw);
        }
    }

    /// Selects a line and places the cursor at the specified character column.
    pub fn select_line_at_char(&mut self, idx: usize, char_col: usize) {
        self.extra_cursors.clear();
        self.commit_active_line();
        let clamped_idx = idx.min(self.lines.len().saturating_sub(1));
        if clamped_idx < self.lines.len() {
            self.active_line = Some(clamped_idx);
            let raw = &self.lines[clamped_idx].raw;
            self.active_editor = text_editor::Content::with_text(raw);
            let byte_col = char_to_byte_idx(raw, char_col);
            self.active_editor.move_to(text_editor::Cursor {
                position: text::Position {
                    line: 0,
                    index: byte_col,
                },
                selection: None,
            });
        }
    }

    /// Commits the active line editor back into the lines list.
    pub fn commit_active_line(&mut self) {
        self.extra_cursors.clear();
        if let Some(idx) = self.active_line.take()
            && idx < self.lines.len()
        {
            let text = self.active_editor.text();
            if text.contains('\n') {
                let split: Vec<String> = text
                    .split('\n')
                    .map(|s| s.trim_end_matches('\r').to_string())
                    .collect();
                if split.is_empty() {
                    self.lines[idx].update(String::new());
                } else {
                    self.lines.remove(idx);
                    for (offset, l) in split.into_iter().enumerate() {
                        self.lines.insert(idx + offset, Line::new(l));
                    }
                }
            } else {
                self.lines[idx].update(text);
            }
            self.refresh_code_blocks();
        }
    }

    /// Returns the character column of the primary cursor on the active line.
    pub fn active_char_col(&self) -> usize {
        let text = self.active_editor.text();
        let byte_idx = self.active_editor.cursor().position.index.min(text.len());
        text[..byte_idx].chars().count()
    }

    /// Applies an editor action to all secondary multi-cursors.
    pub fn apply_to_extra_cursors(&mut self, action: &text_editor::Action) {
        if self.extra_cursors.is_empty() {
            return;
        }

        for (line_idx, char_col) in &mut self.extra_cursors {
            let Some(line) = self.lines.get_mut(*line_idx) else {
                continue;
            };
            let mut raw = line.raw.clone();
            let char_len = raw.chars().count();
            let col = (*char_col).min(char_len);

            match action {
                text_editor::Action::Edit(edit) => match edit {
                    text_editor::Edit::Insert(c) => {
                        let byte_idx = char_to_byte_idx(&raw, col);
                        raw.insert(byte_idx, *c);
                        *char_col = col + 1;
                        line.update(raw);
                    }
                    text_editor::Edit::Paste(pasted) => {
                        let clean = pasted.replace(['\r', '\n'], " ");
                        let byte_idx = char_to_byte_idx(&raw, col);
                        raw.insert_str(byte_idx, &clean);
                        *char_col = col + clean.chars().count();
                        line.update(raw);
                    }
                    text_editor::Edit::Backspace => {
                        if col > 0 {
                            let byte_idx = char_to_byte_idx(&raw, col - 1);
                            raw.remove(byte_idx);
                            *char_col = col - 1;
                            line.update(raw);
                        }
                    }
                    text_editor::Edit::Delete => {
                        if col < char_len {
                            let byte_idx = char_to_byte_idx(&raw, col);
                            raw.remove(byte_idx);
                            *char_col = col;
                            line.update(raw);
                        }
                    }
                    text_editor::Edit::BackspaceWord => {
                        if col > 0 {
                            let chars: Vec<char> = raw.chars().collect();
                            let mut new_col = col;
                            while new_col > 0 && chars[new_col - 1].is_whitespace() {
                                new_col -= 1;
                            }
                            while new_col > 0 && !chars[new_col - 1].is_whitespace() {
                                new_col -= 1;
                            }
                            let start_byte = char_to_byte_idx(&raw, new_col);
                            let end_byte = char_to_byte_idx(&raw, col);
                            raw.replace_range(start_byte..end_byte, "");
                            *char_col = new_col;
                            line.update(raw);
                        }
                    }
                    text_editor::Edit::DeleteWord => {
                        if col < char_len {
                            let chars: Vec<char> = raw.chars().collect();
                            let mut end_col = col;
                            while end_col < char_len && chars[end_col].is_whitespace() {
                                end_col += 1;
                            }
                            while end_col < char_len && !chars[end_col].is_whitespace() {
                                end_col += 1;
                            }
                            let start_byte = char_to_byte_idx(&raw, col);
                            let end_byte = char_to_byte_idx(&raw, end_col);
                            raw.replace_range(start_byte..end_byte, "");
                            *char_col = col;
                            line.update(raw);
                        }
                    }
                    text_editor::Edit::BackspaceLine => {
                        let end_byte = char_to_byte_idx(&raw, col);
                        raw.replace_range(..end_byte, "");
                        *char_col = 0;
                        line.update(raw);
                    }
                    text_editor::Edit::DeleteLine => {
                        let start_byte = char_to_byte_idx(&raw, col);
                        raw.replace_range(start_byte.., "");
                        *char_col = col;
                        line.update(raw);
                    }
                    _ => {}
                },
                text_editor::Action::Move(motion) => match motion {
                    text_editor::Motion::Left => {
                        *char_col = col.saturating_sub(1);
                    }
                    text_editor::Motion::Right => {
                        *char_col = (col + 1).min(char_len);
                    }
                    text_editor::Motion::Home | text_editor::Motion::DocumentStart => {
                        *char_col = 0;
                    }
                    text_editor::Motion::End | text_editor::Motion::DocumentEnd => {
                        *char_col = char_len;
                    }
                    text_editor::Motion::WordLeft => {
                        let chars: Vec<char> = raw.chars().collect();
                        let mut new_col = col;
                        while new_col > 0 && chars[new_col - 1].is_whitespace() {
                            new_col -= 1;
                        }
                        while new_col > 0 && !chars[new_col - 1].is_whitespace() {
                            new_col -= 1;
                        }
                        *char_col = new_col;
                    }
                    text_editor::Motion::WordRight => {
                        let chars: Vec<char> = raw.chars().collect();
                        let mut new_col = col;
                        while new_col < char_len && !chars[new_col].is_whitespace() {
                            new_col += 1;
                        }
                        while new_col < char_len && chars[new_col].is_whitespace() {
                            new_col += 1;
                        }
                        *char_col = new_col;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    /// Returns the full reconstructed document text.
    pub fn full_text(&self) -> String {
        let mut raws: Vec<String> = self.lines.iter().map(|l| l.raw.clone()).collect();
        if let Some(idx) = self.active_line
            && idx < raws.len()
        {
            raws[idx] = self.active_editor.text();
        }
        raws.join("\n")
    }
}

/// The Doggy application state.
pub struct Doggy {
    /// Open tabs.
    tabs: Vec<Tab>,
    /// Index of the active tab.
    active_tab_index: usize,
    /// Next ID for new tabs.
    next_tab_id: usize,
    /// The runtime window ID.
    window_id: Option<window::Id>,
}

/// Messages handled by Doggy.
#[derive(Debug, Clone)]
pub enum Message {
    /// Window ID resolved upon open or discovery.
    WindowOpened(window::Id),
    /// Drag the window via custom top bar.
    DragWindow,
    /// Toggle window maximize / restore.
    ToggleMaximize,
    /// Minimize window.
    MinimizeWindow,
    /// Close window and exit app.
    CloseWindow,
    /// Select a tab by index.
    SelectTab(usize),
    /// Close a tab by index.
    CloseTab(usize),
    /// Create a new empty tab.
    NewTab,
    /// Select a line within the active document to show raw Markdown.
    SelectLine(usize),
    /// Deselect the active line and render all lines.
    DeselectLine,
    /// An action within the active line editor.
    EditorAction(text_editor::Action),
    /// Insert a new empty line below the current active line.
    InsertLineBelow,
    /// Backspace pressed at the start of a line (merges with previous line).
    BackspaceAtLineStart,
    /// Navigate cursor to the line above.
    NavigateLineUp,
    /// Navigate cursor to the line below.
    NavigateLineDown,
    /// Ctrl + D: Duplicate the selected text or the current line(s).
    DuplicateLineOrSelection,
    /// Alt + Up: Move the current line or selection up.
    MoveLineUp,
    /// Alt + Down: Move the current line or selection down.
    MoveLineDown,
    /// Ctrl + A: Select the entire document text as raw Markdown.
    SelectAllMarkdown,
    /// Ctrl + Alt + Up: Add a multi-line cursor on the line above.
    AddCursorAbove,
    /// Ctrl + Alt + Down: Add a multi-line cursor on the line below.
    AddCursorBelow,
    /// Ctrl + S: Save the active document if it has a file path.
    SaveActiveTab,
    /// Clicked in the empty area below the document text.
    ClickBottomArea,
    /// Clicked an interactive link in the rendered preview.
    LinkClicked(markdown::Uri),
    /// Ignored keyboard events caught at the window level.
    GlobalKeyEvent(keyboard::Event),
}

impl Doggy {
    /// Boots the application, opening CLI file arguments if provided or an empty Markdown file otherwise.
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut tabs = Vec::new();
        let mut next_id = 1;

        for arg in args {
            let path = PathBuf::from(&arg);
            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&arg)
                .to_string();

            let content = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    tracing::info!("Opening '{arg}' as new/empty file ({err})");
                    String::new()
                }
            };

            tabs.push(Tab::new(next_id, title, &content, Some(path)));
            next_id += 1;
        }

        if tabs.is_empty() {
            tabs.push(Tab::new(next_id, "Untitled.md".to_string(), "", None));
            next_id += 1;
        }

        let app = Self {
            tabs,
            active_tab_index: 0,
            next_tab_id: next_id,
            window_id: None,
        };

        let init_task = Task::batch([
            widget::operation::focus(ACTIVE_EDITOR_ID),
            window::latest().map(|opt| match opt {
                Some(id) => Message::WindowOpened(id),
                None => Message::SelectTab(0),
            }),
        ]);

        (app, init_task)
    }

    /// Subscription for global keyboard shortcuts and window open events.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::listen().map(Message::GlobalKeyEvent),
            window::open_events().map(Message::WindowOpened),
        ])
    }

    /// Handles application messages and updates editor/tabs state.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window_id = Some(id);
            }
            Message::DragWindow => {
                if let Some(id) = self.window_id {
                    return window::drag(id);
                } else {
                    return window::latest().then(|opt| {
                        if let Some(id) = opt {
                            window::drag(id)
                        } else {
                            Task::none()
                        }
                    });
                }
            }
            Message::ToggleMaximize => {
                if let Some(id) = self.window_id {
                    return window::toggle_maximize(id);
                } else {
                    return window::latest().then(|opt| {
                        if let Some(id) = opt {
                            window::toggle_maximize(id)
                        } else {
                            Task::none()
                        }
                    });
                }
            }
            Message::MinimizeWindow => {
                if let Some(id) = self.window_id {
                    return window::minimize(id, true);
                } else {
                    return window::latest().then(|opt| {
                        if let Some(id) = opt {
                            window::minimize(id, true)
                        } else {
                            Task::none()
                        }
                    });
                }
            }
            Message::CloseWindow => {
                return iced::exit();
            }
            Message::SelectTab(idx) => {
                if idx < self.tabs.len() && idx != self.active_tab_index {
                    if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
                        current.commit_active_line();
                    }
                    self.active_tab_index = idx;
                    if let Some(next_doc) = self.tabs.get_mut(self.active_tab_index)
                        && next_doc.active_line.is_none()
                    {
                        next_doc.select_line(0);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::CloseTab(idx) => {
                if self.tabs.len() <= 1 {
                    let next_id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs[0] = Tab::new(next_id, "Untitled.md".to_string(), "", None);
                    self.active_tab_index = 0;
                } else {
                    self.tabs.remove(idx);
                    if self.active_tab_index >= self.tabs.len() {
                        self.active_tab_index = self.tabs.len() - 1;
                    }
                }
                return widget::operation::focus(ACTIVE_EDITOR_ID);
            }
            Message::NewTab => {
                if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
                    current.commit_active_line();
                }
                let tab_id = self.next_tab_id;
                self.next_tab_id += 1;
                let title = format!("Untitled-{tab_id}.md");
                let new_tab = Tab::new(tab_id, title, "", None);
                self.tabs.push(new_tab);
                self.active_tab_index = self.tabs.len() - 1;
                return widget::operation::focus(ACTIVE_EDITOR_ID);
            }
            Message::SelectLine(idx) => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.select_line(idx);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::DeselectLine => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                }
            }
            Message::EditorAction(action) => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    let is_edit = action.is_edit();
                    doc.apply_to_extra_cursors(&action);
                    doc.active_editor.perform(action);

                    // If editor has multi-line text (e.g. from pasting or Ctrl+A) without active selection,
                    // split back into individual document lines immediately.
                    let editor_text = doc.active_editor.text();
                    if editor_text.contains('\n') && doc.active_editor.cursor().selection.is_none()
                    {
                        let cur_pos = doc.active_editor.cursor().position;
                        let split_lines: Vec<Line> = editor_text
                            .split('\n')
                            .map(|s| Line::new(s.trim_end_matches('\r')))
                            .collect();
                        if let Some(idx) = doc.active_line
                            && idx < doc.lines.len()
                        {
                            let target_line = cur_pos.line.min(split_lines.len().saturating_sub(1));
                            let target_byte = cur_pos
                                .index
                                .min(split_lines.get(target_line).map_or(0, |l| l.raw.len()));
                            let target_char = split_lines
                                .get(target_line)
                                .map_or(0, |l| l.raw[..target_byte].chars().count());

                            doc.lines.remove(idx);
                            for (offset, line) in split_lines.into_iter().enumerate() {
                                doc.lines.insert(idx + offset, line);
                            }
                            doc.refresh_code_blocks();
                            let target_idx = idx + target_line;
                            doc.active_line = None;
                            doc.select_line_at_char(target_idx, target_char);
                            return widget::operation::focus(ACTIVE_EDITOR_ID);
                        }
                    }

                    if is_edit
                        && let Some(idx) = doc.active_line
                        && idx < doc.lines.len()
                    {
                        doc.lines[idx].update(doc.active_editor.text());
                        doc.refresh_code_blocks();
                    }
                }
            }
            Message::InsertLineBelow => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if !doc.extra_cursors.is_empty() {
                        doc.extra_cursors.clear();
                    }
                    if let Some(idx) = doc.active_line
                        && idx < doc.lines.len()
                    {
                        // Split the line at the current cursor position
                        let text = doc.active_editor.text();
                        let cursor = doc.active_editor.cursor();
                        let (start_byte, end_byte) = match cursor.selection {
                            Some(sel) => {
                                let p1 = editor_pos_to_byte_offset(&text, cursor.position);
                                let p2 = editor_pos_to_byte_offset(&text, sel);
                                (p1.min(p2).min(text.len()), p1.max(p2).min(text.len()))
                            }
                            None => {
                                let b = editor_pos_to_byte_offset(&text, cursor.position)
                                    .min(text.len());
                                (b, b)
                            }
                        };
                        let before = text[..start_byte].to_string();
                        let after = text[end_byte..].to_string();

                        doc.lines[idx].update(before);
                        let new_idx = idx + 1;
                        doc.lines.insert(new_idx, Line::new(after));
                        doc.refresh_code_blocks();
                        doc.active_line = None;
                        doc.select_line_at_char(new_idx, 0);
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::BackspaceAtLineStart => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && let Some(idx) = doc.active_line
                    && idx > 0
                    && idx < doc.lines.len()
                {
                    let current_text = doc.active_editor.text();
                    let prev_chars = doc.lines[idx - 1].raw.chars().count();
                    let merged = format!("{}{}", doc.lines[idx - 1].raw, current_text);
                    doc.lines[idx - 1].update(merged);
                    doc.lines.remove(idx);
                    doc.refresh_code_blocks();
                    doc.active_line = None;
                    doc.select_line_at_char(idx - 1, prev_chars);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::NavigateLineUp => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && let Some(idx) = doc.active_line
                    && idx > 0
                {
                    let char_col = doc.active_char_col();
                    doc.select_line_at_char(idx - 1, char_col);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::NavigateLineDown => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && let Some(idx) = doc.active_line
                    && idx + 1 < doc.lines.len()
                {
                    let char_col = doc.active_char_col();
                    doc.select_line_at_char(idx + 1, char_col);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::DuplicateLineOrSelection => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if doc.active_line.is_none() {
                        doc.select_line(0);
                    }

                    // Case 1: There is selected text -> duplicate the selection in-place
                    if let Some(selected_text) = doc.active_editor.selection()
                        && !selected_text.is_empty()
                    {
                        let cursor = doc.active_editor.cursor();
                        let end_pos = match cursor.selection {
                            Some(sel) => cursor.position.max(sel),
                            None => cursor.position,
                        };
                        doc.active_editor.move_to(text_editor::Cursor {
                            position: end_pos,
                            selection: None,
                        });
                        doc.active_editor.perform(text_editor::Action::Edit(
                            text_editor::Edit::Paste(Arc::new(selected_text)),
                        ));
                        let new_end = doc.active_editor.cursor().position;
                        doc.active_editor.move_to(text_editor::Cursor {
                            position: new_end,
                            selection: Some(end_pos),
                        });
                        if let Some(idx) = doc.active_line
                            && idx < doc.lines.len()
                        {
                            doc.lines[idx].update(doc.active_editor.text());
                            doc.refresh_code_blocks();
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }

                    // Case 2: No selection -> duplicate the active line
                    if let Some(idx) = doc.active_line
                        && idx < doc.lines.len()
                    {
                        let char_col = doc.active_char_col();
                        let current_text = doc.active_editor.text();
                        doc.lines[idx].update(current_text.clone());
                        let new_idx = idx + 1;
                        doc.lines.insert(new_idx, Line::new(current_text));
                        doc.refresh_code_blocks();
                        doc.active_line = None;
                        doc.select_line_at_char(new_idx, char_col);
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::MoveLineUp => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    // If in multi-line buffer, split lines first
                    if doc.lines.len() == 1 && doc.active_editor.line_count() > 1 {
                        let cur = doc.active_editor.cursor();
                        let l1 = cur.position.line;
                        let l2 = cur.selection.map_or(l1, |s| s.line);
                        let min_l = l1.min(l2);
                        let max_l = l1.max(l2);
                        doc.commit_active_line();
                        doc.select_line(min_l);
                        for l in (min_l + 1)..=max_l {
                            if l < doc.lines.len() {
                                doc.extra_cursors.push((l, 0));
                            }
                        }
                    }

                    if let Some(active_idx) = doc.active_line
                        && active_idx < doc.lines.len()
                    {
                        let cur = doc.active_editor.cursor();
                        doc.lines[active_idx].update(doc.active_editor.text());

                        let mut indices: Vec<usize> = std::iter::once(active_idx)
                            .chain(doc.extra_cursors.iter().map(|(l, _)| *l))
                            .filter(|&l| l < doc.lines.len())
                            .collect();
                        indices.sort_unstable();
                        indices.dedup();

                        if let Some(&first) = indices.first()
                            && first > 0
                        {
                            for &i in &indices {
                                doc.lines.swap(i, i - 1);
                            }
                            doc.refresh_code_blocks();
                            let new_active = active_idx - 1;
                            doc.active_line = Some(new_active);
                            for (line_idx, _) in &mut doc.extra_cursors {
                                *line_idx -= 1;
                            }
                            doc.active_editor =
                                text_editor::Content::with_text(&doc.lines[new_active].raw);
                            doc.active_editor.move_to(cur);
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::MoveLineDown => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    // If in multi-line buffer, split lines first
                    if doc.lines.len() == 1 && doc.active_editor.line_count() > 1 {
                        let cur = doc.active_editor.cursor();
                        let l1 = cur.position.line;
                        let l2 = cur.selection.map_or(l1, |s| s.line);
                        let min_l = l1.min(l2);
                        let max_l = l1.max(l2);
                        doc.commit_active_line();
                        doc.select_line(min_l);
                        for l in (min_l + 1)..=max_l {
                            if l < doc.lines.len() {
                                doc.extra_cursors.push((l, 0));
                            }
                        }
                    }

                    if let Some(active_idx) = doc.active_line
                        && active_idx < doc.lines.len()
                    {
                        let cur = doc.active_editor.cursor();
                        doc.lines[active_idx].update(doc.active_editor.text());

                        let mut indices: Vec<usize> = std::iter::once(active_idx)
                            .chain(doc.extra_cursors.iter().map(|(l, _)| *l))
                            .filter(|&l| l < doc.lines.len())
                            .collect();
                        indices.sort_unstable();
                        indices.dedup();

                        if let Some(&last) = indices.last()
                            && last + 1 < doc.lines.len()
                        {
                            for &i in indices.iter().rev() {
                                doc.lines.swap(i, i + 1);
                            }
                            doc.refresh_code_blocks();
                            let new_active = active_idx + 1;
                            doc.active_line = Some(new_active);
                            for (line_idx, _) in &mut doc.extra_cursors {
                                *line_idx += 1;
                            }
                            doc.active_editor =
                                text_editor::Content::with_text(&doc.lines[new_active].raw);
                            doc.active_editor.move_to(cur);
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::SelectAllMarkdown => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    let full = doc.full_text();
                    doc.extra_cursors.clear();
                    doc.lines = vec![Line::new(&full)];
                    doc.refresh_code_blocks();
                    doc.active_line = Some(0);
                    doc.active_editor = text_editor::Content::with_text(&full);
                    doc.active_editor.perform(text_editor::Action::SelectAll);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::AddCursorAbove => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if doc.lines.len() == 1 && doc.active_editor.line_count() > 1 {
                        let cur_line = doc.active_editor.cursor().position.line;
                        let cur_col = doc.active_char_col();
                        doc.commit_active_line();
                        doc.select_line_at_char(cur_line, cur_col);
                    }
                    if doc.active_line.is_none() {
                        doc.select_line(0);
                    }
                    if let Some(idx) = doc.active_line {
                        let char_col = doc.active_char_col();
                        let min_line = doc
                            .extra_cursors
                            .iter()
                            .map(|(l, _)| *l)
                            .min()
                            .unwrap_or(idx)
                            .min(idx);
                        if min_line > 0 {
                            let target = min_line - 1;
                            let target_col = char_col.min(doc.lines[target].raw.chars().count());
                            if !doc.extra_cursors.iter().any(|(l, _)| *l == target) {
                                doc.extra_cursors.push((target, target_col));
                                doc.extra_cursors.sort_by_key(|(l, _)| *l);
                            }
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::AddCursorBelow => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if doc.lines.len() == 1 && doc.active_editor.line_count() > 1 {
                        let cur_line = doc.active_editor.cursor().position.line;
                        let cur_col = doc.active_char_col();
                        doc.commit_active_line();
                        doc.select_line_at_char(cur_line, cur_col);
                    }
                    if doc.active_line.is_none() {
                        doc.select_line(0);
                    }
                    if let Some(idx) = doc.active_line {
                        let char_col = doc.active_char_col();
                        let max_line = doc
                            .extra_cursors
                            .iter()
                            .map(|(l, _)| *l)
                            .max()
                            .unwrap_or(idx)
                            .max(idx);
                        if max_line + 1 < doc.lines.len() {
                            let target = max_line + 1;
                            let target_col = char_col.min(doc.lines[target].raw.chars().count());
                            if !doc.extra_cursors.iter().any(|(l, _)| *l == target) {
                                doc.extra_cursors.push((target, target_col));
                                doc.extra_cursors.sort_by_key(|(l, _)| *l);
                            }
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::SaveActiveTab => {
                if let Some(doc) = self.tabs.get(self.active_tab_index) {
                    let path = doc
                        .file_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(&doc.title));
                    if let Err(err) = std::fs::write(&path, doc.full_text()) {
                        tracing::error!("Failed to save '{}': {err}", path.display());
                    } else {
                        tracing::info!("Saved '{}'", path.display());
                    }
                }
            }
            Message::ClickBottomArea => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                    let last_idx = doc.lines.len().saturating_sub(1);
                    if !doc.lines.is_empty() && !doc.lines[last_idx].raw.is_empty() {
                        doc.lines.push(Line::new(""));
                        let new_idx = doc.lines.len() - 1;
                        doc.select_line(new_idx);
                    } else {
                        doc.select_line(last_idx);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::LinkClicked(url) => {
                tracing::info!("Markdown link clicked: {url}");
            }
            Message::GlobalKeyEvent(event) => {
                if let keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                } = event
                    && let Some(msg) = match_shortcut(&key, physical_key, modifiers)
                {
                    return self.update(msg);
                }
            }
        }

        Task::none()
    }

    /// Renders the Obsidian-style live edit user interface with tabs and in-place line editing.
    pub fn view(&self) -> impl Widget<Message> + '_ {
        let theme_val = custom_theme();

        // 1. Top Tab Bar
        let mut tab_row: iced::widget::Row<iced::Element<'_, Message>> = iced::widget::Row::new()
            .spacing(4)
            .align_y(alignment::Vertical::Center);

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = idx == self.active_tab_index;

            let tab_title_click = mouse_area(
                row![
                    icons::file_text().size(13).color(if is_active {
                        Color::from_rgb8(9, 105, 218)
                    } else {
                        Color::from_rgb8(140, 149, 159)
                    }),
                    text(&tab.title).size(12).color(if is_active {
                        Color::from_rgb8(31, 35, 40)
                    } else {
                        Color::from_rgb8(101, 109, 118)
                    }),
                ]
                .spacing(6)
                .align_y(alignment::Vertical::Center),
            )
            .interaction(mouse::Interaction::Pointer)
            .on_press(Message::SelectTab(idx));

            let close_click = mouse_area(
                container(icons::x().size(11).color(Color::from_rgb8(140, 149, 159)))
                    .padding([2, 4]),
            )
            .interaction(mouse::Interaction::Pointer)
            .on_press(Message::CloseTab(idx));

            let tab_widget = container(
                row![tab_title_click, close_click]
                    .spacing(8)
                    .align_y(alignment::Vertical::Center),
            )
            .padding([5, 12])
            .style(move |_theme| container::Style {
                background: Some(Background::Color(if is_active {
                    Color::from_rgb8(255, 255, 255)
                } else {
                    Color::from_rgb8(240, 242, 245)
                })),
                border: Border::default()
                    .rounded(6.0)
                    .color(if is_active {
                        Color::from_rgb8(208, 215, 222)
                    } else {
                        Color::TRANSPARENT
                    })
                    .width(1.0),
                ..Default::default()
            });

            tab_row = tab_row.push(tab_widget.boxed());
        }

        let new_tab_btn = button(
            icons::plus()
                .size(14)
                .color(Color::from_rgb8(101, 109, 118)),
        )
        .style(button::subtle)
        .padding([4, 8])
        .on_press(Message::NewTab);

        tab_row = tab_row.push(new_tab_btn.boxed());

        let drag_area = mouse_area(
            container(space().width(Length::Fill).height(32))
                .width(Length::Fill)
                .height(32),
        )
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleMaximize);

        let min_btn = button(
            icons::minus()
                .size(13)
                .align_x(alignment::Horizontal::Center),
        )
        .padding([8, 14])
        .on_press(Message::MinimizeWindow)
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered => {
                    Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.06)))
                }
                button::Status::Pressed => {
                    Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.12)))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: Color::from_rgb8(101, 109, 118),
                border: Border::default(),
                ..Default::default()
            }
        });

        let max_btn = button(
            icons::square()
                .size(11)
                .align_x(alignment::Horizontal::Center),
        )
        .padding([8, 14])
        .on_press(Message::ToggleMaximize)
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered => {
                    Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.06)))
                }
                button::Status::Pressed => {
                    Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.12)))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: Color::from_rgb8(101, 109, 118),
                border: Border::default(),
                ..Default::default()
            }
        });

        let close_btn = button(icons::x().size(13).align_x(alignment::Horizontal::Center))
            .padding([8, 16])
            .on_press(Message::CloseWindow)
            .style(|_theme, status| match status {
                button::Status::Hovered => button::Style {
                    background: Some(Background::Color(Color::from_rgb8(232, 17, 35))),
                    text_color: Color::WHITE,
                    border: Border::default(),
                    ..Default::default()
                },
                button::Status::Pressed => button::Style {
                    background: Some(Background::Color(Color::from_rgb8(200, 15, 30))),
                    text_color: Color::WHITE,
                    border: Border::default(),
                    ..Default::default()
                },
                _ => button::Style {
                    background: None,
                    text_color: Color::from_rgb8(101, 109, 118),
                    border: Border::default(),
                    ..Default::default()
                },
            });

        let window_controls = row![min_btn, max_btn, close_btn]
            .spacing(0)
            .align_y(alignment::Vertical::Center);

        let top_tab_bar = container(
            row![tab_row, drag_area, window_controls]
                .width(Length::Fill)
                .align_y(alignment::Vertical::Center)
                .spacing(4),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: 2.0,
            bottom: 2.0,
            left: 8.0,
            right: 0.0,
        })
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
            border: Border::default()
                .color(Color::from_rgb8(208, 215, 222))
                .width(0.5),
            ..Default::default()
        });

        // 2. Breadcrumb / Navigation Bar (Live Preview only - switch removed)
        let active_tab = self.tabs.get(self.active_tab_index);

        // 2. Document Lines (Live Preview with raw markdown on line click)
        let mut lines_column: iced::widget::Column<iced::Element<'_, Message>> =
            iced::widget::Column::new().spacing(3).width(820);

        if let Some(doc) = active_tab {
            let cur = doc.active_editor.cursor();
            let is_at_line_start =
                cur.position.line == 0 && cur.position.index == 0 && cur.selection.is_none();
            let is_on_first_subline = cur.position.line == 0;
            let is_on_last_subline = cur.position.line + 1 >= doc.active_editor.line_count().max(1);
            let has_extra_cursors = !doc.extra_cursors.is_empty();

            let code_blocks = find_code_blocks(&doc.lines);
            let tables = find_tables(&doc.lines, &code_blocks);
            let badge_blocks = find_badge_blocks(&doc.lines, &code_blocks, &tables);
            let mut idx = 0;

            while idx < doc.lines.len() {
                // 1. If idx starts a complete fenced code block and the block is inactive, render it as one!
                if let Some(range) = code_blocks.iter().find(|r| r.start == idx) {
                    let is_any_line_active = doc
                        .active_line
                        .is_some_and(|active| active >= range.start && active <= range.end);
                    let has_any_extra_cursor = doc
                        .extra_cursors
                        .iter()
                        .any(|(l, _)| *l >= range.start && *l <= range.end);

                    if !is_any_line_active
                        && !has_any_extra_cursor
                        && let Some(ref items) = doc.lines[range.start].code_block_items
                    {
                        let rendered =
                            markdown::view(items, markdown::Settings::default(), theme_val.clone())
                                .map(Message::LinkClicked);

                        let block_container =
                            container(rendered).width(Length::Fill).padding([2, 4]);

                        let click_target = if range.start + 1 < range.end {
                            range.start + 1
                        } else {
                            range.start
                        };

                        let interactive = mouse_area(block_container)
                            .interaction(mouse::Interaction::Text)
                            .on_press(Message::SelectLine(click_target));

                        lines_column = lines_column.push(interactive.boxed());

                        idx = range.end + 1;
                        continue;
                    }
                }

                // 2. If idx starts a complete Markdown table and the table is inactive, render it as one!
                if let Some(range) = tables.iter().find(|r| r.start == idx) {
                    let is_any_line_active = doc
                        .active_line
                        .is_some_and(|active| active >= range.start && active <= range.end);
                    let has_any_extra_cursor = doc
                        .extra_cursors
                        .iter()
                        .any(|(l, _)| *l >= range.start && *l <= range.end);

                    if !is_any_line_active
                        && !has_any_extra_cursor
                        && let Some(ref items) = doc.lines[range.start].table_items
                    {
                        let rendered =
                            markdown::view(items, markdown::Settings::default(), theme_val.clone())
                                .map(Message::LinkClicked);

                        let table_container = container(rendered)
                            .width(Length::Fill)
                            .padding([4, 6])
                            .style(|_theme| container::Style {
                                background: Some(Background::Color(Color::from_rgb8(
                                    255, 255, 255,
                                ))),
                                border: Border::default()
                                    .rounded(6.0)
                                    .color(Color::from_rgb8(208, 215, 222))
                                    .width(1.0),
                                ..Default::default()
                            });

                        let click_target = if range.start + 2 <= range.end {
                            range.start + 2
                        } else {
                            range.start
                        };

                        let interactive = mouse_area(table_container)
                            .interaction(mouse::Interaction::Text)
                            .on_press(Message::SelectLine(click_target));

                        lines_column = lines_column.push(interactive.boxed());

                        idx = range.end + 1;
                        continue;
                    }
                }

                // 3. If idx starts a group of pure shields.io badge lines, render them horizontally!
                if let Some(range) = badge_blocks.iter().find(|r| r.start == idx) {
                    let is_any_line_active = doc
                        .active_line
                        .is_some_and(|active| active >= range.start && active <= range.end);
                    let has_any_extra_cursor = doc
                        .extra_cursors
                        .iter()
                        .any(|(l, _)| *l >= range.start && *l <= range.end);

                    if !is_any_line_active && !has_any_extra_cursor {
                        let mut all_badges = Vec::new();
                        for line_i in range.start..=range.end {
                            all_badges.extend(extract_badges_from_line(&doc.lines[line_i].raw));
                        }

                        if !all_badges.is_empty() {
                            let mut badge_row =
                                row![].spacing(6).align_y(alignment::Vertical::Center);
                            for badge in &all_badges {
                                badge_row = badge_row.push(render_badge(badge));
                            }

                            let badge_container =
                                container(badge_row).width(Length::Fill).padding([4, 6]);

                            let interactive = mouse_area(badge_container)
                                .interaction(mouse::Interaction::Text)
                                .on_press(Message::SelectLine(range.start));

                            lines_column = lines_column.push(interactive.boxed());

                            idx = range.end + 1;
                            continue;
                        }
                    }
                }

                let line = &doc.lines[idx];
                let is_inside_code_block =
                    code_blocks.iter().any(|r| idx >= r.start && idx <= r.end);
                let is_inside_table = tables.iter().any(|r| idx >= r.start && idx <= r.end);

                if doc.active_line == Some(idx) {
                    // Primary active line: RAW MARKDOWN text editor
                    let active_input = text_editor(&doc.active_editor)
                        .id(ACTIVE_EDITOR_ID)
                        .font(Font::MONOSPACE)
                        .on_action(Message::EditorAction)
                        .padding([2, 4])
                        .key_binding(move |event| {
                            if let Some(msg) =
                                match_shortcut(&event.key, event.physical_key, event.modifiers)
                            {
                                return Some(text_editor::Binding::Custom(msg));
                            }

                            if event.key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                                return Some(text_editor::Binding::Custom(Message::DeselectLine));
                            }

                            // Enter or Shift+Enter: always jumps to the next line below
                            if event.key == keyboard::Key::Named(keyboard::key::Named::Enter) {
                                return Some(text_editor::Binding::Custom(
                                    Message::InsertLineBelow,
                                ));
                            }

                            // Tab: insert 4 spaces (standard for code and lists)
                            if event.key == keyboard::Key::Named(keyboard::key::Named::Tab)
                                && !event.modifiers.shift()
                            {
                                return Some(text_editor::Binding::Sequence(vec![
                                    text_editor::Binding::Insert(' '),
                                    text_editor::Binding::Insert(' '),
                                    text_editor::Binding::Insert(' '),
                                    text_editor::Binding::Insert(' '),
                                ]));
                            }

                            if event.key == keyboard::Key::Named(keyboard::key::Named::Backspace)
                                && !event.modifiers.command()
                                && !event.modifiers.alt()
                                && !event.modifiers.shift()
                                && is_at_line_start
                                && !has_extra_cursors
                                && idx > 0
                            {
                                return Some(text_editor::Binding::Custom(
                                    Message::BackspaceAtLineStart,
                                ));
                            }

                            if !event.modifiers.command()
                                && !event.modifiers.alt()
                                && !event.modifiers.shift()
                                && !has_extra_cursors
                            {
                                if event.key == keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                                    && is_on_first_subline
                                    && idx > 0
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::NavigateLineUp,
                                    ));
                                }
                                if event.key
                                    == keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                                    && is_on_last_subline
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::NavigateLineDown,
                                    ));
                                }
                            }

                            text_editor::Binding::from_key_press(event)
                        })
                        .style(|_theme, _status| text_editor::Style {
                            background: Background::Color(Color::TRANSPARENT),
                            border: Border::default(),
                            placeholder: Color::from_rgb8(140, 149, 159),
                            value: Color::from_rgb8(31, 35, 40),
                            selection: Color::from_rgba8(9, 105, 218, 0.25),
                        });

                    let active_line_widget = container(
                        row![
                            icons::chevron_right()
                                .size(12)
                                .color(Color::from_rgb8(140, 149, 159)),
                            active_input,
                        ]
                        .spacing(6)
                        .align_y(alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba8(9, 105, 218, 0.06))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    lines_column = lines_column.push(active_line_widget.boxed());
                } else if let Some(&(_, char_col)) =
                    doc.extra_cursors.iter().find(|(l, _)| *l == idx)
                {
                    // Secondary multi-cursor line (Ctrl + Alt + Up/Down)
                    let raw = &line.raw;
                    let clamped_col = char_col.min(raw.chars().count());
                    let byte_idx = char_to_byte_idx(raw, clamped_col);
                    let before = &raw[..byte_idx];
                    let after = &raw[byte_idx..];

                    let multi_cursor_row = container(
                        row![
                            icons::chevron_right()
                                .size(12)
                                .color(Color::from_rgb8(9, 105, 218)),
                            container(
                                row![
                                    text(before)
                                        .font(Font::MONOSPACE)
                                        .color(Color::from_rgb8(31, 35, 40)),
                                    text("│")
                                        .font(Font::MONOSPACE)
                                        .color(Color::from_rgb8(9, 105, 218)),
                                    text(after)
                                        .font(Font::MONOSPACE)
                                        .color(Color::from_rgb8(31, 35, 40)),
                                ]
                                .align_y(alignment::Vertical::Center),
                            )
                            .padding([2, 4])
                            .width(Length::Fill),
                        ]
                        .spacing(6)
                        .align_y(alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba8(9, 105, 218, 0.06))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    let interactive_multi = mouse_area(multi_cursor_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_multi.boxed());
                } else if is_inside_code_block {
                    // Inactive line inside an expanded code block: monospace with code background
                    let is_fence = code_blocks.iter().any(|r| idx == r.start || idx == r.end);
                    let text_color = if is_fence {
                        Color::from_rgb8(101, 109, 118)
                    } else {
                        Color::from_rgb8(31, 35, 40)
                    };

                    let code_row = if line.raw.is_empty() {
                        container(
                            row![space().width(11), space().width(Length::Fill).height(18),]
                                .spacing(6)
                                .align_y(alignment::Vertical::Center),
                        )
                    } else {
                        container(
                            row![
                                space().width(11), // align with 'v' indicator
                                text(&line.raw)
                                    .font(Font::MONOSPACE)
                                    .size(13)
                                    .color(text_color),
                            ]
                            .spacing(6)
                            .align_y(alignment::Vertical::Center),
                        )
                    }
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    let interactive_code_line = mouse_area(code_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_code_line.boxed());
                } else if is_inside_table {
                    // Inactive line inside an expanded table: monospace text with subtle background for column alignment
                    let is_delimiter = is_table_delimiter(&line.raw);
                    let text_color = if is_delimiter {
                        Color::from_rgb8(101, 109, 118)
                    } else {
                        Color::from_rgb8(31, 35, 40)
                    };

                    let table_row = container(
                        row![
                            space().width(11), // align with 'v' indicator
                            text(&line.raw)
                                .font(Font::MONOSPACE)
                                .size(13)
                                .color(text_color),
                        ]
                        .spacing(6)
                        .align_y(alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    let interactive_table_line = mouse_area(table_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_table_line.boxed());
                } else if let Some((img_title, img_target)) = parse_image_reference(&line.raw) {
                    // Standalone image or Obsidian attachment: clean GitHub Light image card
                    let img_widget = render_image_card(&img_title, &img_target);
                    let interactive = mouse_area(img_widget)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive.boxed());
                } else if is_pure_badge_line(&line.raw) {
                    // Standalone line containing shields.io badges
                    let badges = extract_badges_from_line(&line.raw);
                    let mut badge_row = row![].spacing(6).align_y(alignment::Vertical::Center);
                    for badge in &badges {
                        badge_row = badge_row.push(render_badge(badge));
                    }

                    let badge_container = container(badge_row).width(Length::Fill).padding([4, 6]);

                    let interactive = mouse_area(badge_container)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive.boxed());
                } else if parse_opening_fence(line.raw.trim_start()).is_some() {
                    // Unclosed fence: render as monospace text, NEVER an empty black bar!
                    let unclosed_row = container(
                        text(&line.raw)
                            .font(Font::MONOSPACE)
                            .size(13)
                            .color(Color::from_rgb8(101, 109, 118)),
                    )
                    .width(Length::Fill)
                    .padding([2, 6]);

                    let interactive = mouse_area(unclosed_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive.boxed());
                } else if line.raw.trim().is_empty() {
                    // Empty line: clickable space for cursor placement
                    let empty_line = mouse_area(space().width(Length::Fill).height(18))
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(empty_line.boxed());
                } else {
                    // Inactive line: formatted Markdown rendering with GitHub Heading dividers
                    let rendered = markdown::view(
                        &line.items,
                        markdown::Settings::default(),
                        theme_val.clone(),
                    )
                    .map(Message::LinkClicked);

                    let is_h1 = line.raw.starts_with("# ");
                    let is_h2 = line.raw.starts_with("## ");

                    let line_widget = if is_h1 || is_h2 {
                        let divider = container(space().width(Length::Fill).height(1))
                            .width(Length::Fill)
                            .style(|_theme| container::Style {
                                background: Some(Background::Color(Color::from_rgb8(
                                    216, 222, 228,
                                ))), // #d8dee4
                                ..Default::default()
                            });
                        container(column![rendered, divider].spacing(4).width(Length::Fill))
                            .width(Length::Fill)
                            .padding([1, 4])
                            .boxed()
                    } else {
                        container(rendered)
                            .width(Length::Fill)
                            .padding([1, 4])
                            .boxed()
                    };

                    let interactive_line = mouse_area(line_widget)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_line.boxed());
                }

                idx += 1;
            }
        }

        // Empty bottom area: clicking here continues writing at the end of the document
        let bottom_spacer = mouse_area(
            container(space().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(240),
        )
        .interaction(mouse::Interaction::Text)
        .on_press(Message::ClickBottomArea);

        lines_column = lines_column.push(bottom_spacer.boxed());

        let centered_doc = container(lines_column)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(Padding {
                top: 24.0,
                right: 32.0,
                bottom: 32.0,
                left: 32.0,
            });

        let main_doc_scroll = scrollable(centered_doc)
            .width(Length::Fill)
            .height(Length::Fill);

        container(
            column![top_tab_bar, main_doc_scroll]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(255, 255, 255))),
            border: Border::default()
                .color(Color::from_rgb8(208, 215, 222))
                .width(1.0),
            ..Default::default()
        })
    }
}

/// Matches editor keyboard shortcuts (Ctrl+D, Alt+Up/Down, Ctrl+A, Ctrl+Alt+Up/Down, Ctrl+S).
fn match_shortcut(
    key: &keyboard::Key,
    physical_key: keyboard::key::Physical,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    // Ctrl + Alt + Up / Down -> Multi-line cursor (VS Code style)
    if modifiers.control() && modifiers.alt() && !modifiers.shift() {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                return Some(Message::AddCursorAbove);
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                return Some(Message::AddCursorBelow);
            }
            _ => {}
        }
    }

    // Alt + Up / Down -> Move line or selection up/down
    if modifiers.alt() && !modifiers.control() && !modifiers.shift() && !modifiers.logo() {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                return Some(Message::MoveLineUp);
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                return Some(Message::MoveLineDown);
            }
            _ => {}
        }
    }

    // Ctrl + D / Ctrl + A / Ctrl + S
    if modifiers.command() && !modifiers.alt() && !modifiers.shift() {
        match key.to_latin(physical_key) {
            Some('d') => return Some(Message::DuplicateLineOrSelection),
            Some('a') => return Some(Message::SelectAllMarkdown),
            Some('s') => return Some(Message::SaveActiveTab),
            _ => {}
        }
    }

    None
}

fn theme(_state: &Doggy) -> Theme {
    custom_theme()
}

fn style(_state: &Doggy, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::from_rgb8(255, 255, 255), // GitHub Light canvas (#ffffff)
        text_color: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
    }
}

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt::try_init();

    iced::application(Doggy::boot, Doggy::update, Doggy::view)
        .title("Doggy")
        .subscription(Doggy::subscription)
        .theme(theme)
        .style(style)
        .fonts([icons::FONT_BYTES])
        .window(iced::window::Settings {
            size: Size::new(960.0, 720.0),
            min_size: Some(Size::new(500.0, 400.0)),
            decorations: false,
            ..Default::default()
        })
        .run()
}
