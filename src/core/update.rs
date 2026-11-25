use std::path::PathBuf;

use iced::{keyboard, window, Size, Subscription, Task};

use crate::core::config::{
    self, ClearMindConfig, ConfigMessage, ConfigUiState, LoadConfigResult,
};
use crate::core::custom_commands::{
    SlashMenuState,
    builtins::{detect_document_trigger_context, expand_suggestion, suggestions_for_context},
    models::TriggerContext,
    variables::VariableContext,
};
use crate::core::editor::{
    self, ClosedTabStack, EditorMessage, SearchState, Tab,
};
use crate::core::editor::live_widget::{EditorKeyCommand, EditorWidgetEvent};
use crate::core::editor::smart_edit::FormatCommand;
use crate::core::keybindings::engine::{
    EscapeTarget, KeyDispatch, KeyInput, RouterContext, route_key_event,
};
use crate::core::keybindings::models::CommandId;
use crate::core::models::{ClearMind, Message};
use crate::core::navbar::{self, NavbarMessage, NavbarState};
use crate::core::print::{self, PrintMessage, PrintPreviewState};
use crate::core::themes::resolve_palette;
use crate::core::watcher;
use crate::core::widgets::toast::{ToastItem, ToastKind};

impl ClearMind {
    pub fn boot() -> (Self, Task<Message>) {
        let config_path = config::config_file_path();
        let mut config = match config::load_config_from_path(&config_path) {
            LoadConfigResult::Loaded { config, .. } => config.clamped(),
            _ => ClearMindConfig::default().clamped(),
        };

        let palette = resolve_palette(&config.themes);
        let config_ui = ConfigUiState::default();
        let navbar = NavbarState {
            word_wrap_enabled: config.word_wrap.enabled,
            full_width_enabled: config.margin.full_width,
            ..NavbarState::default()
        };

        let mut tabs: Vec<Tab> = Vec::new();
        let mut next_tab_num: usize = 1;
        let mut toasts: Vec<ToastItem> = Vec::new();
        let mut next_toast_id: u64 = 1;

        // 1. Open CLI file arguments (`clearmind.exe specs_v1.md`)
        let cli_args: Vec<String> = std::env::args().skip(1).collect();
        for arg in &cli_args {
            if arg.starts_with('-') {
                continue;
            }
            let raw_path = PathBuf::from(arg);
            let path = watcher::normalized_path(&raw_path);
            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(arg.as_str())
                .to_string();
            let bytes = match std::fs::read(&raw_path).or_else(|_| std::fs::read(&path)) {
                Ok(b) => b,
                Err(err) if raw_path.exists() || path.exists() => {
                    toasts.push(ToastItem {
                        id: next_toast_id,
                        kind: ToastKind::Error,
                        message: format!("Could not read {}: {err}", path.display()),
                    });
                    next_toast_id += 1;
                    Vec::new()
                }
                Err(_) => Vec::new(),
            };
            let uuid = format!("tab-{next_tab_num}");
            next_tab_num += 1;
            tabs.push(Tab::from_bytes(uuid, title, Some(path.clone()), &bytes));

            let path_str = path.display().to_string();
            config.session.recent_files.retain(|p| p != &path_str);
            config.session.recent_files.insert(0, path_str.clone());
            config.session.recent_files.truncate(10);
            config.session.active_file = Some(path_str);
        }

        // 2. If no CLI files were passed, restore recovery snapshots or session.open_files
        if tabs.is_empty() {
            for snap in
                config::storage::read_all_recovery_snapshots(&config::storage::recovery_dir())
            {
                let path_opt = snap.file_path.as_ref().map(PathBuf::from);
                let title = path_opt
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Recovered.md")
                    .to_string();
                let mut tab =
                    Tab::from_bytes(snap.tab_uuid, title, path_opt, snap.content.as_bytes());
                tab.is_dirty = true;
                tabs.push(tab);
                next_tab_num += 1;
            }
        }

        if tabs.is_empty() && config.session.restore_session {
            for file_str in &config.session.open_files {
                let path = PathBuf::from(file_str);
                if path.is_file()
                    && let Ok(bytes) = std::fs::read(&path)
                {
                    let title = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Untitled.md")
                        .to_string();
                    let uuid = format!("tab-{next_tab_num}");
                    next_tab_num += 1;
                    tabs.push(Tab::from_bytes(uuid, title, Some(path), &bytes));
                }
            }
        }

        if tabs.is_empty() {
            tabs.push(Tab::new_untitled(
                format!("untitled-{next_tab_num}"),
                format!("Untitled {next_tab_num}"),
            ));
            next_tab_num += 1;
        }

        let _ = config::save_config_to_path(&config, &config_path);

        let window_size = Size::new(
            config.session.window.width as f32,
            config.session.window.height as f32,
        );

        let app = Self {
            config,
            config_ui,
            navbar,
            tabs,
            next_tab_num,
            active_tab: 0,
            closed_tabs: ClosedTabStack::default(),
            search: SearchState::default(),
            window_id: None,
            window_size,
            is_maximized: false,
            toasts,
            confirm_prompt: None,
            quick_pick: None,
            context_menu: None,
            zoom_image: None,
            show_settings: false,
            print_preview: PrintPreviewState::default(),
            show_stats_popover: false,
            slash_popup: None,
            popover_anchor: None,
            palette,
            next_toast_id,
        };

        let _recovery_snapshots =
            config::storage::read_all_recovery_snapshots(&config::storage::recovery_dir());

        let open_task = window::oldest().map(|opt| {
            opt.map(Message::WindowOpened)
                .unwrap_or(Message::Noop)
        });
        let font_task = crate::core::fonts::update::load_system_fonts()
            .map(|m| Message::Config(ConfigMessage::Fonts(m)));

        (app, Task::batch([open_task, font_task]))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.window_id = Some(id);
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_size = size;
                self.config.session.window.width = size.width.max(1.0) as u32;
                self.config.session.window.height = size.height.max(1.0) as u32;
                Task::none()
            }
            Message::DragResize(dir) => {
                if let Some(id) = self.window_id {
                    window::drag_resize(id, dir)
                } else {
                    Task::none()
                }
            }
            Message::WindowEvent(event) => match event {
                window::Event::Opened { .. } => Task::none(),
                window::Event::Resized(size) => {
                    self.window_size = size;
                    Task::none()
                }
                window::Event::FileDropped(path) => {
                    let norm = watcher::normalized_path(&path);
                    if editor::update::is_image_file_path(&norm) {
                        return self.handle_editor(EditorMessage::FileDropped(norm));
                    }
                    let ext = norm
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_default();
                    if matches!(ext.as_str(), "md" | "markdown" | "txt" | "") {
                        if let Ok(bytes) = std::fs::read(&norm) {
                            self.open_loaded_files(vec![(norm, bytes)]);
                        }
                    } else {
                        self.push_toast(
                            ToastKind::Info,
                            format!("Unsupported dropped file: {}", norm.display()),
                        );
                    }
                    Task::none()
                }
                window::Event::CloseRequested => self.flush_and_close(),
                _ => Task::none(),
            },
            Message::KeyboardEvent(event) => self.handle_keyboard_event(event),
            Message::Navbar(nav_msg) => self.handle_navbar(nav_msg),
            Message::Editor(ed_msg) => self.handle_editor(ed_msg),
            Message::Config(cfg_msg) => self.handle_config(cfg_msg),
            Message::Print(print_msg) => self.handle_print(print_msg),
            Message::FileChanged(paths) => {
                for tab in &self.tabs {
                    if let Some(tab_path) = &tab.path {
                        let _changed = paths
                            .iter()
                            .any(|p| watcher::paths_equal(p, tab_path));
                    }
                }
                Task::none()
            }
            Message::FilesOpened(files) => {
                self.open_loaded_files(files);
                Task::none()
            }
            Message::FileSaved(res) => {
                if let Some((path, outcome)) = res {
                    match outcome {
                        Ok(()) => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                                tab.path = Some(path.clone());
                                tab.title = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Untitled.md")
                                    .to_string();
                                tab.is_dirty = false;
                                let _ = config::storage::delete_recovery_snapshot(
                                    &config::storage::recovery_dir(),
                                    &tab.tab_uuid,
                                );
                            }
                            self.record_recent_file(&path);
                            self.push_toast(ToastKind::Success, "File saved");
                        }
                        Err(err) => {
                            self.push_toast(ToastKind::Error, format!("Save failed: {err}"));
                        }
                    }
                }
                Task::none()
            }
            Message::PopoverAnchorFound(rect) => {
                self.popover_anchor = rect;
                Task::none()
            }
            Message::DismissToast(id) => {
                self.toasts.retain(|t| t.id != id);
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::CloseQuickPick => {
                self.quick_pick = None;
                Task::none()
            }
            Message::CloseZoomViewer => {
                self.zoom_image = None;
                Task::none()
            }
            Message::CloseConfirmPrompt => {
                self.confirm_prompt = None;
                Task::none()
            }
            Message::CloseSettingsModal => {
                self.show_settings = false;
                Task::none()
            }
            Message::SlashAccept(idx) => {
                self.accept_slash_suggestion(idx);
                Task::none()
            }
            Message::Noop => Task::none(),
        }
    }

    fn handle_keyboard_event(&mut self, event: keyboard::Event) -> Task<Message> {
        let keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        } = event
        else {
            return Task::none();
        };

        let key_str = match &key {
            keyboard::Key::Named(named) => named_key_label(*named),
            keyboard::Key::Character(c) => c.to_string(),
            keyboard::Key::Unidentified => String::new(),
        };

        let active_tab = self.tabs.get(self.active_tab);
        let has_multi_cursors = active_tab.is_some_and(|t| !t.extra_cursors.is_empty());
        let has_selection = active_tab.is_some_and(|t| !t.selection.is_empty());

        // If we are actively recording a keybinding, capture the chord directly (unless Escape)
        if let Some(recording_cmd) = self.config_ui.keybindings_ui.recording_command {
            if matches!(key.as_ref(), keyboard::Key::Named(keyboard::key::Named::Escape)) {
                self.config_ui.keybindings_ui.recording_command = None;
                self.config_ui.keybindings_ui.conflict_warning = None;
                return Task::none();
            }
            let is_modifier_only = matches!(
                key.as_ref(),
                keyboard::Key::Named(
                    keyboard::key::Named::Control
                        | keyboard::key::Named::Shift
                        | keyboard::key::Named::Alt
                        | keyboard::key::Named::Super
                )
            );
            if !is_modifier_only && !key_str.is_empty() {
                let chord = crate::core::keybindings::engine::format_chord(&KeyInput {
                    key: &key_str.to_uppercase(),
                    ctrl: modifiers.control(),
                    alt: modifiers.alt(),
                    shift: modifiers.shift(),
                    text: text.as_deref(),
                });
                return self.handle_config(ConfigMessage::Keybindings(
                    crate::core::keybindings::models::KeybindingsMessage::CommitBinding {
                        command: recording_cmd,
                        shortcut: chord,
                    },
                ));
            }
            return Task::none();
        }

        // Ctrl+Space explicitly triggers context-aware Intellisense (Prose commands or LaTeX functions).
        if modifiers.control()
            && !modifiers.alt()
            && !self.show_settings
            && !self.print_preview.is_open
            && self.navbar.active_popover.is_none()
            && self.quick_pick.is_none()
            && !self.search.is_open
            && matches!(
                key.as_ref(),
                keyboard::Key::Named(keyboard::key::Named::Space)
            )
        {
            self.open_intellisense_at_cursor(true);
            return Task::none();
        }

        // Navigate or accept the open Slash / LaTeX Intellisense popup.
        if self.slash_popup.is_some()
            && !modifiers.control()
            && !modifiers.alt()
            && !self.show_settings
            && !self.print_preview.is_open
        {
            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    if let Some(menu) = self.slash_popup.as_mut()
                        && !menu.suggestions.is_empty()
                    {
                        menu.selected_index = (menu.selected_index + 1) % menu.suggestions.len();
                    }
                    return Task::none();
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    if let Some(menu) = self.slash_popup.as_mut()
                        && !menu.suggestions.is_empty()
                    {
                        menu.selected_index = if menu.selected_index == 0 {
                            menu.suggestions.len() - 1
                        } else {
                            menu.selected_index - 1
                        };
                    }
                    return Task::none();
                }
                keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Tab) => {
                    if let Some(idx) = self.slash_popup.as_ref().map(|m| m.selected_index) {
                        self.accept_slash_suggestion(idx);
                    }
                    return Task::none();
                }
                _ => {}
            }
        }

        let input = KeyInput {
            key: &key_str,
            ctrl: modifiers.control(),
            alt: modifiers.alt(),
            shift: modifiers.shift(),
            text: text.as_deref(),
        };
        let ctx = RouterContext {
            recording_shortcut: self.config_ui.keybindings_ui.recording_command.is_some(),
            modal_open: self.show_settings || self.print_preview.is_open,
            popover_open: self.navbar.active_popover.is_some(),
            quick_pick_open: self.quick_pick.is_some(),
            slash_open: self.slash_popup.is_some(),
            find_replace_open: self.search.is_open,
            has_multi_cursors,
            has_selection,
        };

        match route_key_event(&self.config.keybindings, &ctx, &input) {
            KeyDispatch::Escape(target) => match target {
                EscapeTarget::CloseRecorder => {
                    self.config_ui.keybindings_ui.recording_command = None;
                }
                EscapeTarget::CloseModal => {
                    self.show_settings = false;
                    self.print_preview.is_open = false;
                    self.navbar.print_preview_open = false;
                }
                EscapeTarget::ClosePopover => self.navbar.active_popover = None,
                EscapeTarget::CloseQuickPick => self.quick_pick = None,
                EscapeTarget::CloseSlashIntellisense => self.slash_popup = None,
                EscapeTarget::CloseFindReplace => self.search.is_open = false,
                EscapeTarget::CollapseMultiCursors => {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.extra_cursors.clear();
                    }
                }
                EscapeTarget::ClearSelection => {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.selection = editor::models::DocumentSelection::collapsed(tab.selection.head);
                    }
                }
                EscapeTarget::None => {
                    self.context_menu = None;
                    self.zoom_image = None;
                    self.confirm_prompt = None;
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.active_line = None;
                    }
                }
            },
            KeyDispatch::Command(cmd) => return self.execute_command(cmd),
            KeyDispatch::TypedText(txt) => {
                if !self.show_settings
                    && !self.print_preview.is_open
                    && self.quick_pick.is_none()
                    && !self.search.is_open
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && tab.active_line.is_none()
                {
                    if txt == "\r" || txt == "\n" {
                        tab.active_line = Some(tab.selection.head.line);
                    } else if !txt.chars().all(|c| c.is_control()) {
                        tab.active_line = Some(tab.selection.head.line);
                        return self.handle_editor(EditorMessage::WidgetEvent(
                            EditorWidgetEvent::InsertText(txt),
                        ));
                    }
                }
            }
            KeyDispatch::Unhandled => {
                if !self.show_settings
                    && !self.print_preview.is_open
                    && self.quick_pick.is_none()
                    && !self.search.is_open
                    && !modifiers.control()
                    && !modifiers.alt()
                {
                    let extend = modifiers.shift();
                    match key.as_ref() {
                        keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                            return self.handle_editor(EditorMessage::PageUp { extend });
                        }
                        keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                            return self.handle_editor(EditorMessage::PageDown { extend });
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            let is_inactive = self
                                .tabs
                                .get(self.active_tab)
                                .is_some_and(|t| t.active_line.is_none());
                            if is_inactive {
                                return self.handle_editor(EditorMessage::ScrollByLines(-3.0));
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            let is_inactive = self
                                .tabs
                                .get(self.active_tab)
                                .is_some_and(|t| t.active_line.is_none());
                            if is_inactive {
                                return self.handle_editor(EditorMessage::ScrollByLines(3.0));
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter) => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab)
                                && tab.active_line.is_none()
                            {
                                tab.active_line = Some(tab.selection.head.line);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Task::none()
    }

    fn execute_command(&mut self, cmd: CommandId) -> Task<Message> {
        match cmd {
            CommandId::FileNewTab => self.handle_navbar(NavbarMessage::NewTab),
            CommandId::FileOpen => Task::perform(pick_markdown_files(), Message::FilesOpened),
            CommandId::FileSave => self.save_active_tab(false),
            CommandId::FileSaveAs => self.save_active_tab(true),
            CommandId::FileCloseTab => {
                if let Some(uuid) = self.tabs.get(self.active_tab).map(|t| t.tab_uuid.clone()) {
                    return self.handle_navbar(NavbarMessage::CloseTab(uuid));
                }
                Task::none()
            }
            CommandId::FileOpenSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }
            CommandId::EditUndo => self.handle_editor(EditorMessage::Undo),
            CommandId::EditRedo => self.handle_editor(EditorMessage::Redo),
            CommandId::EditCopy => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::CopySelection)
            }
            CommandId::EditCut => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::CutSelection)
            }
            CommandId::EditPaste => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                Task::perform(read_clipboard_payload(), |payload| {
                    Message::Editor(EditorMessage::ClipboardPasted(payload))
                })
            }
            CommandId::EditSelectAll => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::SelectAll)
            }
            CommandId::EditDuplicate => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::DuplicateSelectionOrLine)
            }
            CommandId::EditMoveLineUp => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::MoveLinesUp)
            }
            CommandId::EditMoveLineDown => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.search.is_open
                    || self.navbar.active_popover.is_some()
                {
                    return Task::none();
                }
                self.handle_editor(EditorMessage::MoveLinesDown)
            }
            CommandId::EditCopyRichText => self.handle_editor(EditorMessage::CopyRichText),
            CommandId::EditFind => {
                self.search.is_open = true;
                self.search.show_replace = false;
                self.search.focus_query = true;
                self.search.focus_replace = false;
                iced::widget::operation::focus(iced::widget::Id::new(
                    editor::widgets::search_bar::SEARCH_QUERY_INPUT_ID,
                ))
            }
            CommandId::EditReplace => {
                self.search.is_open = true;
                self.search.show_replace = true;
                self.search.focus_query = self.search.query.is_empty();
                self.search.focus_replace = !self.search.query.is_empty();
                let id = if self.search.query.is_empty() {
                    editor::widgets::search_bar::SEARCH_QUERY_INPUT_ID
                } else {
                    editor::widgets::search_bar::SEARCH_REPLACE_INPUT_ID
                };
                iced::widget::operation::focus(iced::widget::Id::new(id))
            }
            CommandId::FormatBold => self.handle_editor(EditorMessage::Format(FormatCommand::Bold)),
            CommandId::FormatItalic => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Italic))
            }
            CommandId::FormatInlineCode => {
                self.handle_editor(EditorMessage::Format(FormatCommand::InlineCode))
            }
            CommandId::FormatStrikethrough => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Strikethrough))
            }
            CommandId::FormatLink => self.handle_editor(EditorMessage::Format(FormatCommand::Link)),
            CommandId::FormatHeading1 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(1)))
            }
            CommandId::FormatHeading2 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(2)))
            }
            CommandId::FormatHeading3 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(3)))
            }
            CommandId::FormatHeading4 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(4)))
            }
            CommandId::FormatHeading5 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(5)))
            }
            CommandId::FormatHeading6 => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Heading(6)))
            }
            CommandId::FormatParagraph => {
                self.handle_editor(EditorMessage::Format(FormatCommand::ClearHeading))
            }
            CommandId::FormatBlockquote => {
                self.handle_editor(EditorMessage::Format(FormatCommand::Blockquote))
            }
            CommandId::FormatTaskToggle => self.handle_editor(EditorMessage::ToggleActiveTask),
            CommandId::FormatTableAlign => self.handle_editor(EditorMessage::FormatActiveTable),
            CommandId::ViewToggleWordWrap => self.handle_navbar(NavbarMessage::ToggleWordWrap),
            CommandId::ViewToggleFullWidth => self.handle_navbar(NavbarMessage::ToggleFullWidth),
            CommandId::ViewToggleFocusMode => {
                self.config.view.focus_mode = !self.config.view.focus_mode;
                Task::none()
            }
            CommandId::ViewToggleTypewriterMode => {
                self.config.view.typewriter_mode = !self.config.view.typewriter_mode;
                Task::none()
            }
            CommandId::ViewZoomIn => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.zoom = (tab.zoom + 0.1).clamp(0.5, 2.0);
                }
                Task::none()
            }
            CommandId::ViewZoomOut => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.zoom = (tab.zoom - 0.1).clamp(0.5, 2.0);
                }
                Task::none()
            }
            CommandId::ViewZoomReset => self.handle_editor(EditorMessage::ResetZoom),
            CommandId::NavNextTab => {
                if self.tabs.len() > 1 {
                    self.active_tab = (self.active_tab + 1) % self.tabs.len();
                }
                Task::none()
            }
            CommandId::NavPrevTab => {
                if self.tabs.len() > 1 {
                    self.active_tab = if self.active_tab == 0 {
                        self.tabs.len() - 1
                    } else {
                        self.active_tab - 1
                    };
                }
                Task::none()
            }
            CommandId::SlashTrigger => {
                if self.show_settings
                    || self.print_preview.is_open
                    || self.navbar.active_popover.is_some()
                    || self.quick_pick.is_some()
                    || self.search.is_open
                {
                    return Task::none();
                }
                let is_viewing = self
                    .tabs
                    .get(self.active_tab)
                    .is_some_and(|t| t.active_line.is_none());
                if is_viewing {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.active_line = Some(tab.selection.head.line);
                    }
                    return self.handle_editor(EditorMessage::WidgetEvent(
                        EditorWidgetEvent::InsertText("/".to_string()),
                    ));
                }
                // In edit mode, LiveMarkdownEditor inserts `/` via handle_editor;
                // ensure the popup is open if `/` is already right before the cursor.
                self.try_open_slash_for_char('/');
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn handle_navbar(&mut self, msg: NavbarMessage) -> Task<Message> {
        match &msg {
            NavbarMessage::DragWindow => {
                if let Some(id) = self.window_id {
                    return window::drag(id);
                }
            }
            NavbarMessage::ToggleMaximizeWindow => {
                if let Some(id) = self.window_id {
                    self.is_maximized = !self.is_maximized;
                    return window::maximize(id, self.is_maximized);
                }
            }
            NavbarMessage::MinimizeWindow => {
                if let Some(id) = self.window_id {
                    return window::minimize(id, true);
                }
            }
            NavbarMessage::CloseWindow => return self.flush_and_close(),
            NavbarMessage::OpenPrintPreview => {
                self.navbar.active_popover = None;
                self.navbar.recent_dropdown_open = false;
                self.show_settings = false;
                if self.print_preview.is_open {
                    self.print_preview.is_open = false;
                    self.navbar.print_preview_open = false;
                    return Task::none();
                }
                self.print_preview.is_open = true;
                self.navbar.print_preview_open = true;
                self.print_preview.error = None;
                self.print_preview.page_handles.clear();
                self.print_preview.pdf_bytes = None;
                return self.trigger_print_preview_compile();
            }
            NavbarMessage::OpenSettingsModal => {
                self.print_preview.is_open = false;
                self.navbar.print_preview_open = false;
                self.show_settings = !self.show_settings;
                return Task::none();
            }
            NavbarMessage::OpenFile => {
                self.navbar.recent_dropdown_open = false;
                return Task::perform(pick_markdown_files(), Message::FilesOpened);
            }
            NavbarMessage::OpenRecentFile(path_str) => {
                self.navbar.recent_dropdown_open = false;
                let raw_path = PathBuf::from(path_str);
                let path = watcher::normalized_path(&raw_path);
                match std::fs::read(&raw_path).or_else(|_| std::fs::read(&path)) {
                    Ok(bytes) => {
                        self.open_loaded_files(vec![(path, bytes)]);
                    }
                    Err(err) => {
                        self.push_toast(
                            ToastKind::Error,
                            format!("Could not open {}: {err}", path.display()),
                        );
                    }
                }
                return Task::none();
            }
            NavbarMessage::ClearRecentFiles => {
                self.navbar.recent_dropdown_open = false;
                self.config.session.recent_files.clear();
                let _ = config::save_config_to_path(&self.config, &config::config_file_path());
                return Task::none();
            }
            NavbarMessage::ToggleWordWrap => {
                self.config.word_wrap.enabled = !self.config.word_wrap.enabled;
                let _ = config::save_config_to_path(&self.config, &config::config_file_path());
            }
            NavbarMessage::ToggleFullWidth => {
                self.config.margin.full_width = !self.config.margin.full_width;
                let _ = config::save_config_to_path(&self.config, &config::config_file_path());
            }
            NavbarMessage::TogglePopover(_) => {
                return navbar::update::update_navbar(&mut self.navbar, msg).map(Message::Navbar);
            }
            NavbarMessage::NewTab => {
                let idx = self.next_tab_num;
                self.next_tab_num += 1;
                self.tabs.push(Tab::new_untitled(
                    format!("untitled-{idx}"),
                    format!("Untitled {idx}"),
                ));
                self.active_tab = self.tabs.len() - 1;
                return Task::none();
            }
            NavbarMessage::SelectTab(uuid) => {
                if let Some(pos) = self.tabs.iter().position(|t| t.tab_uuid == *uuid) {
                    self.active_tab = pos;
                }
                return Task::none();
            }
            NavbarMessage::CloseTab(uuid) => {
                if self.tabs.len() > 1
                    && let Some(pos) = self.tabs.iter().position(|t| t.tab_uuid == *uuid)
                {
                    let _ = config::storage::delete_recovery_snapshot(
                        &config::storage::recovery_dir(),
                        uuid,
                    );
                    self.tabs.remove(pos);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
                return Task::none();
            }
            _ => {}
        }
        navbar::update::update_navbar(&mut self.navbar, msg).map(Message::Navbar)
    }

    fn handle_editor(&mut self, msg: EditorMessage) -> Task<Message> {
        match &msg {
            EditorMessage::OpenFileDialog => {
                return Task::perform(pick_markdown_files(), Message::FilesOpened);
            }
            EditorMessage::SaveAsDialog => {
                return self.save_active_tab(true);
            }
            EditorMessage::ClipboardPasted(editor::update::ClipboardPayload::Error(err)) => {
                self.push_toast(ToastKind::Error, format!("Paste failed: {err}"));
                return Task::none();
            }
            _ => {}
        }

        let slash_hint = match &msg {
            EditorMessage::WidgetEvent(EditorWidgetEvent::InsertText(txt)) => {
                SlashSyncHint::Inserted(txt.clone())
            }
            EditorMessage::WidgetEvent(EditorWidgetEvent::KeyCommand(
                EditorKeyCommand::Backspace
                | EditorKeyCommand::Delete
                | EditorKeyCommand::DeleteWordBackward
                | EditorKeyCommand::DeleteWordForward,
            )) => SlashSyncHint::RefreshIfOpen,
            EditorMessage::WidgetEvent(_)
            | EditorMessage::JumpToLine(_)
            | EditorMessage::ClickCanvasBottom
            | EditorMessage::SelectTableCell { .. }
            | EditorMessage::Undo
            | EditorMessage::Redo => SlashSyncHint::Close,
            _ => SlashSyncHint::Keep,
        };

        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return Task::none();
        };
        let row_height =
            (self.config.fonts.font_size * self.config.fonts.line_height * tab.zoom).max(20.0);
        let viewport_height = (self.window_size.height - 64.0).max(120.0);
        let task = editor::update::update_editor(
            tab,
            &mut self.search,
            msg,
            row_height,
            viewport_height,
            self.config.view.typewriter_mode,
        )
        .map(Message::Editor);

        match slash_hint {
            SlashSyncHint::Inserted(txt) => {
                if txt == "/" {
                    self.try_open_slash_for_char('/');
                } else if txt == "\\" {
                    self.try_open_slash_for_char('\\');
                } else if self.slash_popup.is_some() {
                    self.refresh_slash_popup();
                }
            }
            SlashSyncHint::RefreshIfOpen => {
                if self.slash_popup.is_some() {
                    self.refresh_slash_popup();
                }
            }
            SlashSyncHint::Close => {
                self.slash_popup = None;
            }
            SlashSyncHint::Keep => {}
        }

        task
    }

    fn handle_config(&mut self, msg: ConfigMessage) -> Task<Message> {
        config::update::update(&mut self.config, &mut self.config_ui, msg);
        self.palette = resolve_palette(&self.config.themes);
        let _ = config::save_config_to_path(&self.config, &config::config_file_path());
        Task::none()
    }

    fn handle_print(&mut self, msg: PrintMessage) -> Task<Message> {
        match msg {
            PrintMessage::Close => {
                self.print_preview.is_open = false;
                self.navbar.print_preview_open = false;
                self.print_preview.is_exporting = false;
                Task::none()
            }
            PrintMessage::SetPaper(paper) => {
                self.print_preview.settings.paper = paper;
                self.trigger_print_preview_compile()
            }
            PrintMessage::SetOrientation(orientation) => {
                self.print_preview.settings.orientation = orientation;
                self.trigger_print_preview_compile()
            }
            PrintMessage::SetMargin(margin) => {
                self.print_preview.settings.margin = margin;
                self.trigger_print_preview_compile()
            }
            PrintMessage::SetThemeMode(mode) => {
                self.print_preview.settings.theme_mode = mode;
                self.trigger_print_preview_compile()
            }
            PrintMessage::TogglePageNumbers => {
                self.print_preview.settings.show_page_numbers =
                    !self.print_preview.settings.show_page_numbers;
                self.trigger_print_preview_compile()
            }
            PrintMessage::Compiled(res) => match res {
                Ok(doc) => {
                    if doc.generation != self.print_preview.generation {
                        return Task::none();
                    }
                    self.print_preview.is_compiling = false;
                    self.print_preview.error = None;
                    self.print_preview.page_handles = doc
                        .page_svgs
                        .into_iter()
                        .map(iced::widget::svg::Handle::from_memory)
                        .collect();
                    self.print_preview.pdf_bytes = Some(doc.pdf_bytes);
                    Task::none()
                }
                Err((gen_id, err)) => {
                    if gen_id != self.print_preview.generation {
                        return Task::none();
                    }
                    self.print_preview.is_compiling = false;
                    self.print_preview.error = Some(err);
                    self.print_preview.pdf_bytes = None;
                    Task::none()
                }
            },
            PrintMessage::ConfirmExportPdf => {
                let Some(pdf_bytes) = self.print_preview.pdf_bytes.clone() else {
                    return Task::none();
                };
                self.print_preview.is_exporting = true;
                let (suggested_name, default_dir) = self
                    .tabs
                    .get(self.active_tab)
                    .map(|tab| {
                        let stem = tab
                            .path
                            .as_ref()
                            .and_then(|p| p.file_stem())
                            .and_then(|s| s.to_str())
                            .unwrap_or_else(|| {
                                tab.title
                                    .strip_suffix(".md")
                                    .or_else(|| tab.title.strip_suffix(".markdown"))
                                    .unwrap_or(tab.title.as_str())
                            });
                        let clean_stem = if stem.trim().is_empty() {
                            "document"
                        } else {
                            stem.trim()
                        };
                        let dir = tab.path.as_ref().and_then(|p| p.parent().map(PathBuf::from));
                        (format!("{clean_stem}.pdf"), dir)
                    })
                    .unwrap_or_else(|| ("document.pdf".to_string(), None));

                Task::perform(
                    async move {
                        let path = tokio::task::spawn_blocking(move || {
                            let mut dlg = rfd::FileDialog::new()
                                .add_filter("PDF Document", &["pdf"])
                                .set_file_name(&suggested_name);
                            if let Some(dir) = default_dir
                                && dir.is_dir()
                            {
                                dlg = dlg.set_directory(dir);
                            }
                            dlg.save_file()
                        })
                        .await
                        .ok()??;
                        let final_path = if path.extension().is_none() {
                            path.with_extension("pdf")
                        } else {
                            path
                        };
                        let res = std::fs::write(&final_path, pdf_bytes).map_err(|e| e.to_string());
                        Some((final_path, res))
                    },
                    |res| Message::Print(PrintMessage::PdfExported(res)),
                )
            }
            PrintMessage::PdfExported(res) => {
                self.print_preview.is_exporting = false;
                if let Some((path, outcome)) = res {
                    match outcome {
                        Ok(()) => {
                            self.print_preview.is_open = false;
                            self.navbar.print_preview_open = false;
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("document.pdf");
                            self.push_toast(ToastKind::Success, format!("PDF exported: {name}"));
                        }
                        Err(err) => {
                            self.push_toast(
                                ToastKind::Error,
                                format!("PDF export failed: {err}"),
                            );
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn trigger_print_preview_compile(&mut self) -> Task<Message> {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            self.print_preview.error = Some("No active document to print".to_string());
            return Task::none();
        };
        self.print_preview.generation = self.print_preview.generation.wrapping_add(1);
        let gen_id = self.print_preview.generation;
        self.print_preview.is_compiling = true;
        self.print_preview.error = None;
        self.print_preview.doc_title = tab.title.clone();

        let markdown = tab.rope.to_string();
        let doc_title = tab.title.clone();
        let doc_path = tab.path.clone();
        let settings = self.print_preview.settings;
        let palette = self.palette;

        Task::perform(
            async move {
                match tokio::task::spawn_blocking(move || {
                    print::compile_print_document(
                        gen_id,
                        &markdown,
                        &doc_title,
                        doc_path.as_deref(),
                        settings,
                        palette,
                    )
                })
                .await
                {
                    Ok(Ok(doc)) => Ok(doc),
                    Ok(Err(err)) => Err((gen_id, err)),
                    Err(join_err) => Err((gen_id, format!("Preview task failed: {join_err}"))),
                }
            },
            |res| Message::Print(PrintMessage::Compiled(res)),
        )
    }

    fn try_open_slash_for_char(&mut self, trigger_char: char) {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        let head = tab.selection.head;
        if head.col == 0 || head.line >= tab.rope.len_lines() {
            return;
        }
        let line_str = tab.rope.line(head.line).to_string();
        let line_trimmed = line_str.trim_end_matches(['\r', '\n']);
        let chars: Vec<char> = line_trimmed.chars().collect();
        let trigger_col = head.col - 1;
        if chars.get(trigger_col).copied() != Some(trigger_char) {
            return;
        }

        let all_lines: Vec<String> = (0..tab.rope.len_lines())
            .map(|i| tab.rope.line(i).to_string())
            .collect();
        let ctx = detect_document_trigger_context(&all_lines, head.line, trigger_col);

        if trigger_char == '\\' {
            if ctx != TriggerContext::Math {
                return;
            }
        } else {
            match ctx {
                TriggerContext::Suppressed => return,
                TriggerContext::Prose => {
                    // Only trigger prose `/` at start of line or after whitespace
                    if trigger_col > 0
                        && !chars
                            .get(trigger_col - 1)
                            .is_some_and(|c| c.is_whitespace())
                    {
                        return;
                    }
                }
                TriggerContext::Math => {}
            }
        }

        let suggestions = suggestions_for_context(ctx, "", &self.config.commands);
        if suggestions.is_empty() {
            return;
        }

        self.slash_popup = Some(SlashMenuState {
            context: ctx,
            trigger_line: head.line,
            trigger_col,
            trigger_char: Some(trigger_char),
            query: String::new(),
            arg: None,
            suggestions,
            selected_index: 0,
        });
    }

    fn open_intellisense_at_cursor(&mut self, activate_if_viewing: bool) {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        if activate_if_viewing && tab.active_line.is_none() {
            tab.active_line = Some(tab.selection.head.line);
        }
        let head = tab.selection.head;
        if head.line >= tab.rope.len_lines() {
            return;
        }
        let line_str = tab.rope.line(head.line).to_string();
        let line_trimmed = line_str.trim_end_matches(['\r', '\n']);
        let chars: Vec<char> = line_trimmed.chars().collect();
        let clamped_col = head.col.min(chars.len());

        // Check if cursor is currently inside an active `/...` or `\...` token
        let mut trigger_col = clamped_col;
        let mut trigger_char: Option<char> = None;
        let mut query = String::new();
        let mut arg: Option<String> = None;

        let mut scan = clamped_col;
        while scan > 0 {
            let ch = chars[scan - 1];
            if ch == '/' || ch == '\\' {
                let before_ok = scan == 1 || chars[scan - 2].is_whitespace() || ch == '\\';
                if before_ok {
                    trigger_col = scan - 1;
                    trigger_char = Some(ch);
                    let after: String = chars[scan..clamped_col].iter().collect();
                    if let Some((q, a)) = after.split_once(' ') {
                        query = q.to_string();
                        arg = Some(a.to_string());
                    } else {
                        query = after;
                    }
                }
                break;
            }
            if ch.is_whitespace() {
                break;
            }
            scan -= 1;
        }

        let all_lines: Vec<String> = (0..tab.rope.len_lines())
            .map(|i| tab.rope.line(i).to_string())
            .collect();
        let ctx = detect_document_trigger_context(&all_lines, head.line, trigger_col);
        if ctx == TriggerContext::Suppressed {
            self.slash_popup = None;
            return;
        }

        // If `\` was found outside math, treat as plain Ctrl+Space at caret instead
        if trigger_char == Some('\\') && ctx != TriggerContext::Math {
            trigger_col = clamped_col;
            trigger_char = None;
            query.clear();
            arg = None;
        }

        let suggestions = suggestions_for_context(ctx, &query, &self.config.commands);
        if suggestions.is_empty() {
            self.slash_popup = None;
            return;
        }

        self.slash_popup = Some(SlashMenuState {
            context: ctx,
            trigger_line: head.line,
            trigger_col,
            trigger_char,
            query,
            arg,
            suggestions,
            selected_index: 0,
        });
    }

    fn refresh_slash_popup(&mut self) {
        let Some(menu) = self.slash_popup.as_mut() else {
            return;
        };
        let Some(tab) = self.tabs.get(self.active_tab) else {
            self.slash_popup = None;
            return;
        };
        let head = tab.selection.head;
        if head.line != menu.trigger_line || head.line >= tab.rope.len_lines() {
            self.slash_popup = None;
            return;
        }
        let line_str = tab.rope.line(head.line).to_string();
        let line_trimmed = line_str.trim_end_matches(['\r', '\n']);
        let chars: Vec<char> = line_trimmed.chars().collect();

        let query_start = if let Some(tc) = menu.trigger_char {
            if chars.get(menu.trigger_col).copied() != Some(tc)
                || head.col < menu.trigger_col + 1
            {
                self.slash_popup = None;
                return;
            }
            menu.trigger_col + 1
        } else {
            if head.col < menu.trigger_col {
                self.slash_popup = None;
                return;
            }
            menu.trigger_col
        };

        let end_col = head.col.min(chars.len());
        let typed: String = chars[query_start..end_col].iter().collect();

        if let Some((cmd_part, arg_part)) = typed.split_once(' ') {
            let mut filtered = suggestions_for_context(menu.context, cmd_part, &self.config.commands);
            filtered.retain(|s| {
                s.takes_args && s.trigger.eq_ignore_ascii_case(cmd_part.trim())
            });
            if filtered.is_empty() {
                self.slash_popup = None;
                return;
            }
            menu.query = cmd_part.to_string();
            menu.arg = Some(arg_part.to_string());
            menu.suggestions = filtered;
            menu.selected_index = menu.selected_index.min(menu.suggestions.len() - 1);
        } else {
            let filtered = suggestions_for_context(menu.context, &typed, &self.config.commands);
            if filtered.is_empty() {
                self.slash_popup = None;
                return;
            }
            menu.query = typed;
            menu.arg = None;
            menu.suggestions = filtered;
            menu.selected_index = menu.selected_index.min(menu.suggestions.len() - 1);
        }
    }

    fn accept_slash_suggestion(&mut self, idx: usize) {
        let Some(menu) = self.slash_popup.take() else {
            return;
        };
        let Some(suggestion) = menu.suggestions.get(idx) else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        if menu.trigger_line >= tab.rope.len_lines() {
            return;
        }

        let clipboard_text = if suggestion.template.contains("{{clipboard}}") {
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut cb| cb.get_text().ok())
        } else {
            None
        };
        let var_ctx = VariableContext {
            filename: Some(tab.title.as_str()),
            clipboard: clipboard_text.as_deref(),
            arg: menu.arg.as_deref(),
        };
        let mut expanded = expand_suggestion(suggestion, &var_ctx);

        // If inserting a multi-line Markdown block mid-line after non-whitespace text,
        // start it on a fresh line so fences/tables always begin at column 0.
        let is_block_insert = expanded.text.contains('\n')
            && matches!(
                suggestion.trigger.as_str(),
                "code" | "latex" | "table" | "mermaid" | "callout"
            );
        if is_block_insert && menu.trigger_col > 0 {
            let line_str = tab.rope.line(menu.trigger_line).to_string();
            let before_trigger: String = line_str.chars().take(menu.trigger_col).collect();
            if !before_trigger.trim().is_empty() {
                expanded.text.insert(0, '\n');
                if let Some(off) = expanded.cursor_offset.as_mut() {
                    *off += 1;
                }
            }
        }

        let start_cursor = editor::models::Cursor::new(menu.trigger_line, menu.trigger_col);
        let end_col = if tab.selection.head.line == menu.trigger_line {
            tab.selection.head.col.max(menu.trigger_col)
        } else {
            menu.trigger_col
        };
        let end_cursor = editor::models::Cursor::new(menu.trigger_line, end_col);

        let start_byte = tab.line_col_to_byte(start_cursor);
        let end_byte = tab.line_col_to_byte(end_cursor).max(start_byte);
        let deleted = tab
            .rope
            .slice(tab.rope.byte_to_char(start_byte)..tab.rope.byte_to_char(end_byte))
            .to_string();

        let cursor_rel = expanded.cursor_offset.unwrap_or(expanded.text.len());
        let before_cursor = &expanded.text[..cursor_rel.min(expanded.text.len())];
        let added_newlines = before_cursor.matches('\n').count();
        let after_line = menu.trigger_line + added_newlines;
        let after_col = if added_newlines == 0 {
            menu.trigger_col + before_cursor.chars().count()
        } else {
            before_cursor
                .rsplit('\n')
                .next()
                .map(|s| s.chars().count())
                .unwrap_or(0)
        };
        let after_cursor = editor::models::Cursor::new(after_line, after_col);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let tx = editor::models::Transaction {
            edits: vec![editor::models::Edit {
                start_byte,
                end_byte,
                deleted,
                inserted: expanded.text,
            }],
            selection_before: tab.selection,
            selection_after: editor::models::DocumentSelection::collapsed(after_cursor),
            extra_cursors_before: tab.extra_cursors.clone(),
            extra_cursors_after: Vec::new(),
            active_line_before: tab.active_line,
            active_line_after: Some(after_line),
            scroll_before: tab.scroll_offset,
            timestamp_ms: now_ms,
            is_coalesceable: false,
        };
        tab.apply_transaction(tx);
    }

    fn open_loaded_files(&mut self, files: Vec<(PathBuf, Vec<u8>)>) {
        for (path, bytes) in files {
            if let Some(existing) = self
                .tabs
                .iter()
                .position(|t| t.path.as_ref().is_some_and(|p| watcher::paths_equal(p, &path)))
            {
                self.active_tab = existing;
                continue;
            }

            let uuid = format!(
                "tab-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled.md")
                .to_string();
            let new_tab = Tab::from_bytes(uuid, title, Some(path.clone()), &bytes);
            let reuse_initial = self.tabs.len() == 1
                && self.tabs[0].path.is_none()
                && !self.tabs[0].is_dirty
                && self.tabs[0].rope.len_bytes() == 0;
            if reuse_initial {
                self.tabs[0] = new_tab;
                self.active_tab = 0;
            } else {
                self.tabs.push(new_tab);
                self.active_tab = self.tabs.len() - 1;
            }
            self.record_recent_file(&path);
        }
    }

    fn save_active_tab(&mut self, force_save_as: bool) -> Task<Message> {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return Task::none();
        };
        let bytes = tab.to_disk_bytes();
        let suggested_name = tab.title.clone();
        if !force_save_as && let Some(path) = tab.path.clone() {
            return Task::perform(
                async move {
                    let res = std::fs::write(&path, bytes).map_err(|e| e.to_string());
                    Some((path, res))
                },
                Message::FileSaved,
            );
        }

        Task::perform(
            async move {
                let path = tokio::task::spawn_blocking(move || {
                    rfd::FileDialog::new()
                        .add_filter("Markdown", &["md", "markdown", "txt"])
                        .set_file_name(&suggested_name)
                        .save_file()
                })
                .await
                .ok()??;
                let res = std::fs::write(&path, bytes).map_err(|e| e.to_string());
                Some((path, res))
            },
            Message::FileSaved,
        )
    }

    fn record_recent_file(&mut self, path: &std::path::Path) {
        let s = path.display().to_string();
        self.config.session.recent_files.retain(|p| p != &s);
        self.config.session.recent_files.insert(0, s.clone());
        self.config.session.recent_files.truncate(10);
        self.config.session.active_file = Some(s);
        let _ = config::save_config_to_path(&self.config, &config::config_file_path());
    }

    fn push_toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(ToastItem {
            id,
            kind,
            message: message.into(),
        });
    }

    fn flush_and_close(&self) -> Task<Message> {
        for tab in self.tabs.iter().filter(|t| t.is_dirty) {
            let snapshot = config::storage::RecoverySnapshot {
                tab_uuid: tab.tab_uuid.clone(),
                file_path: tab.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                content: tab.rope.to_string(),
                disk_hash_at_snapshot: None,
            };
            let _ = config::storage::write_recovery_snapshot(
                &config::storage::recovery_dir(),
                &snapshot,
            );
        }
        let _ = config::save_config_to_path(&self.config, &config::config_file_path());
        if let Some(id) = self.window_id {
            window::close(id)
        } else {
            iced::exit()
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            window::events().map(|(_id, event)| Message::WindowEvent(event)),
            keyboard::listen().map(Message::KeyboardEvent),
        ];

        let watched_paths: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter_map(|t| t.path.clone())
            .collect();

        if !watched_paths.is_empty() {
            subs.push(
                Subscription::run_with(watched_paths, |paths| {
                    watcher::file_event_stream(paths.clone())
                })
                .map(Message::FileChanged),
            );
        }

        Subscription::batch(subs)
    }
}

enum SlashSyncHint {
    Inserted(String),
    RefreshIfOpen,
    Close,
    Keep,
}

async fn read_clipboard_payload() -> editor::update::ClipboardPayload {
    match tokio::task::spawn_blocking(|| {
        let mut cb = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                return editor::update::ClipboardPayload::Error(format!(
                    "Clipboard unavailable: {e}"
                ));
            }
        };
        if let Ok(img) = cb.get_image()
            && img.width > 0
            && img.height > 0
            && !img.bytes.is_empty()
        {
            return editor::update::ClipboardPayload::Image {
                rgba: img.bytes.into_owned(),
                width: img.width as u32,
                height: img.height as u32,
            };
        }
        if let Ok(files) = cb.get().file_list()
            && !files.is_empty()
        {
            return editor::update::ClipboardPayload::FilePaths(files);
        }
        if let Ok(txt) = cb.get_text()
            && !txt.is_empty()
        {
            return editor::update::ClipboardPayload::Text(txt);
        }
        editor::update::ClipboardPayload::Empty
    })
    .await
    {
        Ok(payload) => payload,
        Err(e) => editor::update::ClipboardPayload::Error(e.to_string()),
    }
}

async fn pick_markdown_files() -> Vec<(PathBuf, Vec<u8>)> {
    let Some(paths) = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .pick_files()
    })
    .await
    .ok()
    .flatten()
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for path in paths {
        if let Ok(bytes) = std::fs::read(&path) {
            out.push((watcher::normalized_path(&path), bytes));
        }
    }
    out
}

fn named_key_label(named: keyboard::key::Named) -> String {
    use keyboard::key::Named;
    match named {
        Named::ArrowUp => "Up".to_string(),
        Named::ArrowDown => "Down".to_string(),
        Named::ArrowLeft => "Left".to_string(),
        Named::ArrowRight => "Right".to_string(),
        Named::Escape => "Escape".to_string(),
        Named::Enter => "Enter".to_string(),
        Named::Tab => "Tab".to_string(),
        Named::Backspace => "Backspace".to_string(),
        Named::Delete => "Delete".to_string(),
        Named::Home => "Home".to_string(),
        Named::End => "End".to_string(),
        Named::PageUp => "PgUp".to_string(),
        Named::PageDown => "PgDn".to_string(),
        other => format!("{other:?}"),
    }
}
