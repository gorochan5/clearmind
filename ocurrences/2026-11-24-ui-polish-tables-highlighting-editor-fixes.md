# ClearMind UI Polish, Table Parsing, Syntax Highlighting & Editor Interaction Fixes

> **Date:** 2026-11-24  
> **Scope:** Resolved 10 user-reported bugs and aesthetic defects across the editor, Markdown renderer, Settings modal, and Quick Access popovers.

## 1. Context & Symptom

During live testing of `clearmind.exe specs_v1.md`, the user reported:
- Pressing `Esc` while editing inserted a square tofu character (`\x1b`).
- `Ctrl+Shift+Left` / `Ctrl+Shift+Right` did not select words.
- Clicking a line in a large scrolled document jumped/scrolled the viewport upward.
- GFM Markdown tables were not rendering at all.
- Fenced code blocks had no syntax highlighting.
- Quick Access popovers for Fonts and Themes rendered a "box inside a box".
- Settings modal had no right margin (scrollbar clipped the `Reset` buttons in Keybindings).
- Default rectangular `pick_list` dropdowns, Keybindings rows (`file.new_tab` + 3 buttons), and Custom Slash Commands (blank gap + unstyled inputs) looked unpolished.

## 2. Investigation & Evidence

1. **`Esc` tofu character (`src/core/editor/live_widget.rs`):** On Windows, `Key::Named(Named::Escape)` arrives with `text: Some("\u{1b}")`. `LiveMarkdownEditor::update` fell through to the text insertion branch without filtering control characters.
2. **Table parsing failure (`src/core/markdown/table.rs`):** `parse_delimiter_row` wrote `c.strip_prefix(':').unwrap_or(c).strip_suffix(':').unwrap_or(c)`. For left-aligned `:---` columns, `strip_suffix(':')` returned `None`, falling back to the original `c` (`":---"`), which failed `chars().all(|ch| ch == '-')`.
3. **Click scroll-up jump (`src/core/editor/update.rs`):** `PlaceCaret` and `JumpToLine` called `auto_scroll_task`, which approximated `line_y = line * row_height`. In large documents with headings, code blocks, tables, and wrapped paragraphs, `line * row_height` severely underestimated the real Y offset, causing `line_y < tab.scroll_offset` to trigger `scroll_to` upward even though the clicked line was already visible under the cursor.
4. **Double-boxed popovers (`src/core/widgets/popover.rs`, `src/core/fonts/view.rs`, `src/core/themes/view.rs`):** Both `popover_overlay` and each `view_quick_popover` drew a bordered padded card.

## 3. Root Cause

- Missing control-character filtering and word-navigation variants in `LiveMarkdownEditor` / `handle_key_command`.
- Chained `.unwrap_or(c)` bug in `parse_delimiter_row`.
- Naive `line * row_height` scroll targeting triggered on mouse click.
- Missing custom `pick_list` / `menu` / `text_input` / `slider` styling and redundant popover container wrapping.

## 4. Resolution & Takeaways

