//! Copy as rich text: converts Markdown to theme-independent inline-styled HTML and writes via `arboard`.

use pulldown_cmark::{Options, Parser, html};

/// Convert Markdown source to theme-independent inline-styled HTML suitable for Word/Outlook/Gmail/Slack.
pub fn markdown_to_rich_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, opts);
    let mut body = String::new();
    html::push_html(&mut body, parser);

    format!(
        "<div style=\"font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; color: #1f2328; line-height: 1.5;\">{body}</div>"
    )
}

/// Copy both HTML and plain Markdown text to the system clipboard.
pub fn copy_rich_text(markdown: &str) -> Result<(), String> {
    let html = markdown_to_rich_html(markdown);
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_html(html, Some(markdown.to_string()))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_gfm_to_html() {
        let html = markdown_to_rich_html("**bold** and ~~strike~~");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<del>strike</del>"));
    }
}
