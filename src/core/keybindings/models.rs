use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    File,
    Edit,
    Format,
    ViewAndNav,
}

impl CommandCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Format => "Format",
            Self::ViewAndNav => "View & Navigation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandId {
    FileNewTab,
    FileOpen,
    FileSave,
    FileSaveAs,
    FileCloseTab,
    FileReopenClosedTab,
    FileOpenSettings,
    EditUndo,
    EditRedo,
    EditCut,
    EditCopy,
    EditCopyRichText,
    EditPaste,
    EditSelectAll,
    EditDuplicate,
    EditMoveLineUp,
    EditMoveLineDown,
    EditAddCursorAbove,
    EditAddCursorBelow,
    EditFind,
    EditReplace,
    FormatBold,
    FormatItalic,
    FormatStrikethrough,
    FormatInlineCode,
    FormatLink,
    FormatCodeBlock,
    FormatHeading1,
    FormatHeading2,
    FormatHeading3,
    FormatHeading4,
    FormatHeading5,
    FormatHeading6,
    FormatParagraph,
    FormatBlockquote,
    FormatTaskToggle,
    FormatTableAlign,
    ViewToggleWordWrap,
    ViewToggleFullWidth,
    ViewToggleFocusMode,
    ViewToggleTypewriterMode,
    ViewZoomIn,
    ViewZoomOut,
    ViewZoomReset,
    NavNextTab,
    NavPrevTab,
    NavGoToHeading,
    ViewCommandPalette,
    SlashTrigger,
}

