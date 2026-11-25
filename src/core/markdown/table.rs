//! Escape- and code-span-aware GFM table parser, formatter, and interactive mutation helpers.

/// Column alignment in a GFM table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableAlignment {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A parsed GFM table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTable {
    /// Header cells.
    pub headers: Vec<String>,
    /// Column alignments parsed from the delimiter row.
    pub alignments: Vec<TableAlignment>,
    /// Data rows (each normalized to `headers.len()` columns).
    pub rows: Vec<Vec<String>>,
}

/// Direction for moving a row or column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Backward,
    Forward,
}

/// Splits a GFM table row into trimmed cell strings, respecting `\|` escapes
/// and backtick code spans (including multi-backtick spans).
pub fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or_else(|| trimmed.strip_prefix('|').unwrap_or(trimmed));

    // Handle trailing escaped pipe `\|` that shouldn't have been stripped.
    let inner = if trimmed.ends_with("\\|") && !inner.ends_with('\\') {
        trimmed.strip_prefix('|').unwrap_or(trimmed)
    } else {
        inner
    };

    let bytes = inner.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut backtick_fence: Option<usize> = None;

    while i < bytes.len() {
        if bytes[i] == b'\\' && backtick_fence.is_none() {
            // Skip escaped character (e.g., `\|` or `\\`).
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            let count = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            match backtick_fence {
                None => backtick_fence = Some(count),
                Some(open) if open == count => backtick_fence = None,
                Some(_) => {}
            }
            i += count;
            continue;
        }
        if bytes[i] == b'|' && backtick_fence.is_none() {
            cells.push(inner[start..i].trim().to_string());
            start = i + 1;
        }
        i += 1;
    }

    cells.push(inner[start..].trim().to_string());
    cells
}

/// Parses a GFM delimiter row (e.g. `| :--- | :---: | ---: | --- |`).
pub fn parse_delimiter_row(line: &str) -> Option<Vec<TableAlignment>> {
    if !line.contains('-') {
        return None;
    }
    let cells = split_table_row(line);
    if cells.is_empty() {
        return None;
    }
    let mut alignments = Vec::with_capacity(cells.len());
    for cell in cells {
        let c = cell.trim();
        if c.is_empty() {
            return None;
        }
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let after_left = c.strip_prefix(':').unwrap_or(c);
        let core = after_left.strip_suffix(':').unwrap_or(after_left);
        if core.is_empty() || !core.chars().all(|ch| ch == '-') {
            return None;
        }
        let align = match (left, right) {
            (true, true) => TableAlignment::Center,
            (true, false) => TableAlignment::Left,
            (false, true) => TableAlignment::Right,
            (false, false) => TableAlignment::None,
        };
        alignments.push(align);
    }
    Some(alignments)
}

/// Parses a slice of lines into a `ParsedTable`.
/// Requires at least a header line and a valid delimiter line.
pub fn parse_table(lines: &[&str]) -> Option<ParsedTable> {
    if lines.len() < 2 || !lines[0].contains('|') {
        return None;
    }
    let headers = split_table_row(lines[0]);
    let alignments = parse_delimiter_row(lines[1])?;
    if headers.is_empty() || headers.len() != alignments.len() {
        return None;
    }

    let col_count = headers.len();
    let mut rows = Vec::with_capacity(lines.len().saturating_sub(2));
    for &line in &lines[2..] {
        if line.trim().is_empty() || !line.contains('|') {
            break;
        }
        let mut cells = split_table_row(line);
        cells.resize(col_count, String::new());
        rows.push(cells);
    }

    Some(ParsedTable {
        headers,
        alignments,
        rows,
    })
}

/// Formats a `ParsedTable` into aligned, pipe-delimited Markdown lines.
pub fn format_table_pipes(table: &ParsedTable) -> Vec<String> {
    let cols = table.headers.len();
    if cols == 0 {
        return Vec::new();
    }

    let mut widths = vec![3usize; cols];
    for (c, h) in table.headers.iter().enumerate() {
        widths[c] = widths[c].max(h.chars().count());
    }
    for row in &table.rows {
        for (c, cell) in row.iter().take(cols).enumerate() {
            widths[c] = widths[c].max(cell.chars().count());
        }
    }

    let mut out = Vec::with_capacity(table.rows.len() + 2);
    out.push(format_row(&table.headers, &widths));

    let delim_cells: Vec<String> = (0..cols)
        .map(|c| {
            let w = widths[c];
            let align = table.alignments.get(c).copied().unwrap_or_default();
            match align {
                TableAlignment::None => "-".repeat(w),
                TableAlignment::Left => format!(":{}", "-".repeat(w.saturating_sub(1).max(2))),
                TableAlignment::Right => format!("{}:", "-".repeat(w.saturating_sub(1).max(2))),
                TableAlignment::Center => format!(":{}:", "-".repeat(w.saturating_sub(2).max(1))),
            }
        })
        .collect();
    out.push(format!("| {} |", delim_cells.join(" | ")));

    for row in &table.rows {
        out.push(format_row(row, &widths));
    }
    out
}

