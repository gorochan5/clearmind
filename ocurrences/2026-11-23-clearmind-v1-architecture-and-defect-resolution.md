# ClearMind v1.0.0 Architecture & Defect-Resolution Migration

> **Date:** 2026-11-23  
> **Scope:** Full implementation of `specs_v1.md` (Phases 0–4) — decomposition of monolithic `src/main.rs` (4,369 lines) into `src/core/` fractal TEA modules, custom `LiveMarkdownEditor` widget, `iced 0.15.0-dev` API alignment, and pure-Rust Math/Mermaid SVG rendering.

## 1. Context & Symptom

ClearMind originally lived in a single 4,369-line `src/main.rs` plus an 8,702-line `src/icons.rs`. That architecture suffered from:
- Per-line isolated `markdown::parse(trimmed)` calls that broke multi-line code blocks, GFM tables, nested/ordered lists, blockquotes/GitHub Alerts, and reference links.
- Dual incompatible selection models (`selected_lines` vs. `TextEditor` selection), destructive `Ctrl+A`, and multi-cursor layout shifts.
- Missing persistence (`clearmind.toml`), hot-exit crash recovery, borderless window resize edges, and custom `LiveMarkdownEditor` (`Paragraph::hit_test`, IME, 2D selection).

## 2. Investigation & Evidence

1. **Structural & Dependency Audit:**
   - `Cargo.toml` needed `iced` features `["markdown", "highlighter", "advanced", "image", "svg"]` plus `tylax`, `typst`, `typst-svg`, `typst-assets`, `typst-layout`, `comemo`, `merman`, `merman-render`, `pulldown-cmark`, `arboard`, `rfd`, `reqwest` (`default-tls` / `native-tls` on Windows to avoid `aws-lc-sys` CMake/NASM builds), `ropey`, `unicode-segmentation`, `ignore`, `infer`, `opener`, and `toml`.
   - `merman = "=0.8.0-alpha.6"` required `merman-render = { version = "0.8.0", features = ["diagram-class", "diagram-flowchart", "diagram-mindmap"] }` for Cargo feature unification.
2. **`iced 0.15.0-dev` API Verification (`docs/iced/markdown/`):**
   - Custom widgets must implement `iced::advanced::widget::Meta` alongside `Widget`, with `layout()` mutating `tree.size` in-place, `update()` using `shell.capture_event()`, and `shell.request_input_method` called during `window::Event::RedrawRequested`.
   - `iced::Element` does not implement `From<W: Widget>`; views must `use iced::Widget;` and call `.boxed()` and `.map(...).boxed()`.

## 3. Root Cause

The monolithic `src/main.rs` coupled UI rendering, Markdown parsing, file I/O, and state management in one file, while `iced 0.15.0-dev` and `typst 0.15.1` / `merman 0.8.0-alpha.6` had specific trait/feature contracts that differed from older Iced versions.

## 4. Resolution & Takeaways

- **Modular TEA Architecture (`src/core/`):** Reduced `src/main.rs` to 32 lines and decomposed the application into `themes/`, `watcher/`, `config/`, `fonts/`, `word_wrap/`, `margin/`, `keybindings/`, `custom_commands/`, `widgets/`, `markdown/`, `editor/`, and `navbar/`.
- **LiveMarkdownEditor & UI/UX Refinements:**
  - Implemented `CosmicParagraph`-backed glyph measurement (`caret_rect_from_paragraph`), `Paragraph::hit_test`, inline Markdown span styling (`**bold**`, `*italic*`, `_italic_`, `~~strikethrough~~`, `` `inline code` ``, `[links](url)`), interactive checklists (`- [ ] `, `- [x] `, `-[ ] `, `-[x] `), word-wrap height layout, and strict viewport clipping.
  - Set `Clearmind Light` (`#C2CEC0` rosemary primary) as default theme, stored all themes in `clearmind.toml` under `[themes.<name>]` with customizable `theme_icon` emojis, and replaced stepper buttons with sliders and system-font `pick_list` dropdowns.
  - Wired CLI file opening (`clearmind.exe <file>`), safe Windows extended-length path normalization in `watcher::normalized_path`, and the navbar Recent Files dropdown. Note that running `clearmind.exe` from PowerShell resolves `C:\Users\Redseek\.local\bin\clearmind.exe` on `PATH` unless invoked via `cargo install --path .` or `.\target\release\clearmind.exe`.
  - Fixed document scrolling in `iced 0.15.0-dev`:
    1. Wrapped top banners in a dedicated fixed-index `top_banners` child inside `root = column![top_banners, scroll_view, status]` so `scroll_view` never shifts tree index in `Tree::diff_children` when `tab.scroll_offset` changes (previously `top_stack` pushed a sticky-headings bar at `scroll_offset >= row_h`, shifting `scroll_view` from index 0 to index 1 and resetting `Scrollable` state to `0.0` on the very first wheel notch).
    2. Replaced `column(vec)` / `row(vec)` (`Column::from_vec` / `Row::from_vec`) with `column![].extend(vec)` / `row![].extend(vec)` (`Column::with_children` / `Row::with_children`) so `Length::Fill` children are inspected by `Column::push` instead of defaulting to `Length::Fit` (which compressed inner `LiveMarkdownEditor` widths to `0.0`).
    3. Updated `scrollable::on_scroll` to return `scrollable::Action::None` during intermediate smooth-scroll animation frames (`vp.target.is_some()`) and initial `Content`/`Resize` notifications, only publishing `EditorMessage::Scrolled` when a scroll settles, avoiding full-document `view()` rebuilds on every smooth-scroll frame.
    4. Added `PageUp`, `PageDown`, and Viewing-mode `ArrowUp`/`ArrowDown` smooth scrolling, and corrected the `row_height` / `viewport_height` arguments passed to `update_editor`.
- **Verification:** `cargo check --all-targets` and `cargo clippy --all-targets` both pass with exit code 0 and 0 warnings, zero `from_rgb8`/`from_rgba8` literals exist outside `src/core/themes/presets.rs`, and no file in `src/core/` (excluding `icons.rs`) exceeds 800 lines.
- **Takeaway:** Always verify `iced 0.15.0-dev` signatures against `docs/iced/markdown/` before writing widget code, and use `use iced::Widget;` + `.boxed()` / `.map(...).boxed()` for composable TEA views.
