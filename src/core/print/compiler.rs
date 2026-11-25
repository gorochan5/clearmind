//! Markdown -> Typst -> SVG preview pages + vector PDF compiler (`typst` 0.15.1 + `typst-pdf`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use iced::Color;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use crate::core::themes::ThemePalette;

use super::models::{CompiledPrintDoc, PageOrientation, PrintSettings, PrintThemeMode};

struct PrintTypstShared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    has_segoe_ui: bool,
}

static PRINT_TYPST_SHARED: LazyLock<PrintTypstShared> = LazyLock::new(|| {
    let mut fonts: Vec<Font> = typst_assets::fonts()
        .flat_map(|data| Font::iter(Bytes::new(data)))
        .collect();

    // Optionally load Windows Segoe UI faces for crisp modern sans-serif printing,
    // gracefully falling back to bundled Libertinus Serif if unavailable.
    let mut has_segoe_ui = false;
    let win_fonts = Path::new(r"C:\Windows\Fonts");
    for file_name in [
        "segoeui.ttf",
        "segoeuib.ttf",
        "segoeuii.ttf",
        "segoeuiz.ttf",
        "seguisb.ttf",
        "seguisbi.ttf",
    ] {
        let p = win_fonts.join(file_name);
        if let Ok(raw) = std::fs::read(&p) {
            let loaded: Vec<Font> = Font::iter(Bytes::new(raw)).collect();
            if !loaded.is_empty() {
                has_segoe_ui = true;
                fonts.extend(loaded);
            }
        }
    }

    let book = FontBook::from_fonts(&fonts);
    let library = Library::default();
    PrintTypstShared {
        library: LazyHash::new(library),
        book: LazyHash::new(book),
        fonts,
        has_segoe_ui,
    }
});

struct PrintWorld {
    source: Source,
    files: HashMap<String, Bytes>,
}

impl PrintWorld {
    fn new(typst_markup: String, files: HashMap<String, Bytes>) -> Self {
        Self {
            source: Source::detached(typst_markup),
            files,
        }
    }
}

