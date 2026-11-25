//! Navbar state, tab strip items, quick-access popover state, and recent files.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAccessPopover {
    Fonts,
    Keybindings,
    CustomCommands,
    Themes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    pub uuid: String,
    pub title: String,
    pub is_dirty: bool,
    pub has_conflict: bool,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFileEntry {
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NavbarState {
    pub active_popover: Option<QuickAccessPopover>,
    pub recent_dropdown_open: bool,
    pub tab_context_menu: Option<String>,
    pub word_wrap_enabled: bool,
    pub full_width_enabled: bool,
    pub print_preview_open: bool,
    pub is_maximized: bool,
}