- **Editor fixes:** Filtered `c.is_control()` and early-returned on `Named::Escape` in `LiveMarkdownEditor::update`; added `WordLeft`, `WordRight`, `DocHome`, `DocEnd`, `DeleteWordBackward`, `DeleteWordForward` in `live_widget.rs` & `editor/update.rs`; removed naive auto-scroll on `PlaceCaret`, `JumpToLine`, and `ClickCanvasBottom`.
- **Tables & Syntax Highlighting:** Fixed `parse_delimiter_row` and `parse_table` in `src/core/markdown/table.rs`, fixed `scan_blocks` in `src/core/markdown/block_scanner.rs` so ATX headings and paragraph loops never swallow immediately-following GFM tables (such as `### 11.1 Performance Budgets` followed directly by `| Metric | Budget |`), upgraded `view_table_grid` with themed headers/dividers/alignment/inline spans, wired `ScannedBlock::ThematicBreak` (`___`, `---`, `***`) horizontal rule rendering in `src/core/editor/view.rs`, and wired `iced::highlighter::Parser` (`syntect`/`two_face`) mapped to `ThemePalette` syntax colors via `rich_text` in `src/core/editor/view.rs`.
- **UI Redesign:** Added `src/core/widgets/styles.rs` (rounded `pick_list`, dropdown `menu`, `text_input`, rosemary `slider`, and `kbd` shortcut chips), eliminated double-boxed popovers, added right padding + embedded scrollbar spacing in Settings, and redesigned Keybindings (grouped human-readable labels + clickable shortcut pills + chord recording) and Custom Slash Commands (empty-state card, `/trigger` pills, clickable variable chips).
- **Verification:** `cargo check --all-targets` and `cargo clippy --all-targets` pass with 0 errors and 0 warnings.
- **Image Paste, Drag-Drop & Rendering:** Wired `CommandId::EditPaste` (`Ctrl+V`), `EditCopy`, `EditCut`, and `EditSelectAll` in `src/core/update.rs` & `src/core/editor/update.rs` using `arboard` (`get_image()`, `file_list()`, `get_text()`) + PNG encoding via `image::RgbaImage` into `assets/` (or `<app_data_dir>/assets/` for untitled tabs), handled dropped image files in `window::Event::FileDropped`, fixed `<path with spaces>` parsing in `src/core/markdown/image.rs`, and replaced the `[Image: ...]` placeholder in `src/core/editor/view.rs` with actual `iced::widget::image` / `iced::widget::svg` rendering.
- **Plain Blockquotes vs. GitHub Alert Callouts:** Restricted `ScannedBlock::Callout` interception in `src/core/editor/view.rs` to `c.alert.is_some()` so plain `> ...` blockquotes render per-line with their left accent bar and inline Markdown immediately on load instead of falling into a plain-text callout box until clicked, and upgraded GitHub Alert cards (`> [!NOTE]`, `> [!TIP]`, etc.) to render styled headers and inline Markdown via `rich_text`.
- **Interactive Tables & Thin Borders (§6.9 item 5, `Malgel`-style):** Fixed vertical column dividers (`height(Length::Fill)` + `width(Length::Fixed(1.0))` so Iced flex's `CrossFluidDeferred` pass stretches every vertical divider to full row height), added a crisp `1px` outer border, kept tables rendered as a live grid when clicked (no more derendering to raw pipes!), added in-place focused cell editing (`text_input`), right/bottom `+` quick-add buttons, and a floating edit-mode toolbar for inserting/moving/deleting rows & columns and toggling Left/Center/Right column alignment (each rewriting aligned Markdown pipes as a single undo transaction).
- **Missing `"tokio"` feature on `iced` (Root Cause of Silent `Ctrl+V` Paste / `Ctrl+O` Open / `Save As` / File Watcher Failures):** `Cargo.toml` enabled `iced` without the `"tokio"` feature, so `iced::application` defaulted to `futures_executor::ThreadPool` instead of a Tokio runtime. Every call to `tokio::task::spawn_blocking` (`read_clipboard_payload`, `pick_markdown_files`, `save_active_tab`) and `tokio::time::timeout_at` (`watcher::file_event_stream`) panicked silently with `"there is no reactor running"` inside the thread pool! Enabling `"tokio"` on `iced` in `Cargo.toml` fixes clipboard image/text paste, native Open/Save As dialogs, and file watching all at once.
- **Table collapsing to 0px height in edit mode inside `Scrollable`:** `add_col_edge` and `hairline_v` used `.height(Length::Fill)`, which propagated `Length::Fill` up through `Row::diff` (`Fit.cross(Fill) == Fill`) into the outer document `Column` inside `Scrollable`, where remaining height past the first viewport is `0.0`. Setting `.height(Length::Shrink)` on each table row, `table_box`, `grid_with_right_plus`, and the outer table edit `column!` prevents `Length::Fill` propagation while keeping `CrossFluidDeferred` full-height stretching for `hairline_v` and `add_col_edge`.