impl CommandId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileNewTab => "file.new_tab",
            Self::FileOpen => "file.open",
            Self::FileSave => "file.save",
            Self::FileSaveAs => "file.save_as",
            Self::FileCloseTab => "file.close_tab",
            Self::FileReopenClosedTab => "file.reopen_closed_tab",
            Self::FileOpenSettings => "file.open_settings",
            Self::EditUndo => "edit.undo",
            Self::EditRedo => "edit.redo",
            Self::EditCut => "edit.cut",
            Self::EditCopy => "edit.copy",
            Self::EditCopyRichText => "edit.copy_rich_text",
            Self::EditPaste => "edit.paste",
            Self::EditSelectAll => "edit.select_all",
            Self::EditDuplicate => "edit.duplicate",
            Self::EditMoveLineUp => "edit.move_line_up",
            Self::EditMoveLineDown => "edit.move_line_down",
            Self::EditAddCursorAbove => "edit.add_cursor_above",
            Self::EditAddCursorBelow => "edit.add_cursor_below",
            Self::EditFind => "edit.find",
            Self::EditReplace => "edit.replace",
            Self::FormatBold => "format.bold",
            Self::FormatItalic => "format.italic",
            Self::FormatStrikethrough => "format.strikethrough",
            Self::FormatInlineCode => "format.inline_code",
            Self::FormatLink => "format.link",
            Self::FormatCodeBlock => "format.code_block",
            Self::FormatHeading1 => "format.heading_1",
            Self::FormatHeading2 => "format.heading_2",
            Self::FormatHeading3 => "format.heading_3",
            Self::FormatHeading4 => "format.heading_4",
            Self::FormatHeading5 => "format.heading_5",
            Self::FormatHeading6 => "format.heading_6",
            Self::FormatParagraph => "format.paragraph",
            Self::FormatBlockquote => "format.blockquote",
            Self::FormatTaskToggle => "format.task_toggle",
            Self::FormatTableAlign => "format.table_align",
            Self::ViewToggleWordWrap => "view.toggle_word_wrap",
            Self::ViewToggleFullWidth => "view.toggle_full_width",
            Self::ViewToggleFocusMode => "view.toggle_focus_mode",
            Self::ViewToggleTypewriterMode => "view.toggle_typewriter_mode",
            Self::ViewZoomIn => "view.zoom_in",
            Self::ViewZoomOut => "view.zoom_out",
            Self::ViewZoomReset => "view.zoom_reset",
            Self::NavNextTab => "nav.next_tab",
            Self::NavPrevTab => "nav.prev_tab",
            Self::NavGoToHeading => "nav.go_to_heading",
            Self::ViewCommandPalette => "view.command_palette",
            Self::SlashTrigger => "slash.trigger",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FileNewTab => "New Tab",
            Self::FileOpen => "Open File",
            Self::FileSave => "Save",
            Self::FileSaveAs => "Save As",
            Self::FileCloseTab => "Close Tab",
            Self::FileReopenClosedTab => "Reopen Closed Tab",
            Self::FileOpenSettings => "Open Settings",
            Self::EditUndo => "Undo",
            Self::EditRedo => "Redo",
            Self::EditCut => "Cut",
            Self::EditCopy => "Copy",
            Self::EditCopyRichText => "Copy Rich Text",
            Self::EditPaste => "Paste",
            Self::EditSelectAll => "Select All",
            Self::EditDuplicate => "Duplicate Line",
            Self::EditMoveLineUp => "Move Line Up",
            Self::EditMoveLineDown => "Move Line Down",
            Self::EditAddCursorAbove => "Add Cursor Above",
            Self::EditAddCursorBelow => "Add Cursor Below",
            Self::EditFind => "Find",
            Self::EditReplace => "Find & Replace",
            Self::FormatBold => "Bold",
            Self::FormatItalic => "Italic",
            Self::FormatStrikethrough => "Strikethrough",
            Self::FormatInlineCode => "Inline Code",
            Self::FormatLink => "Insert Link",
            Self::FormatCodeBlock => "Code Block",
            Self::FormatHeading1 => "Heading 1",
            Self::FormatHeading2 => "Heading 2",
            Self::FormatHeading3 => "Heading 3",
            Self::FormatHeading4 => "Heading 4",
            Self::FormatHeading5 => "Heading 5",
            Self::FormatHeading6 => "Heading 6",
            Self::FormatParagraph => "Paragraph",
            Self::FormatBlockquote => "Blockquote",
            Self::FormatTaskToggle => "Toggle Task",
            Self::FormatTableAlign => "Format Table",
            Self::ViewToggleWordWrap => "Toggle Word Wrap",
            Self::ViewToggleFullWidth => "Toggle Full Width",
            Self::ViewToggleFocusMode => "Toggle Focus Mode",
            Self::ViewToggleTypewriterMode => "Toggle Typewriter Mode",
            Self::ViewZoomIn => "Zoom In",
            Self::ViewZoomOut => "Zoom Out",
            Self::ViewZoomReset => "Reset Zoom",
            Self::NavNextTab => "Next Tab",
            Self::NavPrevTab => "Previous Tab",
            Self::NavGoToHeading => "Go to Heading",
            Self::ViewCommandPalette => "Command Palette",
            Self::SlashTrigger => "Slash Commands",
        }
    }

    pub fn category(self) -> CommandCategory {
        match self {
            Self::FileNewTab
            | Self::FileOpen
            | Self::FileSave
            | Self::FileSaveAs
            | Self::FileCloseTab
            | Self::FileReopenClosedTab
            | Self::FileOpenSettings => CommandCategory::File,
            Self::EditUndo
            | Self::EditRedo
            | Self::EditCut
            | Self::EditCopy
            | Self::EditCopyRichText
            | Self::EditPaste
            | Self::EditSelectAll
            | Self::EditDuplicate
            | Self::EditMoveLineUp
            | Self::EditMoveLineDown
            | Self::EditAddCursorAbove
            | Self::EditAddCursorBelow
            | Self::EditFind
            | Self::EditReplace => CommandCategory::Edit,
            Self::FormatBold
            | Self::FormatItalic
            | Self::FormatStrikethrough
            | Self::FormatInlineCode
            | Self::FormatLink
            | Self::FormatCodeBlock
            | Self::FormatHeading1
            | Self::FormatHeading2
            | Self::FormatHeading3
            | Self::FormatHeading4
            | Self::FormatHeading5
            | Self::FormatHeading6
            | Self::FormatParagraph
            | Self::FormatBlockquote
            | Self::FormatTaskToggle
            | Self::FormatTableAlign => CommandCategory::Format,
            Self::ViewToggleWordWrap
            | Self::ViewToggleFullWidth
            | Self::ViewToggleFocusMode
            | Self::ViewToggleTypewriterMode
            | Self::ViewZoomIn
            | Self::ViewZoomOut
            | Self::ViewZoomReset
            | Self::NavNextTab
            | Self::NavPrevTab
            | Self::NavGoToHeading
            | Self::ViewCommandPalette
            | Self::SlashTrigger => CommandCategory::ViewAndNav,
        }
    }

    pub fn default_shortcut(self) -> &'static str {
        match self {
            Self::FileNewTab => "Ctrl+N",
            Self::FileOpen => "Ctrl+O",
            Self::FileSave => "Ctrl+S",
            Self::FileSaveAs => "Ctrl+Shift+S",
            Self::FileCloseTab => "Ctrl+W",
            Self::FileReopenClosedTab => "Ctrl+Shift+T",
            Self::FileOpenSettings => "Ctrl+,",
            Self::EditUndo => "Ctrl+Z",
            Self::EditRedo => "Ctrl+Y",
            Self::EditCut => "Ctrl+X",
            Self::EditCopy => "Ctrl+C",
            Self::EditCopyRichText => "Ctrl+Alt+Shift+C",
            Self::EditPaste => "Ctrl+V",
            Self::EditSelectAll => "Ctrl+A",
            Self::EditDuplicate => "Ctrl+D",
            Self::EditMoveLineUp => "Alt+Up",
            Self::EditMoveLineDown => "Alt+Down",
            Self::EditAddCursorAbove => "Ctrl+Alt+Up",
            Self::EditAddCursorBelow => "Ctrl+Alt+Down",
            Self::EditFind => "Ctrl+F",
            Self::EditReplace => "Ctrl+H",
            Self::FormatBold => "Ctrl+B",
            Self::FormatItalic => "Ctrl+I",
            Self::FormatStrikethrough => "Ctrl+Shift+X",
            Self::FormatInlineCode => "Ctrl+E",
            Self::FormatLink => "Ctrl+K",
            Self::FormatCodeBlock => "Ctrl+Shift+C",
            Self::FormatHeading1 => "Ctrl+1",
            Self::FormatHeading2 => "Ctrl+2",
            Self::FormatHeading3 => "Ctrl+3",
            Self::FormatHeading4 => "Ctrl+4",
            Self::FormatHeading5 => "Ctrl+5",
            Self::FormatHeading6 => "Ctrl+6",
            Self::FormatParagraph => "Ctrl+0",
            Self::FormatBlockquote => "Ctrl+Shift+.",
            Self::FormatTaskToggle => "Ctrl+L",
            Self::FormatTableAlign => "Ctrl+Alt+T",
            Self::ViewToggleWordWrap => "Alt+Z",
            Self::ViewToggleFullWidth => "Ctrl+Shift+M",
            Self::ViewToggleFocusMode => "Ctrl+Shift+F",
            Self::ViewToggleTypewriterMode => "",
            Self::ViewZoomIn => "Ctrl+=",
            Self::ViewZoomOut => "Ctrl+-",
            Self::ViewZoomReset => "",
            Self::NavNextTab => "Ctrl+Tab",
            Self::NavPrevTab => "Ctrl+Shift+Tab",
            Self::NavGoToHeading => "Ctrl+Shift+O",
            Self::ViewCommandPalette => "Ctrl+Shift+P",
            Self::SlashTrigger => "/",
        }
    }

    pub fn all() -> &'static [CommandId] {
        &[
            Self::FileNewTab,
            Self::FileOpen,
            Self::FileSave,
            Self::FileSaveAs,
            Self::FileCloseTab,
            Self::FileReopenClosedTab,
            Self::FileOpenSettings,
            Self::EditUndo,
            Self::EditRedo,
            Self::EditCut,
            Self::EditCopy,
            Self::EditCopyRichText,
            Self::EditPaste,
            Self::EditSelectAll,
            Self::EditDuplicate,
            Self::EditMoveLineUp,
            Self::EditMoveLineDown,
            Self::EditAddCursorAbove,
            Self::EditAddCursorBelow,
            Self::EditFind,
            Self::EditReplace,
            Self::FormatBold,
            Self::FormatItalic,
            Self::FormatStrikethrough,
            Self::FormatInlineCode,
            Self::FormatLink,
            Self::FormatCodeBlock,
            Self::FormatHeading1,
            Self::FormatHeading2,
            Self::FormatHeading3,
            Self::FormatHeading4,
            Self::FormatHeading5,
            Self::FormatHeading6,
            Self::FormatParagraph,
            Self::FormatBlockquote,
            Self::FormatTaskToggle,
            Self::FormatTableAlign,
            Self::ViewToggleWordWrap,
            Self::ViewToggleFullWidth,
            Self::ViewToggleFocusMode,
            Self::ViewToggleTypewriterMode,
            Self::ViewZoomIn,
            Self::ViewZoomOut,
            Self::ViewZoomReset,
            Self::NavNextTab,
            Self::NavPrevTab,
            Self::NavGoToHeading,
            Self::ViewCommandPalette,
            Self::SlashTrigger,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeybindingsConfig {
    pub overrides: BTreeMap<String, String>,
}

impl KeybindingsConfig {
    pub fn shortcut_for(&self, cmd: CommandId) -> &str {
        self.overrides
            .get(cmd.as_str())
            .map(String::as_str)
            .unwrap_or_else(|| cmd.default_shortcut())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeybindingsUiState {
    pub search_query: String,
    pub recording_command: Option<CommandId>,
    pub conflict_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindingsMessage {
    SearchChanged(String),
    StartRecording(CommandId),
    CancelRecording,
    CommitBinding { command: CommandId, shortcut: String },
    Unbind(CommandId),
    ResetOne(CommandId),
    ResetAll,
}
