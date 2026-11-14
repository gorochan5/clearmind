mod icons;

use std::path::{Path, PathBuf};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use iced::alignment;
use iced::futures::SinkExt;
use iced::keyboard;
use iced::mouse;
use iced::widget::{
    self, Id, Widget, button, checkbox, column, container, markdown, mouse_area, rich_text, row,
    scrollable, space, text, text_editor, text_input,
};
use iced::{
    Background, Border, Code, Color, Font, Length, Padding, Size, Subscription, Task, Theme, window,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

const ACTIVE_EDITOR_ID: &str = "clearmind-active-line-editor";
const DOCUMENT_SCROLL_ID: &str = "clearmind-document-scroll";
const SEARCH_INPUT_ID: &str = "clearmind-search-input";
const REPLACE_INPUT_ID: &str = "clearmind-replace-input";

const MAX_UNDO_HISTORY: usize = 120;

const FLAG_IN_CODE: u8 = 1 << 0;
const FLAG_IN_TABLE: u8 = 1 << 1;
const FLAG_IN_BADGE: u8 = 1 << 2;
const FLAG_CODE_START: u8 = 1 << 3;
const FLAG_CODE_END: u8 = 1 << 4;
const FLAG_TABLE_START: u8 = 1 << 5;
const FLAG_BADGE_START: u8 = 1 << 6;
const FLAG_TABLE_DELIMITER: u8 = 1 << 7;

fn normalize_text(text: &str) -> String {
    if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text.to_string()
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    PathBuf::from(path.to_string_lossy().trim_start_matches("\\\\?\\"))
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let left_s = left.to_string_lossy();
    let right_s = right.to_string_lossy();
    let left_clean = left_s.trim_start_matches("\\\\?\\");
    let right_clean = right_s.trim_start_matches("\\\\?\\");
    #[cfg(windows)]
    {
        left_clean.eq_ignore_ascii_case(right_clean)
    }
    #[cfg(not(windows))]
    {
        left_clean == right_clean
    }
}

fn file_event_stream(paths: Vec<PathBuf>) -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(100, async move |mut output| {
        let (event_tx, event_rx) = std::sync::mpsc::channel::<PathBuf>();
        let mut watcher: Option<RecommendedWatcher> =
            match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        let _ = event_tx.send(path);
                    }
                }
            }) {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    tracing::error!("Failed to create file watcher: {error}");
                    None
                }
            };

        if let Some(watcher) = watcher.as_mut() {
            let mut parents = Vec::new();
            for path in paths {
                if let Some(parent) = path.parent() {
                    let parent = normalized_path(parent);
                    if !parents.contains(&parent) {
                        parents.push(parent);
                    }
                }
            }
            for parent in parents {
                if let Err(error) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
                    tracing::warn!("Could not watch '{}': {error}", parent.display());
                }
            }
        }

        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        let _stop_guard = StopFileWatcher(stopped);
        std::thread::spawn(move || {
            let _watcher = watcher;
            while !worker_stopped.load(Ordering::Relaxed) {
                let first = match event_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(path) => path,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let mut changed = vec![normalized_path(&first)];
                loop {
                    match event_rx.recv_timeout(std::time::Duration::from_millis(75)) {
                        Ok(path) => {
                            let path = normalized_path(&path);
                            if !changed.contains(&path) {
                                changed.push(path);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if iced::futures::executor::block_on(output.send(Message::FilesChanged(changed)))
                    .is_err()
                {
                    break;
                }
            }
        });
        std::future::pending::<()>().await;
    })
}

struct StopFileWatcher(Arc<AtomicBool>);

impl Drop for StopFileWatcher {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Generates the GitHub Light mode custom theme.
fn custom_theme() -> Theme {
    let seed = iced::theme::palette::Seed {
        background: Color::from_rgb8(255, 255, 255), // Main canvas (#ffffff)
        text: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
        primary: Color::from_rgb8(31, 58, 50),       // Links and accents (#1f3a32)
        success: Color::from_rgb8(26, 127, 55),      // Green (#1a7f37)
        warning: Color::from_rgb8(154, 103, 0),      // Yellow/Orange (#9a6700)
        danger: Color::from_rgb8(207, 34, 46),       // Red (#cf222e)
    };

    Theme::custom("GitHub Light", seed)
}

/// Returns the cached GitHub Light theme, building it once.
fn cached_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(custom_theme)
}

/// Returns true when line text can introduce or alter fences, tables, or badges.
fn line_may_affect_structure(raw: &str) -> bool {
    let trimmed = raw.trim_start();
    parse_opening_fence(raw).is_some()
        || trimmed.starts_with("```")
        || trimmed.starts_with("~~~")
        || raw.contains("shields.io/")
        || is_table_delimiter(raw)
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

/// Case-insensitive substring search returning `(char_start, char_len)` for each match on a line.
fn find_case_insensitive_char_matches(haystack: &str, needle_lower: &str) -> Vec<(usize, usize)> {
    if needle_lower.is_empty() || haystack.is_empty() {
        return Vec::new();
    }
    if haystack.is_ascii() && needle_lower.is_ascii() {
        let hay_lower = haystack.to_ascii_lowercase();
        let needle_len = needle_lower.len();
        return hay_lower
            .match_indices(needle_lower)
            .map(|(byte_idx, _)| (byte_idx, needle_len))
            .collect();
    }

    let needle_chars: Vec<char> = needle_lower.chars().collect();
    let n_len = needle_chars.len();
    if n_len == 0 {
        return Vec::new();
    }
    let hay_chars: Vec<char> = haystack.chars().collect();
    if hay_chars.len() < n_len {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let mut i = 0;
    while i + n_len <= hay_chars.len() {
        let is_match = hay_chars[i..i + n_len]
            .iter()
            .zip(needle_chars.iter())
            .all(|(&hc, &nc)| hc.to_lowercase().eq(std::iter::once(nc)));
        if is_match {
            matches.push((i, n_len));
            i += n_len;
        } else {
            i += 1;
        }
    }
    matches
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
    let bytes = raw.as_bytes();
    let indent = bytes.iter().take_while(|&&b| b == b' ').count();
    if indent > 3 {
        return None;
    }
    let trimmed = &raw[indent..];
    if trimmed.starts_with("```") {
        let len = trimmed.bytes().take_while(|&b| b == b'`').count();
        if len >= 3 {
            let rest = &trimmed[len..];
            if !rest.as_bytes().contains(&b'`') {
                return Some((len, '`'));
            }
        }
    } else if trimmed.starts_with("~~~") {
        let len = trimmed.bytes().take_while(|&b| b == b'~').count();
        if len >= 3 {
            return Some((len, '~'));
        }
    }
    None
}

/// Checks if a line is a valid closing fence matching `required_char` and `min_len`.
pub fn parse_closing_fence(raw: &str, required_char: char, min_len: usize) -> bool {
    let bytes = raw.as_bytes();
    let indent = bytes.iter().take_while(|&&b| b == b' ').count();
    if indent > 3 {
        return false;
    }
    let trimmed = &raw[indent..];
    let req_byte = required_char as u8;
    let count = trimmed.bytes().take_while(|&b| b == req_byte).count();
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
    if !trimmed
        .bytes()
        .all(|b| b == b'|' || b == b'-' || b == b':' || b.is_ascii_whitespace())
    {
        return false;
    }
    let mut found_cell = false;
    for cell in trimmed.split('|') {
        let tc = cell.trim();
        if tc.is_empty() {
            continue;
        }
        found_cell = true;
        let has_hyphen = tc.bytes().any(|b| b == b'-');
        let valid_chars = tc.bytes().all(|b| b == b'-' || b == b':');
        if !has_hyphen || !valid_chars {
            return false;
        }
    }
    found_cell
}

/// Parses column horizontal alignments from a GFM table delimiter row.
pub fn parse_table_alignments(raw: &str) -> Vec<alignment::Horizontal> {
    raw.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| {
            let tc = cell.trim();
            let starts_colon = tc.starts_with(':');
            let ends_colon = tc.ends_with(':');
            match (starts_colon, ends_colon) {
                (true, true) => alignment::Horizontal::Center,
                (false, true) => alignment::Horizontal::Right,
                _ => alignment::Horizontal::Left,
            }
        })
        .collect()
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

/// Scans document lines and identifies all complete Markdown tables [start..=end] in O(N + K).
pub fn find_tables(lines: &[Line], code_blocks: &[CodeBlockRange]) -> Vec<TableRange> {
    let mut ranges = Vec::new();
    let mut i = 0;
    let mut cb_idx = 0;

    let is_in_code = |line_idx: usize, cb_cursor: &mut usize| -> bool {
        while *cb_cursor < code_blocks.len() && code_blocks[*cb_cursor].end < line_idx {
            *cb_cursor += 1;
        }
        code_blocks
            .get(*cb_cursor)
            .is_some_and(|cb| line_idx >= cb.start && line_idx <= cb.end)
    };

    while i + 1 < lines.len() {
        if is_in_code(i, &mut cb_idx) {
            i = code_blocks[cb_idx].end + 1;
            continue;
        }

        if is_potential_table_row(&lines[i].raw)
            && !is_in_code(i + 1, &mut cb_idx)
            && is_table_delimiter(&lines[i + 1].raw)
            && !is_table_delimiter(&lines[i].raw)
        {
            let start = i;
            let mut end = i + 1;

            let mut j = i + 2;
            while j < lines.len() {
                if is_in_code(j, &mut cb_idx) {
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

/// Cached cell in a pre-parsed Markdown table.
#[derive(Debug, Clone)]
pub struct TableCell {
    pub raw: String,
    pub items: Vec<markdown::Item>,
    pub char_len: usize,
    pub has_rich_formatting: bool,
}

/// Pre-parsed representation of a Markdown table.
#[derive(Debug, Clone)]
pub struct ParsedTable {
    pub source: String,
    pub alignments: Vec<alignment::Horizontal>,
    pub rows: Vec<Vec<TableCell>>,
    pub column_count: usize,
}

impl ParsedTable {
    pub fn parse(source: String, lines: &[Line]) -> Self {
        let mut alignments = Vec::new();
        let mut rows: Vec<Vec<TableCell>> = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            if idx == 1 && is_table_delimiter(&line.raw) {
                alignments = parse_table_alignments(&line.raw);
                continue;
            }
            if is_table_delimiter(&line.raw) {
                continue;
            }
            let cells: Vec<TableCell> = line
                .raw
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|c| {
                    let trimmed = c.trim().to_string();
                    let char_len = trimmed.chars().count();
                    let has_rich_formatting = trimmed.contains('`')
                        || trimmed.contains('*')
                        || trimmed.contains('_')
                        || trimmed.contains('[')
                        || trimmed.contains('~');
                    let items = if has_rich_formatting {
                        markdown::parse(&trimmed).collect()
                    } else {
                        Vec::new()
                    };
                    TableCell {
                        raw: trimmed,
                        items,
                        char_len,
                        has_rich_formatting,
                    }
                })
                .collect();
            rows.push(cells);
        }

        let column_count = rows
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(1)
            .max(alignments.len())
            .max(1);

        Self {
            source,
            alignments,
            rows,
            column_count,
        }
    }
}

/// Parsed metadata for a GFM task list item (`- [ ]` or `- [x]`).
#[derive(Debug, Clone)]
pub struct TaskItemInfo {
    pub indent: usize,
    pub checked: bool,
    pub body_items: Vec<markdown::Item>,
}

/// Parses a GFM task list item from a raw line if it starts with `- [ ]`, `- [x]`, `* [ ]`, etc.
pub fn parse_task_item(raw: &str) -> Option<TaskItemInfo> {
    let indent = raw.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
    let trimmed = &raw[indent..];
    let after_bullet = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))?;

    let (checked, rest) = if let Some(r) = after_bullet.strip_prefix("[ ]") {
        (false, r)
    } else {
        let r = after_bullet
            .strip_prefix("[x]")
            .or_else(|| after_bullet.strip_prefix("[X]"))?;
        (true, r)
    };

    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }

    let body = rest.trim_start();
    let body_items = markdown::parse(body).collect();
    Some(TaskItemInfo {
        indent,
        checked,
        body_items,
    })
}

/// Toggles `[ ]` <-> `[x]` in a task list raw string.
pub fn toggle_task_in_raw(raw: &str, new_checked: bool) -> Option<String> {
    let indent = raw.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
    let trimmed = &raw[indent..];
    if !(trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ [")) {
        return None;
    }
    let bracket_idx = indent + 2; // index of '['
    if raw.len() < bracket_idx + 3 {
        return None;
    }
    let mut out = String::with_capacity(raw.len());
    out.push_str(&raw[..bracket_idx + 1]);
    out.push(if new_checked { 'x' } else { ' ' });
    out.push_str(&raw[bracket_idx + 2..]);
    Some(out)
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
    let raw_bytes = replaced.as_bytes();
    let mut bytes = Vec::with_capacity(raw_bytes.len());
    let mut i = 0;
    while i < raw_bytes.len() {
        if raw_bytes[i] == b'%'
            && i + 2 < raw_bytes.len()
            && let Ok(hex_str) = std::str::from_utf8(&raw_bytes[i + 1..i + 3])
            && let Ok(b) = u8::from_str_radix(hex_str, 16)
        {
            bytes.push(b);
            i += 3;
            continue;
        }
        bytes.push(raw_bytes[i]);
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&bytes);
    decoded.replace('\u{0000}', "-")
}

/// Parses a shields.io color string into an iced `Color`.
pub fn parse_badge_color(color_str: &str) -> Color {
    let clean = color_str.trim().trim_start_matches('#');
    if clean.eq_ignore_ascii_case("brightgreen") {
        Color::from_rgb8(76, 201, 71)
    } else if clean.eq_ignore_ascii_case("green") {
        Color::from_rgb8(68, 204, 17)
    } else if clean.eq_ignore_ascii_case("yellowgreen") {
        Color::from_rgb8(151, 202, 0)
    } else if clean.eq_ignore_ascii_case("yellow") {
        Color::from_rgb8(223, 179, 23)
    } else if clean.eq_ignore_ascii_case("orange") || clean.eq_ignore_ascii_case("important") {
        Color::from_rgb8(254, 125, 55)
    } else if clean.eq_ignore_ascii_case("red") || clean.eq_ignore_ascii_case("critical") {
        Color::from_rgb8(224, 93, 68)
    } else if clean.eq_ignore_ascii_case("blue") || clean.eq_ignore_ascii_case("informational") {
        Color::from_rgb8(0, 126, 198)
    } else if clean.eq_ignore_ascii_case("lightgrey")
        || clean.eq_ignore_ascii_case("lightgray")
        || clean.eq_ignore_ascii_case("inactive")
    {
        Color::from_rgb8(159, 159, 159)
    } else if clean.eq_ignore_ascii_case("purple") || clean.eq_ignore_ascii_case("blueviolet") {
        Color::from_rgb8(138, 43, 226)
    } else if clean.eq_ignore_ascii_case("success") {
        Color::from_rgb8(46, 164, 79)
    } else if clean.len() == 6 && clean.is_ascii() {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            Color::from_rgb8(r, g, b)
        } else {
            Color::from_rgb8(101, 109, 118)
        }
    } else if clean.len() == 3 && clean.is_ascii() {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..1], 16),
            u8::from_str_radix(&clean[1..2], 16),
            u8::from_str_radix(&clean[2..3], 16),
        ) {
            Color::from_rgb8(r * 17, g * 17, b * 17)
        } else {
            Color::from_rgb8(101, 109, 118)
        }
    } else {
        Color::from_rgb8(101, 109, 118)
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

/// Extracts all shields.io badges from a line of text (UTF-8 safe).
pub fn extract_badges_from_line(raw: &str) -> Vec<BadgeInfo> {
    let mut badges = Vec::new();
    let mut cur = raw;

    while let Some(shields_idx) = cur.find("shields.io/") {
        let prefix = &cur[..shields_idx];
        let url_start = prefix
            .char_indices()
            .rev()
            .find(|&(_, c)| c == '(' || c == '"' || c == '\'' || c == '<' || c.is_whitespace())
            .map_or(0, |(pos, c)| pos + c.len_utf8());

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
        } else if in_link_target || c == '[' || c == ']' || c == '!' || c.is_whitespace() {
            // inside URL target or markdown formatting characters
        } else {
            cleaned.push(c);
        }
    }
    cleaned.replace("&nbsp;", "").is_empty()
}

/// Represents a contiguous range of lines that are pure shields.io badges [start..=end].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeBlockRange {
    pub start: usize,
    pub end: usize,
}

