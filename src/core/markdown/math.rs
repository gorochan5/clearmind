//! Sandboxed LaTeX -> Typst -> SVG math compiler using `tylax` + `typst` (0.15.1).

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst_layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// Default maximum number of cached compiled math SVGs.
const MATH_CACHE_CAPACITY: usize = 256;

/// Result of compiling a math expression to SVG.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledMathSvg {
    /// Rendered SVG markup.
    pub svg: String,
    /// Approximate baseline offset in points (for inline `#box[$...$]`), if available.
    pub baseline_pt: Option<f32>,
}

/// Shared static resources for the sandboxed Typst world (initialized lazily on first math block).
struct TypstShared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

static TYPST_SHARED: LazyLock<TypstShared> = LazyLock::new(|| {
    let fonts: Vec<Font> = typst_assets::fonts()
        .flat_map(|data| Font::iter(Bytes::new(data)))
        .collect();
    let book = FontBook::from_fonts(&fonts);
    let library = Library::default();
    TypstShared {
        library: LazyHash::new(library),
        book: LazyHash::new(book),
        fonts,
    }
});

struct SandboxedMathWorld {
    source: Source,
}

impl SandboxedMathWorld {
    fn new(typst_markup: String) -> Self {
        let source = Source::detached(typst_markup);
        Self { source }
    }
}

impl World for SandboxedMathWorld {
    fn library(&self) -> &LazyHash<Library> {
        &TYPST_SHARED.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &TYPST_SHARED.book
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
        Err(FileError::NotFound(id.vpath().get_with_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        TYPST_SHARED.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        None
    }
}

/// Content-hash LRU cache for compiled math SVGs.
#[derive(Debug)]
pub struct MathSvgCache {
    capacity: usize,
    entries: HashMap<u64, Result<CompiledMathSvg, String>>,
    order: VecDeque<u64>,
}

impl Default for MathSvgCache {
    fn default() -> Self {
        Self::new(MATH_CACHE_CAPACITY)
    }
}

impl MathSvgCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: u64) -> Option<Result<CompiledMathSvg, String>> {
        let val = self.entries.get(&key)?.clone();
        self.order.retain(|&k| k != key);
        self.order.push_back(key);
        Some(val)
    }

    pub fn insert(&mut self, key: u64, val: Result<CompiledMathSvg, String>) {
        if self.entries.contains_key(&key) {
            self.order.retain(|&k| k != key);
        } else if self.entries.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.order.push_back(key);
        self.entries.insert(key, val);
    }
}

static GLOBAL_MATH_CACHE: LazyLock<Mutex<MathSvgCache>> =
    LazyLock::new(|| Mutex::new(MathSvgCache::default()));

/// Computes a deterministic FNV-1a hash for `(latex, inline, color_hex)`.
pub fn math_cache_key(latex: &str, inline: bool, color_hex: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in latex.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= inline as u64;
    hash = hash.wrapping_mul(0x100000001b3);
    for b in color_hex.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Compiles LaTeX math (`inline` = `#box[$...$]`, display = `$ ... $`) to SVG
/// using `tylax` + `typst` (`0.15.1`), with `comemo::evict(30)` and content-hash caching.
pub fn compile_math_svg(
    latex: &str,
    inline: bool,
    color_hex: &str,
) -> Result<CompiledMathSvg, String> {
    let key = math_cache_key(latex, inline, color_hex);
    if let Ok(mut guard) = GLOBAL_MATH_CACHE.lock()
        && let Some(cached) = guard.get(key)
    {
        return cached;
    }

    let result = compile_math_uncached(latex, inline, color_hex);
    if let Ok(mut guard) = GLOBAL_MATH_CACHE.lock() {
        guard.insert(key, result.clone());
    }
    result
}

fn compile_math_uncached(
    latex: &str,
    inline: bool,
    color_hex: &str,
) -> Result<CompiledMathSvg, String> {
    let trimmed = latex.trim();
    if trimmed.is_empty() {
        return Err("Empty math expression".to_string());
    }

    let typst_expr = tylax::latex_to_typst(trimmed);
    let sanitized_color = sanitize_hex_color(color_hex);

    let markup = if inline {
        format!(
            "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
             #set text(fill: rgb(\"{sanitized_color}\"), size: 12pt)\n\
             #box[${typst_expr}$]"
        )
    } else {
        format!(
            "#set page(width: auto, height: auto, margin: 4pt, fill: none)\n\
             #set text(fill: rgb(\"{sanitized_color}\"), size: 14pt)\n\
             $ {typst_expr} $"
        )
    };

    let world = SandboxedMathWorld::new(markup);
    let warned = typst::compile::<PagedDocument>(&world);
    comemo::evict(30);

    let document = warned.output.map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.message.to_string()).collect();
        if msgs.is_empty() {
            "Typst compilation failed".to_string()
        } else {
            msgs.join("; ")
        }
    })?;

    let page = document
        .pages()
        .first()
        .ok_or_else(|| "Typst produced no pages".to_string())?;

    let svg = typst_svg::svg(page, &typst_svg::SvgOptions::default());
    let baseline_pt = if inline { Some(2.0) } else { None };

    Ok(CompiledMathSvg { svg, baseline_pt })
}

fn sanitize_hex_color(hex: &str) -> String {
    let trimmed = hex.trim();
    if trimmed.starts_with('#')
        && (trimmed.len() == 7 || trimmed.len() == 9 || trimmed.len() == 4)
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        trimmed.to_string()
    } else {
        "#e6edf3".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_inline_and_display_math_to_svg() {
        let inline_res = compile_math_svg(r"\frac{a}{b}", true, "#ffffff").unwrap();
        assert!(inline_res.svg.contains("<svg"));
        assert!(inline_res.baseline_pt.is_some());

        let display_res = compile_math_svg(r"\sum_{i=1}^n i^2", false, "#ffffff").unwrap();
        assert!(display_res.svg.contains("<svg"));
        assert!(display_res.baseline_pt.is_none());
    }

    #[test]
    fn returns_diagnostic_error_on_empty_math() {
        assert!(compile_math_svg("   ", true, "#ffffff").is_err());
    }
}
