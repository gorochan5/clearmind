//! Fenced code block parsing, language info-string extraction, and unclosed fence detection.

/// The delimiter character used for a fenced code block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceKind {
    /// Backtick fence (`` ` ``).
    Backtick,
    /// Tilde fence (`~`).
    Tilde,
}

impl FenceKind {
    /// Returns the underlying fence character.
    pub fn ch(self) -> char {
        match self {
            Self::Backtick => '`',
            Self::Tilde => '~',
        }
    }
}

/// Opening fence metadata parsed from a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceOpen {
    /// Backtick or tilde.
    pub kind: FenceKind,
    /// Number of fence characters (>= 3).
    pub len: usize,
    /// Raw info string after the opening fence (trimmed).
    pub info: String,
    /// Extracted language token (first whitespace/comma-delimited word, lowercase), if any.
    pub language: Option<String>,
}

/// A parsed fenced code block spanning `[start_line..=end_line]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedCodeBlock {
    /// 0-based line index of the opening fence.
    pub start_line: usize,
    /// 0-based line index of the closing fence (or last line if unclosed).
    pub end_line: usize,
    /// Fence kind.
    pub kind: FenceKind,
    /// Fence length.
    pub fence_len: usize,
    /// Raw info string.
    pub info: String,
    /// Normalized language identifier, if present.
    pub language: Option<String>,
    /// True if no matching closing fence was found before EOF.
    pub unclosed: bool,
}

/// Represents an unclosed fenced code block detected in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnclosedFence {
    /// 0-based line index where the unclosed fence opened.
    pub start_line: usize,
    /// Fence kind.
    pub kind: FenceKind,
    /// Opening fence length.
    pub fence_len: usize,
    /// Extracted language, if any.
    pub language: Option<String>,
}

/// Parses an opening code fence line (allowing up to 3 leading spaces).
///
/// Per CommonMark, backtick fences cannot contain backticks in their info string.
pub fn parse_fence_open(line: &str) -> Option<FenceOpen> {
    let trimmed_end = line.trim_end_matches(['\r', '\n']);
    let leading_spaces = trimmed_end.bytes().take_while(|&b| b == b' ').count();
    if leading_spaces > 3 {
        return None;
    }
    let rest = &trimmed_end[leading_spaces..];
    let first = rest.chars().next()?;
    let kind = match first {
        '`' => FenceKind::Backtick,
        '~' => FenceKind::Tilde,
        _ => return None,
    };

    let len = rest.chars().take_while(|&c| c == first).count();
    if len < 3 {
        return None;
    }

    let info_raw = rest[len..].trim();
    if kind == FenceKind::Backtick && info_raw.contains('`') {
        return None;
    }

    let language = extract_language(info_raw);
    Some(FenceOpen {
        kind,
        len,
        info: info_raw.to_string(),
        language,
    })
}

/// Checks whether `line` is a valid closing fence for `open`.
pub fn is_fence_close(line: &str, open: &FenceOpen) -> bool {
    let trimmed_end = line.trim_end_matches(['\r', '\n']);
    let leading_spaces = trimmed_end.bytes().take_while(|&b| b == b' ').count();
    if leading_spaces > 3 {
        return false;
    }
    let rest = &trimmed_end[leading_spaces..];
    let ch = open.kind.ch();
    let len = rest.chars().take_while(|&c| c == ch).count();
    if len < open.len {
        return false;
    }
    rest[len..].trim().is_empty()
}

/// Extracts the normalized language identifier from a fence info string.
///
/// Examples: `"rust,no_run"` -> `Some("rust")`, `"mermaid"` -> `Some("mermaid")`, `""` -> `None`.
pub fn extract_language(info: &str) -> Option<String> {
    let token = info
        .split(|c: char| c.is_whitespace() || c == ',' || c == '{')
        .next()
        .unwrap_or("")
        .trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_ascii_lowercase())
    }
}

/// Scans `lines` and returns all fenced code blocks (including unclosed ones).
pub fn scan_fenced_code_blocks(lines: &[&str]) -> Vec<FencedCodeBlock> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(open) = parse_fence_open(lines[i]) {
            let start_line = i;
            i += 1;
            let mut closed = false;
            while i < lines.len() {
                if is_fence_close(lines[i], &open) {
                    blocks.push(FencedCodeBlock {
                        start_line,
                        end_line: i,
                        kind: open.kind,
                        fence_len: open.len,
                        info: open.info.clone(),
                        language: open.language.clone(),
                        unclosed: false,
                    });
                    closed = true;
                    i += 1;
                    break;
                }
                i += 1;
            }
            if !closed {
                blocks.push(FencedCodeBlock {
                    start_line,
                    end_line: lines.len().saturating_sub(1),
                    kind: open.kind,
                    fence_len: open.len,
                    info: open.info.clone(),
                    language: open.language.clone(),
                    unclosed: true,
                });
            }
        } else {
            i += 1;
        }
    }
    blocks
}

/// Returns any unclosed fence at the end of `lines`.
pub fn detect_unclosed_fence(lines: &[&str]) -> Option<UnclosedFence> {
    scan_fenced_code_blocks(lines)
        .into_iter()
        .find(|b| b.unclosed)
        .map(|b| UnclosedFence {
            start_line: b.start_line,
            kind: b.kind,
            fence_len: b.fence_len,
            language: b.language,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_backtick_and_tilde_fences() {
        let open = parse_fence_open("```rust,no_run").unwrap();
        assert_eq!(open.kind, FenceKind::Backtick);
        assert_eq!(open.len, 3);
        assert_eq!(open.language.as_deref(), Some("rust"));

        let tilde = parse_fence_open("   ~~~~python").unwrap();
        assert_eq!(tilde.kind, FenceKind::Tilde);
        assert_eq!(tilde.len, 4);
        assert_eq!(tilde.language.as_deref(), Some("python"));
    }

    #[test]
    fn rejects_backtick_in_info_string() {
        assert!(parse_fence_open("```rust `foo`").is_none());
    }

    #[test]
    fn detects_unclosed_fence() {
        let lines = vec!["# Title", "```rust", "let x = 1;"];
        let unclosed = detect_unclosed_fence(&lines).unwrap();
        assert_eq!(unclosed.start_line, 1);
        assert_eq!(unclosed.language.as_deref(), Some("rust"));
    }

    #[test]
    fn closes_matching_fence_only_with_enough_chars() {
        let lines = vec!["````markdown", "```", "hello", "````"];
        let blocks = scan_fenced_code_blocks(&lines);
        assert_eq!(blocks.len(), 1);
        assert!(!blocks[0].unclosed);
        assert_eq!(blocks[0].start_line, 0);
        assert_eq!(blocks[0].end_line, 3);
    }
}