/// Scans document lines and identifies contiguous groups of badge-only lines in O(N + K).
pub fn find_badge_blocks(
    lines: &[Line],
    code_blocks: &[CodeBlockRange],
    tables: &[TableRange],
) -> Vec<BadgeBlockRange> {
    let mut ranges = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut cb_idx = 0;
    let mut tb_idx = 0;

    for (idx, line) in lines.iter().enumerate() {
        while cb_idx < code_blocks.len() && code_blocks[cb_idx].end < idx {
            cb_idx += 1;
        }
        let is_in_cb = code_blocks
            .get(cb_idx)
            .is_some_and(|cb| idx >= cb.start && idx <= cb.end);

        while tb_idx < tables.len() && tables[tb_idx].end < idx {
            tb_idx += 1;
        }
        let is_in_tbl = tables
            .get(tb_idx)
            .is_some_and(|tb| idx >= tb.start && idx <= tb.end);

        if !is_in_cb && !is_in_tbl && line.is_pure_badge {
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
    } else if badge.label.eq_ignore_ascii_case("linguagem")
        && badge.message.eq_ignore_ascii_case("rust")
    {
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
            icons::image().size(20).color(Color::from_rgb8(31, 58, 50)),
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

/// Counts visual leading indentation columns (`' '` = 1, `'\t'` = 4).
fn count_leading_indent(s: &str) -> usize {
    let mut cols = 0;
    for b in s.bytes() {
        match b {
            b' ' => cols += 1,
            b'\t' => cols += 4,
            _ => break,
        }
    }
    cols
}

/// A discrete line in the document with cached parsed Markdown items.
#[derive(Debug, Clone)]
pub struct Line {
    /// Raw text of the line.
    pub raw: String,
    /// Visual leading indentation in space columns (`' '` = 1, `'\t'` = 4).
    pub indent_spaces: usize,
    /// Pre-parsed Markdown items for zero-cost view rendering.
    pub items: Vec<markdown::Item>,
    /// Cached source text for a closed code block starting at this line.
    pub code_block_source: Option<String>,
    /// Pre-parsed code block Markdown items if this line starts a closed code block.
    pub code_block_items: Option<Vec<markdown::Item>>,
    /// Pre-parsed table representation if this line starts a Markdown table.
    pub parsed_table: Option<ParsedTable>,
    /// Pre-extracted shields.io badges for this line, if any.
    pub badges: Option<Vec<BadgeInfo>>,
    /// Pre-parsed image reference `(title, target)` when the line is a standalone image.
    pub image: Option<(String, String)>,
    /// Pre-parsed task list metadata (`- [ ]` / `- [x]`), if this line is a task item.
    pub task_item: Option<TaskItemInfo>,
    /// Whether this line is a pure badges-only line.
    pub is_pure_badge: bool,
}

impl Line {
    /// Creates a new line and parses its Markdown items.
    pub fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let indent_spaces = count_leading_indent(&raw);
        let trimmed = raw.trim_start_matches([' ', '\t']);
        let items = markdown::parse(trimmed).collect();
        let mut line = Self {
            raw,
            indent_spaces,
            items,
            code_block_source: None,
            code_block_items: None,
            parsed_table: None,
            badges: None,
            image: None,
            task_item: None,
            is_pure_badge: false,
        };
        line.refresh_inline_meta();
        line
    }

    /// Updates the line text and regenerates pre-parsed items if the text actually changed.
    pub fn update(&mut self, new_raw: String) -> bool {
        if new_raw == self.raw {
            return false;
        }
        self.indent_spaces = count_leading_indent(&new_raw);
        let trimmed = new_raw.trim_start_matches([' ', '\t']);
        self.items = markdown::parse(trimmed).collect();
        self.raw = new_raw;
        self.code_block_source = None;
        self.code_block_items = None;
        self.parsed_table = None;
        self.refresh_inline_meta();
        true
    }

    /// Refreshes badge/image/task metadata derived from `raw` (kept off the view hot path).
    fn refresh_inline_meta(&mut self) {
        let trimmed = self.raw.trim_start();
        if trimmed.starts_with("![[") || trimmed.starts_with("![") || trimmed.starts_with("<img ") {
            self.image = parse_image_reference(&self.raw);
        } else {
            self.image = None;
        }

        if trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ [") {
            self.task_item = parse_task_item(&self.raw);
        } else {
            self.task_item = None;
        }

        if self.raw.contains("shields.io/") {
            self.is_pure_badge = is_pure_badge_line(&self.raw);
            let badges = extract_badges_from_line(&self.raw);
            self.badges = if badges.is_empty() {
                None
            } else {
                Some(badges)
            };
        } else {
            self.is_pure_badge = false;
            self.badges = None;
        }
    }
}

/// Snapshot of a tab's document state for document-level Undo / Redo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoSnapshot {
    pub lines: Vec<String>,
    pub active_line: Option<usize>,
    pub cursor_char_col: usize,
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
    /// Last normalized content observed on disk.
    pub disk_text: String,
    /// Cached dirty state (`true` when buffer differs from `disk_text`).
    pub is_dirty: bool,
    /// Cached total word count.
    pub word_count: usize,
    /// Cached total character count.
    pub char_count: usize,
    /// External disk content awaiting a user decision; `None` means deleted.
    pub external_change: Option<Option<String>>,
    /// Whether the user asked to confirm overwriting an external change.
    pub confirm_overwrite: bool,
    /// Whether the warning was dismissed while retaining the conflict state.
    pub conflict_dismissed: bool,
    /// Lines of the document.
    pub lines: Vec<Line>,
    /// Index of the currently active line showing raw Markdown.
    pub active_line: Option<usize>,
    /// Buffer for the active line editor.
    pub active_editor: text_editor::Content,
    /// Additional multi-line cursors `(line_index, char_column)` created via Ctrl+Alt+Up/Down.
    pub extra_cursors: Vec<(usize, usize)>,
    /// Cached fenced code block ranges (updated only on structure refresh).
    pub code_blocks: Vec<CodeBlockRange>,
    /// Cached Markdown table ranges.
    pub tables: Vec<TableRange>,
    /// Cached contiguous pure-badge line ranges.
    pub badge_blocks: Vec<BadgeBlockRange>,
    /// Per-line structure membership flags (`FLAG_*`).
    pub line_flags: Vec<u8>,
    /// Document-level undo stack.
    pub undo_stack: Vec<UndoSnapshot>,
    /// Document-level redo stack.
    pub redo_stack: Vec<UndoSnapshot>,
    /// Tracks consecutive non-whitespace character insertions on the same line for undo coalescing.
    pub coalesce_line: Option<usize>,
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
            disk_text: normalize_text(content),
            is_dirty: false,
            word_count: 0,
            char_count: 0,
            external_change: None,
            confirm_overwrite: false,
            conflict_dismissed: false,
            lines,
            active_line: None,
            active_editor: text_editor::Content::new(),
            extra_cursors: Vec::new(),
            code_blocks: Vec::new(),
            tables: Vec::new(),
            badge_blocks: Vec::new(),
            line_flags: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            coalesce_line: None,
        };
        tab.refresh_document_blocks();
        tab.recompute_stats_and_dirty();
        tab
    }

    /// Captures a snapshot of the current document lines and cursor position.
    pub fn current_snapshot(&self) -> UndoSnapshot {
        let mut raw_lines: Vec<String> = self.lines.iter().map(|l| l.raw.clone()).collect();
        if let Some(active) = self.active_line
            && active < raw_lines.len()
        {
            raw_lines[active] = self.active_editor.text();
        }
        let cursor_char_col = if self.active_line.is_some() {
            self.active_char_col()
        } else {
            0
        };
        UndoSnapshot {
            lines: raw_lines,
            active_line: self.active_line,
            cursor_char_col,
        }
    }

    /// Pushes the current document state onto the undo stack before a mutation.
    pub fn push_undo(&mut self) {
        self.coalesce_line = None;
        let snap = self.current_snapshot();
        if self.undo_stack.last() != Some(&snap) {
            if self.undo_stack.len() >= MAX_UNDO_HISTORY {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(snap);
        }
        self.redo_stack.clear();
    }

    /// Restores a snapshot while preserving unchanged `Line` caches.
    fn apply_snapshot(&mut self, snap: UndoSnapshot) {
        self.extra_cursors.clear();
        self.coalesce_line = None;

        let mut new_lines = Vec::with_capacity(snap.lines.len().max(1));
        for (idx, raw) in snap.lines.into_iter().enumerate() {
            if let Some(existing) = self.lines.get(idx)
                && existing.raw == raw
            {
                new_lines.push(existing.clone());
            } else {
                new_lines.push(Line::new(raw));
            }
        }
        if new_lines.is_empty() {
            new_lines.push(Line::new(""));
        }
        self.lines = new_lines;
        self.active_line = None;
        self.refresh_document_blocks();

        if let Some(active) = snap.active_line {
            let clamped = active.min(self.lines.len() - 1);
            self.select_line_at_char(clamped, snap.cursor_char_col);
        }
        self.recompute_stats_and_dirty();
    }

    /// Undoes the last document change.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.current_snapshot();
        self.redo_stack.push(current);
        self.apply_snapshot(prev);
        true
    }

    /// Redoes the last undone document change.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.current_snapshot();
        self.undo_stack.push(current);
        self.apply_snapshot(next);
        true
    }

    /// Recomputes cached word count, character count, and `is_dirty` flag (kept off `view()`).
    pub fn recompute_stats_and_dirty(&mut self) {
        let mut words = 0usize;
        let mut chars = 0usize;
        let active_text = self.active_line.map(|_| self.active_editor.text());

        for (i, line) in self.lines.iter().enumerate() {
            let s = if self.active_line == Some(i) {
                active_text.as_deref().unwrap_or(&line.raw)
            } else {
                &line.raw
            };
            words += s.split_whitespace().count();
            chars += s.chars().count();
        }
        self.word_count = words;
        self.char_count = chars;

        // Compare against disk_text without allocating a second normalized copy when possible
        let full = self.full_text();
        self.is_dirty = full != self.disk_text;
    }

    /// Rebuilds `line_flags` from the cached structure ranges.
    fn rebuild_line_flags(&mut self) {
        self.line_flags.clear();
        self.line_flags.resize(self.lines.len(), 0);
        for range in &self.code_blocks {
            for i in range.start..=range.end {
                if let Some(flag) = self.line_flags.get_mut(i) {
                    *flag |= FLAG_IN_CODE;
                }
            }
            if let Some(flag) = self.line_flags.get_mut(range.start) {
                *flag |= FLAG_CODE_START;
            }
            if let Some(flag) = self.line_flags.get_mut(range.end) {
                *flag |= FLAG_CODE_END;
            }
        }
        for range in &self.tables {
            for i in range.start..=range.end {
                if let Some(flag) = self.line_flags.get_mut(i) {
                    *flag |= FLAG_IN_TABLE;
                    if is_table_delimiter(&self.lines[i].raw) {
                        *flag |= FLAG_TABLE_DELIMITER;
                    }
                }
            }
            if let Some(flag) = self.line_flags.get_mut(range.start) {
                *flag |= FLAG_TABLE_START;
            }
        }
        for range in &self.badge_blocks {
            for i in range.start..=range.end {
                if let Some(flag) = self.line_flags.get_mut(i) {
                    *flag |= FLAG_IN_BADGE;
                }
            }
            if let Some(flag) = self.line_flags.get_mut(range.start) {
                *flag |= FLAG_BADGE_START;
            }
        }
    }

    /// Returns true when editing `idx` from `old_raw` to `new_raw` may change document structure.
    pub fn edit_needs_structure_refresh(&self, idx: usize, old_raw: &str, new_raw: &str) -> bool {
        if self
            .line_flags
            .get(idx)
            .is_some_and(|f| f & (FLAG_IN_CODE | FLAG_IN_TABLE | FLAG_IN_BADGE) != 0)
        {
            return true;
        }
        for neighbor in [idx.saturating_sub(1), idx + 1] {
            if neighbor != idx
                && self
                    .line_flags
                    .get(neighbor)
                    .is_some_and(|f| f & (FLAG_IN_CODE | FLAG_IN_TABLE | FLAG_IN_BADGE) != 0)
            {
                return true;
            }
        }
        if line_may_affect_structure(old_raw) || line_may_affect_structure(new_raw) {
            return true;
        }
        // Check if adding/editing a pipe on this line connects with an adjacent potential table row/delimiter
        if old_raw.contains('|') || new_raw.contains('|') {
            let prev_is_table_candidate = idx > 0
                && self
                    .lines
                    .get(idx - 1)
                    .is_some_and(|l| is_potential_table_row(&l.raw));
            let next_is_table_candidate = self
                .lines
                .get(idx + 1)
                .is_some_and(|l| is_potential_table_row(&l.raw));
            if prev_is_table_candidate || next_is_table_candidate {
                return true;
            }
        }
        false
    }

    /// Scans document lines for fenced code blocks, tables, and badge blocks; caches ranges and parsed items.
    pub fn refresh_document_blocks(&mut self) {
        let code_blocks = find_code_blocks(&self.lines);
        for range in &code_blocks {
            let block_text = self.lines[range.start..=range.end]
                .iter()
                .map(|l| l.raw.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let needs_parse = self.lines.get(range.start).is_none_or(|l| {
                l.code_block_source.as_deref() != Some(block_text.as_str())
                    || l.code_block_items.is_none()
            });
            if needs_parse {
                let items: Vec<markdown::Item> = markdown::parse(&block_text).collect();
                if let Some(first_line) = self.lines.get_mut(range.start) {
                    first_line.code_block_source = Some(block_text);
                    first_line.code_block_items = Some(items);
                }
            }
        }

        let tables = find_tables(&self.lines, &code_blocks);
        for range in &tables {
            let table_text = self.lines[range.start..=range.end]
                .iter()
                .map(|l| l.raw.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let needs_parse = self
                .lines
                .get(range.start)
                .and_then(|l| l.parsed_table.as_ref())
                .is_none_or(|pt| pt.source != table_text);
            if needs_parse {
                let parsed = ParsedTable::parse(table_text, &self.lines[range.start..=range.end]);
                if let Some(first_line) = self.lines.get_mut(range.start) {
                    first_line.parsed_table = Some(parsed);
                }
            }
        }

        let badge_blocks = find_badge_blocks(&self.lines, &code_blocks, &tables);
        self.code_blocks = code_blocks;
        self.tables = tables;
        self.badge_blocks = badge_blocks;
        self.rebuild_line_flags();
    }

    /// Updates a single line and refreshes structure only when the edit can affect it.
    pub fn update_line_preserving_structure(&mut self, idx: usize, new_raw: String) {
        let Some(existing) = self.lines.get(idx) else {
            return;
        };
        if existing.raw == new_raw {
            return;
        }
        let needs_refresh = self.edit_needs_structure_refresh(idx, &existing.raw, &new_raw);
        if let Some(line) = self.lines.get_mut(idx) {
            line.update(new_raw);
        }
        if needs_refresh {
            self.refresh_document_blocks();
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
        self.coalesce_line = None;
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
        self.coalesce_line = None;
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

    /// Commits the active line editor back into the lines list (no-op if unchanged).
    pub fn commit_active_line(&mut self) {
        self.extra_cursors.clear();
        self.coalesce_line = None;
        if let Some(idx) = self.active_line.take()
            && idx < self.lines.len()
        {
            let text = self.active_editor.text();
            let mut changed = false;
            if text.contains('\n') {
                let split: Vec<String> = text
                    .split('\n')
                    .map(|s| s.trim_end_matches('\r').to_string())
                    .collect();
                if split.is_empty() {
                    changed = self.lines[idx].update(String::new());
                } else {
                    self.lines.remove(idx);
                    for (offset, l) in split.into_iter().enumerate() {
                        self.lines.insert(idx + offset, Line::new(l));
                    }
                    changed = true;
                }
            } else if text != self.lines[idx].raw {
                changed = self.lines[idx].update(text);
            }
            if changed {
                self.refresh_document_blocks();
                self.recompute_stats_and_dirty();
            }
        }
    }

    /// Returns the character column of the primary cursor on the active line without allocating the full buffer.
    pub fn active_char_col(&self) -> usize {
        let cur = self.active_editor.cursor().position;
        if let Some(line) = self.active_editor.line(cur.line) {
            let byte_idx = cur.index.min(line.text.len());
            line.text[..byte_idx].chars().count()
        } else {
            0
        }
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
        let mut capacity = self.lines.len().saturating_sub(1); // newlines
        for line in &self.lines {
            capacity += line.raw.len();
        }
        let mut out = String::with_capacity(capacity);
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if self.active_line == Some(i) {
                out.push_str(&self.active_editor.text());
            } else {
                out.push_str(&line.raw);
            }
        }
        out
    }

    fn replace_from_disk(&mut self, text: &str) {
        self.push_undo();
        let text = normalize_text(text);
        self.lines = if text.is_empty() {
            vec![Line::new("")]
        } else {
            text.split('\n').map(Line::new).collect()
        };
        self.active_line = None;
        self.active_editor = text_editor::Content::new();
        self.extra_cursors.clear();
        self.disk_text = text;
        self.is_dirty = false;
        self.external_change = None;
        self.confirm_overwrite = false;
        self.conflict_dismissed = false;
        self.refresh_document_blocks();
        self.recompute_stats_and_dirty();
    }

    fn save_to_disk(&mut self, overwrite: bool) {
        let Some(path) = self.file_path.clone() else {
            tracing::warn!(
                "Tab '{}' has no backing file path yet; pass a file path via CLI to save",
                self.title
            );
            return;
        };
        if !overwrite {
            if let Ok(disk) = std::fs::read_to_string(&path) {
                let disk = normalize_text(&disk);
                if disk != self.disk_text {
                    self.external_change = Some(Some(disk));
                    self.confirm_overwrite = true;
                    self.conflict_dismissed = false;
                    return;
                }
            }
            if self.external_change.is_some() {
                self.confirm_overwrite = true;
                return;
            }
        }
        let text = self.full_text();
        match std::fs::write(&path, &text) {
            Ok(()) => {
                self.file_path = Some(normalized_path(&path));
                self.disk_text = normalize_text(&text);
                self.is_dirty = false;
                self.external_change = None;
                self.confirm_overwrite = false;
                self.conflict_dismissed = false;
                tracing::info!("Saved '{}'", path.display());
            }
            Err(err) => tracing::error!("Failed to save '{}': {err}", path.display()),
        }
    }
}

/// The ClearMind application state.
pub struct ClearMind {
    /// Open tabs.
    tabs: Vec<Tab>,
    /// Index of the active tab.
    active_tab_index: usize,
    /// Next ID for new tabs.
    next_tab_id: usize,
    /// The runtime window ID.
    window_id: Option<window::Id>,
    /// Whether the Find bar is open.
    search_open: bool,
    /// Whether the Replace row inside the Find bar is open.
    replace_open: bool,
    /// Current search query string.
    search_query: String,
    /// Current replacement text string.
    replace_query: String,
    /// All matches `(line_idx, char_start, char_len)` in the active document.
    search_matches: Vec<(usize, usize, usize)>,
    /// Index into `search_matches` for the currently focused match.
    active_match_index: Option<usize>,
    /// Anchor line for Shift+Up/Down multi-line range selection.
    selection_anchor: Option<usize>,
    /// Inclusive line range `(start_line, end_line)` for multi-line block selection.
    selected_lines: Option<(usize, usize)>,
}

/// Messages handled by ClearMind.
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
    /// Close the currently active tab.
    CloseActiveTab,
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
    /// Delete pressed at the end of a line (merges with next line).
    DeleteAtLineEnd,
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
    /// Ctrl + Z: Document-level Undo.
    Undo,
    /// Ctrl + Y / Ctrl + Shift + Z: Document-level Redo.
    Redo,
    /// Ctrl + Alt + Up: Add a multi-line cursor on the line above.
    AddCursorAbove,
    /// Ctrl + Alt + Down: Add a multi-line cursor on the line below.
    AddCursorBelow,
    /// Ctrl + S: Save the active document if it has a file path.
    SaveActiveTab,
    /// Toggle a GFM task list checkbox on the given line.
    ToggleTask(usize, bool),
    /// Paths changed by the external file watcher.
    FilesChanged(Vec<PathBuf>),
    /// Reload the active tab from the changed disk content.
    ReloadExternal,
    /// Keep local edits and dismiss the warning.
    KeepExternal,
    /// Confirm replacing the changed disk file with local content.
    OverwriteExternal,
    /// Cancel a pending overwrite confirmation.
    CancelOverwrite,
    /// Clicked in the empty area below the document text.
    ClickBottomArea,
    /// Clicked an interactive link in the rendered preview.
    LinkClicked(markdown::Uri),
    /// Global keyboard events caught at the window level.
    GlobalKeyEvent(keyboard::Event),
    /// Search input text changed.
    SearchChanged(String),
    /// Replace input text changed.
    ReplaceChanged(String),
    /// Advance to the next search match.
    SearchNext,
    /// Move to the previous search match.
    SearchPrev,
    /// Toggle the Replace row in the search bar.
    ToggleReplaceBar,
    /// Replace the currently active search match.
    ReplaceCurrent,
    /// Replace all matches in the active document.
    ReplaceAll,
    /// Close the search bar.
    CloseSearch,
    /// Open the Find bar (Ctrl+F).
    OpenSearch,
    /// Open the Find & Replace bar (Ctrl+H).
    OpenReplace,
    /// Extend or shrink multi-line selection by `delta` lines.
    SelectRange(isize),
    /// Copy multi-line selection to clipboard.
    CopySelection,
    /// Cut multi-line selection to clipboard.
    CutSelection,
    /// Paste clipboard over multi-line selection.
    PasteSelectedLines,
    /// Clipboard text read for pasting.
    PasteText(String),
    /// Clipboard write completed.
    ClipboardDone,
}

impl ClearMind {
    fn navigate_live_line(&mut self, direction: isize, extend: bool) -> Task<Message> {
        let Some(doc) = self.tabs.get_mut(self.active_tab_index) else {
            return Task::none();
        };
        if doc.lines.is_empty() {
            return Task::none();
        }
        let current_line = doc.active_line.unwrap_or(0).min(doc.lines.len() - 1);
        let target_line = if direction < 0 {
            current_line.saturating_sub(1)
        } else {
            (current_line + 1).min(doc.lines.len() - 1)
        };
        let column = if doc.active_line.is_some() {
            doc.active_char_col()
        } else {
            0
        };
        if extend {
            let anchor = *self.selection_anchor.get_or_insert(current_line);
            self.selected_lines = Some((anchor.min(target_line), anchor.max(target_line)));
        } else {
            self.selection_anchor = None;
            self.selected_lines = None;
        }
        doc.select_line_at_char(target_line, column);
        widget::operation::focus(ACTIVE_EDITOR_ID)
    }

    /// Boots the application, opening CLI file arguments if provided or an empty Markdown file otherwise.
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut tabs = Vec::new();
        let mut next_id = 1;

        for arg in args {
            let path = normalized_path(&PathBuf::from(&arg));
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
            search_open: false,
            replace_open: false,
            search_query: String::new(),
            replace_query: String::new(),
            search_matches: Vec::new(),
            active_match_index: None,
            selection_anchor: None,
            selected_lines: None,
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

    /// Subscription for global keyboard shortcuts, window open events, and external file changes.
    pub fn subscription(&self) -> Subscription<Message> {
        let paths: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter_map(|tab| tab.file_path.clone())
            .collect();
        Subscription::batch([
            keyboard::listen().map(Message::GlobalKeyEvent),
            window::open_events().map(Message::WindowOpened),
            Subscription::run_with(paths, |paths| file_event_stream(paths.clone())),
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
                self.selected_lines = None;
                self.selection_anchor = None;
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
                    if self.search_open {
                        self.recompute_search_matches(false);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::CloseTab(idx) => {
                if idx >= self.tabs.len() {
                    return Task::none();
                }
                self.selected_lines = None;
                self.selection_anchor = None;
                if self.tabs.len() <= 1 {
                    let next_id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs[0] = Tab::new(next_id, "Untitled.md".to_string(), "", None);
                    self.active_tab_index = 0;
                } else {
                    self.tabs.remove(idx);
                    if idx < self.active_tab_index {
                        self.active_tab_index -= 1;
                    } else if self.active_tab_index >= self.tabs.len() {
                        self.active_tab_index = self.tabs.len() - 1;
                    }
                }
                if self.search_open {
                    self.recompute_search_matches(false);
                }
                return widget::operation::focus(ACTIVE_EDITOR_ID);
            }
            Message::CloseActiveTab => {
                return self.update(Message::CloseTab(self.active_tab_index));
            }
            Message::NewTab => {
                self.selected_lines = None;
                self.selection_anchor = None;
                if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
                    current.commit_active_line();
                }
                let tab_id = self.next_tab_id;
                self.next_tab_id += 1;
                let title = format!("Untitled-{tab_id}.md");
                let mut new_tab = Tab::new(tab_id, title, "", None);
                new_tab.select_line(0);
                self.tabs.push(new_tab);
                self.active_tab_index = self.tabs.len() - 1;
                if self.search_open {
                    self.recompute_search_matches(false);
                }
                return widget::operation::focus(ACTIVE_EDITOR_ID);
            }
            Message::SelectLine(idx) => {
                self.selected_lines = None;
                self.selection_anchor = None;
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.select_line(idx);
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::DeselectLine => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                }
                self.selected_lines = None;
                self.selection_anchor = None;
            }
            Message::EditorAction(action) => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    let is_edit = action.is_edit();
                    let had_extra = !doc.extra_cursors.is_empty();

                    if is_edit {
                        self.selected_lines = None;
                        self.selection_anchor = None;
                        let is_coalesced_char = matches!(
                            action,
                            text_editor::Action::Edit(text_editor::Edit::Insert(c))
                            if !c.is_whitespace() && !had_extra
                        );
                        if is_coalesced_char {
                            if doc.coalesce_line != doc.active_line {
                                doc.push_undo();
                                doc.coalesce_line = doc.active_line;
                            }
                        } else {
                            doc.push_undo();
                        }
                    }

                    doc.apply_to_extra_cursors(&action);
                    doc.active_editor.perform(action);

                    // Only inspect/allocate the full editor text when an edit occurred or multi-line buffer is active.
                    if is_edit || doc.active_editor.line_count() > 1 {
                        let editor_text = doc.active_editor.text();
                        if editor_text.contains('\n')
                            && doc.active_editor.cursor().selection.is_none()
                        {
                            let cur_pos = doc.active_editor.cursor().position;
                            let split_lines: Vec<Line> = editor_text
                                .split('\n')
                                .map(|s| Line::new(s.trim_end_matches('\r')))
                                .collect();
                            if let Some(idx) = doc.active_line
                                && idx < doc.lines.len()
                            {
                                let target_line =
                                    cur_pos.line.min(split_lines.len().saturating_sub(1));
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
                                doc.refresh_document_blocks();
                                let target_idx = idx + target_line;
                                doc.active_line = None;
                                doc.select_line_at_char(target_idx, target_char);
                                doc.recompute_stats_and_dirty();
                                if self.search_open {
                                    self.recompute_search_matches(false);
                                }
                                return widget::operation::focus(ACTIVE_EDITOR_ID);
                            }
                        }

                        if is_edit
                            && let Some(idx) = doc.active_line
                            && idx < doc.lines.len()
                        {
                            if had_extra {
                                doc.lines[idx].update(editor_text);
                                doc.refresh_document_blocks();
                            } else {
                                doc.update_line_preserving_structure(idx, editor_text);
                            }
                            doc.recompute_stats_and_dirty();
                            if self.search_open {
                                self.recompute_search_matches(false);
                            }
                        }
                    }
                }
            }
            Message::InsertLineBelow => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.push_undo();
                    doc.extra_cursors.clear();
                    if let Some(idx) = doc.active_line
                        && idx < doc.lines.len()
                    {
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
                        doc.refresh_document_blocks();
                        doc.active_line = None;
                        doc.select_line_at_char(new_idx, 0);
                        doc.recompute_stats_and_dirty();
                        if self.search_open {
                            self.recompute_search_matches(false);
                        }
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
                    doc.push_undo();
                    let current_text = doc.active_editor.text();
                    let prev_chars = doc.lines[idx - 1].raw.chars().count();
                    let merged = format!("{}{}", doc.lines[idx - 1].raw, current_text);
                    doc.lines[idx - 1].update(merged);
                    doc.lines.remove(idx);
                    doc.refresh_document_blocks();
                    doc.active_line = None;
                    doc.select_line_at_char(idx - 1, prev_chars);
                    doc.recompute_stats_and_dirty();
                    if self.search_open {
                        self.recompute_search_matches(false);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::DeleteAtLineEnd => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && let Some(idx) = doc.active_line
                    && idx + 1 < doc.lines.len()
                {
                    doc.push_undo();
                    let current_text = doc.active_editor.text();
                    let cursor_col = doc.active_char_col();
                    let merged = format!("{}{}", current_text, doc.lines[idx + 1].raw);
                    doc.lines[idx].update(merged);
                    doc.lines.remove(idx + 1);
                    doc.refresh_document_blocks();
                    doc.active_line = None;
                    doc.select_line_at_char(idx, cursor_col);
                    doc.recompute_stats_and_dirty();
                    if self.search_open {
                        self.recompute_search_matches(false);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::ToggleTask(line_idx, new_checked) => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && line_idx < doc.lines.len()
                    && let Some(updated_raw) =
                        toggle_task_in_raw(&doc.lines[line_idx].raw, new_checked)
                {
                    doc.push_undo();
                    doc.lines[line_idx].update(updated_raw);
                    doc.recompute_stats_and_dirty();
                }
            }
            Message::Undo => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && doc.undo()
                {
                    if self.search_open {
                        self.recompute_search_matches(false);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::Redo => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && doc.redo()
                {
                    if self.search_open {
                        self.recompute_search_matches(false);
                    }
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                }
            }
            Message::NavigateLineUp => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && doc.active_line.unwrap_or(0) > 0
                {
                    return self.navigate_live_line(-1, false);
                }
            }
            Message::NavigateLineDown => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && doc.active_line.unwrap_or(0) + 1 < doc.lines.len()
                {
                    return self.navigate_live_line(1, false);
                }
            }
            Message::SearchChanged(query) => {
                self.search_query = query;
                self.recompute_search_matches(false);
                return self.scroll_to_active_match(false);
            }
            Message::ReplaceChanged(replacement) => {
                self.replace_query = replacement;
            }
            Message::SearchNext => {
                if !self.search_matches.is_empty() {
                    let next = self
                        .active_match_index
                        .map_or(0, |i| (i + 1) % self.search_matches.len());
                    self.active_match_index = Some(next);
                    return self.scroll_to_active_match(true);
                }
            }
            Message::SearchPrev => {
                if !self.search_matches.is_empty() {
                    let len = self.search_matches.len();
                    let prev = self.active_match_index.map_or(0, |i| (i + len - 1) % len);
                    self.active_match_index = Some(prev);
                    return self.scroll_to_active_match(true);
                }
            }
            Message::ToggleReplaceBar => {
                self.replace_open = !self.replace_open;
                if self.replace_open {
                    return widget::operation::focus(Id::new(REPLACE_INPUT_ID));
                }
            }
            Message::ReplaceCurrent => {
                let Some(match_idx) = self.active_match_index else {
                    return Task::none();
                };
                let Some(&(line_idx, char_start, char_len)) = self.search_matches.get(match_idx)
                else {
                    return Task::none();
                };
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && line_idx < doc.lines.len()
                {
                    doc.commit_active_line();
                    doc.push_undo();
                    let mut raw = doc.lines[line_idx].raw.clone();
                    let start_b = char_to_byte_idx(&raw, char_start);
                    let end_b = char_to_byte_idx(&raw, char_start + char_len);
                    raw.replace_range(start_b..end_b, &self.replace_query);
                    doc.update_line_preserving_structure(line_idx, raw);
                    doc.recompute_stats_and_dirty();
                    self.recompute_search_matches(false);
                    if !self.search_matches.is_empty() {
                        let clamped = match_idx.min(self.search_matches.len() - 1);
                        self.active_match_index = Some(clamped);
                        return self.scroll_to_active_match(false);
                    }
                }
            }
            Message::ReplaceAll => {
                if self.search_query.is_empty() || self.search_matches.is_empty() {
                    return Task::none();
                }
                let needle_lower = self.search_query.to_lowercase();
                let replacement = self.replace_query.clone();
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                    doc.push_undo();
                    for line in &mut doc.lines {
                        let matches = find_case_insensitive_char_matches(&line.raw, &needle_lower);
                        if !matches.is_empty() {
                            let mut updated = line.raw.clone();
                            for &(char_start, char_len) in matches.iter().rev() {
                                let start_b = char_to_byte_idx(&updated, char_start);
                                let end_b = char_to_byte_idx(&updated, char_start + char_len);
                                updated.replace_range(start_b..end_b, &replacement);
                            }
                            line.update(updated);
                        }
                    }
                    doc.refresh_document_blocks();
                    doc.recompute_stats_and_dirty();
                    self.recompute_search_matches(false);
                }
            }
            Message::CloseSearch => {
                self.search_open = false;
                self.replace_open = false;
                self.search_matches.clear();
                self.active_match_index = None;
                return widget::operation::focus(ACTIVE_EDITOR_ID);
            }
            Message::OpenSearch => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(sel) = doc.active_editor.selection()
                        && !sel.is_empty()
                        && !sel.contains('\n')
                    {
                        self.search_query = sel;
                    }
                    doc.commit_active_line();
                }
                self.search_open = true;
                self.recompute_search_matches(false);
                return widget::operation::focus(Id::new(SEARCH_INPUT_ID));
            }
            Message::OpenReplace => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(sel) = doc.active_editor.selection()
                        && !sel.is_empty()
                        && !sel.contains('\n')
                    {
                        self.search_query = sel;
                    }
                    doc.commit_active_line();
                }
                self.search_open = true;
                self.replace_open = true;
                self.recompute_search_matches(false);
                return widget::operation::focus(Id::new(SEARCH_INPUT_ID));
            }
            Message::SelectRange(delta) => {
                return self.navigate_live_line(delta, true);
            }
            Message::CopySelection => {
                let text = if let Some((start, end)) = self.selected_lines {
                    self.tabs.get(self.active_tab_index).map(|doc| {
                        let end_clamped = end.min(doc.lines.len().saturating_sub(1));
                        doc.lines[start.min(end_clamped)..=end_clamped]
                            .iter()
                            .map(|line| line.raw.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                } else {
                    self.tabs
                        .get(self.active_tab_index)
                        .and_then(|doc| doc.active_editor.selection())
                };
                if let Some(text) = text {
                    return iced::clipboard::write(text).map(|_| Message::ClipboardDone);
                }
            }
            Message::CutSelection => {
                if let Some((start, end)) = self.selected_lines
                    && let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                {
                    doc.commit_active_line();
                    doc.push_undo();
                    let end_clamped = end.min(doc.lines.len().saturating_sub(1));
                    let start_clamped = start.min(end_clamped);
                    let copied = doc.lines[start_clamped..=end_clamped]
                        .iter()
                        .map(|line| line.raw.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    doc.lines.drain(start_clamped..=end_clamped);
                    if doc.lines.is_empty() {
                        doc.lines.push(Line::new(""));
                    }
                    doc.refresh_document_blocks();
                    let target = start_clamped.min(doc.lines.len() - 1);
                    doc.active_line = None;
                    doc.select_line(target);
                    doc.recompute_stats_and_dirty();
                    self.selected_lines = None;
                    self.selection_anchor = None;
                    return Task::batch([
                        iced::clipboard::write(copied).map(|_| Message::ClipboardDone),
                        widget::operation::focus(ACTIVE_EDITOR_ID),
                    ]);
                }
            }
            Message::PasteSelectedLines => {
                return iced::clipboard::read_text().map(|result| {
                    Message::PasteText(
                        result.map_or_else(|_| String::new(), |text| (*text).clone()),
                    )
                });
            }
            Message::PasteText(text) => {
                if let Some((start, end)) = self.selected_lines
                    && let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                {
                    doc.commit_active_line();
                    doc.push_undo();
                    let end_clamped = end.min(doc.lines.len().saturating_sub(1));
                    let start_clamped = start.min(end_clamped);
                    let replacement: Vec<Line> = text
                        .trim_end_matches('\n')
                        .split('\n')
                        .map(|s| Line::new(s.trim_end_matches('\r')))
                        .collect();
                    doc.lines.splice(start_clamped..=end_clamped, replacement);
                    if doc.lines.is_empty() {
                        doc.lines.push(Line::new(""));
                    }
                    doc.refresh_document_blocks();
                    let target = start_clamped.min(doc.lines.len() - 1);
                    doc.active_line = None;
                    doc.select_line(target);
                    doc.recompute_stats_and_dirty();
                    self.selected_lines = None;
                    self.selection_anchor = None;
                    return widget::operation::focus(ACTIVE_EDITOR_ID);
                } else if !text.is_empty() {
                    return self.update(Message::EditorAction(text_editor::Action::Edit(
                        text_editor::Edit::Paste(Arc::new(text)),
                    )));
                }
            }
            Message::ClipboardDone => {}
            Message::DuplicateLineOrSelection => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    if doc.active_line.is_none() {
                        doc.select_line(0);
                    }
                    doc.push_undo();

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
                            doc.refresh_document_blocks();
                            doc.recompute_stats_and_dirty();
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
                        doc.refresh_document_blocks();
                        doc.active_line = None;
                        doc.select_line_at_char(new_idx, char_col);
                        doc.recompute_stats_and_dirty();
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::MoveLineUp => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.push_undo();
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
                            doc.refresh_document_blocks();
                            let new_active = active_idx - 1;
                            doc.active_line = Some(new_active);
                            for (line_idx, _) in &mut doc.extra_cursors {
                                *line_idx -= 1;
                            }
                            doc.active_editor =
                                text_editor::Content::with_text(&doc.lines[new_active].raw);
                            doc.active_editor.move_to(cur);
                            doc.recompute_stats_and_dirty();
                        }
                        return widget::operation::focus(ACTIVE_EDITOR_ID);
                    }
                }
            }
            Message::MoveLineDown => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.push_undo();
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
                            doc.refresh_document_blocks();
                            let new_active = active_idx + 1;
                            doc.active_line = Some(new_active);
                            for (line_idx, _) in &mut doc.extra_cursors {
                                *line_idx += 1;
                            }
                            doc.active_editor =
                                text_editor::Content::with_text(&doc.lines[new_active].raw);
                            doc.active_editor.move_to(cur);
                            doc.recompute_stats_and_dirty();
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
                    doc.refresh_document_blocks();
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
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.save_to_disk(false);
                }
            }
            Message::OverwriteExternal => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.save_to_disk(true);
                }
            }
            Message::CancelOverwrite => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.confirm_overwrite = false;
                }
            }
            Message::KeepExternal => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.conflict_dismissed = true;
                    doc.confirm_overwrite = false;
                }
            }
            Message::ReloadExternal => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    match doc.external_change.take() {
                        Some(Some(text)) => doc.replace_from_disk(&text),
                        Some(None) => doc.replace_from_disk(""),
                        None => {}
                    }
                }
            }
            Message::FilesChanged(paths) => {
                for doc in &mut self.tabs {
                    let Some(file_path) = doc.file_path.as_deref() else {
                        continue;
                    };
                    if !paths.iter().any(|path| paths_equal(path, file_path)) {
                        continue;
                    }
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(text) => Some(normalize_text(&text)),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        Err(error) => {
                            tracing::warn!("Could not read '{}': {error}", file_path.display());
                            continue;
                        }
                    };
                    if content.as_deref() == Some(doc.disk_text.as_str()) {
                        continue;
                    }
                    if !doc.is_dirty {
                        if let Some(text) = content.as_deref() {
                            doc.replace_from_disk(text);
                        } else {
                            doc.external_change = Some(None);
                            doc.conflict_dismissed = false;
                        }
                    } else {
                        doc.external_change = Some(content);
                        doc.confirm_overwrite = false;
                        doc.conflict_dismissed = false;
                    }
                }
            }
            Message::ClickBottomArea => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                    let last_idx = doc.lines.len().saturating_sub(1);
                    if !doc.lines.is_empty() && !doc.lines[last_idx].raw.is_empty() {
                        doc.push_undo();
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
            Message::GlobalKeyEvent(event) => match event {
                keyboard::Event::ModifiersChanged(modifiers) => {
                    if modifiers.shift() && self.selection_anchor.is_none() {
                        self.selection_anchor = self
                            .tabs
                            .get(self.active_tab_index)
                            .and_then(|doc| doc.active_line);
                    }
                    if !modifiers.shift() {
                        self.selection_anchor = None;
                    }
                }
                keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                } => {
                    let has_line_selection = self.selected_lines.is_some();
                    if let Some(msg) = match_shortcut(
                        &key,
                        physical_key,
                        modifiers,
                        has_line_selection,
                        self.search_open,
                    ) {
                        return self.update(msg);
                    }

                    // Escape closes search when search is open, or deselects line when editor is unfocused
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        if self.search_open {
                            return self.update(Message::CloseSearch);
                        }
                        return self.update(Message::DeselectLine);
                    }

                    // Line navigation shortcuts when no active line editor is mounted
                    let has_active_line = self
                        .tabs
                        .get(self.active_tab_index)
                        .is_some_and(|doc| doc.active_line.is_some());
                    if !has_active_line && !self.search_open {
                        if modifiers.control()
                            && !modifiers.alt()
                            && !modifiers.shift()
                            && !modifiers.logo()
                        {
                            match key.as_ref() {
                                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                    return self.navigate_live_line(-1, false);
                                }
                                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                                    return self.navigate_live_line(1, false);
                                }
                                _ => {}
                            }
                        } else if modifiers.shift()
                            && !modifiers.control()
                            && !modifiers.alt()
                            && !modifiers.logo()
                        {
                            match key.as_ref() {
                                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                                    return self.navigate_live_line(-1, true);
                                }
                                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                                    return self.navigate_live_line(1, true);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            },
        }

        Task::none()
    }

    /// Recomputes all case-insensitive search matches across the active document.
    fn recompute_search_matches(&mut self, reset_to_first: bool) {
        self.search_matches.clear();
        let Some(doc) = self.tabs.get(self.active_tab_index) else {
            self.active_match_index = None;
            return;
        };
        if self.search_query.is_empty() {
            self.active_match_index = None;
            return;
        }
        let needle_lower = self.search_query.to_lowercase();
        for (line_idx, line) in doc.lines.iter().enumerate() {
            for (char_col, char_len) in find_case_insensitive_char_matches(&line.raw, &needle_lower)
            {
                self.search_matches.push((line_idx, char_col, char_len));
            }
        }
        if self.search_matches.is_empty() {
            self.active_match_index = None;
        } else if reset_to_first || self.active_match_index.is_none() {
            self.active_match_index = Some(0);
        } else if let Some(idx) = self.active_match_index {
            self.active_match_index = Some(idx.min(self.search_matches.len() - 1));
        }
    }

    /// Scrolls the viewport to the active search match; optionally selects the line when navigating explicitly.
    fn scroll_to_active_match(&mut self, select_in_editor: bool) -> Task<Message> {
        let Some(match_idx) = self.active_match_index else {
            return Task::none();
        };
        let Some(&(line_idx, char_col, _)) = self.search_matches.get(match_idx) else {
            return Task::none();
        };
        let total_lines = self
            .tabs
            .get(self.active_tab_index)
            .map_or(1, |doc| doc.lines.len().max(1));
        if select_in_editor && let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
            doc.select_line_at_char(line_idx, char_col);
        }
        let rel_y = if total_lines <= 1 {
            0.0
        } else {
            (line_idx as f32 / (total_lines - 1) as f32).clamp(0.0, 1.0)
        };
        widget::operation::scrollable::snap_to(
            Id::new(DOCUMENT_SCROLL_ID),
            scrollable::RelativeOffset { x: 0.0, y: rel_y },
            widget::operation::Animation::Instant,
        )
    }

    /// Renders the Obsidian-style live edit user interface with tabs and in-place line editing.
    pub fn view(&self) -> impl Widget<Message> + '_ {
        let theme_val = cached_theme();
        let code_viewer = GitHubPrimerViewer { theme: theme_val };
        let markdown_settings = markdown::Settings::default();
        let has_line_selection = self.selected_lines.is_some();
        let search_open = self.search_open;

        // 1. Top Tab Bar
        let mut tab_row: iced::widget::Row<iced::Element<'_, Message>> = iced::widget::Row::new()
            .spacing(4)
            .align_y(alignment::Vertical::Center);

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = idx == self.active_tab_index;

            let mut tab_label = row![
                icons::file_text().size(13).color(if is_active {
                    Color::from_rgb8(31, 58, 50)
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
            .align_y(alignment::Vertical::Center);
            if tab.external_change.is_some() {
                tab_label = tab_label.push(
                    text("⚠")
                        .size(12)
                        .color(Color::from_rgb8(154, 103, 0))
                        .boxed(),
                );
            } else if tab.is_dirty {
                tab_label = tab_label.push(
                    text("•")
                        .size(14)
                        .color(Color::from_rgb8(31, 58, 50))
                        .boxed(),
                );
            }
            let tab_title_click = mouse_area(tab_label)
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
            container(space().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleMaximize);

        let min_btn = button(
            container(
                icons::minus()
                    .size(13)
                    .align_x(alignment::Horizontal::Center),
            )
            .center_y(Length::Fill),
        )
        .height(Length::Fill)
        .padding([0, 14])
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
            container(
                icons::square()
                    .size(11)
                    .align_x(alignment::Horizontal::Center),
            )
            .center_y(Length::Fill),
        )
        .height(Length::Fill)
        .padding([0, 14])
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

        let close_btn = button(
            container(icons::x().size(13).align_x(alignment::Horizontal::Center))
                .center_y(Length::Fill),
        )
        .height(Length::Fill)
        .padding([0, 16])
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
            .height(Length::Fill)
            .spacing(0)
            .align_y(alignment::Vertical::Center);

        let top_tab_bar = container(
            row![tab_row, drag_area, window_controls]
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(alignment::Vertical::Center)
                .spacing(4),
        )
        .width(Length::Fill)
        .height(36)
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
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

        // 2. Document Lines (Virtualized Live Preview with raw markdown on line click)
        let active_tab = self.tabs.get(self.active_tab_index);
        let active_match_line = self
            .active_match_index
            .and_then(|i| self.search_matches.get(i))
            .map(|&(line, _, _)| line);

        let mut lines_column: iced::widget::Column<iced::Element<'_, Message>> =
            iced::widget::Column::new().spacing(3).width(820);

        if let Some(doc) = active_tab {
            let cur = doc.active_editor.cursor();
            let line_count = doc.active_editor.line_count().max(1);
            let is_at_line_start = cur.position.index == 0 && cur.selection.is_none();
            let last_subline_len = doc
                .active_editor
                .line(cur.position.line)
                .map_or(0, |l| l.text.len());
            let is_at_line_end = cur.selection.is_none()
                && cur.position.line + 1 >= line_count
                && cur.position.index >= last_subline_len;
            let is_on_first_subline = cur.position.line == 0;
            let is_on_last_subline = cur.position.line + 1 >= line_count;
            let has_extra_cursors = !doc.extra_cursors.is_empty();

            let code_blocks = &doc.code_blocks;
            let tables = &doc.tables;
            let badge_blocks = &doc.badge_blocks;
            let mut cb_cursor = 0;
            let mut tb_cursor = 0;
            let mut bb_cursor = 0;
            let mut idx = 0;

            while idx < doc.lines.len() {
                let flags = doc.line_flags.get(idx).copied().unwrap_or(0);

                // 1. Complete fenced code block starting at idx
                if flags & FLAG_CODE_START != 0 {
                    while cb_cursor < code_blocks.len() && code_blocks[cb_cursor].start < idx {
                        cb_cursor += 1;
                    }
                    if let Some(range) = code_blocks.get(cb_cursor).filter(|r| r.start == idx) {
                        let is_any_line_active = doc
                            .active_line
                            .is_some_and(|active| active >= range.start && active <= range.end);
                        let has_any_extra_cursor = doc
                            .extra_cursors
                            .iter()
                            .any(|(l, _)| *l >= range.start && *l <= range.end);

                        if !is_any_line_active
                            && !has_any_extra_cursor
                            && !self.selected_lines.is_some_and(|(start, end)| {
                                start <= range.end && end >= range.start
                            })
                            && let Some(items) = doc.lines[range.start].code_block_items.as_ref()
                        {
                            let rendered =
                                markdown::view_with(items, markdown_settings, &code_viewer);
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
                }

                // 2. Complete Markdown table starting at idx (using cached ParsedTable!)
                if flags & FLAG_TABLE_START != 0 {
                    while tb_cursor < tables.len() && tables[tb_cursor].start < idx {
                        tb_cursor += 1;
                    }
                    if let Some(range) = tables.get(tb_cursor).filter(|r| r.start == idx) {
                        let is_any_line_active = doc
                            .active_line
                            .is_some_and(|active| active >= range.start && active <= range.end);
                        let has_any_extra_cursor = doc
                            .extra_cursors
                            .iter()
                            .any(|(l, _)| *l >= range.start && *l <= range.end);

                        if !is_any_line_active
                            && !has_any_extra_cursor
                            && !self.selected_lines.is_some_and(|(start, end)| {
                                start <= range.end && end >= range.start
                            })
                            && let Some(parsed_table) = doc.lines[range.start].parsed_table.as_ref()
                        {
                            let column_count = parsed_table.column_count;
                            let cell_width = 810.0 / column_count as f32;
                            let usable_chars =
                                ((cell_width - 20.0) / 7.4).floor().max(1.0) as usize;
                            let mut table = column![].width(Length::Fill);

                            for (row_index, cells) in parsed_table.rows.iter().enumerate() {
                                let is_header = row_index == 0;
                                let mut table_row =
                                    row![].width(Length::Fill).align_y(alignment::Vertical::Top);
                                let max_visual_lines = (0..column_count)
                                    .map(|col_idx| {
                                        let len =
                                            cells.get(col_idx).map_or(0, |cell| cell.char_len);
                                        len.div_ceil(usable_chars).max(1)
                                    })
                                    .max()
                                    .unwrap_or(1);
                                let row_height = (max_visual_lines as f32 * 19.0 + 16.0).max(38.0);

                                for column_index in 0..column_count {
                                    let horiz_align = parsed_table
                                        .alignments
                                        .get(column_index)
                                        .copied()
                                        .unwrap_or(alignment::Horizontal::Left);
                                    let cell_elem: iced::Element<'_, Message> =
                                        if let Some(cell) = cells.get(column_index) {
                                            if cell.has_rich_formatting && !cell.items.is_empty() {
                                                markdown::view(
                                                    &cell.items,
                                                    markdown::Settings::with_text_size(13),
                                                    theme_val.clone(),
                                                )
                                                .map(Message::LinkClicked)
                                                .boxed()
                                            } else {
                                                text(&cell.raw)
                                                    .size(13)
                                                    .font(if is_header {
                                                        Font {
                                                            weight: iced::font::Weight::Bold,
                                                            ..Font::DEFAULT
                                                        }
                                                    } else {
                                                        Font::DEFAULT
                                                    })
                                                    .color(Color::from_rgb8(31, 35, 40))
                                                    .boxed()
                                            }
                                        } else {
                                            space().width(Length::Shrink).boxed()
                                        };

                                    let background = if is_header {
                                        Color::from_rgb8(240, 243, 246)
                                    } else if row_index % 2 == 1 {
                                        Color::from_rgb8(255, 255, 255)
                                    } else {
                                        Color::from_rgb8(246, 248, 250)
                                    };

                                    table_row = table_row.push(
                                        container(cell_elem)
                                            .width(Length::FillPortion(1))
                                            .height(Length::Fixed(row_height))
                                            .align_x(horiz_align)
                                            .align_y(alignment::Vertical::Center)
                                            .padding([7, 10])
                                            .style(move |_| container::Style {
                                                background: Some(Background::Color(background)),
                                                border: Border::default()
                                                    .color(Color::from_rgb8(208, 215, 222))
                                                    .width(if is_header { 1.0 } else { 0.5 }),
                                                ..Default::default()
                                            }),
                                    );
                                }
                                table = table.push(table_row);
                            }

                            let table_container =
                                container(table).width(Length::Fill).padding([4, 0]);

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
                }

                // 3. Pure shields.io badge block starting at idx
                if flags & FLAG_BADGE_START != 0 {
                    while bb_cursor < badge_blocks.len() && badge_blocks[bb_cursor].start < idx {
                        bb_cursor += 1;
                    }
                    if let Some(range) = badge_blocks.get(bb_cursor).filter(|r| r.start == idx) {
                        let is_any_line_active = doc
                            .active_line
                            .is_some_and(|active| active >= range.start && active <= range.end);
                        let has_any_extra_cursor = doc
                            .extra_cursors
                            .iter()
                            .any(|(l, _)| *l >= range.start && *l <= range.end);

                        if !is_any_line_active
                            && !has_any_extra_cursor
                            && !self.selected_lines.is_some_and(|(start, end)| {
                                start <= range.end && end >= range.start
                            })
                        {
                            let mut badge_row =
                                row![].spacing(6).align_y(alignment::Vertical::Center);
                            let mut has_badges = false;
                            for line_i in range.start..=range.end {
                                if let Some(ref badges) = doc.lines[line_i].badges {
                                    for badge in badges {
                                        badge_row = badge_row.push(render_badge(badge));
                                        has_badges = true;
                                    }
                                }
                            }

                            if has_badges {
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
                }

                let line = &doc.lines[idx];
                let is_selected = self
                    .selected_lines
                    .is_some_and(|(start, end)| idx >= start && idx <= end);
                let is_search_highlighted = self.search_open
                    && !self.search_query.is_empty()
                    && self.search_matches.iter().any(|&(l, _, _)| l == idx);
                let is_active_search_match = active_match_line == Some(idx);
                let is_inside_code_block = flags & FLAG_IN_CODE != 0;
                let is_inside_table = flags & FLAG_IN_TABLE != 0;

                if doc.active_line == Some(idx) {
                    // Primary active line: RAW MARKDOWN text editor
                    let active_input = text_editor(&doc.active_editor)
                        .id(ACTIVE_EDITOR_ID)
                        .font(Font::MONOSPACE)
                        .on_action(Message::EditorAction)
                        .padding([2, 4])
                        .key_binding(move |event| {
                            if !event.is_focused {
                                return None;
                            }
                            // Suppress global shortcuts inside text_editor so GlobalKeyEvent dispatches them once.
                            if match_shortcut(
                                &event.key,
                                event.physical_key,
                                event.modifiers,
                                has_line_selection,
                                search_open,
                            )
                            .is_some()
                            {
                                return None;
                            }

                            if event.modifiers.control()
                                && !event.modifiers.alt()
                                && !event.modifiers.shift()
                            {
                                if event.key == keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::NavigateLineUp,
                                    ));
                                }
                                if event.key
                                    == keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::NavigateLineDown,
                                    ));
                                }
                            }
                            if event.modifiers.shift()
                                && !event.modifiers.control()
                                && !event.modifiers.alt()
                            {
                                if event.key == keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::SelectRange(-1),
                                    ));
                                }
                                if event.key
                                    == keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                                {
                                    return Some(text_editor::Binding::Custom(
                                        Message::SelectRange(1),
                                    ));
                                }
                            }
                            if event.key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                                return Some(text_editor::Binding::Custom(if search_open {
                                    Message::CloseSearch
                                } else {
                                    Message::DeselectLine
                                }));
                            }

                            if event.key == keyboard::Key::Named(keyboard::key::Named::Enter) {
                                return Some(text_editor::Binding::Custom(
                                    Message::InsertLineBelow,
                                ));
                            }

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

                            if event.key == keyboard::Key::Named(keyboard::key::Named::Delete)
                                && !event.modifiers.command()
                                && !event.modifiers.alt()
                                && !event.modifiers.shift()
                                && is_at_line_end
                                && !has_extra_cursors
                                && idx + 1 < doc.lines.len()
                            {
                                return Some(text_editor::Binding::Custom(
                                    Message::DeleteAtLineEnd,
                                ));
                            }

                            if !event.modifiers.command()
                                && !event.modifiers.alt()
                                && !has_extra_cursors
                            {
                                if event.key == keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                                    && is_on_first_subline
                                    && idx > 0
                                {
                                    return Some(text_editor::Binding::Custom(
                                        if event.modifiers.shift() {
                                            Message::SelectRange(-1)
                                        } else {
                                            Message::NavigateLineUp
                                        },
                                    ));
                                }
                                if event.key
                                    == keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                                    && is_on_last_subline
                                {
                                    return Some(text_editor::Binding::Custom(
                                        if event.modifiers.shift() {
                                            Message::SelectRange(1)
                                        } else {
                                            Message::NavigateLineDown
                                        },
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
                            selection: Color::from_rgba8(31, 58, 50, 0.25),
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
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(if is_selected {
                            Color::from_rgba8(31, 58, 50, 0.16)
                        } else {
                            Color::from_rgba8(31, 58, 50, 0.06)
                        })),
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
                                .color(Color::from_rgb8(31, 58, 50)),
                            container(
                                row![
                                    text(before)
                                        .font(Font::MONOSPACE)
                                        .color(Color::from_rgb8(31, 35, 40)),
                                    text("│")
                                        .font(Font::MONOSPACE)
                                        .color(Color::from_rgb8(31, 58, 50)),
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
                        background: Some(Background::Color(Color::from_rgba8(31, 58, 50, 0.06))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    let interactive_multi = mouse_area(multi_cursor_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_multi.boxed());
                } else if is_inside_code_block {
                    // Inactive line inside an expanded code block: GitHub Primer Light monospace
                    let is_fence = flags & (FLAG_CODE_START | FLAG_CODE_END) != 0;
                    let text_color = if is_fence {
                        Color::from_rgb8(110, 119, 129) // Primer --color-prettylights-syntax-comment (#6e7781)
                    } else {
                        Color::from_rgb8(31, 35, 40) // Primer --fgColor-default (#1f2328)
                    };

                    let code_row = if line.raw.is_empty() {
                        container(
                            row![space().width(11), space().width(Length::Fill).height(18)]
                                .spacing(6)
                                .align_y(alignment::Vertical::Center),
                        )
                    } else {
                        container(
                            row![
                                space().width(11),
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
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(if is_selected {
                            Color::from_rgba8(31, 58, 50, 0.14)
                        } else if is_active_search_match {
                            Color::from_rgba8(255, 196, 0, 0.28)
                        } else if is_search_highlighted {
                            Color::from_rgba8(255, 220, 100, 0.18)
                        } else {
                            Color::from_rgb8(246, 248, 250) // Primer --bgColor-muted (#f6f8fa)
                        })),
                        border: Border::default()
                            .rounded(3.0)
                            .color(Color::from_rgb8(208, 215, 222)) // Primer --borderColor-default (#d0d7de)
                            .width(0.5),
                        ..Default::default()
                    });

                    let interactive_code_line = mouse_area(code_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_code_line.boxed());
                } else if is_inside_table {
                    // Inactive line inside an expanded table: monospace text with cached delimiter flag
                    let is_delimiter = flags & FLAG_TABLE_DELIMITER != 0;
                    let text_color = if is_delimiter {
                        Color::from_rgb8(101, 109, 118)
                    } else {
                        Color::from_rgb8(31, 35, 40)
                    };

                    let table_row = container(
                        row![
                            space().width(11),
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
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(if is_selected {
                            Color::from_rgba8(31, 58, 50, 0.14)
                        } else {
                            Color::from_rgb8(246, 248, 250)
                        })),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    let interactive_table_line = mouse_area(table_row)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_table_line.boxed());
                } else if let Some(ref task) = line.task_item {
                    // Interactive GFM Task List item (- [ ] / - [x])
                    let indent_px = (task.indent as f32 * 8.0).min(120.0);
                    let cb = checkbox(task.checked)
                        .size(15)
                        .on_toggle(move |checked| Message::ToggleTask(idx, checked));
                    let body_widget: iced::Element<'_, Message> = if task.body_items.is_empty() {
                        space().width(Length::Fill).height(18).boxed()
                    } else {
                        markdown::view(&task.body_items, markdown_settings, theme_val.clone())
                            .map(Message::LinkClicked)
                            .boxed()
                    };
                    let clickable_body = mouse_area(container(body_widget).width(Length::Fill))
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    let task_row = container(
                        row![space().width(indent_px), cb, clickable_body,]
                            .spacing(8)
                            .align_y(alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([2, 4])
                    .style(move |_| container::Style {
                        background: if is_selected {
                            Some(Background::Color(Color::from_rgba8(31, 58, 50, 0.14)))
                        } else if is_active_search_match {
                            Some(Background::Color(Color::from_rgba8(255, 196, 0, 0.28)))
                        } else if is_search_highlighted {
                            Some(Background::Color(Color::from_rgba8(255, 220, 100, 0.18)))
                        } else {
                            None
                        },
                        ..Default::default()
                    });

                    lines_column = lines_column.push(task_row.boxed());
                } else if let Some((img_title, img_target)) = line.image.as_ref() {
                    // Standalone image or Obsidian attachment: clean GitHub Light image card
                    let img_widget = render_image_card(img_title, img_target);
                    let interactive = mouse_area(img_widget)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive.boxed());
                } else if line.is_pure_badge {
                    // Standalone line containing shields.io badges
                    let mut badge_row = row![].spacing(6).align_y(alignment::Vertical::Center);
                    if let Some(ref badges) = line.badges {
                        for badge in badges {
                            badge_row = badge_row.push(render_badge(badge));
                        }
                    }

                    let badge_container = container(badge_row).width(Length::Fill).padding([4, 6]);

                    let interactive = mouse_area(badge_container)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive.boxed());
                } else if parse_opening_fence(&line.raw).is_some() {
                    // Unclosed fence: render as monospace text
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
                    // Inactive line: formatted Markdown rendering with GitHub Heading dividers and preserved indentation
                    let rendered =
                        markdown::view_with(&line.items, markdown_settings, &code_viewer);
                    let indent_left = (line.indent_spaces as f32 * 6.0).min(160.0);

                    let is_h1 = line.raw.starts_with("# ");
                    let is_h2 = line.raw.starts_with("## ");
                    let row_bg = if is_selected {
                        Some(Background::Color(Color::from_rgba8(31, 58, 50, 0.14)))
                    } else if is_active_search_match {
                        Some(Background::Color(Color::from_rgba8(255, 196, 0, 0.28)))
                    } else if is_search_highlighted {
                        Some(Background::Color(Color::from_rgba8(255, 220, 100, 0.18)))
                    } else {
                        None
                    };

                    let line_widget = if is_h1 || is_h2 {
                        let divider = container(space().width(Length::Fill).height(1))
                            .width(Length::Fill)
                            .style(|_theme| container::Style {
                                background: Some(Background::Color(Color::from_rgb8(
                                    216, 222, 228,
                                ))),
                                ..Default::default()
                            });
                        container(column![rendered, divider].spacing(4).width(Length::Fill))
                            .width(Length::Fill)
                            .padding([1, 4])
                            .style(move |_| container::Style {
                                background: row_bg,
                                ..Default::default()
                            })
                            .boxed()
                    } else {
                        container(rendered)
                            .width(Length::Fill)
                            .padding(Padding {
                                top: 1.0,
                                right: 4.0,
                                bottom: 1.0,
                                left: 4.0 + indent_left,
                            })
                            .style(move |_| container::Style {
                                background: row_bg,
                                ..Default::default()
                            })
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
            .id(Id::new(DOCUMENT_SCROLL_ID))
            .width(Length::Fill)
            .height(Length::Fill);

        let mut main_column: iced::widget::Column<iced::Element<'_, Message>> =
            column![top_tab_bar]
                .width(Length::Fill)
                .height(Length::Fill);

        if let Some(doc) = active_tab
            && doc.external_change.is_some()
            && (!doc.conflict_dismissed || doc.confirm_overwrite)
        {
            let message = if doc.confirm_overwrite {
                "This file changed on disk. Overwrite those changes?"
            } else if doc.external_change == Some(None) {
                "This file was deleted on disk. Reload or keep your buffer."
            } else {
                "This file changed on disk. Saving will overwrite those changes."
            };
            let action_buttons = if doc.confirm_overwrite {
                row![
                    button(text("Overwrite").size(12)).on_press(Message::OverwriteExternal),
                    button(text("Cancel").size(12)).on_press(Message::CancelOverwrite),
                    button(text("Reload from disk").size(12)).on_press(Message::ReloadExternal),
                ]
                .spacing(6)
            } else {
                row![
                    button(text("Reload from disk").size(12)).on_press(Message::ReloadExternal),
                    button(text("Keep my edits").size(12)).on_press(Message::KeepExternal),
                ]
                .spacing(6)
            };
            let banner = container(
                row![text(message).size(13), action_buttons]
                    .spacing(12)
                    .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding([8, 12])
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgb8(255, 248, 220))),
                border: Border::default()
                    .color(Color::from_rgb8(220, 190, 100))
                    .width(1.0),
                ..Default::default()
            });
            main_column = main_column.push(banner.boxed());
        }

        if self.search_open {
            let match_status = if self.search_query.is_empty() {
                String::new()
            } else if let Some(idx) = self.active_match_index {
                format!("{} of {}", idx + 1, self.search_matches.len())
            } else {
                "0 matches".to_string()
            };

            let find_row = row![
                button(text(if self.replace_open { "▾" } else { "▸" }).size(12))
                    .style(button::subtle)
                    .padding([4, 6])
                    .on_press(Message::ToggleReplaceBar),
                text("Find").size(12).color(Color::from_rgb8(101, 109, 118)),
                text_input("Find in document...", &self.search_query)
                    .id(Id::new(SEARCH_INPUT_ID))
                    .size(13)
                    .padding([5, 8])
                    .on_input(Message::SearchChanged)
                    .on_submit(Message::SearchNext)
                    .width(Length::Fixed(260.0)),
                text(match_status)
                    .size(12)
                    .color(Color::from_rgb8(101, 109, 118)),
                button(text("↑").size(12))
                    .style(button::subtle)
                    .padding([4, 8])
                    .on_press(Message::SearchPrev),
                button(text("↓").size(12))
                    .style(button::subtle)
                    .padding([4, 8])
                    .on_press(Message::SearchNext),
                space().width(Length::Fill),
                button(text("×").size(14))
                    .style(button::subtle)
                    .padding([2, 8])
                    .on_press(Message::CloseSearch),
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center);

            let mut search_col = column![find_row].spacing(6);
            if self.replace_open {
                let replace_row = row![
                    space().width(22),
                    text("Replace")
                        .size(12)
                        .color(Color::from_rgb8(101, 109, 118)),
                    text_input("Replace with...", &self.replace_query)
                        .id(Id::new(REPLACE_INPUT_ID))
                        .size(13)
                        .padding([5, 8])
                        .on_input(Message::ReplaceChanged)
                        .on_submit(Message::ReplaceCurrent)
                        .width(Length::Fixed(260.0)),
                    button(text("Replace").size(12))
                        .style(button::subtle)
                        .padding([4, 10])
                        .on_press(Message::ReplaceCurrent),
                    button(text("Replace all").size(12))
                        .style(button::subtle)
                        .padding([4, 10])
                        .on_press(Message::ReplaceAll),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center);
                search_col = search_col.push(replace_row.boxed());
            }

            let search_panel = container(search_col)
                .width(Length::Fill)
                .padding([6, 12])
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
                    border: Border::default()
                        .color(Color::from_rgb8(208, 215, 222))
                        .width(0.5),
                    ..Default::default()
                });
            main_column = main_column.push(search_panel.boxed());
        }

        main_column = main_column.push(main_doc_scroll.boxed());

        // 3. Bottom Status Bar
        if let Some(doc) = active_tab {
            let file_status = match doc.file_path.as_ref() {
                Some(p) => {
                    let state = if doc.is_dirty {
                        " • Modified"
                    } else {
                        " • Saved"
                    };
                    format!("{}{}", p.display(), state)
                }
                None => {
                    if doc.is_dirty {
                        format!("{} • Unsaved", doc.title)
                    } else {
                        doc.title.clone()
                    }
                }
            };

            let cursor_info = if let Some(line_idx) = doc.active_line {
                let col = doc.active_char_col() + 1;
                if doc.extra_cursors.is_empty() {
                    format!("Ln {}, Col {}", line_idx + 1, col)
                } else {
                    format!(
                        "Ln {}, Col {} (+{} cursors)",
                        line_idx + 1,
                        col,
                        doc.extra_cursors.len()
                    )
                }
            } else {
                format!("{} lines", doc.lines.len())
            };

            let stats_info = format!(
                "{} words  •  {} chars  •  {}",
                doc.word_count, doc.char_count, cursor_info
            );

            let status_bar = container(
                row![
                    text(file_status)
                        .size(11)
                        .color(Color::from_rgb8(101, 109, 118)),
                    space().width(Length::Fill),
                    text(stats_info)
                        .size(11)
                        .color(Color::from_rgb8(101, 109, 118)),
                ]
                .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding([4, 12])
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
                border: Border::default()
                    .color(Color::from_rgb8(208, 215, 222))
                    .width(0.5),
                ..Default::default()
            });

            main_column = main_column.push(status_bar.boxed());
        }

        container(main_column.width(Length::Fill).height(Length::Fill))
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

/// Matches application-level keyboard shortcuts.
fn match_shortcut(
    key: &keyboard::Key,
    physical_key: keyboard::key::Physical,
    modifiers: keyboard::Modifiers,
    has_line_selection: bool,
    search_open: bool,
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

    // Ctrl + Shift + Z -> Redo
    if modifiers.command()
        && !modifiers.alt()
        && modifiers.shift()
        && let Some('z') = key.to_latin(physical_key)
    {
        return Some(Message::Redo);
    }

    // Command/Ctrl shortcuts
    if modifiers.command() && !modifiers.alt() && !modifiers.shift() {
        match key.to_latin(physical_key) {
            Some('n') => return Some(Message::NewTab),
            Some('w') => return Some(Message::CloseActiveTab),
            Some('f') => return Some(Message::OpenSearch),
            Some('h') => return Some(Message::OpenReplace),
            Some('s') => return Some(Message::SaveActiveTab),
            // Only intercept editor-wide shortcuts when search input is not focused
            Some('d') if !search_open => return Some(Message::DuplicateLineOrSelection),
            Some('a') if !search_open => return Some(Message::SelectAllMarkdown),
            Some('z') if !search_open => return Some(Message::Undo),
            Some('y') if !search_open => return Some(Message::Redo),
            // Only intercept C/X/V when a multi-line Shift+Arrow block is selected
            Some('c') if has_line_selection => return Some(Message::CopySelection),
            Some('x') if has_line_selection => return Some(Message::CutSelection),
            Some('v') if has_line_selection => return Some(Message::PasteSelectedLines),
            _ => {}
        }
    }

    None
}

/// GitHub Primer Light Default (`--color-prettylights-syntax-*`) syntax highlighter for Markdown code blocks.
struct GitHubPrimerLightHighlighter;

impl text::Highlighter<Code, Theme> for GitHubPrimerLightHighlighter {
    fn id(&self) -> &str {
        "github-primer-light"
    }

    fn highlight(&self, code: Code, _theme: &Theme) -> text::highlighter::Style {
        let color = match code {
            // --color-prettylights-syntax-comment (pl-c): #6e7781
            Code::Comment => Some(Color::from_rgb8(110, 119, 129)),
            // --color-prettylights-syntax-keyword (pl-k): #cf222e
            Code::Keyword => Some(Color::from_rgb8(207, 34, 46)),
            // --color-prettylights-syntax-string (pl-s / pl-pds): #0a3069
            Code::String => Some(Color::from_rgb8(10, 48, 105)),
            // --color-prettylights-syntax-constant (pl-c1): #0550ae
            Code::Constant | Code::Support => Some(Color::from_rgb8(5, 80, 174)),
            // --color-prettylights-syntax-entity (pl-e / pl-en): #8250df
            Code::Function => Some(Color::from_rgb8(130, 80, 223)),
            // --color-prettylights-syntax-variable (pl-v / pl-smw): #953800
            Code::Type | Code::Variable => Some(Color::from_rgb8(149, 56, 0)),
            // --color-prettylights-syntax-entity-tag (pl-ent): #116329
            Code::Path => Some(Color::from_rgb8(17, 99, 41)),
            // --color-prettylights-syntax-invalid-illegal-text: #82071e
            Code::Invalid => Some(Color::from_rgb8(130, 7, 30)),
            // --fgColor-default: #1f2328
            Code::Punctuation | Code::Other => Some(Color::from_rgb8(31, 35, 40)),
        };

        text::highlighter::Style {
            color,
            style: (code == Code::Comment).then_some(iced::font::Style::Italic),
        }
    }
}

/// Custom Markdown [`markdown::Viewer`] that renders fenced code blocks using GitHub Primer Light Default.
struct GitHubPrimerViewer<'a> {
    theme: &'a Theme,
}

impl<'a> markdown::Viewer<'a, Message> for GitHubPrimerViewer<'a> {
    fn theme(&self) -> &Theme {
        self.theme
    }

    fn highlighter(&self) -> &dyn text::Highlighter<Code, Theme> {
        &GitHubPrimerLightHighlighter
    }

    fn on_link_click(url: markdown::Uri) -> Message {
        Message::LinkClicked(url)
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [markdown::Text],
    ) -> impl Widget<Message> + 'a {
        let padding = settings.code_block_size / 0.85 * 0.75;

        container(
            scrollable(column(lines.iter().map(|line| {
                rich_text(line.spans(settings, self.theme(), self.highlighter()))
                    .on_link_click(Self::on_link_click)
                    .font(settings.code_block_font)
                    .size(settings.code_block_size)
                    .line_height(settings.line_height)
                    .selectable(true)
            })))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default()
                    .width(padding / 2.0)
                    .scroller_width(padding / 2.0),
            ))
            .spacing(padding),
        )
        .width(Length::Fill)
        .padding(padding)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(246, 248, 250))), // Primer --bgColor-muted (#f6f8fa)
            text_color: Some(Color::from_rgb8(31, 35, 40)), // Primer --fgColor-default (#1f2328)
            border: Border::default()
                .rounded(6.0)
                .color(Color::from_rgb8(208, 215, 222)) // Primer --borderColor-default (#d0d7de)
                .width(1.0),
            ..Default::default()
        })
    }
}

fn theme(_state: &ClearMind) -> Theme {
    cached_theme().clone()
}

fn style(_state: &ClearMind, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::from_rgb8(255, 255, 255), // GitHub Light canvas (#ffffff)
        text_color: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
    }
}

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt::try_init();

    iced::application(ClearMind::boot, ClearMind::update, ClearMind::view)
        .title("ClearMind")
        .subscription(ClearMind::subscription)
        .theme(theme)
        .style(style)
        .font(Font::new("Segoe UI"))
        .fonts([icons::FONT_BYTES])
        .window(iced::window::Settings {
            size: Size::new(960.0, 720.0),
            min_size: Some(Size::new(500.0, 400.0)),
            decorations: false,
            ..Default::default()
        })
        .run()
}
