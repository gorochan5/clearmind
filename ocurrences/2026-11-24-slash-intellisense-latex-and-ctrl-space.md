# Slash Commands, LaTeX Intellisense & `Ctrl+Space` Not Triggering

**Date:** 2026-11-24

## 1. Context & Symptom
Typing `/` in the editor did not open the native command suggestions (`/code`, `/latex`, `/table`, `/mermaid`, etc.), entering a LaTeX block did not switch Intellisense to LaTeX math functions, and pressing `Ctrl+Space` showed no Intellisense popup at all.

## 2. Investigation & Evidence
- `src/core/custom_commands/builtins.rs` and `src/core/custom_commands/variables.rs` already contained `builtin_prose_commands()`, `builtin_math_commands()`, `suggestions_for_context()`, and `expand_template()`.
- However, `ClearMind::slash_popup` in `src/core/models.rs` was an unused `Option<Vec<String>>` placeholder:
  - Nothing populated it when typing `/`, typing `\` inside math, or pressing `Ctrl+Space`.
  - `CommandId::SlashTrigger` fell through to `_ => Task::none()` in `src/core/update.rs`.
  - No floating popup widget was rendered in `src/core/view.rs`.
  - `LiveMarkdownEditor::update` always captured `ArrowUp`, `ArrowDown`, `Enter`, and `Tab` even when a completion popup needed them.
  - `detect_trigger_context` only inspected `$` counts on the current line and did not detect multi-line `$$ ... $$` display math blocks or fenced code blocks across preceding lines.

## 3. Root Cause
The Slash / LaTeX Intellisense backend helpers existed in isolation without state management (`SlashMenuState`), document-level math/code context detection, `Ctrl+Space` + `/` + `\` editor triggers, keyboard arbitration against `LiveMarkdownEditor`, atomic template insertion, or a floating popup view layer.

## 4. Resolution & Takeaways
- Added `SlashMenuState` in `src/core/custom_commands/models.rs` and document-aware `detect_document_trigger_context()` in `src/core/custom_commands/builtins.rs` (detecting multi-line `$$ ... $$` display math blocks, inline `$ ... $`, fenced code blocks, inline code spans, and URLs).
- Expanded `builtin_math_commands()` with 31 LaTeX functions (`\frac`, `\sum`, `\int`, `\prod`, `\lim`, `\sqrt`, matrices, cases, Greek symbols, and operators) and added dynamic inline argument expansion (`expand_suggestion`) for `/table [RxC]`, `/code [lang]`, `/mermaid [type]`, and `/callout [kind]`.
- Added floating `view_slash_popup` in `src/core/custom_commands/view.rs` and layered it in `src/core/view.rs`.
- Gated `ArrowUp`/`ArrowDown`/`Enter`/`Tab` in `LiveMarkdownEditor` via `slash_menu_open` so keyboard navigation operates on the popup without mutating the line, and wired `/`, `\` (in math), `Ctrl+Space`, live filtering, mouse click acceptance, `Esc` dismissal, and atomic undoable template expansion with `{{cursor}}` placement in `src/core/update.rs`.
- Made `DisplayMath` and `Mermaid` blocks clickable to re-enter edit mode and wired cached Typst math SVG rendering for display math blocks in `src/core/editor/view.rs`.
- Validated with `cargo check --all-targets` and `cargo clippy --all-targets` (0 errors, 0 warnings).
