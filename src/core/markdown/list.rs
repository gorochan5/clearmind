//! Ordered, unordered, and task list parsing plus auto-renumbering.

/// Tab width in spaces (1 tab = 4 spaces = 24px).
pub const TAB_SPACES: usize = 4;
/// Pixel width per indentation level (24px).
pub const INDENT_PX_PER_LEVEL: f32 = 24.0;

/// Kind of list marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    /// Unordered bullet (`-`, `*`, `+`).
    Unordered(char),
    /// Ordered list item with its numeric value and delimiter (`.` or `)`).
    Ordered { number: u64, delimiter: char },
    /// Task list item (`- [ ]` or `- [x]` / `- [X]`).
    Task { marker: char, checked: bool },
}

/// A parsed list item on a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Leading indentation measured in spaces (tabs expanded to 4 spaces).
    pub indent_spaces: usize,
    /// Nesting depth (`indent_spaces / 4`).
    pub depth: usize,
    /// Marker kind.
    pub kind: ListKind,
    /// Byte offset in the line where the item's content begins.
    pub content_start: usize,
    /// Trimmed content text after the marker.
    pub content: String,
}

impl ListItem {
    /// Returns the bullet glyph for unordered lists based on nesting depth (`•`, `◦`, `▪`).
    pub fn bullet_glyph(&self) -> &'static str {
        match self.depth % 3 {
            0 => "•",
            1 => "◦",
            _ => "▪",
        }
    }

    /// Returns the horizontal indentation in pixels (`depth * 24.0`).
    pub fn indent_px(&self) -> f32 {
        self.depth as f32 * INDENT_PX_PER_LEVEL
    }
}

/// Measures leading indentation in spaces, expanding `\t` to `TAB_SPACES` (4).
pub fn measure_indent_spaces(line: &str) -> (usize, usize) {
    let mut spaces = 0usize;
    let mut byte_idx = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => {
                spaces += 1;
                byte_idx += 1;
            }
            '\t' => {
                spaces += TAB_SPACES;
                byte_idx += 1;
            }
            _ => break,
        }
    }
    (spaces, byte_idx)
}

/// Parses a single line as a list item (unordered, task, or ordered).
pub fn parse_list_item(line: &str) -> Option<ListItem> {
    let trimmed_end = line.trim_end_matches(['\r', '\n']);
    let (indent_spaces, start) = measure_indent_spaces(trimmed_end);
    let rest = &trimmed_end[start..];
    let depth = indent_spaces / TAB_SPACES;

    // Unordered or task list: `- `, `* `, `+ `
    let mut chars = rest.chars();
    let first = chars.next()?;
    if matches!(first, '-' | '*' | '+') {
        let after_marker = &rest[1..];
        if after_marker.starts_with(' ') || after_marker.starts_with('\t') {
            let after_ws = &after_marker[1..];
            // Check for task list `[ ] ` or `[x] ` or `[X] `
            if after_ws.len() >= 3
                && after_ws.starts_with('[')
                && after_ws.as_bytes()[2] == b']'
            {
                let check_ch = after_ws.as_bytes()[1];
                if matches!(check_ch, b' ' | b'x' | b'X') {
                    let after_box = &after_ws[3..];
                    if after_box.is_empty()
                        || after_box.starts_with(' ')
                        || after_box.starts_with('\t')
                    {
                        let content_offset = if after_box.is_empty() { 0 } else { 1 };
                        let content_start = start + 1 + 1 + 3 + content_offset;
                        return Some(ListItem {
                            indent_spaces,
                            depth,
                            kind: ListKind::Task {
                                marker: first,
                                checked: check_ch != b' ',
                            },
                            content_start,
                            content: trimmed_end[content_start..].to_string(),
                        });
                    }
                }
            }

            let content_start = start + 2;
            return Some(ListItem {
                indent_spaces,
                depth,
                kind: ListKind::Unordered(first),
                content_start,
                content: trimmed_end[content_start..].to_string(),
            });
        }
    }

    // Ordered list: `1. ` or `1) `
    let digits_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits_len > 0 && digits_len <= 9 {
        let after_digits = &rest[digits_len..];
        let delim = after_digits.chars().next()?;
        if matches!(delim, '.' | ')') {
            let after_delim = &after_digits[1..];
            if after_delim.starts_with(' ') || after_delim.starts_with('\t') {
                let number: u64 = rest[..digits_len].parse().ok()?;
                let content_start = start + digits_len + 2;
                return Some(ListItem {
                    indent_spaces,
                    depth,
                    kind: ListKind::Ordered {
                        number,
                        delimiter: delim,
                    },
                    content_start,
                    content: trimmed_end[content_start..].to_string(),
                });
            }
        }
    }

    None
}

/// Renumbers contiguous ordered list items at each indentation depth starting from 1
/// (or from the first item's number if `preserve_start` is true).
pub fn renumber_ordered_list(lines: &[String], preserve_start: bool) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    // Stack of (indent_spaces, next_number)
    let mut counters: Vec<(usize, u64)> = Vec::new();

    for line in lines {
        if let Some(item) = parse_list_item(line) {
            if let ListKind::Ordered { number, delimiter } = item.kind {
                // Pop deeper levels
                while counters
                    .last()
                    .is_some_and(|&(indent, _)| indent > item.indent_spaces)
                {
                    counters.pop();
                }
                let next_num = if let Some(top) = counters
                    .last_mut()
                    .filter(|(indent, _)| *indent == item.indent_spaces)
                {
                    let n = top.1;
                    top.1 += 1;
                    n
                } else {
                    let start_num = if preserve_start { number } else { 1 };
                    counters.push((item.indent_spaces, start_num + 1));
                    start_num
                };

                let (_, start_byte) = measure_indent_spaces(line);
                let indent_prefix = &line[..start_byte];
                let content = &line[item.content_start..];
                out.push(format!("{indent_prefix}{next_num}{delimiter} {content}"));
                continue;
            }
        } else if !line.trim().is_empty() {
            counters.clear();
        }
        out.push(line.clone());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unordered_task_and_ordered_items() {
        let u = parse_list_item("  - hello").unwrap();
        assert_eq!(u.kind, ListKind::Unordered('-'));
        assert_eq!(u.depth, 0);
        assert_eq!(u.bullet_glyph(), "•");

        let t = parse_list_item("\t* [x] done").unwrap();
        assert_eq!(
            t.kind,
            ListKind::Task {
                marker: '*',
                checked: true
            }
        );
        assert_eq!(t.depth, 1);
        assert_eq!(t.indent_px(), 24.0);

        let o = parse_list_item("12) item").unwrap();
        assert_eq!(
            o.kind,
            ListKind::Ordered {
                number: 12,
                delimiter: ')'
            }
        );
    }

    #[test]
    fn renumbers_ordered_list_preserving_nesting() {
        let input = vec![
            "5. first".to_string(),
            "9. second".to_string(),
            "    3. nested a".to_string(),
            "    7. nested b".to_string(),
            "2. third".to_string(),
        ];
        let renumbered = renumber_ordered_list(&input, false);
        assert_eq!(
            renumbered,
            vec![
                "1. first",
                "2. second",
                "    1. nested a",
                "    2. nested b",
                "3. third",
            ]
        );
    }
}
