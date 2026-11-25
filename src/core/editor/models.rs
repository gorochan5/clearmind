//! Document buffer, selection, undo/redo history, search, and closed-tab models.

use ropey::Rope;
use std::collections::VecDeque;
use std::path::PathBuf;

pub const MAX_UNDO_TRANSACTIONS: usize = 1_000;
pub const MAX_UNDO_BYTES: usize = 32 * 1024 * 1024;
pub const COALESCE_WINDOW_MS: u64 = 750;
pub const MAX_CLOSED_TABS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DocumentSelection {
    pub anchor: Cursor,
    pub head: Cursor,
}

impl DocumentSelection {
    pub const fn collapsed(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            head: cursor,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn ordered(&self) -> (Cursor, Cursor) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start_byte: usize,
    pub end_byte: usize,
    pub deleted: String,
    pub inserted: String,
}

impl Edit {
    pub fn byte_size(&self) -> usize {
        self.deleted.len() + self.inserted.len() + std::mem::size_of::<Self>()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub edits: Vec<Edit>,
    pub selection_before: DocumentSelection,
    pub selection_after: DocumentSelection,
    pub extra_cursors_before: Vec<Cursor>,
    pub extra_cursors_after: Vec<Cursor>,
    pub active_line_before: Option<usize>,
    pub active_line_after: Option<usize>,
    pub scroll_before: f32,
    pub timestamp_ms: u64,
    pub is_coalesceable: bool,
}

impl Transaction {
    pub fn byte_size(&self) -> usize {
        let edits_size: usize = self.edits.iter().map(Edit::byte_size).sum();
        let cursors_size = (self.extra_cursors_before.len() + self.extra_cursors_after.len())
            * std::mem::size_of::<Cursor>();
        edits_size + cursors_size + std::mem::size_of::<Self>()
    }

    fn can_coalesce_with(&self, next: &Self) -> bool {
        if !self.is_coalesceable || !next.is_coalesceable {
            return false;
        }
        if next.timestamp_ms.saturating_sub(self.timestamp_ms) > COALESCE_WINDOW_MS {
            return false;
        }
        if self.edits.len() != 1 || next.edits.len() != 1 {
            return false;
        }
        if self.selection_after.head.line != next.selection_before.head.line {
            return false;
        }
        let prev_edit = &self.edits[0];
        let next_edit = &next.edits[0];
        // Consecutive single-char insert at end of previous insert
        let is_contiguous_insert = prev_edit.deleted.is_empty()
            && next_edit.deleted.is_empty()
            && next_edit.inserted.chars().count() == 1
            && !next_edit.inserted.contains('\n')
            && next_edit.start_byte == prev_edit.start_byte + prev_edit.inserted.len();
        // Consecutive single-char backspace immediately before previous delete
        let is_contiguous_delete = prev_edit.inserted.is_empty()
            && next_edit.inserted.is_empty()
            && next_edit.deleted.chars().count() == 1
            && !next_edit.deleted.contains('\n')
            && next_edit.end_byte == prev_edit.start_byte;
        is_contiguous_insert || is_contiguous_delete
    }

    fn merge(&mut self, next: Self) {
        let prev_edit = &mut self.edits[0];
        let next_edit = &next.edits[0];
        if prev_edit.deleted.is_empty() && next_edit.deleted.is_empty() {
            prev_edit.inserted.push_str(&next_edit.inserted);
        } else if prev_edit.inserted.is_empty() && next_edit.inserted.is_empty() {
            let mut combined = next_edit.deleted.clone();
            combined.push_str(&prev_edit.deleted);
            prev_edit.deleted = combined;
            prev_edit.start_byte = next_edit.start_byte;
        }
        self.selection_after = next.selection_after;
        self.extra_cursors_after = next.extra_cursors_after;
        self.active_line_after = next.active_line_after;
        self.timestamp_ms = next.timestamp_ms;
    }
}

#[derive(Debug, Clone, Default)]
pub struct UndoHistory {
    undo_stack: VecDeque<Transaction>,
    redo_stack: Vec<Transaction>,
    total_bytes: usize,
}

impl UndoHistory {
    pub fn push(&mut self, tx: Transaction) {
        self.redo_stack.clear();
        if let Some(last) = self.undo_stack.back_mut()
            && last.can_coalesce_with(&tx)
        {
            let old_size = last.byte_size();
            last.merge(tx);
            let new_size = last.byte_size();
            self.total_bytes = self.total_bytes.saturating_sub(old_size) + new_size;
            self.enforce_caps();
            return;
        }
        self.total_bytes += tx.byte_size();
        self.undo_stack.push_back(tx);
        self.enforce_caps();
    }

