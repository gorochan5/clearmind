# `Ctrl+D` Line/Selection Duplication & Unwired Line Shortcuts

**Date:** 2026-11-24

## 1. Context & Symptom
Pressing `Ctrl+D` in the editor did not duplicate the current line (or selection).

## 2. Investigation & Evidence
- Checked `src/core/keybindings/models.rs`: `CommandId::EditDuplicate` was registered with default shortcut `Ctrl+D`.
- Checked `src/core/update.rs`: `execute_command` did not handle `CommandId::EditDuplicate`, `CommandId::EditMoveLineUp`, `CommandId::EditMoveLineDown`, or `CommandId::FormatTaskToggle` — they all fell through to `_ => Task::none()`.

## 3. Root Cause
`CommandId::EditDuplicate` (along with `EditMoveLineUp`, `EditMoveLineDown`, and `FormatTaskToggle`) had no handler wired in `ClearMind::execute_command` or `update_editor`.

## 4. Resolution & Takeaways
- Added `EditorMessage::DuplicateSelectionOrLine`, `MoveLinesUp`, `MoveLinesDown`, and `ToggleActiveTask` in `src/core/editor/update.rs`, backed by atomic undoable transactions:
  - `duplicate_selection_or_line`: duplicates the active line below the cursor when collapsed, duplicates inline selection right after the selection, or duplicates all covered lines below for multi-line selections.
  - `move_selected_lines`: moves active line(s) up/down (`Alt+Up` / `Alt+Down`).
  - `toggle_task_on_line`: toggles checklist items (`Ctrl+L`).
- Wired all four commands in `ClearMind::execute_command` (`src/core/update.rs`) with modal/search/popover guards.
- Guarded `LiveMarkdownEditor::update` (`src/core/editor/live_widget.rs`) to skip `EditorKeyCommand` when `Alt` is held (and skip `ArrowUp`/`ArrowDown`/`Enter` when `Ctrl` is held), preventing `LiveMarkdownEditor` from moving the caret before `Alt+Up` / `Alt+Down` swaps the active line.
- Verified with `cargo check --all-targets` and `cargo clippy --all-targets` (0 errors, 0 warnings).
