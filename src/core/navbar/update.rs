//! Navbar update logic and window chrome actions.

use iced::Task;

use super::models::{NavbarState, QuickAccessPopover};

#[derive(Debug, Clone)]
pub enum NavbarMessage {
    SelectTab(String),
    CloseTab(String),
    OpenTabContextMenu(String),
    NewTab,
    OpenFile,
    OpenRecentFile(String),
    ClearRecentFiles,
    ToggleRecentDropdown,
    TogglePopover(QuickAccessPopover),
    ToggleWordWrap,
    ToggleFullWidth,
    OpenPrintPreview,
    OpenSettingsModal,
    DragWindow,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
}

pub fn update_navbar(state: &mut NavbarState, msg: NavbarMessage) -> Task<NavbarMessage> {
    match msg {
        NavbarMessage::TogglePopover(pop) => {
            if state.active_popover == Some(pop) {
                state.active_popover = None;
            } else {
                state.active_popover = Some(pop);
            }
        }
        NavbarMessage::ToggleRecentDropdown => {
            state.recent_dropdown_open = !state.recent_dropdown_open;
        }
        NavbarMessage::ToggleWordWrap => {
            state.word_wrap_enabled = !state.word_wrap_enabled;
        }
        NavbarMessage::ToggleFullWidth => {
            state.full_width_enabled = !state.full_width_enabled;
        }
        NavbarMessage::OpenTabContextMenu(uuid) => {
            state.tab_context_menu = Some(uuid);
        }
        NavbarMessage::ToggleMaximizeWindow => {
            state.is_maximized = !state.is_maximized;
        }
        _ => {}
    }
    Task::none()
}