    fn enforce_caps(&mut self) {
        while self.undo_stack.len() > MAX_UNDO_TRANSACTIONS || self.total_bytes > MAX_UNDO_BYTES {
            if let Some(oldest) = self.undo_stack.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(oldest.byte_size());
            } else {
                break;
            }
        }
    }

    pub fn pop_undo(&mut self) -> Option<Transaction> {
        let tx = self.undo_stack.pop_back()?;
        self.total_bytes = self.total_bytes.saturating_sub(tx.byte_size());
        Some(tx)
    }

    pub fn push_redo(&mut self, tx: Transaction) {
        self.redo_stack.push(tx);
    }

    pub fn pop_redo(&mut self) -> Option<Transaction> {
        self.redo_stack.pop()
    }

    pub fn push_undo_raw(&mut self, tx: Transaction) {
        self.total_bytes += tx.byte_size();
        self.undo_stack.push_back(tx);
        self.enforce_caps();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConflictState {
    #[default]
    None,
    ExternalModified {
        detected_at_ms: u64,
    },
    SaveFailed {
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub tab_uuid: String,
    pub title: String,
    pub path: Option<PathBuf>,
    pub rope: Rope,
    pub line_ending: LineEnding,
    pub has_bom: bool,
    pub is_dirty: bool,
    pub active_line: Option<usize>,
    pub active_table_col: usize,
    pub selection: DocumentSelection,
    pub extra_cursors: Vec<Cursor>,
    pub goal_x: Option<f32>,
    pub scroll_offset: f32,
    pub history: UndoHistory,
    pub conflict: ConflictState,
    pub is_deleted_on_disk: bool,
    pub raw_only_mode: bool,
    pub session_words_added: isize,
    pub initial_word_count: usize,
    pub zoom: f32,
}

impl Tab {
    pub fn new_untitled(tab_uuid: String, title: String) -> Self {
        Self {
            tab_uuid,
            title,
            path: None,
            rope: Rope::new(),
            line_ending: LineEnding::Lf,
            has_bom: false,
            is_dirty: false,
            active_line: None,
            active_table_col: 0,
            selection: DocumentSelection::default(),
            extra_cursors: Vec::new(),
            goal_x: None,
            scroll_offset: 0.0,
            history: UndoHistory::default(),
            conflict: ConflictState::None,
            is_deleted_on_disk: false,
            raw_only_mode: false,
            session_words_added: 0,
            initial_word_count: 0,
            zoom: 1.0,
        }
    }

    pub fn from_bytes(tab_uuid: String, title: String, path: Option<PathBuf>, bytes: &[u8]) -> Self {
        let (has_bom, slice) = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            (true, &bytes[3..])
        } else {
            (false, bytes)
        };
        let raw = String::from_utf8_lossy(slice);
        let line_ending = if raw.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        let normalized = raw.replace("\r\n", "\n");
        let rope = Rope::from_str(&normalized);
        let initial_word_count = normalized.split_whitespace().count();
        Self {
            tab_uuid,
            title,
            path,
            rope,
            line_ending,
            has_bom,
            is_dirty: false,
            active_line: None,
            active_table_col: 0,
            selection: DocumentSelection::default(),
            extra_cursors: Vec::new(),
            goal_x: None,
            scroll_offset: 0.0,
            history: UndoHistory::default(),
            conflict: ConflictState::None,
            is_deleted_on_disk: false,
            raw_only_mode: false,
            session_words_added: 0,
            initial_word_count,
            zoom: 1.0,
        }
    }

    pub fn to_disk_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if self.has_bom {
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        let text = self.rope.to_string();
        if self.line_ending == LineEnding::Crlf {
            out.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
        } else {
            out.extend_from_slice(text.as_bytes());
        }
        out
    }

    pub fn line_col_to_byte(&self, cursor: Cursor) -> usize {
        let line = cursor.line.min(self.rope.len_lines().saturating_sub(1));
        let line_start_char = self.rope.line_to_char(line);
        let line_slice = self.rope.line(line);
        let line_chars = line_slice.len_chars();
        let col = cursor.col.min(line_chars);
        self.rope.char_to_byte(line_start_char + col)
    }

    pub fn byte_to_line_col(&self, byte_idx: usize) -> Cursor {
        let clamped = byte_idx.min(self.rope.len_bytes());
        let char_idx = self.rope.byte_to_char(clamped);
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Cursor::new(line, char_idx.saturating_sub(line_start))
    }

    pub fn apply_transaction(&mut self, tx: Transaction) {
        for edit in tx.edits.iter().rev() {
            let start_char = self.rope.byte_to_char(edit.start_byte.min(self.rope.len_bytes()));
            let end_char = self.rope.byte_to_char(edit.end_byte.min(self.rope.len_bytes()));
            self.rope.remove(start_char..end_char);
            self.rope.insert(start_char, &edit.inserted);
        }
        self.selection = tx.selection_after;
        self.extra_cursors = tx.extra_cursors_after.clone();
        self.active_line = tx.active_line_after;
        self.is_dirty = true;
        self.history.push(tx);
    }

    pub fn undo(&mut self) -> bool {
        let Some(tx) = self.history.pop_undo() else {
            return false;
        };
        for edit in &tx.edits {
            let start_char = self.rope.byte_to_char(edit.start_byte.min(self.rope.len_bytes()));
            let end_byte = edit.start_byte + edit.inserted.len();
            let end_char = self.rope.byte_to_char(end_byte.min(self.rope.len_bytes()));
            self.rope.remove(start_char..end_char);
            self.rope.insert(start_char, &edit.deleted);
        }
        self.selection = tx.selection_before;
        self.extra_cursors = tx.extra_cursors_before.clone();
        self.active_line = tx.active_line_before;
        self.scroll_offset = tx.scroll_before;
        self.history.push_redo(tx);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(tx) = self.history.pop_redo() else {
            return false;
        };
        for edit in tx.edits.iter().rev() {
            let start_char = self.rope.byte_to_char(edit.start_byte.min(self.rope.len_bytes()));
            let end_char = self.rope.byte_to_char(edit.end_byte.min(self.rope.len_bytes()));
            self.rope.remove(start_char..end_char);
            self.rope.insert(start_char, &edit.inserted);
        }
        self.selection = tx.selection_after;
        self.extra_cursors = tx.extra_cursors_after.clone();
        self.active_line = tx.active_line_after;
        self.is_dirty = true;
        self.history.push_undo_raw(tx);
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub line: usize,
    pub char_col: usize,
    pub char_len: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub is_open: bool,
    pub show_replace: bool,
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub matches: Vec<SearchMatch>,
    pub active_match: Option<usize>,
    pub focus_query: bool,
    pub focus_replace: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosedTabEntry {
    pub path: Option<PathBuf>,
    pub title: String,
    pub active_line: Option<usize>,
    pub scroll_offset: f32,
}

#[derive(Debug, Clone, Default)]
pub struct ClosedTabStack {
    entries: VecDeque<ClosedTabEntry>,
}

impl ClosedTabStack {
    pub fn push(&mut self, entry: ClosedTabEntry) {
        if self.entries.len() >= MAX_CLOSED_TABS {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn pop(&mut self) -> Option<ClosedTabEntry> {
        self.entries.pop_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_insert_tx(pos: usize, ch: &str, col_after: usize, ts: u64) -> Transaction {
        Transaction {
            edits: vec![Edit {
                start_byte: pos,
                end_byte: pos,
                deleted: String::new(),
                inserted: ch.to_string(),
            }],
            selection_before: DocumentSelection::collapsed(Cursor::new(0, col_after - 1)),
            selection_after: DocumentSelection::collapsed(Cursor::new(0, col_after)),
            extra_cursors_before: Vec::new(),
            extra_cursors_after: Vec::new(),
            active_line_before: Some(0),
            active_line_after: Some(0),
            scroll_before: 0.0,
            timestamp_ms: ts,
            is_coalesceable: true,
        }
    }

    #[test]
    fn undo_redo_inverse_and_coalescing() {
        let mut tab = Tab::new_untitled("u1".into(), "Untitled.md".into());
        tab.apply_transaction(make_insert_tx(0, "a", 1, 100));
        tab.apply_transaction(make_insert_tx(1, "b", 2, 300));
        tab.apply_transaction(make_insert_tx(2, "c", 3, 500));
        assert_eq!(tab.rope.to_string(), "abc");
        // All 3 coalesced into 1 undo step
        assert!(tab.undo());
        assert_eq!(tab.rope.to_string(), "");
        assert!(!tab.undo());
        // Redo restores all 3
        assert!(tab.redo());
        assert_eq!(tab.rope.to_string(), "abc");
    }

    #[test]
    fn no_coalesce_after_750ms() {
        let mut tab = Tab::new_untitled("u2".into(), "Untitled.md".into());
        tab.apply_transaction(make_insert_tx(0, "a", 1, 100));
        tab.apply_transaction(make_insert_tx(1, "b", 2, 1000));
        assert_eq!(tab.rope.to_string(), "ab");
        assert!(tab.undo());
        assert_eq!(tab.rope.to_string(), "a");
        assert!(tab.undo());
        assert_eq!(tab.rope.to_string(), "");
    }
}
