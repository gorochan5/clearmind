//! Cohesive multi-line Markdown block scanner, document-wide `ReferenceDefinitionIndex`,
//! and bidirectional `DisplayMap` (`to_display` / `to_source`).

use std::collections::HashMap;

use crate::core::markdown::badge::{is_pure_badge_line, parse_badges_in_line, ParsedBadge};
use crate::core::markdown::callout::{parse_callout_at, CalloutBlock};
use crate::core::markdown::code_block::{
    is_fence_close, parse_fence_open, FenceKind, UnclosedFence,
};
use crate::core::markdown::list::{parse_list_item, ListItem};
use crate::core::markdown::table::{parse_table, ParsedTable};

/// Document-wide reference link definition index (`[label]: url "title"`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceDefinitionIndex {
    /// Normalized lowercase label -> `(url, optional_title)`.
    pub definitions: HashMap<String, (String, Option<String>)>,
}

impl ReferenceDefinitionIndex {
    /// Resolves a reference label (case-insensitive, trimmed).
    pub fn get(&self, label: &str) -> Option<&(String, Option<String>)> {
        self.definitions.get(&label.trim().to_ascii_lowercase())
    }
}

/// Footnote definition (`[^id]: text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteDefinition {
    pub id: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

/// High-level block classification produced by `scan_blocks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScannedBlock {
    Frontmatter {
        start_line: usize,
        end_line: usize,
        raw: String,
    },
    CodeBlock {
        start_line: usize,
        end_line: usize,
        kind: FenceKind,
        fence_len: usize,
        language: Option<String>,
        unclosed: bool,
    },
    Mermaid {
        start_line: usize,
        end_line: usize,
        source: String,
        unclosed: bool,
    },
    DisplayMath {
        start_line: usize,
        end_line: usize,
        latex: String,
        unclosed: bool,
    },
    Callout(CalloutBlock),
    Table {
        start_line: usize,
        end_line: usize,
        table: ParsedTable,
    },
    List {
        start_line: usize,
        end_line: usize,
        items: Vec<(usize, ListItem)>,
    },
    SetextHeading {
        start_line: usize,
        end_line: usize,
        level: u8,
        text: String,
    },
    ThematicBreak {
        line: usize,
    },
    FootnoteDefinition(FootnoteDefinition),
    BadgeBlock {
        line: usize,
        badges: Vec<ParsedBadge>,
    },
    Paragraph {
        start_line: usize,
        end_line: usize,
    },
}

/// Full document scan result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentScan {
    pub blocks: Vec<ScannedBlock>,
    pub references: ReferenceDefinitionIndex,
    pub unclosed_fence: Option<UnclosedFence>,
}

