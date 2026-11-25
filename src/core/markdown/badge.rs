//! Shields.io URL & Markdown badge parser and cache key helper.

/// A parsed Shields.io badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBadge {
    /// Alt text (e.g., `"build"`).
    pub alt: String,
    /// Badge image URL (always `https://img.shields.io/...` or `http://...`).
    pub url: String,
    /// Optional link target if wrapped in `[![alt](url)](href)`.
    pub link: Option<String>,
    /// Optional label parsed from `/badge/<label>-<message>-<color>` when present.
    pub label: Option<String>,
    /// Optional message parsed from `/badge/<label>-<message>-<color>` when present.
    pub message: Option<String>,
}

/// Returns `true` if `url` points to a Shields.io badge endpoint.
pub fn is_shields_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://img.shields.io/") || lower.starts_with("http://img.shields.io/")
}

/// Computes a deterministic cache key for a badge URL.
pub fn badge_cache_key(url: &str) -> u64 {
    // FNV-1a 64-bit hash for fast, deterministic cache keys.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in url.trim().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Extracts all Shields.io badges from a single line of Markdown.
/// Supports both `![alt](https://img.shields.io/...)` and `[![alt](https://img.shields.io/...)](href)`.
pub fn parse_badges_in_line(line: &str) -> Vec<ParsedBadge> {
    let mut badges = Vec::new();
    let mut rest = line;

    while let Some(img_start) = rest.find("![") {
        let is_linked = img_start > 0 && rest.as_bytes()[img_start - 1] == b'[';
        let after_bang = &rest[img_start + 2..];
        let Some(alt_end) = after_bang.find("](") else {
            rest = after_bang;
            continue;
        };
        let alt = &after_bang[..alt_end];
        let after_paren = &after_bang[alt_end + 2..];
        let Some(url_end) = after_paren.find(')') else {
            break;
        };
        let url = after_paren[..url_end].trim();
        let mut consumed = img_start + 2 + alt_end + 2 + url_end + 1;
        let mut link = None;

        if is_linked {
            let after_img = &rest[consumed..];
            if let Some(after_close_bracket) = after_img.strip_prefix("](")
                && let Some(href_end) = after_close_bracket.find(')')
            {
                link = Some(after_close_bracket[..href_end].trim().to_string());
                consumed += 2 + href_end + 1;
            }
        }

        if is_shields_url(url) {
            let (label, message) = parse_static_badge_parts(url);
            badges.push(ParsedBadge {
                alt: alt.to_string(),
                url: url.to_string(),
                link,
                label,
                message,
            });
        }

        rest = &rest[consumed..];
    }

    badges
}

/// Returns `true` if `line` consists solely of one or more Shields.io badges (and whitespace).
pub fn is_pure_badge_line(line: &str) -> bool {
    let badges = parse_badges_in_line(line);
    if badges.is_empty() {
        return false;
    }
    // Check that removing all badge markdown leaves only whitespace.
    let mut remaining = line.to_string();
    for b in &badges {
        if let Some(href) = &b.link {
            let full = format!("[![{}]({})]({})", b.alt, b.url, href);
            remaining = remaining.replace(&full, "");
        } else {
            let full = format!("![{}]({})", b.alt, b.url);
            remaining = remaining.replace(&full, "");
        }
    }
    remaining.trim().is_empty()
}

fn parse_static_badge_parts(url: &str) -> (Option<String>, Option<String>) {
    let Some(idx) = url.find("/badge/") else {
        return (None, None);
    };
    let path = &url[idx + "/badge/".len()..];
    let path = path.split('?').next().unwrap_or(path);
    let mut parts = path.splitn(3, '-');
    let label = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    let message = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    (label, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linked_and_unlinked_shields_badges() {
        let line = "[![CI](https://img.shields.io/badge/build-passing-brightgreen)](https://example.com) ![v1](https://img.shields.io/badge/version-1.0.0-blue)";
        let badges = parse_badges_in_line(line);
        assert_eq!(badges.len(), 2);
        assert_eq!(badges[0].alt, "CI");
        assert_eq!(badges[0].link.as_deref(), Some("https://example.com"));
        assert_eq!(badges[0].label.as_deref(), Some("build"));
        assert_eq!(badges[0].message.as_deref(), Some("passing"));
        assert!(is_pure_badge_line(line));
    }
}
