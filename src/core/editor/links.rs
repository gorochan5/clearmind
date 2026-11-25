//! Local file reference resolver, magic-byte type override, and safe-open allowlist.

use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCategory {
    Image,
    Markdown,
    Document,
    Folder,
    Other,
    Missing,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "svg"];
const MARKDOWN_EXTS: &[&str] = &["md", "markdown"];
const DOCUMENT_EXTS: &[&str] = &[
    "pdf", "txt", "rtf", "doc", "docx", "odt", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp", "epub",
];

/// Classify a local path using its extension and `infer` magic-byte verification.
pub fn classify_local_path(path: &Path) -> LinkCategory {
    if !path.exists() {
        return LinkCategory::Missing;
    }
    if path.is_dir() {
        return LinkCategory::Folder;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let by_ext = if IMAGE_EXTS.contains(&ext.as_str()) {
        LinkCategory::Image
    } else if MARKDOWN_EXTS.contains(&ext.as_str()) {
        LinkCategory::Markdown
    } else if DOCUMENT_EXTS.contains(&ext.as_str()) {
        LinkCategory::Document
    } else {
        LinkCategory::Other
    };

    // Magic-byte override: if `infer` identifies an executable/archive masquerading as an allowed type,
    // downgrade to `Other` so it is never executed.
    if matches!(by_ext, LinkCategory::Image | LinkCategory::Document)
        && let Ok(Some(kind)) = infer::get_from_path(path)
        && matches!(kind.matcher_type(), infer::MatcherType::App | infer::MatcherType::Archive)
    {
        return LinkCategory::Other;
    }

    by_ext
}

/// Build a case-insensitive filename -> PathBuf index under `root` respecting `.gitignore`.
pub fn build_wiki_index(root: &Path) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    for entry in WalkBuilder::new(root).hidden(false).build().flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                index.entry(name.to_ascii_lowercase()).or_insert_with(|| path.to_path_buf());
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                index.entry(stem.to_ascii_lowercase()).or_insert_with(|| path.to_path_buf());
            }
        }
    }
    index
}

/// Resolve a local reference (relative, absolute, `file://`, or `[[wiki]]` name).
pub fn resolve_reference(
    target: &str,
    doc_dir: Option<&Path>,
    wiki_index: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    let clean = target.split('#').next().unwrap_or(target).trim();
    if clean.is_empty() {
        return None;
    }
    if let Some(stripped) = clean.strip_prefix("file:///") {
        return Some(PathBuf::from(stripped));
    }
    let candidate = Path::new(clean);
    if candidate.is_absolute() {
        return Some(candidate.to_path_buf());
    }
    if let Some(dir) = doc_dir {
        let joined = dir.join(candidate);
        if joined.exists() {
            return Some(joined);
        }
    }
    wiki_index.get(&clean.to_ascii_lowercase()).cloned()
}

/// Open a local path safely: Images/Documents open with default app; `Other` reveals in Explorer.
pub fn open_local_safely(path: &Path, force_reveal: bool) -> Result<(), String> {
    if force_reveal {
        return opener::reveal(path).map_err(|e| e.to_string());
    }
    match classify_local_path(path) {
        LinkCategory::Image | LinkCategory::Document | LinkCategory::Folder => {
            opener::open(path).map_err(|e| e.to_string())
        }
        LinkCategory::Other => opener::reveal(path).map_err(|e| e.to_string()),
        LinkCategory::Markdown => Ok(()), // Handled by ClearMind tab opener
        LinkCategory::Missing => Err(format!("File not found: {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_path_classified_as_missing() {
        assert_eq!(
            classify_local_path(Path::new("nonexistent_clearmind_file_12345.md")),
            LinkCategory::Missing
        );
    }
}
