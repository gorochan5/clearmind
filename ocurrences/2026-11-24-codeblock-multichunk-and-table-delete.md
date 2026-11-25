# `/code` Missing Closing Fence on Multi-Chunk Rope & Unable to Delete Inserted Tables

**Date:** 2026-11-24

## 1. Context & Symptom
1. Running `/code` appeared to only show the opening ```` ``` ```` line above the cursor without the closing ```` ``` ```` line below it.
2. After inserting a table (or clicking any existing table), there was no way to delete the entire table from the UI.

## 2. Investigation & Evidence
- Checked `src/core/custom_commands/builtins.rs`: `/code`'s template already expanded to `"```\n\n```"` with cursor on line `trigger_line + 1`.
- Inspected `src/core/editor/view.rs:542-543`:
  ```rust
  let line_slice = tab.rope.line(line_idx);
  let raw_line = line_slice.as_str().unwrap_or("");
  ```
- In `ropey`, `RopeSlice::as_str()` returns `None` whenever a line spans across two internal B-tree chunks — which happens right after inserting `"```\n\n```"` before an existing newline in a multi-chunk file (such as `specs_v1.md`). Because `unwrap_or("")` fell back to `""`, the closing ```` ``` ```` line rendered invisibly as a blank line!
- Checked `src/core/editor/widgets/table_grid.rs` and `src/core/editor/update.rs`: `TableOp` had `DeleteRow` (data rows only) and `DeleteCol` (when >1 column), but no `DeleteTable` operation, and clicking a table always stays in grid-edit mode instead of raw Markdown.

## 3. Root Cause
1. `view_editor` used `RopeSlice::as_str().unwrap_or("")` instead of falling back to the full concatenated `raw_lines_owned[line_idx]` string for multi-chunk rope lines.
2. Interactive table edit mode lacked a `TableOp::DeleteTable` action and toolbar button.

## 4. Resolution & Takeaways
- Changed `LiveMarkdownEditor::raw_text` from `&'a str` to `Cow<'a, str>` in `src/core/editor/live_widget.rs` and updated `src/core/editor/view.rs` to fall back to `Cow::Owned(raw_lines_owned[line_idx].clone())` whenever `line_slice.as_str()` is `None` (zero extra allocation on single-chunk lines, 100% accurate on multi-chunk lines).
- Updated `accept_slash_suggestion` in `src/core/update.rs` so multi-line block commands (`/code`, `/latex`, `/table`, `/mermaid`, `/callout`) triggered mid-line after non-whitespace text automatically start on a new line.
- Added `TableOp::DeleteTable` and `delete_table_block()` in `src/core/editor/update.rs` plus a danger-tinted `Delete` button in the floating table edit toolbar (`src/core/editor/widgets/table_grid.rs`) that removes the entire table block in a single undoable transaction (`Ctrl+Z` restores it).
- Verified with `cargo check --all-targets` and `cargo clippy --all-targets` (0 errors, 0 warnings).