/// Scans a Markdown document (line slice) into cohesive blocks and a document-wide
/// `ReferenceDefinitionIndex`.
pub fn scan_blocks(lines: &[&str]) -> DocumentScan {
    let mut scan = DocumentScan::default();
    if lines.is_empty() {
        return scan;
    }

    let mut i = 0usize;

    // 1. YAML frontmatter at line 0 (`---` ... `---` or `...`)
    if lines[0].trim() == "---"
        && let Some(end_idx) = (1..lines.len()).find(|&idx| {
            let t = lines[idx].trim();
            t == "---" || t == "..."
        })
    {
            let raw = lines[1..end_idx].join("\n");
            scan.blocks.push(ScannedBlock::Frontmatter {
                start_line: 0,
                end_line: end_idx,
                raw,
            });
            i = end_idx + 1;
        }

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // 2. Reference link definition `[label]: url "title"`
        if let Some((label, url, title)) = parse_reference_definition(line) {
            scan.references.definitions.insert(label, (url, title));
            i += 1;
            continue;
        }

        // 3. Footnote definition `[^id]: ...`
        if let Some((id, first_text)) = parse_footnote_start(line) {
            let start_line = i;
            let mut end_line = i;
            let mut body = vec![first_text];
            i += 1;
            while i < lines.len() && (lines[i].starts_with("    ") || lines[i].starts_with('\t')) {
                body.push(lines[i].trim().to_string());
                end_line = i;
                i += 1;
            }
            scan.blocks
                .push(ScannedBlock::FootnoteDefinition(FootnoteDefinition {
                    id,
                    start_line,
                    end_line,
                    text: body.join("\n"),
                }));
            continue;
        }

        // 4. Display math `$$ ... $$`
        if trimmed.starts_with("$$") {
            let start_line = i;
            let after_open = trimmed.strip_prefix("$$").unwrap_or("");
            if after_open.ends_with("$$") && after_open.len() >= 2 {
                let latex = after_open.strip_suffix("$$").unwrap_or("").trim().to_string();
                scan.blocks.push(ScannedBlock::DisplayMath {
                    start_line,
                    end_line: i,
                    latex,
                    unclosed: false,
                });
                i += 1;
                continue;
            }
            let mut body = Vec::new();
            if !after_open.trim().is_empty() {
                body.push(after_open.to_string());
            }
            i += 1;
            let mut closed = false;
            while i < lines.len() {
                let t = lines[i].trim();
                if let Some(before_close) = t.strip_suffix("$$") {
                    if !before_close.trim().is_empty() {
                        body.push(before_close.to_string());
                    }
                    scan.blocks.push(ScannedBlock::DisplayMath {
                        start_line,
                        end_line: i,
                        latex: body.join("\n"),
                        unclosed: false,
                    });
                    closed = true;
                    i += 1;
                    break;
                }
                body.push(lines[i].to_string());
                i += 1;
            }
            if !closed {
                scan.blocks.push(ScannedBlock::DisplayMath {
                    start_line,
                    end_line: lines.len() - 1,
                    latex: body.join("\n"),
                    unclosed: true,
                });
            }
            continue;
        }

        // 5. Fenced code block (including Mermaid)
        if let Some(open) = parse_fence_open(line) {
            let start_line = i;
            i += 1;
            let mut closed = false;
            let mut body = Vec::new();
            while i < lines.len() {
                if is_fence_close(lines[i], &open) {
                    closed = true;
                    break;
                }
                body.push(lines[i]);
                i += 1;
            }
            let end_line = if closed { i } else { lines.len() - 1 };
            let unclosed = !closed;
            if unclosed && scan.unclosed_fence.is_none() {
                scan.unclosed_fence = Some(UnclosedFence {
                    start_line,
                    kind: open.kind,
                    fence_len: open.len,
                    language: open.language.clone(),
                });
            }
            if open.language.as_deref() == Some("mermaid") {
                scan.blocks.push(ScannedBlock::Mermaid {
                    start_line,
                    end_line,
                    source: body.join("\n"),
                    unclosed,
                });
            } else {
                scan.blocks.push(ScannedBlock::CodeBlock {
                    start_line,
                    end_line,
                    kind: open.kind,
                    fence_len: open.len,
                    language: open.language,
                    unclosed,
                });
            }
            if closed {
                i += 1;
            }
            continue;
        }

        // 6. Continuous blockquote / GitHub Alert
        if let Some(callout) = parse_callout_at(lines, i) {
            i = callout.end_line + 1;
            scan.blocks.push(ScannedBlock::Callout(callout));
            continue;
        }

        // 7. GFM Table
        if i + 1 < lines.len()
            && let Some(table) = parse_table(&lines[i..])
        {
            let end_line = i + 1 + table.rows.len();
            scan.blocks.push(ScannedBlock::Table {
                start_line: i,
                end_line,
                table,
            });
            i = end_line + 1;
            continue;
        }

        // 8. List (ordered / unordered / task)
        if let Some(first_item) = parse_list_item(line) {
            let start_line = i;
            let mut items = vec![(i, first_item)];
            let mut end_line = i;
            i += 1;
            while i < lines.len() {
                if let Some(item) = parse_list_item(lines[i]) {
                    items.push((i, item));
                    end_line = i;
                    i += 1;
                } else {
                    break;
                }
            }
            scan.blocks.push(ScannedBlock::List {
                start_line,
                end_line,
                items,
            });
            continue;
        }

        // 9. Thematic break (`---`, `***`, `___`)
        if is_thematic_break(trimmed) {
            scan.blocks.push(ScannedBlock::ThematicBreak { line: i });
            i += 1;
            continue;
        }

        // 10. Setext heading (`===` or `---` on next line)
        if i + 1 < lines.len()
            && let Some(level) = parse_setext_underline(lines[i + 1])
        {
            scan.blocks.push(ScannedBlock::SetextHeading {
                start_line: i,
                end_line: i + 1,
                level,
                text: trimmed.to_string(),
            });
            i += 2;
            continue;
        }

        // 11. Pure badge block
        if is_pure_badge_line(line) {
            scan.blocks.push(ScannedBlock::BadgeBlock {
                line: i,
                badges: parse_badges_in_line(line),
            });
            i += 1;
            continue;
        }

        // 11.5 ATX heading (`# ` .. `###### `) — always a single-line block, never swallows following tables/blocks
        if is_atx_heading(trimmed) {
            i += 1;
            continue;
        }

        // 12. Paragraph
        let start_line = i;
        let mut end_line = i;
        i += 1;
        while i < lines.len() {
            let next = lines[i].trim();
            if next.is_empty()
                || is_atx_heading(next)
                || next.starts_with("$$")
                || parse_fence_open(lines[i]).is_some()
                || parse_callout_at(lines, i).is_some()
                || (i + 1 < lines.len() && parse_table(&lines[i..]).is_some())
                || parse_list_item(lines[i]).is_some()
                || is_thematic_break(next)
            {
                break;
            }
            end_line = i;
            i += 1;
        }
        scan.blocks.push(ScannedBlock::Paragraph {
            start_line,
            end_line,
        });
    }

    scan
}

fn is_atx_heading(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return false;
    }
    match trimmed[hashes..].chars().next() {
        None => true,
        Some(c) => c == ' ' || c == '\t',
    }
}

fn is_thematic_break(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 3 {
        return false;
    }
    let first = compact.chars().next().unwrap_or(' ');
    matches!(first, '-' | '*' | '_') && compact.chars().all(|c| c == first)
}

