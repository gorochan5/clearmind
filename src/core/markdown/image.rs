//! Image reference parser (Obsidian `![[...]]`, Markdown `![alt](url)`, HTML `<img>`)
//! and bounded remote media LRU cache (64 MB cap, 10 MB max body, 10s timeout).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Maximum total bytes stored in the remote media LRU cache (64 MB).
pub const MEDIA_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum single response body size (10 MB).
pub const MEDIA_MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
/// HTTP request timeout for remote media (10s).
pub const MEDIA_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Source syntax of a parsed image reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSyntax {
    /// Standard Markdown `![alt](target "title")`
    Markdown,
    /// Obsidian wiki-embed `![[target|alias]]`
    ObsidianEmbed,
    /// Raw HTML `<img src="..." alt="..." ...>`
    HtmlImg,
}

/// A parsed image reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedImage {
    pub syntax: ImageSyntax,
    pub target: String,
    pub alt: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Returns `true` if `url` uses an allowed remote scheme (`http://` or `https://`).
pub fn is_allowed_remote_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Parses a single image reference from a line (Markdown, Obsidian embed, or HTML `<img>`).
pub fn parse_image_in_line(line: &str) -> Option<ParsedImage> {
    if let Some(img) = parse_obsidian_embed(line) {
        return Some(img);
    }
    if let Some(img) = parse_markdown_image(line) {
        return Some(img);
    }
    parse_html_img(line)
}

fn parse_obsidian_embed(line: &str) -> Option<ParsedImage> {
    let start = line.find("![[")?;
    let after = &line[start + 3..];
    let end = after.find("]]")?;
    let inner = &after[..end];
    let (target, alt_part) = match inner.split_once('|') {
        Some((t, a)) => (t.trim(), a.trim()),
        None => (inner.trim(), ""),
    };
    if target.is_empty() {
        return None;
    }
    let width = alt_part.parse::<u32>().ok();
    Some(ParsedImage {
        syntax: ImageSyntax::ObsidianEmbed,
        target: target.to_string(),
        alt: if width.is_some() {
            String::new()
        } else {
            alt_part.to_string()
        },
        width,
        height: None,
    })
}

fn parse_markdown_image(line: &str) -> Option<ParsedImage> {
    let start = line.find("![")?;
    let after = &line[start + 2..];
    let alt_end = after.find("](")?;
    let alt = &after[..alt_end];
    let after_paren = &after[alt_end + 2..];
    let url_end = after_paren.find(')')?;
    let raw_target = after_paren[..url_end].trim();
    // Support `<path with spaces>` syntax as well as optional `"title"` / `'title'` suffixes.
    let target = if let Some(rest) = raw_target.strip_prefix('<') {
        if let Some(close) = rest.find('>') {
            rest[..close].trim()
        } else {
            rest.trim()
        }
    } else if let Some(quote_idx) = raw_target.find(" \"").or_else(|| raw_target.find(" '")) {
        raw_target[..quote_idx].trim()
    } else {
        raw_target
    };
    if target.is_empty() {
        return None;
    }
    Some(ParsedImage {
        syntax: ImageSyntax::Markdown,
        target: target.to_string(),
        alt: alt.to_string(),
        width: None,
        height: None,
    })
}

fn parse_html_img(line: &str) -> Option<ParsedImage> {
    let lower = line.to_ascii_lowercase();
    let start = lower.find("<img")?;
    let after = &line[start..];
    let end = after.find('>')?;
    let tag = &after[..=end];

    let src = extract_html_attr(tag, "src")?;
    let alt = extract_html_attr(tag, "alt").unwrap_or_default();
    let width = extract_html_attr(tag, "width").and_then(|v| v.parse::<u32>().ok());
    let height = extract_html_attr(tag, "height").and_then(|v| v.parse::<u32>().ok());

    Some(ParsedImage {
        syntax: ImageSyntax::HtmlImg,
        target: src,
        alt,
        width,
        height,
    })
}

fn extract_html_attr(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let pattern = format!("{attr}=");
    let idx = lower.find(&pattern)?;
    let rest = tag[idx + pattern.len()..].trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let inner = &rest[1..];
        let end = inner.find(quote)?;
        Some(inner[..end].to_string())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

/// Cached remote media payload.
#[derive(Debug, Clone)]
pub struct CachedMedia {
    pub bytes: Arc<[u8]>,
    pub content_type: Option<String>,
}

/// Byte-bounded LRU cache for remote media (64 MB cap).
#[derive(Debug)]
pub struct RemoteMediaCache {
    max_bytes: usize,
    current_bytes: usize,
    entries: HashMap<String, CachedMedia>,
    order: VecDeque<String>,
}

impl Default for RemoteMediaCache {
    fn default() -> Self {
        Self::new(MEDIA_CACHE_MAX_BYTES)
    }
}

impl RemoteMediaCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, url: &str) -> Option<CachedMedia> {
        let item = self.entries.get(url)?.clone();
        self.touch(url);
        Some(item)
    }

    pub fn insert(&mut self, url: String, media: CachedMedia) {
        let size = media.bytes.len();
        if size > self.max_bytes {
            return;
        }
        if let Some(old) = self.entries.remove(&url) {
            self.current_bytes = self.current_bytes.saturating_sub(old.bytes.len());
            self.order.retain(|k| k != &url);
        }
        while self.current_bytes + size > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.current_bytes = self.current_bytes.saturating_sub(removed.bytes.len());
            }
        }
        self.current_bytes += size;
        self.order.push_back(url.clone());
        self.entries.insert(url, media);
    }

    fn touch(&mut self, url: &str) {
        self.order.retain(|k| k != url);
        self.order.push_back(url.to_string());
    }
}

/// Fetches remote media over `http`/`https` with a 10s timeout and 10 MB body limit,
/// using `cache` when available.
pub async fn fetch_remote_media(
    url: &str,
    cache: &Mutex<RemoteMediaCache>,
) -> Result<CachedMedia, String> {
    if !is_allowed_remote_url(url) {
        return Err("Only http:// and https:// URLs are allowed".to_string());
    }
    if let Ok(mut guard) = cache.lock()
        && let Some(hit) = guard.get(url)
    {
        return Ok(hit);
    }

    let client = reqwest::Client::builder()
        .timeout(MEDIA_FETCH_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    if let Some(len) = resp.content_length()
        && len as usize > MEDIA_MAX_BODY_BYTES
    {
        return Err(format!(
            "Response too large ({len} bytes > {MEDIA_MAX_BODY_BYTES})"
        ));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read body: {e}"))?;

    if bytes.len() > MEDIA_MAX_BODY_BYTES {
        return Err(format!(
            "Response too large ({} bytes > {MEDIA_MAX_BODY_BYTES})",
            bytes.len()
        ));
    }

    let media = CachedMedia {
        bytes: Arc::from(bytes.as_ref()),
        content_type,
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(url.to_string(), media.clone());
    }

    Ok(media)
}
