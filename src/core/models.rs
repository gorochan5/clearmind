use std::path::PathBuf;

use iced::{keyboard, window, Point, Rectangle, Size};

use crate::core::config::{ClearMindConfig, ConfigMessage, ConfigUiState};
use crate::core::custom_commands::SlashMenuState;
use crate::core::editor::{ClosedTabStack, EditorMessage, SearchState, Tab};
use crate::core::navbar::{NavbarMessage, NavbarState};
use crate::core::print::{PrintMessage, PrintPreviewState};
use crate::core::themes::ThemePalette;
use crate::core::widgets::{ConfirmPrompt, ContextMenuItem, QuickPickItem, ToastItem};

pub struct ClearMind {
    pub config: ClearMindConfig,
    pub config_ui: ConfigUiState,
    pub navbar: NavbarState,
    pub tabs: Vec<Tab>,
    pub next_tab_num: usize,
    pub active_tab: usize,
    pub closed_tabs: ClosedTabStack,
    pub search: SearchState,
    pub window_id: Option<window::Id>,
    pub window_size: Size,
    pub is_maximized: bool,
    pub toasts: Vec<ToastItem>,
    pub confirm_prompt: Option<ConfirmPrompt>,
    pub quick_pick: Option<Vec<QuickPickItem<Message>>>,
    pub context_menu: Option<(Point, Vec<ContextMenuItem<Message>>)>,
    pub zoom_image: Option<String>,
    pub show_settings: bool,
    pub print_preview: PrintPreviewState,
    pub show_stats_popover: bool,
    pub slash_popup: Option<SlashMenuState>,
    pub popover_anchor: Option<Rectangle>,
    pub palette: ThemePalette,
    pub next_toast_id: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navbar(NavbarMessage),
    Editor(EditorMessage),
    Config(ConfigMessage),
    Print(PrintMessage),
    WindowOpened(window::Id),
    WindowResized(Size),
    DragResize(window::Direction),
    WindowEvent(window::Event),
    KeyboardEvent(keyboard::Event),
    FileChanged(Vec<PathBuf>),
    FilesOpened(Vec<(PathBuf, Vec<u8>)>),
    FileSaved(Option<(PathBuf, Result<(), String>)>),
    PopoverAnchorFound(Option<Rectangle>),
    DismissToast(u64),
    CloseContextMenu,
    CloseQuickPick,
    CloseZoomViewer,
    CloseConfirmPrompt,
    CloseSettingsModal,
    SlashAccept(usize),
    Noop,
}
