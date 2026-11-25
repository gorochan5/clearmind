//! Smart Markdown editing, auto-pair delimiters, 6-rule formatting, and multi-cursor helpers.

use super::models::{Cursor, DocumentSelection, Edit, Tab, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatCommand {
    Bold,
    Italic,
    InlineCode,
    Strikethrough,
    Link,
    Heading(u8),
    ClearHeading,
    Blockquote,
    BulletList,
    Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartEditResult {
    pub edits: Vec<Edit>,
    pub selection_after: DocumentSelection,
    pub extra_cursors_after: Vec<Cursor>,
    pub active_line_after: Option<usize>,
    pub is_coalesceable: bool,
}

/// Deduplicate and sort cursors in ascending order.
pub fn dedup_cursors(primary: Cursor, extras: &mut Vec<Cursor>) {
    extras.retain(|c| *c != primary);
    extras.sort();
    extras.dedup();
}

/// Return all cursors sorted in descending `(line, col)` order for safe multi-cursor edits.
pub fn cursors_descending(primary: Cursor, extras: &[Cursor]) -> Vec<Cursor> {
    let mut all = Vec::with_capacity(1 + extras.len());
    all.push(primary);
    all.extend_from_slice(extras);
    all.sort_by(|a, b| b.cmp(a));
    all.dedup();
    all
}

/// Smart `Home` (`SoftHome`): toggles between start of content (after indent/list/quote prefix) and col 0.
pub fn soft_home_col(line_text: &str, current_col: usize) -> usize {
    let trimmed_ws = line_text.len() - line_text.trim_start_matches([' ', '\t']).len();
    let after_ws = &line_text[trimmed_ws..];
    let prefix_len = if let Some(rest) = after_ws.strip_prefix("- [ ] ").or_else(|| after_ws.strip_prefix("- [x] ")) {
        let _ = rest;
        6
    } else if after_ws.starts_with("- ") || after_ws.starts_with("* ") || after_ws.starts_with("> ") {
        2
    } else {
        0
    };
    let content_col = line_text[..trimmed_ws + prefix_len].chars().count();
    if current_col == content_col {
        0
    } else {
        content_col
    }
}

/// Smart `Enter` continuation prefix for lists, tasks, blockquotes, and indentation.
pub fn smart_enter_insertion(line_text: &str) -> String {
    let indent_bytes = line_text.len() - line_text.trim_start_matches([' ', '\t']).len();
    let indent = &line_text[..indent_bytes];
    let rest = &line_text[indent_bytes..];

    if let Some(after) = rest.strip_prefix("- [ ] ").or_else(|| rest.strip_prefix("- [x] ")) {
        if after.trim().is_empty() {
            return "\n".to_string();
        }
        return format!("\n{indent}- [ ] ");
    }
    if let Some(after) = rest.strip_prefix("- ") {
        if after.trim().is_empty() {
            return "\n".to_string();
        }
        return format!("\n{indent}- ");
    }
    if let Some(after) = rest.strip_prefix("* ") {
        if after.trim().is_empty() {
            return "\n".to_string();
        }
        return format!("\n{indent}* ");
    }
    if let Some(after) = rest.strip_prefix("> ") {
        if after.trim().is_empty() {
            return "\n".to_string();
        }
        return format!("\n{indent}> ");
    }
    if let Some(dot_idx) = rest.find(". ")
        && let Ok(num) = rest[..dot_idx].parse::<usize>()
    {
        let after = &rest[dot_idx + 2..];
        if after.trim().is_empty() {
            return "\n".to_string();
        }
        return format!("\n{indent}{}. ", num + 1);
    }
    format!("\n{indent}")
}

/// Auto-pair delimiter lookup. Returns `Some((open, close))` if `ch` should auto-pair.
pub fn auto_pair_for(ch: char, prev_char: Option<char>, next_char: Option<char>, in_code_block: bool) -> Option<(char, char)> {
    match ch {
        '(' => Some(('(', ')')),
        '[' => Some(('[', ']')),
        '{' => Some(('{', '}')),
        '"' => {
            if prev_char.is_some_and(|c| c.is_alphanumeric()) {
                None
            } else {
                Some(('"', '"'))
            }
        }
        '`' if !in_code_block => Some(('`', '`')),
        '$' if !in_code_block => {
            if next_char.is_some_and(|c| c.is_ascii_digit()) {
                None
            } else {
                Some(('$', '$'))
            }
        }
        _ => None,
    }
}

/// Apply a 6-rule inline or block formatting command to the active tab.
pub fn apply_format_command(tab: &mut Tab, cmd: FormatCommand, timestamp_ms: u64) {
    let sel = tab.selection;
    let (start_cur, end_cur) = sel.ordered();
    let start_byte = tab.line_col_to_byte(start_cur);
    let end_byte = tab.line_col_to_byte(end_cur);
    let selected = tab.rope.slice(
        tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte),
    ).to_string();

    let (inserted, new_sel) = match cmd {
        FormatCommand::Bold => wrap_inline("**", "**", &selected, start_cur, end_cur),
        FormatCommand::Italic => wrap_inline("*", "*", &selected, start_cur, end_cur),
        FormatCommand::InlineCode => wrap_inline("`", "`", &selected, start_cur, end_cur),
        FormatCommand::Strikethrough => wrap_inline("~~", "~~", &selected, start_cur, end_cur),
        FormatCommand::Link => {
            let text = if selected.is_empty() { "text" } else { &selected };
            let ins = format!("[{text}](url)");
            let c = Cursor::new(start_cur.line, start_cur.col + text.chars().count() + 3);
            (ins, DocumentSelection::collapsed(c))
        }
        FormatCommand::Heading(level) => {
            let hashes = "#".repeat(level.clamp(1, 6) as usize);
            let line_start = tab.line_col_to_byte(Cursor::new(start_cur.line, 0));
            let line_str = tab.rope.line(start_cur.line).to_string();
            let trimmed = line_str.trim_start_matches('#').trim_start();
            let new_line = format!("{hashes} {trimmed}");
            let tx = Transaction {
                edits: vec![Edit {
                    start_byte: line_start,
                    end_byte: line_start + line_str.len(),
                    deleted: line_str,
                    inserted: new_line,
                }],
                selection_before: sel,
                selection_after: sel,
                extra_cursors_before: tab.extra_cursors.clone(),
                extra_cursors_after: tab.extra_cursors.clone(),
                active_line_before: tab.active_line,
                active_line_after: tab.active_line,
                scroll_before: tab.scroll_offset,
                timestamp_ms,
                is_coalesceable: false,
            };
            tab.apply_transaction(tx);
            return;
        }
        FormatCommand::ClearHeading => {
            let line_start = tab.line_col_to_byte(Cursor::new(start_cur.line, 0));
            let line_str = tab.rope.line(start_cur.line).to_string();
            let trimmed = line_str.trim_start_matches('#').trim_start().to_string();
            let tx = Transaction {
                edits: vec![Edit {
                    start_byte: line_start,
                    end_byte: line_start + line_str.len(),
                    deleted: line_str,
                    inserted: trimmed,
                }],
                selection_before: sel,
                selection_after: sel,
                extra_cursors_before: tab.extra_cursors.clone(),
                extra_cursors_after: tab.extra_cursors.clone(),
                active_line_before: tab.active_line,
                active_line_after: tab.active_line,
                scroll_before: tab.scroll_offset,
                timestamp_ms,
                is_coalesceable: false,
            };
            tab.apply_transaction(tx);
            return;
        }
        FormatCommand::Blockquote => {
            let ins = format!("> {selected}");
            (ins, DocumentSelection::collapsed(Cursor::new(start_cur.line, start_cur.col + 2)))
        }
        FormatCommand::BulletList => {
            let ins = format!("- {selected}");
            (ins, DocumentSelection::collapsed(Cursor::new(start_cur.line, start_cur.col + 2)))
        }
        FormatCommand::Table => {
            let ins = "| Col 1 | Col 2 |\n| --- | --- |\n|  |  |".to_string();
            (ins, DocumentSelection::collapsed(Cursor::new(start_cur.line, 2)))
        }
    };

    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted: selected,
            inserted,
        }],
        selection_before: sel,
        selection_after: new_sel,
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: tab.extra_cursors.clone(),
        active_line_before: tab.active_line,
        active_line_after: tab.active_line,
        scroll_before: tab.scroll_offset,
        timestamp_ms,
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn wrap_inline(
    open: &str,
    close: &str,
    selected: &str,
    start_cur: Cursor,
    end_cur: Cursor,
) -> (String, DocumentSelection) {
    if selected.starts_with(open) && selected.ends_with(close) && selected.len() >= open.len() + close.len() {
        let unwrapped = &selected[open.len()..selected.len() - close.len()];
        let new_end = Cursor::new(end_cur.line, end_cur.col.saturating_sub(open.len() + close.len()));
        return (
            unwrapped.to_string(),
            DocumentSelection {
                anchor: start_cur,
                head: new_end,
            },
        );
    }
    let ins = format!("{open}{selected}{close}");
    let caret = if selected.is_empty() {
        Cursor::new(start_cur.line, start_cur.col + open.chars().count())
    } else {
        Cursor::new(end_cur.line, end_cur.col + open.chars().count() + close.chars().count())
    };
    (ins, DocumentSelection::collapsed(caret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soft_home_and_smart_enter() {
        assert_eq!(soft_home_col("  - [ ] hello", 10), 8);
        assert_eq!(soft_home_col("  - [ ] hello", 8), 0);
        assert_eq!(smart_enter_insertion("  - item"), "\n  - ");
        assert_eq!(smart_enter_insertion("  - "), "\n");
        assert_eq!(smart_enter_insertion("1. first"), "\n2. ");
    }

    #[test]
    fn test_dedup_cursors_descending() {
        let mut extras = vec![Cursor::new(1, 2), Cursor::new(3, 0), Cursor::new(1, 2)];
        dedup_cursors(Cursor::new(1, 2), &mut extras);
        assert_eq!(extras, vec![Cursor::new(3, 0)]);
        let desc = cursors_descending(Cursor::new(1, 2), &extras);
        assert_eq!(desc, vec![Cursor::new(3, 0), Cursor::new(1, 2)]);
    }
}