fn format_row(cells: &[String], widths: &[usize]) -> String {
    let padded: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let cell = cells.get(i).map(String::as_str).unwrap_or("");
            let len = cell.chars().count();
            if len < w {
                format!("{cell}{}", " ".repeat(w - len))
            } else {
                cell.to_string()
            }
        })
        .collect();
    format!("| {} |", padded.join(" | "))
}

impl ParsedTable {
    /// Inserts an empty data row at `row_idx` (clamped to `0..=rows.len()`).
    pub fn insert_row(&mut self, row_idx: usize) {
        let idx = row_idx.min(self.rows.len());
        self.rows.insert(idx, vec![String::new(); self.headers.len()]);
    }

    /// Deletes data row at `row_idx` if it exists.
    pub fn delete_row(&mut self, row_idx: usize) -> bool {
        if row_idx < self.rows.len() {
            self.rows.remove(row_idx);
            true
        } else {
            false
        }
    }

    /// Moves data row at `row_idx` one step in `dir`.
    pub fn move_row(&mut self, row_idx: usize, dir: MoveDirection) -> bool {
        let target = match dir {
            MoveDirection::Backward if row_idx > 0 => row_idx - 1,
            MoveDirection::Forward if row_idx + 1 < self.rows.len() => row_idx + 1,
            _ => return false,
        };
        if row_idx < self.rows.len() {
            self.rows.swap(row_idx, target);
            true
        } else {
            false
        }
    }

    /// Inserts an empty column at `col_idx` (clamped to `0..=headers.len()`).
    pub fn insert_column(&mut self, col_idx: usize, header: impl Into<String>) {
        let idx = col_idx.min(self.headers.len());
        self.headers.insert(idx, header.into());
        self.alignments.insert(idx, TableAlignment::None);
        for row in &mut self.rows {
            row.insert(idx, String::new());
        }
    }

    /// Deletes column at `col_idx` if more than one column remains.
    pub fn delete_column(&mut self, col_idx: usize) -> bool {
        if self.headers.len() <= 1 || col_idx >= self.headers.len() {
            return false;
        }
        self.headers.remove(col_idx);
        self.alignments.remove(col_idx);
        for row in &mut self.rows {
            if col_idx < row.len() {
                row.remove(col_idx);
            }
        }
        true
    }

    /// Moves column at `col_idx` one step in `dir`.
    pub fn move_column(&mut self, col_idx: usize, dir: MoveDirection) -> bool {
        let target = match dir {
            MoveDirection::Backward if col_idx > 0 => col_idx - 1,
            MoveDirection::Forward if col_idx + 1 < self.headers.len() => col_idx + 1,
            _ => return false,
        };
        if col_idx < self.headers.len() {
            self.headers.swap(col_idx, target);
            self.alignments.swap(col_idx, target);
            for row in &mut self.rows {
                if col_idx < row.len() && target < row.len() {
                    row.swap(col_idx, target);
                }
            }
            true
        } else {
            false
        }
    }

    /// Sets alignment for column `col_idx`.
    pub fn set_column_alignment(&mut self, col_idx: usize, alignment: TableAlignment) -> bool {
        if let Some(slot) = self.alignments.get_mut(col_idx) {
            *slot = alignment;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_pipes_respecting_escapes_and_code_spans() {
        let row = r"| `a | b` | c \| d | ``e | f`` |";
        let cells = split_table_row(row);
        assert_eq!(cells, vec!["`a | b`", r"c \| d", "``e | f``"]);
    }

    #[test]
    fn parses_formats_and_mutates_table() {
        let lines = vec![
            "| Name | Score |",
            "| :--- | ---: |",
            "| Alice | 10 |",
        ];
        let mut table = parse_table(&lines).unwrap();
        assert_eq!(
            table.alignments,
            vec![TableAlignment::Left, TableAlignment::Right]
        );
        table.insert_row(1);
        table.rows[1] = vec!["Bob".into(), "20".into()];
        table.move_row(1, MoveDirection::Backward);
        assert_eq!(table.rows[0][0], "Bob");

        let formatted = format_table_pipes(&table);
        assert_eq!(formatted[0], "| Name  | Score |");
        assert_eq!(formatted[1], "| :---- | ----: |");
    }
}