impl World for PrintWorld {
    fn library(&self) -> &LazyHash<Library> {
        &PRINT_TYPST_SHARED.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &PRINT_TYPST_SHARED.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_with_slash().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let vpath = id.vpath().get_with_slash();
        if let Some(bytes) = self.files.get(vpath) {
            Ok(bytes.clone())
        } else {
            Err(FileError::NotFound(vpath.into()))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        PRINT_TYPST_SHARED.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        None
    }
}

#[derive(Debug, Clone)]
struct PrintColors {
    page_bg: String,
    text: String,
    muted: String,
    accent: String,
    surface: String,
    header_bg: String,
    stripe_bg: String,
    border: String,
    code_bg: String,
}

fn color_to_hex(c: Color) -> String {
    let r = (c.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (c.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (c.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn resolve_print_colors(settings: PrintSettings, palette: &ThemePalette) -> PrintColors {
    match settings.theme_mode {
        PrintThemeMode::CleanPaper => PrintColors {
            page_bg: "#ffffff".to_string(),
            text: "#18221e".to_string(),
            muted: "#56635c".to_string(),
            accent: "#1f3a32".to_string(),
            surface: "#f5f7f4".to_string(),
            header_bg: "#edf2ec".to_string(),
            stripe_bg: "#fafbf9".to_string(),
            border: "#d2dcd0".to_string(),
            code_bg: "#f2f5f1".to_string(),
        },
        PrintThemeMode::ActiveTheme => PrintColors {
            page_bg: color_to_hex(palette.background),
            text: color_to_hex(palette.text),
            muted: color_to_hex(palette.text_muted),
            accent: color_to_hex(palette.accent_text),
            surface: color_to_hex(palette.surface),
            header_bg: color_to_hex(palette.surface_hover),
            stripe_bg: color_to_hex(palette.surface),
            border: color_to_hex(palette.border),
            code_bg: color_to_hex(palette.surface),
        },
    }
}

/// Compiles Markdown source into per-page preview SVGs and exact vector PDF bytes.
pub fn compile_print_document(
    generation: u64,
    markdown: &str,
    doc_title: &str,
    doc_path: Option<&Path>,
    settings: PrintSettings,
    palette: ThemePalette,
) -> Result<CompiledPrintDoc, String> {
    let mut virtual_files: HashMap<String, Bytes> = HashMap::new();
    let typst_markup = markdown_to_typst(
        markdown,
        doc_title,
        doc_path,
        settings,
        &palette,
        &mut virtual_files,
    );

    let world = PrintWorld::new(typst_markup, virtual_files);
    let warned = typst::compile::<PagedDocument>(&world);
    comemo::evict(30);

    let document = warned.output.map_err(|errors| {
        let msgs: Vec<String> = errors
            .iter()
            .take(5)
            .map(|e| e.message.to_string())
            .collect();
        if msgs.is_empty() {
            "Typst compilation failed".to_string()
        } else {
            msgs.join("; ")
        }
    })?;

    let pages = document.pages();
    if pages.is_empty() {
        return Err("Document produced no pages".to_string());
    }

    let page_svgs: Vec<Vec<u8>> = pages
        .iter()
        .map(|page| typst_svg::svg(page, &typst_svg::SvgOptions::default()).into_bytes())
        .collect();

    let pdf_bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errs| {
        let msgs: Vec<String> = errs.iter().take(5).map(|e| e.message.to_string()).collect();
        if msgs.is_empty() {
            "PDF generation failed".to_string()
        } else {
            msgs.join("; ")
        }
    })?;

    Ok(CompiledPrintDoc {
        generation,
        page_svgs,
        pdf_bytes,
    })
}

fn strip_frontmatter(md: &str) -> &str {
    let trimmed = md.strip_prefix('\u{feff}').unwrap_or(md);
    if let Some(rest) = trimmed
        .strip_prefix("---\r\n")
        .or_else(|| trimmed.strip_prefix("---\n"))
        && let Some(idx) = rest.find("\n---")
    {
        let after = &rest[idx + 4..];
        if let Some(after_nl) = after
            .strip_prefix("\r\n")
            .or_else(|| after.strip_prefix('\n'))
        {
            return after_nl;
        }
        if after.trim().is_empty() {
            return "";
        }
    }
    trimmed
}

fn escape_typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

fn escape_typst_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '\\' | '#' | '$' | '@' | '<' | '>' | '*' | '_' | '`' | '[' | ']' | '~' | '/' | '='
            | '+' | '-' => {
                out.push('\\');
                out.push(ch);
            }
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

fn resolve_local_image(target: &str, doc_path: Option<&Path>) -> Option<PathBuf> {
    let trimmed = target.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("data:")
    {
        return None;
    }
    let decoded = trimmed.replace("%20", " ");
    let candidate = PathBuf::from(&decoded);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate);
    }
    if let Some(doc) = doc_path
        && let Some(dir) = doc.parent()
    {
        let joined = dir.join(&candidate);
        if joined.is_file() {
            return Some(joined);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let joined = cwd.join(&candidate);
        if joined.is_file() {
            return Some(joined);
        }
    }
    None
}

#[derive(Debug)]
struct TableBuilder {
    alignments: Vec<Alignment>,
    header_cells: Vec<String>,
    body_rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    in_head: bool,
}

impl TableBuilder {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header_cells: Vec::new(),
            body_rows: Vec::new(),
            current_row: Vec::new(),
            in_head: false,
        }
    }

    fn render(self) -> String {
        let col_count = self
            .alignments
            .len()
            .max(self.header_cells.len())
            .max(self.body_rows.iter().map(|r| r.len()).max().unwrap_or(0))
            .max(1);

        let cols = vec!["1fr"; col_count].join(", ");
        let aligns: Vec<&str> = (0..col_count)
            .map(|i| match self.alignments.get(i).copied().unwrap_or(Alignment::None) {
                Alignment::Center => "center",
                Alignment::Right => "right",
                Alignment::Left | Alignment::None => "left",
            })
            .collect();

        let mut out = String::new();
        out.push_str("#v(4pt)\n#table(\n");
        out.push_str(&format!("  columns: ({cols}),\n"));
        out.push_str(&format!("  align: ({}),\n", aligns.join(", ")));
        out.push_str("  inset: (x: 8pt, y: 6pt),\n");
        out.push_str("  stroke: 0.5pt + cm-border,\n");
        out.push_str(
            "  fill: (_, y) => if y == 0 { cm-header-bg } else if calc.odd(y) { none } else { cm-stripe-bg },\n",
        );

        if !self.header_cells.is_empty() {
            out.push_str("  table.header(\n");
            for i in 0..col_count {
                let cell = self.header_cells.get(i).map(|s| s.trim()).unwrap_or("");
                out.push_str(&format!("    [#strong[{cell}]],\n"));
            }
            out.push_str("  ),\n");
        }

        for row in &self.body_rows {
            for i in 0..col_count {
                let cell = row.get(i).map(|s| s.trim()).unwrap_or("");
                out.push_str(&format!("  [{cell}],\n"));
            }
        }

        out.push_str(")\n#v(6pt)\n\n");
        out
    }
}

fn markdown_to_typst(
    markdown: &str,
    doc_title: &str,
    doc_path: Option<&Path>,
    settings: PrintSettings,
    palette: &ThemePalette,
    virtual_files: &mut HashMap<String, Bytes>,
) -> String {
    let body_md = strip_frontmatter(markdown);
    let colors = resolve_print_colors(settings, palette);
    let page_bg = &colors.page_bg;
    let flipped = matches!(settings.orientation, PageOrientation::Landscape);
    let body_font = if PRINT_TYPST_SHARED.has_segoe_ui {
        "(\"Segoe UI\", \"Libertinus Serif\")"
    } else {
        "\"Libertinus Serif\""
    };
    let footer_rule = if settings.show_page_numbers {
        format!(
            "footer: context align(right, text(size: 8.5pt, fill: rgb(\"{}\"))[#counter(page).display(\"1 / 1\", both: true)])",
            colors.muted
        )
    } else {
        "footer: none".to_string()
    };

    let mut out = String::with_capacity(body_md.len() * 2 + 2048);
    out.push_str(&format!(
        r#"#let cm-text = rgb("{text}")
#let cm-muted = rgb("{muted}")
#let cm-accent = rgb("{accent}")
#let cm-surface = rgb("{surface}")
#let cm-header-bg = rgb("{header_bg}")
#let cm-stripe-bg = rgb("{stripe_bg}")
#let cm-border = rgb("{border}")
#let cm-code-bg = rgb("{code_bg}")

#set document(title: "{title}")
#set page(
  paper: "{paper}",
  flipped: {flipped},
  margin: (x: {margin}, y: {margin}),
  fill: rgb("{page_bg}"),
  {footer_rule},
)
#set text(font: {body_font}, size: 10pt, fill: cm-text)
#set par(leading: 0.68em, spacing: 0.9em, justify: false)
#show raw: set text(font: "DejaVu Sans Mono", size: 8.8pt)
#show raw.where(block: false): it => box(
  fill: cm-code-bg,
  inset: (x: 3.5pt, y: 1.5pt),
  outset: (y: 1pt),
  radius: 3pt,
  it,
)
#show raw.where(block: true): it => block(
  width: 100%,
  fill: cm-code-bg,
  stroke: 0.5pt + cm-border,
  inset: 9pt,
  radius: 5pt,
  breakable: true,
  it,
)
#show heading: set block(sticky: true, above: 1.3em, below: 0.55em)
#show heading.where(level: 1): it => {{
  set text(size: 19pt, weight: "bold", fill: cm-text)
  block(below: 0.7em)[
    #it
    #v(-4pt)
    #line(length: 100%, stroke: 0.6pt + cm-border)
  ]
}}
#show heading.where(level: 2): set text(size: 14.5pt, weight: "bold", fill: cm-text)
#show heading.where(level: 3): set text(size: 12pt, weight: "semibold", fill: cm-text)
#show heading.where(level: 4): set text(size: 10.8pt, weight: "semibold", fill: cm-text)
#show heading.where(level: 5): set text(size: 10pt, weight: "semibold", fill: cm-muted)
#show heading.where(level: 6): set text(size: 9.5pt, weight: "semibold", fill: cm-muted)
#show link: it => text(fill: cm-accent)[#underline(stroke: 0.4pt + cm-accent, offset: 2pt, it)]

