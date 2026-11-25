//! Continuous blockquote and GitHub Alert (`> [!NOTE]` .. `> [!CAUTION]`) parser.

/// GitHub-flavored Alert kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    /// Parses an alert tag such as `[!NOTE]` (case-insensitive).
    pub fn from_tag(tag: &str) -> Option<Self> {
        let inner = tag.trim().strip_prefix("[!")?.strip_suffix(']')?;
        match inner.to_ascii_uppercase().as_str() {
            "NOTE" => Some(Self::Note),
            "TIP" => Some(Self::Tip),
            "IMPORTANT" => Some(Self::Important),
            "WARNING" => Some(Self::Warning),
            "CAUTION" => Some(Self::Caution),
            _ => None,
        }
    }

    /// Returns the display label for this alert kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }
}

/// A continuous blockquote or GitHub Alert block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalloutBlock {
    /// 0-based start line index.
    pub start_line: usize,
    /// 0-based inclusive end line index.
    pub end_line: usize,
    /// `Some(AlertKind)` if the first line is a GitHub Alert marker, else `None` (plain blockquote).
    pub alert: Option<AlertKind>,
    /// Stripped inner lines (without the leading `> `).
    pub body_lines: Vec<String>,
}

/// Strips one leading blockquote marker (`>` with optional single following space) from `line`.
pub fn strip_blockquote_prefix(line: &str) -> Option<&str> {
    let trimmed_end = line.trim_end_matches(['\r', '\n']);
    let leading_spaces = trimmed_end.bytes().take_while(|&b| b == b' ').count();
    if leading_spaces > 3 {
        return None;
    }
    let rest = &trimmed_end[leading_spaces..];
    let after_gt = rest.strip_prefix('>')?;
    Some(after_gt.strip_prefix(' ').unwrap_or(after_gt))
}

/// Parses a continuous blockquote / GitHub Alert starting at `start_line`.
/// Returns `None` if `lines[start_line]` does not start with `>`.
pub fn parse_callout_at(lines: &[&str], start_line: usize) -> Option<CalloutBlock> {
    let first_inner = strip_blockquote_prefix(lines.get(start_line)?)?;
    let mut end_line = start_line;
    let mut body_lines = Vec::new();

    let alert = AlertKind::from_tag(first_inner);
    if alert.is_none() {
        body_lines.push(first_inner.to_string());
    }

    let mut i = start_line + 1;
    while i < lines.len() {
        if let Some(inner) = strip_blockquote_prefix(lines[i]) {
            body_lines.push(inner.to_string());
            end_line = i;
            i += 1;
        } else {
            break;
        }
    }

    Some(CalloutBlock {
        start_line,
        end_line,
        alert,
        body_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_alert_and_plain_blockquote() {
        let lines = vec![
            "> [!WARNING]",
            "> Careful with this.",
            "> Second line.",
            "normal",
        ];
        let block = parse_callout_at(&lines, 0).unwrap();
        assert_eq!(block.alert, Some(AlertKind::Warning));
        assert_eq!(block.start_line, 0);
        assert_eq!(block.end_line, 2);
        assert_eq!(block.body_lines, vec!["Careful with this.", "Second line."]);

        let plain = vec!["> just a quote", "> continued"];
        let q = parse_callout_at(&plain, 0).unwrap();
        assert_eq!(q.alert, None);
        assert_eq!(q.body_lines.len(), 2);
    }
}