fn parse_setext_underline(line: &str) -> Option<u8> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c == '=') {
        Some(1)
    } else if trimmed.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

fn parse_reference_definition(line: &str) -> Option<(String, String, Option<String>)> {
    let trimmed = line.trim();
    let after_open = trimmed.strip_prefix('[')?;
    let close_idx = after_open.find("]:")?;
    let label = after_open[..close_idx].trim();
    if label.is_empty() || label.starts_with('^') {
        return None;
    }
    let rest = after_open[close_idx + 2..].trim();
    if rest.is_empty() {
        return None;
    }
    let mut parts = rest.splitn(2, char::is_whitespace);
    let url = parts
        .next()?
        .trim_matches(|c| c == '<' || c == '>')
        .to_string();
    let title = parts
        .next()
        .map(|t| t.trim().trim_matches(|c| c == '"' || c == '\'' || c == '(' || c == ')').to_string())
        .filter(|t| !t.is_empty());
    Some((label.to_ascii_lowercase(), url, title))
}

fn parse_footnote_start(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let after = trimmed.strip_prefix("[^")?;
    let close = after.find("]:")?;
    let id = after[..close].trim();
    if id.is_empty() {
        return None;
    }
    let text = after[close + 2..].trim().to_string();
    Some((id.to_string(), text))
}

// ---------------------------------------------------------------------------
// DisplayMap: InlineAnalysis + DisplayLine with bidirectional byte mapping
// ---------------------------------------------------------------------------

/// Inline analysis of a single source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineAnalysis {
    pub raw_len: usize,
    pub display_len: usize,
}

/// A line with bidirectional mapping between raw Markdown byte offsets and
/// display (rendered/stripped) byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLine {
    pub display_text: String,
    /// `raw_to_display[raw_byte] = display_byte` (length `raw_len + 1`).
    raw_to_display: Vec<usize>,
    /// `display_to_raw[display_byte] = raw_byte` (length `display_len + 1`).
    display_to_raw: Vec<usize>,
}

/// Bidirectional byte-offset map for a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMap {
    pub analysis: InlineAnalysis,
    pub line: DisplayLine,
}

impl DisplayMap {
    /// Builds a `DisplayMap` from a raw Markdown line and its display representation
    /// (for example, after hiding inline syntax delimiters).
    pub fn from_raw_and_display(raw: &str, display: String) -> Self {
        let raw_len = raw.len();
        let display_len = display.len();

        let mut raw_to_display = vec![0usize; raw_len + 1];
        let mut display_to_raw = vec![0usize; display_len + 1];

        for (r, slot) in raw_to_display.iter_mut().enumerate() {
            *slot = (r * display_len).checked_div(raw_len).unwrap_or(0);
        }
        for (d, slot) in display_to_raw.iter_mut().enumerate() {
            *slot = (d * raw_len).checked_div(display_len).unwrap_or(0);
        }

        Self {
            analysis: InlineAnalysis {
                raw_len,
                display_len,
            },
            line: DisplayLine {
                display_text: display,
                raw_to_display,
                display_to_raw,
            },
        }
    }

    /// Maps a raw byte offset to the corresponding display byte offset.
    pub fn to_display(&self, raw_byte: usize) -> usize {
        let clamped = raw_byte.min(self.analysis.raw_len);
        self.line.raw_to_display[clamped]
    }

    /// Maps a display byte offset back to the corresponding raw byte offset.
    pub fn to_source(&self, display_byte: usize) -> usize {
        let clamped = display_byte.min(self.analysis.display_len);
        self.line.display_to_raw[clamped]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_frontmatter_refs_alerts_tables_and_lists() {
        let doc = [
            "---",
            "title: ClearMind",
            "---",
            "",
            "[docs]: https://example.com \"Docs\"",
            "",
            "> [!NOTE]",
            "> Hello world",
            "",
            "| A | B |",
            "| - | - |",
            "| 1 | 2 |",
            "",
            "1. First",
            "2. Second",
        ];
        let scan = scan_blocks(&doc);
        assert!(matches!(
            scan.blocks.first(),
            Some(ScannedBlock::Frontmatter {
                start_line: 0,
                end_line: 2,
                ..
            })
        ));
        assert_eq!(
            scan.references.get("docs"),
            Some(&("https://example.com".to_string(), Some("Docs".to_string())))
        );
        assert!(scan
            .blocks
            .iter()
            .any(|b| matches!(b, ScannedBlock::Callout(_))));
        assert!(scan
            .blocks
            .iter()
            .any(|b| matches!(b, ScannedBlock::Table { .. })));
        assert!(scan
            .blocks
            .iter()
            .any(|b| matches!(b, ScannedBlock::List { .. })));
    }

    #[test]
    fn display_map_round_trips_bounds() {
        let map = DisplayMap::from_raw_and_display("**hello**", "hello".to_string());
        assert_eq!(map.to_display(0), 0);
        assert_eq!(map.to_display(9), 5);
        assert_eq!(map.to_source(0), 0);
        assert_eq!(map.to_source(5), 9);
    }
}