"#,
        text = colors.text,
        muted = colors.muted,
        accent = colors.accent,
        surface = colors.surface,
        header_bg = colors.header_bg,
        stripe_bg = colors.stripe_bg,
        border = colors.border,
        code_bg = colors.code_bg,
        title = escape_typst_string(doc_title),
        paper = settings.paper.typst_name(),
        flipped = flipped,
        margin = settings.margin.typst_mm(),
        page_bg = page_bg,
        footer_rule = footer_rule,
        body_font = body_font,
    ));

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(body_md, opts);

    let mut buffers: Vec<String> = vec![String::new()];
    let mut table_stack: Vec<TableBuilder> = Vec::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();
    let mut in_image = false;
    let mut image_alt = String::new();
    let mut image_url = String::new();
    let mut image_counter: usize = 0;
    let mut link_stack: Vec<String> = Vec::new();
    let mut in_list_item_first_para: Vec<bool> = Vec::new();

    for event in parser {
        if in_code_block {
            match event {
                Event::Text(t) => code_buf.push_str(&t),
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    let trimmed_code = code_buf.trim_end_matches('\n');
                    let raw_call = if code_lang.is_empty() {
                        format!("#raw(\"{}\", block: true)\n\n", escape_typst_string(trimmed_code))
                    } else {
                        format!(
                            "#raw(\"{}\", block: true, lang: \"{}\")\n\n",
                            escape_typst_string(trimmed_code),
                            escape_typst_string(&code_lang)
                        )
                    };
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str(&raw_call);
                    }
                    code_buf.clear();
                    code_lang.clear();
                }
                _ => {}
            }
            continue;
        }

        if in_image {
            match event {
                Event::Text(t) | Event::Code(t) => image_alt.push_str(&t),
                Event::End(TagEnd::Image) => {
                    in_image = false;
                    let rendered = render_typst_image(
                        &image_url,
                        &image_alt,
                        doc_path,
                        virtual_files,
                        &mut image_counter,
                    );
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str(&rendered);
                    }
                    image_alt.clear();
                    image_url.clear();
                }
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => {
                    let prefix = match level {
                        HeadingLevel::H1 => "=",
                        HeadingLevel::H2 => "==",
                        HeadingLevel::H3 => "===",
                        HeadingLevel::H4 => "====",
                        HeadingLevel::H5 => "=====",
                        HeadingLevel::H6 => "======",
                    };
                    if let Some(cur) = buffers.last_mut() {
                        if !cur.is_empty() && !cur.ends_with('\n') {
                            cur.push('\n');
                        }
                        cur.push_str(prefix);
                        cur.push(' ');
                    }
                }
                Tag::BlockQuote(_) => {
                    buffers.push(String::new());
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_buf.clear();
                    code_lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::List(start) => {
                    if let Some(cur) = buffers.last_mut()
                        && !cur.is_empty()
                        && !cur.ends_with('\n')
                    {
                        cur.push('\n');
                    }
                    list_stack.push(start);
                }
                Tag::Item => {
                    let depth = list_stack.len().saturating_sub(1);
                    let indent = "  ".repeat(depth);
                    let marker = match list_stack.last().copied().flatten() {
                        Some(_) => "+ ",
                        None => "- ",
                    };
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str(&indent);
                        cur.push_str(marker);
                    }
                    in_list_item_first_para.push(true);
                }
                Tag::Emphasis => {
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str("#emph[");
                    }
                }
                Tag::Strong => {
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str("#strong[");
                    }
                }
                Tag::Strikethrough => {
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str("#strike[");
                    }
                }
                Tag::Link { dest_url, .. } => {
                    let url = dest_url.to_string();
                    let is_external = url.starts_with("http://")
                        || url.starts_with("https://")
                        || url.starts_with("mailto:");
                    if let Some(cur) = buffers.last_mut() {
                        if is_external {
                            cur.push_str(&format!(
                                "#link(\"{}\")[",
                                escape_typst_string(&url)
                            ));
                        } else {
                            cur.push_str("#text(fill: cm-accent)[");
                        }
                    }
                    link_stack.push(url);
                }
                Tag::Image { dest_url, .. } => {
                    in_image = true;
                    image_url = dest_url.to_string();
                    image_alt.clear();
                }
                Tag::Table(alignments) => {
                    table_stack.push(TableBuilder::new(alignments));
                }
                Tag::TableHead => {
                    if let Some(tb) = table_stack.last_mut() {
                        tb.in_head = true;
                        tb.current_row.clear();
                    }
                }
                Tag::TableRow => {
                    if let Some(tb) = table_stack.last_mut() {
                        tb.current_row.clear();
                    }
                }
                Tag::TableCell => {
                    buffers.push(String::new());
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    if let Some(cur) = buffers.last_mut() {
                        if !list_stack.is_empty() {
                            if let Some(first) = in_list_item_first_para.last_mut() {
                                *first = false;
                            }
                            cur.push('\n');
                        } else {
                            cur.push_str("\n\n");
                        }
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(cur) = buffers.last_mut() {
                        cur.push_str("\n\n");
                    }
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(inner) = buffers.pop()
                        && let Some(parent) = buffers.last_mut()
                    {
                        parent.push_str(&format!(
                            "#block(width: 100%, fill: cm-surface, stroke: (left: 2.5pt + cm-accent), inset: (left: 10pt, right: 8pt, y: 6pt), radius: (right: 4pt), breakable: true)[{}]\n\n",
                            inner.trim()
                        ));
                    }
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    if list_stack.is_empty()
                        && let Some(cur) = buffers.last_mut()
                    {
                        cur.push('\n');
                    }
                }
                TagEnd::Item => {
                    in_list_item_first_para.pop();
                    if let Some(cur) = buffers.last_mut()
                        && !cur.ends_with('\n')
                    {
                        cur.push('\n');
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    if let Some(cur) = buffers.last_mut() {
                        cur.push(']');
                    }
                }
                TagEnd::Link => {
                    link_stack.pop();
                    if let Some(cur) = buffers.last_mut() {
                        cur.push(']');
                    }
                }
                TagEnd::TableCell => {
                    if let Some(cell_content) = buffers.pop()
                        && let Some(tb) = table_stack.last_mut()
                    {
                        tb.current_row.push(cell_content);
                    }
                }
                TagEnd::TableHead => {
                    if let Some(tb) = table_stack.last_mut() {
                        tb.header_cells = std::mem::take(&mut tb.current_row);
                        tb.in_head = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some(tb) = table_stack.last_mut() {
                        let row = std::mem::take(&mut tb.current_row);
                        if tb.in_head {
                            tb.header_cells = row;
                        } else {
                            tb.body_rows.push(row);
                        }
                    }
                }
                TagEnd::Table => {
                    if let Some(tb) = table_stack.pop()
                        && let Some(cur) = buffers.last_mut()
                    {
                        cur.push_str(&tb.render());
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if let Some(cur) = buffers.last_mut() {
                    cur.push_str(&escape_typst_content(&text));
                }
            }
            Event::Code(code) => {
                if let Some(cur) = buffers.last_mut() {
                    cur.push_str(&format!("#raw(\"{}\")", escape_typst_string(&code)));
                }
            }
            Event::SoftBreak => {
                if let Some(cur) = buffers.last_mut() {
                    cur.push(' ');
                }
            }
            Event::HardBreak => {
                if let Some(cur) = buffers.last_mut() {
                    cur.push_str(" \\\n");
                }
            }
            Event::Rule => {
                if let Some(cur) = buffers.last_mut() {
                    cur.push_str("\n#v(4pt)\n#line(length: 100%, stroke: 0.6pt + cm-border)\n#v(4pt)\n\n");
                }
            }
            Event::TaskListMarker(checked) => {
                if let Some(cur) = buffers.last_mut() {
                    if checked {
                        cur.push_str("#box(width: 8pt, height: 8pt, baseline: 1pt, fill: cm-accent, radius: 1.5pt)[#align(center + horizon, text(size: 6.5pt, fill: white, weight: \"bold\")[✓])] ");
                    } else {
                        cur.push_str("#box(width: 8pt, height: 8pt, baseline: 1pt, stroke: 0.8pt + cm-muted, radius: 1.5pt)[] ");
                    }
                }
            }
            Event::Html(_) | Event::InlineHtml(_) | Event::FootnoteReference(_) => {}
            _ => {}
        }
    }

    if let Some(main_body) = buffers.first() {
        if main_body.trim().is_empty() {
            out.push_str("#text(fill: cm-muted)[#emph[(Empty document)]]\n");
        } else {
            out.push_str(main_body);
        }
    }

    out
}

fn render_typst_image(
    url: &str,
    alt: &str,
    doc_path: Option<&Path>,
    virtual_files: &mut HashMap<String, Bytes>,
    counter: &mut usize,
) -> String {
    if let Some(resolved) = resolve_local_image(url, doc_path)
        && let Ok(bytes) = std::fs::read(&resolved)
    {
        let ext = resolved
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "png".to_string());
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "svg") {
            let id = *counter;
            *counter += 1;
            let vpath = format!("/assets/img_{id}.{ext}");
            virtual_files.insert(vpath.clone(), Bytes::new(bytes));
            return format!(
                "\n#align(center)[#image(\"{}\", width: 85%)]\n",
                escape_typst_string(&vpath)
            );
        }
    }
    let label = if alt.trim().is_empty() { url } else { alt };
    format!(
        "#box(fill: cm-surface, stroke: 0.5pt + cm-border, inset: (x: 6pt, y: 3pt), radius: 3pt)[#text(size: 8.5pt, fill: cm-muted)[Image: {}]]",
        escape_typst_content(label)
    )
}
