//! Mermaid diagram compiler using `merman` (`SvgPipelinePreset::ResvgSafe`)
//! with a 10s deadline, `sized(svg)` viewBox dimension rewriting, and content-hash LRU cache.

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use merman::render::{RenderOutput, RenderRequest, SvgRequest};
use merman::svg::{SvgOutputPolicy, SvgPipelinePreset, SvgRenderOptions};
use merman::{Engine, OperationControl, Renderer};

/// Maximum time allowed for a single Mermaid diagram render.
pub const MERMAID_DEADLINE: Duration = Duration::from_secs(10);
const MERMAID_CACHE_CAPACITY: usize = 128;

/// Content-hash LRU cache for compiled Mermaid SVGs.
#[derive(Debug)]
pub struct MermaidSvgCache {
    capacity: usize,
    entries: HashMap<u64, Result<String, String>>,
    order: VecDeque<u64>,
}

impl Default for MermaidSvgCache {
    fn default() -> Self {
        Self::new(MERMAID_CACHE_CAPACITY)
    }
}

impl MermaidSvgCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: u64) -> Option<Result<String, String>> {
        let val = self.entries.get(&key)?.clone();
        self.order.retain(|&k| k != key);
        self.order.push_back(key);
        Some(val)
    }

    pub fn insert(&mut self, key: u64, val: Result<String, String>) {
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

static GLOBAL_MERMAID_CACHE: LazyLock<Mutex<MermaidSvgCache>> =
    LazyLock::new(|| Mutex::new(MermaidSvgCache::default()));

/// Computes a deterministic FNV-1a hash for Mermaid source.
pub fn mermaid_cache_key(source: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in source.trim().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Compiles Mermaid diagram source to a `ResvgSafe` SVG with pixel `width`/`height`
/// rewritten from `viewBox`, using a 10s deadline and content-hash LRU cache.
pub fn compile_mermaid_svg(source: &str) -> Result<String, String> {
    let key = mermaid_cache_key(source);
    if let Ok(mut guard) = GLOBAL_MERMAID_CACHE.lock()
        && let Some(cached) = guard.get(key)
    {
        return cached;
    }

    let result = compile_mermaid_uncached(source);
    if let Ok(mut guard) = GLOBAL_MERMAID_CACHE.lock() {
        guard.insert(key, result.clone());
    }
    result
}

fn compile_mermaid_uncached(source: &str) -> Result<String, String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err("Empty Mermaid diagram".to_string());
    }

    static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::new);
    let renderer = Renderer::new().with_engine(ENGINE.clone());
    let output_policy = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        ..SvgOutputPolicy::default()
    };
    let request = SvgRequest {
        pipeline: Some(output_policy.pipeline()),
        options: SvgRenderOptions::default(),
        ..SvgRequest::default()
    };
    let control = OperationControl::new().with_deadline(MERMAID_DEADLINE);
    match renderer.render(RenderRequest::svg(trimmed, control, request)) {
        Ok(RenderOutput::Svg(Some(svg))) => Ok(sized(svg.svg())),
        Ok(_) => Err("Unexpected Mermaid render output".to_string()),
        Err(e) => Err(format!("Mermaid render error: {e}")),
    }
}

/// Rewrites percentage `width="100%"` / `height="100%"` attributes on the root `<svg>`
/// to concrete pixel dimensions derived from `viewBox="minX minY width height"`.
pub fn sized(svg: &str) -> String {
    let Some(vb_start) = svg.find("viewBox=\"") else {
        return svg.to_string();
    };
    let after_vb = &svg[vb_start + "viewBox=\"".len()..];
    let Some(vb_end) = after_vb.find('"') else {
        return svg.to_string();
    };
    let nums: Vec<&str> = after_vb[..vb_end].split_whitespace().collect();
    if nums.len() != 4 {
        return svg.to_string();
    }
    let w = nums[2];
    let h = nums[3];

    let mut out = svg.to_string();
    if out.contains("width=\"100%\"") {
        out = out.replacen("width=\"100%\"", &format!("width=\"{w}\""), 1);
    }
    if out.contains("height=\"100%\"") {
        out = out.replacen("height=\"100%\"", &format!("height=\"{h}\""), 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_rewrites_percentage_dimensions_from_viewbox() {
        let raw = r#"<svg viewBox="0 0 320 180" width="100%" height="100%"></svg>"#;
        let out = sized(raw);
        assert!(out.contains(r#"width="320""#));
        assert!(out.contains(r#"height="180""#));
    }

    #[test]
    fn compiles_simple_flowchart_to_svg() {
        let svg = compile_mermaid_svg("graph TD;\n  A-->B;").unwrap();
        assert!(svg.contains("<svg"));
    }
}
