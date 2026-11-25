//! Editor update logic, auto-scroll, checklist toggling, clipboard image paste, file drop, and `rfd` file dialogs.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use iced::Task;
use iced::widget::operation::Animation;
use iced::widget::operation::scrollable::{AbsoluteOffset, RelativeOffset, scroll_by, scroll_to, snap_to};

use super::links::{
    LinkCategory, build_wiki_index, classify_local_path, open_local_safely, resolve_reference,
};
use super::live_widget::{EditorKeyCommand, EditorWidgetEvent};
use super::models::{
    Cursor, DocumentSelection, Edit, LineEnding, SearchMatch, SearchState, Tab, Transaction,
};
use super::rich_copy::copy_rich_text;
use super::smart_edit::{
    FormatCommand, apply_format_command, auto_pair_for, cursors_descending, dedup_cursors,
    smart_enter_insertion, soft_home_col,
};
use super::view::{EDITOR_SCROLLABLE_ID, parse_checklist_line};
use crate::core::markdown::list::renumber_ordered_list;
use crate::core::markdown::table::{
    MoveDirection, ParsedTable, TableAlignment, format_table_pipes, parse_table,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOp {
    InsertRowAbove(usize),
    InsertRowBelow(usize),
    DeleteRow(usize),
    MoveRowUp(usize),
    MoveRowDown(usize),
    InsertColLeft(usize),
    InsertColRight(usize),
    DeleteCol(usize),
    MoveColLeft(usize),
    MoveColRight(usize),
    SetColAlign(usize, TableAlignment),
    FormatPipes,
    DeleteTable,
}

#[derive(Debug, Clone)]
pub enum ClipboardPayload {
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    FilePaths(Vec<PathBuf>),
    Text(String),
    Error(String),
    Empty,
}

#[derive(Debug, Clone)]
pub enum EditorMessage {
    WidgetEvent(EditorWidgetEvent),
    Scrolled(f32),
    ScrollByLines(f32),
    PageUp { extend: bool },
    PageDown { extend: bool },
    Undo,
    Redo,
    CopySelection,
    CutSelection,
    SelectAll,
    DuplicateSelectionOrLine,
    MoveLinesUp,
    MoveLinesDown,
    ToggleActiveTask,
    ClipboardPasted(ClipboardPayload),
    Format(FormatCommand),
    SearchQueryChanged(String),
    SearchReplaceChanged(String),
    SearchToggleReplaceExpanded,
    SearchToggleCase,
    SearchToggleWord,
    SearchToggleRegex,
    SearchPrev,
    SearchNext,
    ReplaceOne,
    ReplaceAll,
    CloseSearch,
    ReloadFromDisk,
    KeepLocalChanges,
    ResetZoom,
    ToggleLineEnding,
    PasteImageFromClipboard,
    ImageBytesPasted(Option<(Vec<u8>, u32, u32)>),
    FileDropped(PathBuf),
    OpenFileDialog,
    SaveAsDialog,
    CopyRichText,
    CopyText(String),
    JumpToLine(usize),
    ClickCanvasBottom,
    OpenZoomViewer(String),
    SelectTableCell {
        table_start: usize,
        row_offset: usize,
        col: usize,
    },
    TableEditCell {
        start_line: usize,
        end_line: usize,
        row_offset: usize,
        col: usize,
        value: String,
    },
    TableAction {
        start_line: usize,
        end_line: usize,
        op: TableOp,
    },
    ExitTableEdit,
    FormatActiveTable,
    TableAddRow(usize),
    TableAddCol(usize),
    ToggleTask(usize, bool),
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn update_editor(
    tab: &mut Tab,
    search: &mut SearchState,
    msg: EditorMessage,
    row_height: f32,
    viewport_height: f32,
    typewriter_mode: bool,
) -> Task<EditorMessage> {
    match msg {
        EditorMessage::WidgetEvent(ev) => match ev {
            EditorWidgetEvent::PlaceCaret { cursor, extend } => {
                tab.active_line = Some(cursor.line);
                if extend {
                    tab.selection.head = cursor;
                } else {
                    tab.selection = DocumentSelection::collapsed(cursor);
                }
                // Do not auto-scroll on mouse click: the clicked line is already visible under the cursor.
                return Task::none();
            }
            EditorWidgetEvent::DragSelect { head } => {
                tab.selection.head = head;
            }
            EditorWidgetEvent::InsertText(txt) => {
                let mut inserted = txt.clone();
                let mut caret_advance = txt.chars().count();
                if caret_advance == 1
                    && let Some(ch) = txt.chars().next()
                {
                    let line_str = tab.rope.line(tab.selection.head.line).to_string();
                    let col = tab.selection.head.col;
                    let prev_char = col.checked_sub(1).and_then(|c| line_str.chars().nth(c));
                    let next_char = line_str.chars().nth(col);
                    if let Some((open, close)) = auto_pair_for(ch, prev_char, next_char, false) {
                        inserted = format!("{open}{close}");
                        caret_advance = 1;
                    }
                }

                dedup_cursors(tab.selection.head, &mut tab.extra_cursors);
                let _descending = cursors_descending(tab.selection.head, &tab.extra_cursors);

                let (start, end) = tab.selection.ordered();
                let start_byte = tab.line_col_to_byte(start);
                let end_byte = tab.line_col_to_byte(end);
                let deleted = tab
                    .rope
                    .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                    .to_string();
                let new_col = start.col + caret_advance;
                let after = Cursor::new(start.line, new_col);
                let tx = Transaction {
                    edits: vec![Edit {
                        start_byte,
                        end_byte,
                        deleted,
                        inserted,
                    }],
                    selection_before: tab.selection,
                    selection_after: DocumentSelection::collapsed(after),
                    extra_cursors_before: tab.extra_cursors.clone(),
                    extra_cursors_after: tab.extra_cursors.clone(),
                    active_line_before: tab.active_line,
                    active_line_after: Some(after.line),
                    scroll_before: tab.scroll_offset,
                    timestamp_ms: now_ms(),
                    is_coalesceable: true,
                };
                tab.apply_transaction(tx);
            }
            EditorWidgetEvent::KeyCommand(cmd) => {
                handle_key_command(tab, cmd);
                match cmd {
                    EditorKeyCommand::DocHome { .. } => {
                        return snap_to(
                            EDITOR_SCROLLABLE_ID,
                            RelativeOffset::START,
                            Animation::Instant,
                        );
                    }
                    EditorKeyCommand::DocEnd { .. } => {
                        return snap_to(
                            EDITOR_SCROLLABLE_ID,
                            RelativeOffset::END,
                            Animation::Instant,
                        );
                    }
                    EditorKeyCommand::Up { .. } if typewriter_mode => {
                        return scroll_by(
                            EDITOR_SCROLLABLE_ID,
                            AbsoluteOffset {
                                x: 0.0,
                                y: -row_height,
                            },
                            Animation::Instant,
                        );
                    }
                    EditorKeyCommand::Down { .. } | EditorKeyCommand::Enter if typewriter_mode => {
                        return scroll_by(
                            EDITOR_SCROLLABLE_ID,
                            AbsoluteOffset {
                                x: 0.0,
                                y: row_height,
                            },
                            Animation::Instant,
                        );
                    }
                    _ => return Task::none(),
                }
            }
            EditorWidgetEvent::OpenLink {
                target,
                reveal_in_explorer,
            } => {
                let doc_dir = tab.path.as_deref().and_then(|p| p.parent());
                let wiki_index = doc_dir.map(build_wiki_index).unwrap_or_default();
                if let Some(resolved) = resolve_reference(&target, doc_dir, &wiki_index) {
                    let cat = classify_local_path(&resolved);
                    if cat != LinkCategory::Markdown {
                        let _ = open_local_safely(&resolved, reveal_in_explorer);
                    }
                }
            }
        },
        EditorMessage::Scrolled(y) => {
            tab.scroll_offset = y;
        }
        EditorMessage::ScrollByLines(lines) => {
            return scroll_by(
                EDITOR_SCROLLABLE_ID,
                AbsoluteOffset {
                    x: 0.0,
                    y: lines * row_height,
                },
                Animation::Smooth,
            );
        }
        EditorMessage::PageUp { extend } => {
            let page_lines = ((viewport_height / row_height).floor() as usize).max(5);
            let mut head = tab.selection.head;
            head.line = head.line.saturating_sub(page_lines);
            head.col = head.col.min(visible_line_len(tab, head.line));
            if tab.active_line.is_some() {
                tab.active_line = Some(head.line);
            }
            tab.selection = if extend {
                DocumentSelection {
                    anchor: tab.selection.anchor,
                    head,
                }
            } else {
                DocumentSelection::collapsed(head)
            };
            return scroll_by(
                EDITOR_SCROLLABLE_ID,
                AbsoluteOffset {
                    x: 0.0,
                    y: -(viewport_height * 0.85),
                },
                Animation::Smooth,
            );
        }
        EditorMessage::PageDown { extend } => {
            let page_lines = ((viewport_height / row_height).floor() as usize).max(5);
            let max_line = tab.rope.len_lines().saturating_sub(1);
            let mut head = tab.selection.head;
            head.line = (head.line + page_lines).min(max_line);
            head.col = head.col.min(visible_line_len(tab, head.line));
            if tab.active_line.is_some() {
                tab.active_line = Some(head.line);
            }
            tab.selection = if extend {
                DocumentSelection {
                    anchor: tab.selection.anchor,
                    head,
                }
            } else {
                DocumentSelection::collapsed(head)
            };
            return scroll_by(
                EDITOR_SCROLLABLE_ID,
                AbsoluteOffset {
                    x: 0.0,
                    y: viewport_height * 0.85,
                },
                Animation::Smooth,
            );
        }
        EditorMessage::Undo => {
            tab.undo();
        }
        EditorMessage::Redo => {
            tab.redo();
        }
        EditorMessage::CopySelection => {
            let text = selected_or_line_text(tab);
            if !text.is_empty() {
                return iced::clipboard::write(text).discard();
            }
        }
        EditorMessage::CutSelection => {
            let (start, end) = if !tab.selection.is_empty() {
                tab.selection.ordered()
            } else {
                let line = tab.selection.head.line;
                (Cursor::new(line, 0), Cursor::new(line, visible_line_len(tab, line)))
            };
            let start_byte = tab.line_col_to_byte(start);
            let end_byte = tab.line_col_to_byte(end);
            let cut = tab
                .rope
                .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                .to_string();
            if !cut.is_empty() {
                delete_range(tab, start, end, false);
                return iced::clipboard::write(cut).discard();
            }
        }
        EditorMessage::SelectAll => {
            let last_line = tab.rope.len_lines().saturating_sub(1);
            let last_col = visible_line_len(tab, last_line);
            tab.active_line = Some(tab.selection.head.line);
            tab.selection = DocumentSelection {
                anchor: Cursor::new(0, 0),
                head: Cursor::new(last_line, last_col),
            };
        }
        EditorMessage::DuplicateSelectionOrLine => {
            duplicate_selection_or_line(tab);
        }
        EditorMessage::MoveLinesUp => {
            move_selected_lines(tab, -1);
        }
        EditorMessage::MoveLinesDown => {
            move_selected_lines(tab, 1);
        }
        EditorMessage::ToggleActiveTask => {
            let line = tab
                .active_line
                .unwrap_or(tab.selection.head.line)
                .min(tab.rope.len_lines().saturating_sub(1));
            toggle_task_on_line(tab, line);
        }
        EditorMessage::ClipboardPasted(payload) => match payload {
            ClipboardPayload::Image {
                rgba,
                width,
                height,
            } => {
                if let Ok(md_link) = save_pasted_rgba_image(tab, &rgba, width, height) {
                    insert_markdown_image_link(tab, &md_link);
                }
            }
            ClipboardPayload::FilePaths(paths) => {
                let mut links = Vec::new();
                for path in paths {
                    if is_image_file_path(&path)
                        && let Ok(md_link) = import_or_link_image_file(tab, &path)
                    {
                        links.push(md_link);
                    }
                }
                if !links.is_empty() {
                    insert_markdown_image_link(tab, &links.join("\n"));
                }
            }
            ClipboardPayload::Text(raw_txt) => {
                if !raw_txt.is_empty() {
                    let normalized = raw_txt.replace("\r\n", "\n");
                    let trimmed = normalized.trim();
                    let is_url =
                        trimmed.starts_with("http://") || trimmed.starts_with("https://");
                    if !tab.selection.is_empty() && is_url && !trimmed.contains('\n') {
                        let (start, end) = tab.selection.ordered();
                        let start_byte = tab.line_col_to_byte(start);
                        let end_byte = tab.line_col_to_byte(end);
                        let deleted = tab
                            .rope
                            .slice(
                                tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte),
                            )
                            .to_string();
                        let inserted = format!("[{deleted}]({trimmed})");
                        let after = advance_cursor(start, &inserted);
                        let tx = Transaction {
                            edits: vec![Edit {
                                start_byte,
                                end_byte,
                                deleted,
                                inserted,
                            }],
                            selection_before: tab.selection,
                            selection_after: DocumentSelection::collapsed(after),
                            extra_cursors_before: tab.extra_cursors.clone(),
                            extra_cursors_after: tab.extra_cursors.clone(),
                            active_line_before: tab.active_line,
                            active_line_after: Some(after.line),
                            scroll_before: tab.scroll_offset,
                            timestamp_ms: now_ms(),
                            is_coalesceable: false,
                        };
                        tab.apply_transaction(tx);
                    } else {
                        insert_text_at_selection(tab, &normalized, false);
                    }
                }
            }
            ClipboardPayload::Empty | ClipboardPayload::Error(_) => {}
        },
        EditorMessage::Format(cmd) => {
            apply_format_command(tab, cmd, now_ms());
        }
        EditorMessage::SearchQueryChanged(q) => {
            search.query = q;
            recompute_search_matches(tab, search);
            select_active_search_match(tab, search);
        }
        EditorMessage::SearchReplaceChanged(r) => {
            search.replace = r;
        }
        EditorMessage::SearchToggleReplaceExpanded => {
            search.show_replace = !search.show_replace;
        }
        EditorMessage::SearchToggleCase => {
            search.case_sensitive = !search.case_sensitive;
            recompute_search_matches(tab, search);
            select_active_search_match(tab, search);
        }
        EditorMessage::SearchToggleWord => {
            search.whole_word = !search.whole_word;
            recompute_search_matches(tab, search);
            select_active_search_match(tab, search);
        }
        EditorMessage::SearchToggleRegex => {
            search.use_regex = !search.use_regex;
            recompute_search_matches(tab, search);
            select_active_search_match(tab, search);
        }
        EditorMessage::SearchPrev => {
            if !search.matches.is_empty() {
                let idx = search.active_match.unwrap_or(0);
                search.active_match =
                    Some(if idx == 0 { search.matches.len() - 1 } else { idx - 1 });
                select_active_search_match(tab, search);
            }
        }
        EditorMessage::SearchNext => {
            if !search.matches.is_empty() {
                let idx = search.active_match.unwrap_or(0);
                search.active_match = Some((idx + 1) % search.matches.len());
                select_active_search_match(tab, search);
            }
        }
        EditorMessage::ReplaceOne => {
            if let Some(idx) = search.active_match
                && let Some(m) = search.matches.get(idx).cloned()
            {
                let start = Cursor::new(m.line, m.char_col);
                let end = Cursor::new(m.line, m.char_col + m.char_len);
                let start_byte = tab.line_col_to_byte(start);
                let end_byte = tab.line_col_to_byte(end).max(start_byte);
                let deleted = tab
                    .rope
                    .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                    .to_string();
                let after_col = m.char_col + search.replace.chars().count();
                let tx = Transaction {
                    edits: vec![Edit {
                        start_byte,
                        end_byte,
                        deleted,
                        inserted: search.replace.clone(),
                    }],
                    selection_before: tab.selection,
                    selection_after: DocumentSelection::collapsed(Cursor::new(m.line, after_col)),
                    extra_cursors_before: tab.extra_cursors.clone(),
                    extra_cursors_after: Vec::new(),
                    active_line_before: tab.active_line,
                    active_line_after: Some(m.line),
                    scroll_before: tab.scroll_offset,
                    timestamp_ms: now_ms(),
                    is_coalesceable: false,
                };
                tab.apply_transaction(tx);
                recompute_search_matches(tab, search);
                if !search.matches.is_empty() {
                    search.active_match = Some(idx.min(search.matches.len() - 1));
                    select_active_search_match(tab, search);
                }
            }
        }
        EditorMessage::ReplaceAll => {
            if !search.matches.is_empty() {
                let mut edits = Vec::with_capacity(search.matches.len());
                for m in &search.matches {
                    let start = Cursor::new(m.line, m.char_col);
                    let end = Cursor::new(m.line, m.char_col + m.char_len);
                    let start_byte = tab.line_col_to_byte(start);
                    let end_byte = tab.line_col_to_byte(end).max(start_byte);
                    let deleted = tab
                        .rope
                        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                        .to_string();
                    edits.push(Edit {
                        start_byte,
                        end_byte,
                        deleted,
                        inserted: search.replace.clone(),
                    });
                }
                if !edits.is_empty() {
                    let tx = Transaction {
                        edits,
                        selection_before: tab.selection,
                        selection_after: tab.selection,
                        extra_cursors_before: tab.extra_cursors.clone(),
                        extra_cursors_after: tab.extra_cursors.clone(),
                        active_line_before: tab.active_line,
                        active_line_after: tab.active_line,
                        scroll_before: tab.scroll_offset,
                        timestamp_ms: now_ms(),
                        is_coalesceable: false,
                    };
                    tab.apply_transaction(tx);
                    recompute_search_matches(tab, search);
                }
            }
        }
        EditorMessage::CloseSearch => {
            search.is_open = false;
        }
        EditorMessage::KeepLocalChanges => {
            tab.conflict = super::models::ConflictState::None;
        }
        EditorMessage::ReloadFromDisk => {
            tab.conflict = super::models::ConflictState::None;
        }
        EditorMessage::ResetZoom => {
            tab.zoom = 1.0;
        }
        EditorMessage::ToggleLineEnding => {
            tab.line_ending = match tab.line_ending {
                LineEnding::Lf => LineEnding::Crlf,
                LineEnding::Crlf => LineEnding::Lf,
            };
            tab.is_dirty = true;
        }
        EditorMessage::PasteImageFromClipboard => {
            return iced::clipboard::read_image().map(|res| {
                EditorMessage::ImageBytesPasted(
                    res.ok()
                        .map(|img| (img.rgba.to_vec(), img.size.width, img.size.height)),
                )
            });
        }
        EditorMessage::ImageBytesPasted(Some((rgba, width, height))) => {
            if let Ok(md_link) = save_pasted_rgba_image(tab, &rgba, width, height) {
                insert_markdown_image_link(tab, &md_link);
            }
        }
        EditorMessage::ImageBytesPasted(None) => {}
        EditorMessage::FileDropped(path) => {
            if is_image_file_path(&path)
                && let Ok(md_link) = import_or_link_image_file(tab, &path)
            {
                insert_markdown_image_link(tab, &md_link);
            }
        }
        EditorMessage::OpenFileDialog => {
            let _ = rfd::AsyncFileDialog::new().add_filter("Markdown", &["md", "markdown"]);
        }
        EditorMessage::SaveAsDialog => {
            let _ = rfd::AsyncFileDialog::new().add_filter("Markdown", &["md", "markdown"]);
        }
        EditorMessage::CopyRichText => {
            let full = tab.rope.to_string();
            let _ = copy_rich_text(&full);
        }
        EditorMessage::CopyText(txt) => {
            return iced::clipboard::write(txt).discard();
        }
        EditorMessage::JumpToLine(line) => {
            let clamped = line.min(tab.rope.len_lines().saturating_sub(1));
            tab.active_line = Some(clamped);
            tab.selection = DocumentSelection::collapsed(Cursor::new(clamped, 0));
            return Task::none();
        }
        EditorMessage::ClickCanvasBottom => {
            let last_line = tab.rope.len_lines().saturating_sub(1);
            let line_len = visible_line_len(tab, last_line);
            tab.active_line = Some(last_line);
            tab.selection = DocumentSelection::collapsed(Cursor::new(last_line, line_len));
            return Task::none();
        }
        EditorMessage::OpenZoomViewer(_source) => {}
        EditorMessage::SelectTableCell {
            table_start,
            row_offset,
            col,
        } => {
            let target_line = (table_start + row_offset).min(tab.rope.len_lines().saturating_sub(1));
            tab.active_line = Some(target_line);
            tab.active_table_col = col;
            tab.selection = DocumentSelection::collapsed(Cursor::new(target_line, 0));
            return iced::widget::operation::focus(
                crate::core::editor::widgets::table_grid::ACTIVE_TABLE_CELL_ID,
            );
        }
        EditorMessage::TableEditCell {
            start_line,
            end_line,
            row_offset,
            col,
            value,
        } => {
            let safe_val = value.replace('\n', " ").replace('\r', "");
            apply_table_mutation(
                tab,
                start_line,
                end_line,
                |table| {
                    if row_offset == 0 {
                        if let Some(cell) = table.headers.get_mut(col) {
                            if *cell == safe_val {
                                return None;
                            }
                            *cell = safe_val;
                            return Some((start_line, col));
                        }
                    } else if row_offset >= 2
                        && let Some(row) = table.rows.get_mut(row_offset - 2)
                        && let Some(cell) = row.get_mut(col)
                    {
                        if *cell == safe_val {
                            return None;
                        }
                        *cell = safe_val;
                        return Some((start_line + row_offset, col));
                    }
                    None
                },
                true,
            );
        }
        EditorMessage::TableAction {
            start_line,
            end_line,
            op,
        } => {
            if op == TableOp::DeleteTable {
                delete_table_block(tab, start_line, end_line);
                return Task::none();
            }
            let cur_line = tab.active_line.unwrap_or(start_line);
            let cur_col = tab.active_table_col;
            apply_table_mutation(
                tab,
                start_line,
                end_line,
                |table| match op {
                    TableOp::InsertRowAbove(row_offset) => {
                        let data_idx = row_offset.saturating_sub(2);
                        table.insert_row(data_idx);
                        Some((start_line + 2 + data_idx, cur_col))
                    }
                    TableOp::InsertRowBelow(row_offset) => {
                        let insert_idx = if row_offset < 2 {
                            0
                        } else {
                            (row_offset - 2 + 1).min(table.rows.len())
                        };
                        table.insert_row(insert_idx);
                        Some((start_line + 2 + insert_idx, cur_col))
                    }
                    TableOp::DeleteRow(row_offset) => {
                        if row_offset >= 2 {
                            let data_idx = row_offset - 2;
                            if table.delete_row(data_idx) {
                                let new_line = if table.rows.is_empty() {
                                    start_line
                                } else {
                                    start_line + 2 + data_idx.min(table.rows.len() - 1)
                                };
                                Some((new_line, cur_col))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    TableOp::MoveRowUp(row_offset) => {
                        if row_offset >= 3 {
                            let data_idx = row_offset - 2;
                            if table.move_row(data_idx, MoveDirection::Backward) {
                                Some((start_line + row_offset - 1, cur_col))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    TableOp::MoveRowDown(row_offset) => {
                        if row_offset >= 2 {
                            let data_idx = row_offset - 2;
                            if table.move_row(data_idx, MoveDirection::Forward) {
                                Some((start_line + row_offset + 1, cur_col))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    TableOp::InsertColLeft(col) => {
                        let idx = col.min(table.headers.len());
                        table.insert_column(idx, "Column");
                        Some((cur_line, idx))
                    }
                    TableOp::InsertColRight(col) => {
                        let idx = (col + 1).min(table.headers.len());
                        table.insert_column(idx, "Column");
                        Some((cur_line, idx))
                    }
                    TableOp::DeleteCol(col) => {
                        if table.delete_column(col) {
                            let new_col = col.min(table.headers.len().saturating_sub(1));
                            Some((cur_line, new_col))
                        } else {
                            None
                        }
                    }
                    TableOp::MoveColLeft(col) => {
                        if table.move_column(col, MoveDirection::Backward) {
                            Some((cur_line, col.saturating_sub(1)))
                        } else {
                            None
                        }
                    }
                    TableOp::MoveColRight(col) => {
                        if table.move_column(col, MoveDirection::Forward) {
                            Some((cur_line, col + 1))
                        } else {
                            None
                        }
                    }
                    TableOp::SetColAlign(col, align) => {
                        if table.set_column_alignment(col, align) {
                            Some((cur_line, cur_col))
                        } else {
                            None
                        }
                    }
                    TableOp::FormatPipes | TableOp::DeleteTable => Some((cur_line, cur_col)),
                },
                false,
            );
        }
        EditorMessage::ExitTableEdit => {
            tab.active_line = None;
        }
        EditorMessage::FormatActiveTable => {
            if let Some(active) = tab.active_line {
                let cur_col = tab.active_table_col;
                let total = tab.rope.len_lines();
                let owned: Vec<String> = (0..total).map(|i| tab.rope.line(i).to_string()).collect();
                let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
                let scan = crate::core::markdown::block_scanner::scan_blocks(&refs);
                for block in scan.blocks {
                    if let crate::core::markdown::block_scanner::ScannedBlock::Table {
                        start_line,
                        end_line,
                        ..
                    } = block
                        && active >= start_line
                        && active <= end_line
                    {
                        apply_table_mutation(
                            tab,
                            start_line,
                            end_line,
                            |_| Some((active, cur_col)),
                            false,
                        );
                        break;
                    }
                }
            }
        }
        EditorMessage::TableAddRow(_start_line) | EditorMessage::TableAddCol(_start_line) => {}
        EditorMessage::ToggleTask(line_idx, checked) => {
            if line_idx < tab.rope.len_lines() {
                let line_str = tab.rope.line(line_idx).to_string();
                let raw_trimmed = line_str.trim_end_matches(['\r', '\n']);
                if parse_checklist_line(raw_trimmed).is_some()
                    && let Some(bracket_byte) = raw_trimmed.find('[')
                    && bracket_byte + 2 < raw_trimmed.len()
                    && raw_trimmed.as_bytes()[bracket_byte + 2] == b']'
                {
                    let char_col = raw_trimmed[..bracket_byte + 1].chars().count();
                    let start = Cursor::new(line_idx, char_col);
                    let end = Cursor::new(line_idx, char_col + 1);
                    let start_byte = tab.line_col_to_byte(start);
                    let end_byte = tab.line_col_to_byte(end);
                    let deleted = tab
                        .rope
                        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                        .to_string();
                    let inserted = if checked {
                        "x".to_string()
                    } else {
                        " ".to_string()
                    };
                    if deleted != inserted {
                        let tx = Transaction {
                            edits: vec![Edit {
                                start_byte,
                                end_byte,
                                deleted,
                                inserted,
                            }],
                            selection_before: tab.selection,
                            selection_after: tab.selection,
                            extra_cursors_before: tab.extra_cursors.clone(),
                            extra_cursors_after: tab.extra_cursors.clone(),
                            active_line_before: tab.active_line,
                            active_line_after: tab.active_line,
                            scroll_before: tab.scroll_offset,
                            timestamp_ms: now_ms(),
                            is_coalesceable: false,
                        };
                        tab.apply_transaction(tx);
                    }
                }
            }
        }
    }
    Task::none()
}

fn visible_line_len(tab: &Tab, line: usize) -> usize {
    let s = tab.rope.line(line).to_string();
    s.trim_end_matches(['\r', '\n']).chars().count()
}

fn delete_range(tab: &mut Tab, start: Cursor, end: Cursor, is_coalesceable: bool) {
    let start_byte = tab.line_col_to_byte(start);
    let end_byte = tab.line_col_to_byte(end);
    let deleted = tab
        .rope
        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
        .to_string();
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted,
            inserted: String::new(),
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection::collapsed(start),
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: tab.extra_cursors.clone(),
        active_line_before: tab.active_line,
        active_line_after: Some(start.line),
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable,
    };
    tab.apply_transaction(tx);
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn prev_word_col(line_str: &str, col: usize) -> usize {
    let chars: Vec<char> = line_str.trim_end_matches(['\r', '\n']).chars().collect();
    let mut i = col.min(chars.len());
    if i == 0 {
        return 0;
    }
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    if i == 0 {
        return 0;
    }
    if is_word_char(chars[i - 1]) {
        while i > 0 && is_word_char(chars[i - 1]) {
            i -= 1;
        }
    } else {
        while i > 0 && !chars[i - 1].is_whitespace() && !is_word_char(chars[i - 1]) {
            i -= 1;
        }
    }
    i
}

fn next_word_col(line_str: &str, col: usize) -> usize {
    let chars: Vec<char> = line_str.trim_end_matches(['\r', '\n']).chars().collect();
    let len = chars.len();
    let mut i = col.min(len);
    if i >= len {
        return len;
    }
    if chars[i].is_whitespace() {
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
    } else if is_word_char(chars[i]) {
        while i < len && is_word_char(chars[i]) {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
    } else {
        while i < len && !chars[i].is_whitespace() && !is_word_char(chars[i]) {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
    }
    i
}

fn update_selection(tab: &mut Tab, head: Cursor, extend: bool) {
    tab.selection = if extend {
        DocumentSelection {
            anchor: tab.selection.anchor,
            head,
        }
    } else {
        DocumentSelection::collapsed(head)
    };
}

fn handle_key_command(tab: &mut Tab, cmd: EditorKeyCommand) {
    let mut head = tab.selection.head;
    match cmd {
        EditorKeyCommand::Left { extend } => {
            if !extend && !tab.selection.is_empty() {
                let (start, _) = tab.selection.ordered();
                head = start;
                tab.active_line = Some(head.line);
            } else if head.col > 0 {
                head.col -= 1;
            } else if head.line > 0 {
                head.line -= 1;
                head.col = visible_line_len(tab, head.line);
                tab.active_line = Some(head.line);
            }
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::Right { extend } => {
            if !extend && !tab.selection.is_empty() {
                let (_, end) = tab.selection.ordered();
                head = end;
                tab.active_line = Some(head.line);
            } else {
                let line_len = visible_line_len(tab, head.line);
                if head.col < line_len {
                    head.col += 1;
                } else if head.line + 1 < tab.rope.len_lines() {
                    head.line += 1;
                    head.col = 0;
                    tab.active_line = Some(head.line);
                }
            }
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::WordLeft { extend } => {
            if head.col > 0 {
                let line_str = tab.rope.line(head.line).to_string();
                head.col = prev_word_col(&line_str, head.col);
            } else if head.line > 0 {
                head.line -= 1;
                head.col = visible_line_len(tab, head.line);
                tab.active_line = Some(head.line);
            }
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::WordRight { extend } => {
            let line_len = visible_line_len(tab, head.line);
            if head.col < line_len {
                let line_str = tab.rope.line(head.line).to_string();
                head.col = next_word_col(&line_str, head.col);
            } else if head.line + 1 < tab.rope.len_lines() {
                head.line += 1;
                head.col = 0;
                tab.active_line = Some(head.line);
            }
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::Up { extend } if head.line > 0 => {
            head.line -= 1;
            head.col = head.col.min(visible_line_len(tab, head.line));
            tab.active_line = Some(head.line);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::Down { extend } if head.line + 1 < tab.rope.len_lines() => {
            head.line += 1;
            head.col = head.col.min(visible_line_len(tab, head.line));
            tab.active_line = Some(head.line);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::Home { extend } => {
            let line_str = tab.rope.line(head.line).to_string();
            head.col = soft_home_col(&line_str, head.col);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::End { extend } => {
            head.col = visible_line_len(tab, head.line);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::DocHome { extend } => {
            head = Cursor::new(0, 0);
            tab.active_line = Some(0);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::DocEnd { extend } => {
            let last_line = tab.rope.len_lines().saturating_sub(1);
            head = Cursor::new(last_line, visible_line_len(tab, last_line));
            tab.active_line = Some(last_line);
            update_selection(tab, head, extend);
        }
        EditorKeyCommand::DeleteWordBackward => {
            if !tab.selection.is_empty() {
                let (start, end) = tab.selection.ordered();
                delete_range(tab, start, end, false);
            } else if head.col > 0 {
                let line_str = tab.rope.line(head.line).to_string();
                let start_col = prev_word_col(&line_str, head.col);
                delete_range(tab, Cursor::new(head.line, start_col), head, false);
            } else if head.line > 0 {
                let prev_line = head.line - 1;
                let prev_len = visible_line_len(tab, prev_line);
                delete_range(tab, Cursor::new(prev_line, prev_len), head, false);
            }
        }
        EditorKeyCommand::DeleteWordForward => {
            if !tab.selection.is_empty() {
                let (start, end) = tab.selection.ordered();
                delete_range(tab, start, end, false);
            } else {
                let line_len = visible_line_len(tab, head.line);
                if head.col < line_len {
                    let line_str = tab.rope.line(head.line).to_string();
                    let end_col = next_word_col(&line_str, head.col);
                    delete_range(tab, head, Cursor::new(head.line, end_col), false);
                } else if head.line + 1 < tab.rope.len_lines() {
                    delete_range(tab, head, Cursor::new(head.line + 1, 0), false);
                }
            }
        }
        EditorKeyCommand::Backspace => {
            if !tab.selection.is_empty() {
                let (start, end) = tab.selection.ordered();
                delete_range(tab, start, end, false);
            } else if head.col > 0 {
                let start = Cursor::new(head.line, head.col - 1);
                delete_range(tab, start, head, true);
            } else if head.line > 0 {
                let prev_line = head.line - 1;
                let prev_len = visible_line_len(tab, prev_line);
                let start = Cursor::new(prev_line, prev_len);
                let end = Cursor::new(head.line, 0);
                delete_range(tab, start, end, false);
            }
        }
        EditorKeyCommand::Delete => {
            if !tab.selection.is_empty() {
                let (start, end) = tab.selection.ordered();
                delete_range(tab, start, end, false);
            } else {
                let line_len = visible_line_len(tab, head.line);
                if head.col < line_len {
                    let end = Cursor::new(head.line, head.col + 1);
                    delete_range(tab, head, end, false);
                } else if head.line + 1 < tab.rope.len_lines() {
                    let start = Cursor::new(head.line, line_len);
                    let end = Cursor::new(head.line + 1, 0);
                    delete_range(tab, start, end, false);
                }
            }
        }
        EditorKeyCommand::Tab => {
            let (start, end) = tab.selection.ordered();
            let start_byte = tab.line_col_to_byte(start);
            let end_byte = tab.line_col_to_byte(end);
            let deleted = tab
                .rope
                .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                .to_string();
            let after = Cursor::new(start.line, start.col + 4);
            let tx = Transaction {
                edits: vec![Edit {
                    start_byte,
                    end_byte,
                    deleted,
                    inserted: "    ".to_string(),
                }],
                selection_before: tab.selection,
                selection_after: DocumentSelection::collapsed(after),
                extra_cursors_before: tab.extra_cursors.clone(),
                extra_cursors_after: tab.extra_cursors.clone(),
                active_line_before: tab.active_line,
                active_line_after: Some(after.line),
                scroll_before: tab.scroll_offset,
                timestamp_ms: now_ms(),
                is_coalesceable: false,
            };
            tab.apply_transaction(tx);
        }
        EditorKeyCommand::ShiftTab => {
            let line_str = tab.rope.line(head.line).to_string();
            let leading_spaces = line_str.chars().take(4).take_while(|&c| c == ' ').count();
            if leading_spaces > 0 {
                let start = Cursor::new(head.line, 0);
                let end = Cursor::new(head.line, leading_spaces);
                let after = Cursor::new(head.line, head.col.saturating_sub(leading_spaces));
                let start_byte = tab.line_col_to_byte(start);
                let end_byte = tab.line_col_to_byte(end);
                let deleted = tab
                    .rope
                    .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                    .to_string();
                let tx = Transaction {
                    edits: vec![Edit {
                        start_byte,
                        end_byte,
                        deleted,
                        inserted: String::new(),
                    }],
                    selection_before: tab.selection,
                    selection_after: DocumentSelection::collapsed(after),
                    extra_cursors_before: tab.extra_cursors.clone(),
                    extra_cursors_after: tab.extra_cursors.clone(),
                    active_line_before: tab.active_line,
                    active_line_after: Some(after.line),
                    scroll_before: tab.scroll_offset,
                    timestamp_ms: now_ms(),
                    is_coalesceable: false,
                };
                tab.apply_transaction(tx);
            }
        }
        EditorKeyCommand::Enter => {
            let line_str = tab.rope.line(head.line).to_string();
            let ins = smart_enter_insertion(&line_str);
            let byte_pos = tab.line_col_to_byte(head);
            let after = Cursor::new(head.line + 1, ins.chars().count().saturating_sub(1));
            let tx = Transaction {
                edits: vec![Edit {
                    start_byte: byte_pos,
                    end_byte: byte_pos,
                    deleted: String::new(),
                    inserted: ins,
                }],
                selection_before: tab.selection,
                selection_after: DocumentSelection::collapsed(after),
                extra_cursors_before: tab.extra_cursors.clone(),
                extra_cursors_after: tab.extra_cursors.clone(),
                active_line_before: tab.active_line,
                active_line_after: Some(after.line),
                scroll_before: tab.scroll_offset,
                timestamp_ms: now_ms(),
                is_coalesceable: false,
            };
            tab.apply_transaction(tx);
            let lines: Vec<String> = (0..tab.rope.len_lines())
                .map(|i| tab.rope.line(i).to_string())
                .collect();
            let _ = renumber_ordered_list(&lines, true);
        }
        _ => {}
    }
}

fn auto_scroll_task(
    tab: &Tab,
    line: usize,
    row_height: f32,
    viewport_height: f32,
    typewriter_mode: bool,
) -> Task<EditorMessage> {
    let line_y = line as f32 * row_height;
    let target_y = if typewriter_mode {
        (line_y - viewport_height * 0.5).max(0.0)
    } else if line_y < tab.scroll_offset + row_height {
        (line_y - row_height).max(0.0)
    } else if line_y + row_height * 2.0 > tab.scroll_offset + viewport_height {
        (line_y + row_height * 2.0 - viewport_height).max(0.0)
    } else {
        return Task::none();
    };

    scroll_to(
        EDITOR_SCROLLABLE_ID,
        AbsoluteOffset {
            x: 0.0,
            y: target_y,
        },
        Animation::Instant,
    )
}

pub fn is_image_file_path(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg"
    )
}

fn selected_or_line_text(tab: &Tab) -> String {
    let (start, end) = if !tab.selection.is_empty() {
        tab.selection.ordered()
    } else {
        let line = tab.selection.head.line;
        (
            Cursor::new(line, 0),
            Cursor::new(line, visible_line_len(tab, line)),
        )
    };
    let start_byte = tab.line_col_to_byte(start);
    let end_byte = tab.line_col_to_byte(end);
    tab.rope
        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
        .to_string()
}

fn advance_cursor(start: Cursor, inserted: &str) -> Cursor {
    if let Some(last_nl) = inserted.rfind('\n') {
        let added_lines = inserted.chars().filter(|&c| c == '\n').count();
        let last_col = inserted[last_nl + 1..].chars().count();
        Cursor::new(start.line + added_lines, last_col)
    } else {
        Cursor::new(start.line, start.col + inserted.chars().count())
    }
}

fn insert_text_at_selection(tab: &mut Tab, inserted: &str, keep_inactive: bool) {
    let (start, end) = tab.selection.ordered();
    let start_byte = tab.line_col_to_byte(start);
    let end_byte = tab.line_col_to_byte(end);
    let deleted = tab
        .rope
        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
        .to_string();
    let after = advance_cursor(start, inserted);
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted,
            inserted: inserted.to_string(),
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection::collapsed(after),
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: tab.extra_cursors.clone(),
        active_line_before: tab.active_line,
        active_line_after: if keep_inactive {
            None
        } else {
            Some(after.line)
        },
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn insert_markdown_image_link(tab: &mut Tab, md_link: &str) {
    let head = tab.selection.head;
    let cur_line = if head.line < tab.rope.len_lines() {
        tab.rope
            .line(head.line)
            .to_string()
            .trim_end_matches(['\r', '\n'])
            .to_string()
    } else {
        String::new()
    };
    let prefix = if cur_line.trim().is_empty() {
        ""
    } else {
        "\n"
    };
    let payload = format!("{prefix}{md_link}\n");
    insert_text_at_selection(tab, &payload, true);
}

fn timestamp_slug() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}

fn unique_dest_path(dir: &std::path::Path, base: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{base}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 1..10_000 {
        let candidate = dir.join(format!("{base}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{base}-{}.{ext}", now_ms()))
}

fn format_md_image_target(path_str: &str) -> String {
    let slash = path_str.replace('\\', "/");
    if slash.contains(' ') {
        format!("![](<{slash}>)")
    } else {
        format!("![]({slash})")
    }
}

fn save_pasted_rgba_image(
    tab: &Tab,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<String, String> {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "Invalid RGBA image dimensions".to_string())?;
    let stamp = timestamp_slug();

    if let Some(doc_path) = &tab.path
        && let Some(doc_dir) = doc_path.parent()
    {
        let assets_dir = doc_dir.join("assets");
        std::fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
        let stem = doc_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");
        let dest = unique_dest_path(&assets_dir, &format!("{stem}-{stamp}"), "png");
        img.save_with_format(&dest, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let file_name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("pasted.png");
        return Ok(format_md_image_target(&format!("assets/{file_name}")));
    }

    let assets_dir = crate::core::config::storage::app_data_dir().join("assets");
    std::fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    let dest = unique_dest_path(&assets_dir, &format!("pasted-{stamp}"), "png");
    img.save_with_format(&dest, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(format_md_image_target(&dest.to_string_lossy()))
}

fn import_or_link_image_file(tab: &Tab, src: &std::path::Path) -> Result<String, String> {
    if let Some(doc_path) = &tab.path
        && let Some(doc_dir) = doc_path.parent()
    {
        if let Ok(rel) = src.strip_prefix(doc_dir) {
            return Ok(format_md_image_target(&rel.to_string_lossy()));
        }
        let assets_dir = doc_dir.join("assets");
        std::fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let dest = unique_dest_path(&assets_dir, stem, ext);
        std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
        let file_name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.png");
        return Ok(format_md_image_target(&format!("assets/{file_name}")));
    }

    Ok(format_md_image_target(&src.to_string_lossy()))
}

fn recompute_search_matches(tab: &Tab, search: &mut SearchState) {
    search.matches.clear();
    if search.query.is_empty() {
        search.active_match = None;
        return;
    }

    let needle_chars: Vec<char> = if search.case_sensitive {
        search.query.chars().collect()
    } else {
        search.query.to_lowercase().chars().collect()
    };
    let needle_len = needle_chars.len();
    if needle_len == 0 {
        search.active_match = None;
        return;
    }

    for line_idx in 0..tab.rope.len_lines() {
        let raw_line = tab.rope.line(line_idx).to_string();
        let trimmed = raw_line.trim_end_matches(['\r', '\n']);
        let hay_orig: Vec<char> = trimmed.chars().collect();
        if hay_orig.len() < needle_len {
            continue;
        }
        let hay_cmp: Vec<char> = if search.case_sensitive {
            hay_orig.clone()
        } else {
            hay_orig
                .iter()
                .map(|c| c.to_lowercase().next().unwrap_or(*c))
                .collect()
        };

        let mut col = 0usize;
        while col + needle_len <= hay_cmp.len() {
            if hay_cmp[col..col + needle_len] == needle_chars[..] {
                let word_ok = if search.whole_word {
                    let before_ok = col == 0 || !is_word_char(hay_orig[col - 1]);
                    let after_ok =
                        col + needle_len == hay_orig.len() || !is_word_char(hay_orig[col + needle_len]);
                    before_ok && after_ok
                } else {
                    true
                };
                if word_ok {
                    search.matches.push(SearchMatch {
                        line: line_idx,
                        char_col: col,
                        char_len: needle_len,
                    });
                    col += needle_len;
                    continue;
                }
            }
            col += 1;
        }
    }

    if search.matches.is_empty() {
        search.active_match = None;
    } else {
        let cur_line = tab.selection.head.line;
        let first_at_or_after = search
            .matches
            .iter()
            .position(|m| m.line >= cur_line)
            .unwrap_or(0);
        search.active_match = Some(first_at_or_after);
    }
}

fn select_active_search_match(tab: &mut Tab, search: &SearchState) {
    let Some(idx) = search.active_match else {
        return;
    };
    let Some(m) = search.matches.get(idx) else {
        return;
    };
    tab.active_line = Some(m.line);
    tab.selection = DocumentSelection {
        anchor: Cursor::new(m.line, m.char_col),
        head: Cursor::new(m.line, m.char_col + m.char_len),
    };
}

fn duplicate_selection_or_line(tab: &mut Tab) {
    let total_lines = tab.rope.len_lines();
    if total_lines == 0 {
        return;
    }

    if !tab.selection.is_empty() {
        let (start, end) = tab.selection.ordered();
        if start.line == end.line {
            let start_byte = tab.line_col_to_byte(start);
            let end_byte = tab.line_col_to_byte(end).max(start_byte);
            let selected = tab
                .rope
                .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
                .to_string();
            let added_chars = selected.chars().count();
            let after_sel = DocumentSelection {
                anchor: end,
                head: Cursor::new(end.line, end.col + added_chars),
            };
            let tx = Transaction {
                edits: vec![Edit {
                    start_byte: end_byte,
                    end_byte,
                    deleted: String::new(),
                    inserted: selected,
                }],
                selection_before: tab.selection,
                selection_after: after_sel,
                extra_cursors_before: tab.extra_cursors.clone(),
                extra_cursors_after: Vec::new(),
                active_line_before: tab.active_line,
                active_line_after: Some(end.line),
                scroll_before: tab.scroll_offset,
                timestamp_ms: now_ms(),
                is_coalesceable: false,
            };
            tab.apply_transaction(tx);
            return;
        }

        // Multi-line selection: duplicate lines start.line..=end.line directly below end.line
        let last_idx = end.line.min(total_lines - 1);
        let first_idx = start.line.min(last_idx);
        let line_count = last_idx - first_idx + 1;
        let block_lines: Vec<String> = (first_idx..=last_idx)
            .map(|i| {
                tab.rope
                    .line(i)
                    .to_string()
                    .trim_end_matches(['\r', '\n'])
                    .to_string()
            })
            .collect();
        let insert_at_col = visible_line_len(tab, last_idx);
        let insert_byte = tab.line_col_to_byte(Cursor::new(last_idx, insert_at_col));
        let inserted = format!("\n{}", block_lines.join("\n"));
        let new_start = Cursor::new(first_idx + line_count, start.col);
        let new_end = Cursor::new(last_idx + line_count, end.col);
        let tx = Transaction {
            edits: vec![Edit {
                start_byte: insert_byte,
                end_byte: insert_byte,
                deleted: String::new(),
                inserted,
            }],
            selection_before: tab.selection,
            selection_after: DocumentSelection {
                anchor: new_start,
                head: new_end,
            },
            extra_cursors_before: tab.extra_cursors.clone(),
            extra_cursors_after: Vec::new(),
            active_line_before: tab.active_line,
            active_line_after: Some(new_end.line),
            scroll_before: tab.scroll_offset,
            timestamp_ms: now_ms(),
            is_coalesceable: false,
        };
        tab.apply_transaction(tx);
        return;
    }

    // Collapsed cursor: duplicate active line directly below and move cursor to the copy
    let line = tab
        .active_line
        .unwrap_or(tab.selection.head.line)
        .min(total_lines - 1);
    let col = tab.selection.head.col;
    let raw_line = tab.rope.line(line).to_string();
    let trimmed = raw_line.trim_end_matches(['\r', '\n']);
    let line_len = trimmed.chars().count();
    let insert_byte = tab.line_col_to_byte(Cursor::new(line, line_len));
    let inserted = format!("\n{trimmed}");
    let after = Cursor::new(line + 1, col.min(line_len));
    let tx = Transaction {
        edits: vec![Edit {
            start_byte: insert_byte,
            end_byte: insert_byte,
            deleted: String::new(),
            inserted,
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection::collapsed(after),
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: Vec::new(),
        active_line_before: tab.active_line,
        active_line_after: Some(after.line),
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn move_selected_lines(tab: &mut Tab, delta: isize) {
    let total_lines = tab.rope.len_lines();
    if total_lines < 2 {
        return;
    }
    let (start, end) = tab.selection.ordered();
    let first = start.line.min(total_lines - 1);
    let last = end.line.min(total_lines - 1);

    let (swap_start, swap_end, new_first, new_last) = if delta < 0 {
        if first == 0 {
            return;
        }
        (first - 1, last, first - 1, last - 1)
    } else {
        if last + 1 >= total_lines {
            return;
        }
        (first, last + 1, first + 1, last + 1)
    };

    let mut lines: Vec<String> = (swap_start..=swap_end)
        .map(|i| {
            tab.rope
                .line(i)
                .to_string()
                .trim_end_matches(['\r', '\n'])
                .to_string()
        })
        .collect();
    let had_trailing_nl = tab.rope.line(swap_end).to_string().ends_with('\n');
    if delta < 0 {
        let moved_down = lines.remove(0);
        lines.push(moved_down);
    } else {
        let moved_up = lines.pop().unwrap_or_default();
        lines.insert(0, moved_up);
    }

    let mut inserted = lines.join("\n");
    if had_trailing_nl {
        inserted.push('\n');
    }

    let start_byte = tab.rope.line_to_byte(swap_start);
    let end_byte = if swap_end + 1 < total_lines {
        tab.rope.line_to_byte(swap_end + 1)
    } else {
        tab.rope.len_bytes()
    };
    let deleted = tab
        .rope
        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
        .to_string();

    let new_anchor = if tab.selection.anchor.line <= tab.selection.head.line {
        Cursor::new(new_first, tab.selection.anchor.col)
    } else {
        Cursor::new(new_last, tab.selection.anchor.col)
    };
    let new_head = if tab.selection.head.line >= tab.selection.anchor.line {
        Cursor::new(new_last, tab.selection.head.col)
    } else {
        Cursor::new(new_first, tab.selection.head.col)
    };
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted,
            inserted,
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection {
            anchor: new_anchor,
            head: new_head,
        },
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: Vec::new(),
        active_line_before: tab.active_line,
        active_line_after: Some(new_head.line),
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn toggle_task_on_line(tab: &mut Tab, line: usize) {
    if line >= tab.rope.len_lines() {
        return;
    }
    let raw = tab.rope.line(line).to_string();
    let trimmed = raw.trim_end_matches(['\r', '\n']);
    let has_nl = raw.ends_with('\n');
    let next_body = if let Some((indent, is_checked, prefix_chars)) = parse_checklist_line(trimmed) {
        let rest = trimmed
            .char_indices()
            .nth(prefix_chars)
            .map(|(b, _)| &trimmed[b..])
            .unwrap_or("");
        let spaces = " ".repeat(indent);
        let mark = if is_checked { " " } else { "x" };
        format!("{spaces}- [{mark}] {rest}")
    } else {
        let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
        let spaces = &trimmed[..indent];
        let rest = trimmed[indent..]
            .strip_prefix("- ")
            .or_else(|| trimmed[indent..].strip_prefix("* "))
            .unwrap_or(&trimmed[indent..]);
        format!("{spaces}- [ ] {rest}")
    };
    let mut inserted = next_body;
    if has_nl {
        inserted.push('\n');
    }
    let start_byte = tab.rope.line_to_byte(line);
    let end_byte = if line + 1 < tab.rope.len_lines() {
        tab.rope.line_to_byte(line + 1)
    } else {
        tab.rope.len_bytes()
    };
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted: raw,
            inserted,
        }],
        selection_before: tab.selection,
        selection_after: tab.selection,
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: tab.extra_cursors.clone(),
        active_line_before: tab.active_line,
        active_line_after: tab.active_line,
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn delete_table_block(tab: &mut Tab, start_line: usize, end_line: usize) {
    let total_lines = tab.rope.len_lines();
    if total_lines == 0 || start_line >= total_lines {
        return;
    }
    let clamped_end = end_line.min(total_lines - 1);
    let mut start_char = tab.rope.line_to_char(start_line);
    let end_char = if clamped_end + 1 < total_lines {
        tab.rope.line_to_char(clamped_end + 1)
    } else {
        tab.rope.len_chars()
    };
    // If deleting up to EOF and preceded by a newline, strip the preceding newline too.
    if end_char == tab.rope.len_chars()
        && start_char > 0
        && tab.rope.char(start_char - 1) == '\n'
    {
        start_char -= 1;
    }
    let start_byte = tab.rope.char_to_byte(start_char);
    let end_byte = tab.rope.char_to_byte(end_char);
    let deleted = tab.rope.slice(start_char..end_char).to_string();
    let remaining_lines = total_lines.saturating_sub(clamped_end - start_line + 1);
    let after_line = start_line.min(remaining_lines.saturating_sub(1));
    let after = Cursor::new(after_line, 0);
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted,
            inserted: String::new(),
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection::collapsed(after),
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: Vec::new(),
        active_line_before: tab.active_line,
        active_line_after: None,
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable: false,
    };
    tab.apply_transaction(tx);
}

fn apply_table_mutation(
    tab: &mut Tab,
    start_line: usize,
    end_line: usize,
    mutate: impl FnOnce(&mut ParsedTable) -> Option<(usize, usize)>,
    is_coalesceable: bool,
) {
    let total_lines = tab.rope.len_lines();
    if start_line >= total_lines || end_line >= total_lines || end_line < start_line {
        return;
    }

    let raw_lines_owned: Vec<String> = (start_line..=end_line)
        .map(|i| tab.rope.line(i).to_string())
        .collect();
    let raw_refs: Vec<&str> = raw_lines_owned.iter().map(|s| s.as_str()).collect();
    let Some(mut table) = parse_table(&raw_refs) else {
        return;
    };

    let Some((next_line, next_col)) = mutate(&mut table) else {
        return;
    };

    let formatted_lines = format_table_pipes(&table);
    let has_trailing_nl = raw_lines_owned
        .last()
        .is_some_and(|l| l.ends_with('\n'));
    let mut inserted = formatted_lines.join("\n");
    if has_trailing_nl {
        inserted.push('\n');
    }

    let start_byte = tab.rope.line_to_byte(start_line);
    let end_byte = if end_line + 1 < total_lines {
        tab.rope.line_to_byte(end_line + 1)
    } else {
        tab.rope.len_bytes()
    };
    let deleted = tab
        .rope
        .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
        .to_string();

    if deleted == inserted {
        return;
    }

    let clamped_line = next_line.min(start_line + formatted_lines.len().saturating_sub(1));
    let tx = Transaction {
        edits: vec![Edit {
            start_byte,
            end_byte,
            deleted,
            inserted,
        }],
        selection_before: tab.selection,
        selection_after: DocumentSelection::collapsed(Cursor::new(clamped_line, 0)),
        extra_cursors_before: tab.extra_cursors.clone(),
        extra_cursors_after: tab.extra_cursors.clone(),
        active_line_before: tab.active_line,
        active_line_after: Some(clamped_line),
        scroll_before: tab.scroll_offset,
        timestamp_ms: now_ms(),
        is_coalesceable,
    };
    tab.apply_transaction(tx);
    tab.active_table_col = next_col.min(table.headers.len().saturating_sub(1));
}
