use iced::mouse;
use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{window, Border, Element, Length, Padding, Theme, Widget};

use crate::core::config;
use crate::core::editor::view::{view_editor, EditorViewConfig};
use crate::core::fonts;
use crate::core::models::{ClearMind, Message};
use crate::core::navbar::models::{QuickAccessPopover, TabItem};
use crate::core::navbar::view::view_navbar;
use crate::core::print;
use crate::core::themes;
use crate::core::widgets::{
    modal_overlay, modal_overlay_sized, popover_overlay, view_confirm_prompt, view_context_menu,
    view_quick_pick, view_toast,
};

const EDGE_SIZE: f32 = 6.0;
const CORNER_SIZE: f32 = 12.0;

impl ClearMind {
    pub fn theme(&self) -> Theme {
        self.palette.to_iced_theme()
    }

    pub fn style(&self, _theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: self.palette.background,
            text_color: self.palette.text,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let palette = &self.palette;

        let tab_items: Vec<TabItem> = self
            .tabs
            .iter()
            .map(|t| TabItem {
                uuid: t.tab_uuid.clone(),
                title: t.title.clone(),
                is_dirty: t.is_dirty,
                has_conflict: !matches!(t.conflict, crate::core::editor::models::ConflictState::None),
                is_deleted: t.is_deleted_on_disk,
            })
            .collect();
        let active_uuid = self.tabs.get(self.active_tab).map(|t| t.tab_uuid.as_str());

        let navbar_el = view_navbar(&self.navbar, &tab_items, active_uuid, palette)
            .map(Message::Navbar)
            .boxed();

        let editor_el: Element<'_, Message> = if let Some(tab) = self.tabs.get(self.active_tab) {
            let word_target = tab
                .path
                .as_ref()
                .and_then(|p| self.config.writing_targets.get(&p.display().to_string()).copied());
            let cfg = EditorViewConfig {
                full_width: self.config.margin.full_width,
                content_max_width: self.config.margin.content_max_width,
                word_wrap: self.config.word_wrap.enabled,
                font_size: self.config.fonts.font_size,
                code_size: self.config.fonts.code_size,
                line_height: self.config.fonts.line_height,
                focus_mode: self.config.view.focus_mode,
                typewriter_mode: self.config.view.typewriter_mode,
                word_target: word_target.map(|w| w as usize),
                viewport_height: (self.window_size.height - 36.0).max(100.0),
                body_font: self.config.fonts.body_font(),
                code_font: self.config.fonts.code_font(),
                slash_menu_open: self.slash_popup.is_some(),
            };
            view_editor(tab, &self.search, cfg, palette)
                .map(Message::Editor)
                .boxed()
        } else {
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed()
        };

        let main_layout = column![navbar_el, editor_el]
            .width(Length::Fill)
            .height(Length::Fill);

        let mut layers: Vec<Element<'_, Message>> = vec![main_layout.boxed()];

        if !self.is_maximized {
            layers.push(resize_frame());
        }

        if self.navbar.recent_dropdown_open {
            layers.push(view_recent_files_dropdown(
                &self.config.session.recent_files,
                palette,
            ));
        }

        if let Some(popover) = &self.navbar.active_popover {
            let content: Option<Element<'_, Message>> = match popover {
                QuickAccessPopover::Themes => Some(
                    themes::view::view_quick_popover(&self.config.themes, palette)
                        .map(|m| Message::Config(crate::core::config::ConfigMessage::Themes(m)))
                        .boxed(),
                ),
                QuickAccessPopover::Fonts => Some(
                    fonts::view::view_quick_popover(
                        &self.config.fonts,
                        &self.config_ui.available_fonts,
                        palette,
                    )
                    .map(|m| Message::Config(crate::core::config::ConfigMessage::Fonts(m)))
                    .boxed(),
                ),
                QuickAccessPopover::Keybindings => Some(
                    crate::core::keybindings::view::view_quick_popover(
                        &self.config.keybindings,
                        &self.config_ui.keybindings_ui,
                        palette,
                    )
                    .map(|m| Message::Config(crate::core::config::ConfigMessage::Keybindings(m)))
                    .boxed(),
                ),
                QuickAccessPopover::CustomCommands => Some(
                    crate::core::custom_commands::view::view_quick_popover(
                        &self.config.commands,
                        &self.config_ui.custom_commands_ui,
                        palette,
                    )
                    .map(|m| {
                        Message::Config(crate::core::config::ConfigMessage::CustomCommands(m))
                    })
                    .boxed(),
                ),
            };

            if let Some(body) = content {
                let anchor_y = self
                    .popover_anchor
                    .map(|r| r.y + r.height + 4.0)
                    .unwrap_or(40.0);
                let right_offset = self
                    .popover_anchor
                    .map(|r| (self.window_size.width - r.x - r.width).max(12.0))
                    .unwrap_or(24.0);
                layers.push(popover_overlay(
                    Space::new().width(Length::Fill).height(Length::Fill).boxed(),
                    body,
                    palette,
                    anchor_y,
                    right_offset,
                    Message::Navbar(crate::core::navbar::NavbarMessage::TogglePopover(
                        *popover,
                    )),
                ));
            }
        }

        if self.show_settings {
            let modal_body = config::view::view_modal(&self.config, &self.config_ui, palette)
                .map(Message::Config)
                .boxed();
            layers.push(modal_overlay(
                Space::new().width(Length::Fill).height(Length::Fill).boxed(),
                modal_body,
                palette,
                Message::CloseSettingsModal,
                780.0,
            ));
        }

        if self.print_preview.is_open {
            let modal_w = (self.window_size.width - 80.0).clamp(620.0, 940.0);
            let modal_h = (self.window_size.height - 68.0).clamp(480.0, 780.0);
            let print_body =
                print::view_print_preview_modal(&self.print_preview, palette, modal_w)
                    .map(Message::Print)
                    .boxed();
            layers.push(modal_overlay_sized(
                Space::new().width(Length::Fill).height(Length::Fill).boxed(),
                print_body,
                palette,
                Message::Print(print::PrintMessage::Close),
                modal_w,
                modal_h,
            ));
        }

        if let Some(svg_str) = &self.zoom_image {
            layers.push(crate::core::editor::widgets::zoom_viewer::view_zoom_viewer(
                Some(svg_str.as_bytes()),
                1.0,
                palette.background,
                palette.text,
                Message::Noop,
                Message::Noop,
                Message::Noop,
                Message::CloseZoomViewer,
            ));
        }

        if let Some(menu) = &self.slash_popup {
            let (anchor_x, anchor_y) = self.slash_popup_position(menu);
            layers.push(crate::core::custom_commands::view::view_slash_popup(
                menu,
                palette,
                anchor_x,
                anchor_y,
                Message::SlashAccept,
            ));
        }

        if self.search.is_open {
            use crate::core::editor::EditorMessage;
            layers.push(
                crate::core::editor::widgets::search_bar::view_search_bar(
                    &self.search,
                    palette,
                    EditorMessage::SearchQueryChanged,
                    EditorMessage::SearchReplaceChanged,
                    EditorMessage::SearchToggleReplaceExpanded,
                    EditorMessage::SearchToggleCase,
                    EditorMessage::SearchToggleWord,
                    EditorMessage::SearchToggleRegex,
                    EditorMessage::SearchPrev,
                    EditorMessage::SearchNext,
                    EditorMessage::ReplaceOne,
                    EditorMessage::ReplaceAll,
                    EditorMessage::CloseSearch,
                )
                .map(Message::Editor)
                .boxed(),
            );
        }

        if let Some(items) = &self.quick_pick {
            layers.push(view_quick_pick(
                "Quick Open...",
                "",
                items,
                0,
                palette,
                |_| Message::Noop,
            ));
        }

        if let Some((_pos, items)) = &self.context_menu {
            layers.push(view_context_menu(items, palette));
        }

        if let Some(prompt) = &self.confirm_prompt {
            layers.push(view_confirm_prompt(
                prompt,
                palette,
                Message::CloseConfirmPrompt,
                None,
                Message::CloseConfirmPrompt,
            ));
        }

        if !self.toasts.is_empty() {
            let toast_col = column![]
                .extend(
                    self.toasts
                        .iter()
                        .map(|t| view_toast(t, palette, Message::DismissToast(t.id))),
                )
                .spacing(8);

            layers.push(
                container(toast_col)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_right(Length::Fill)
                    .align_bottom(Length::Fill)
                    .padding(16)
                    .boxed(),
            );
        }

        stack(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }

    fn slash_popup_position(&self, menu: &crate::core::custom_commands::SlashMenuState) -> (f32, f32) {
        let content_w = if self.config.margin.full_width {
            (self.window_size.width - 96.0).max(360.0)
        } else {
            self.config
                .margin
                .content_max_width
                .min(self.window_size.width - 64.0)
                .max(360.0)
        };
        let left_margin = ((self.window_size.width - content_w) * 0.5).max(32.0);
        let char_w = (self.config.fonts.font_size * 0.52).max(7.0);
        let raw_x = left_margin + (menu.trigger_col as f32 * char_w);
        let anchor_x = raw_x.clamp(20.0, (self.window_size.width - 372.0).max(20.0));

        let zoom = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.zoom)
            .unwrap_or(1.0);
        let scroll_y = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.scroll_offset)
            .unwrap_or(0.0);
        let row_h = (self.config.fonts.font_size * self.config.fonts.line_height * zoom).max(24.0);
        // Navbar (36px) + top padding (24px) + one line below active caret
        let est_y = 64.0 + ((menu.trigger_line + 1) as f32 * row_h) - scroll_y;
        let max_y = (self.window_size.height - 320.0).max(72.0);
        let anchor_y = est_y.clamp(72.0, max_y);
        (anchor_x, anchor_y)
    }
}

fn resize_edge<'a>(
    direction: window::Direction,
    width: Length,
    height: Length,
    interaction: mouse::Interaction,
) -> Element<'a, Message> {
    mouse_area(Space::new().width(width).height(height))
        .on_press(Message::DragResize(direction))
        .interaction(interaction)
        .boxed()
}

fn resize_frame<'a>() -> Element<'a, Message> {
    let top_row = row![
        resize_edge(
            window::Direction::NorthWest,
            Length::Fixed(CORNER_SIZE),
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingDiagonallyDown,
        ),
        resize_edge(
            window::Direction::North,
            Length::Fill,
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingVertically,
        ),
        resize_edge(
            window::Direction::NorthEast,
            Length::Fixed(CORNER_SIZE),
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingDiagonallyUp,
        ),
    ]
    .width(Length::Fill);

    let mid_row = row![
        resize_edge(
            window::Direction::West,
            Length::Fixed(EDGE_SIZE),
            Length::Fill,
            mouse::Interaction::ResizingHorizontally,
        ),
        Space::new().width(Length::Fill).height(Length::Fill),
        resize_edge(
            window::Direction::East,
            Length::Fixed(EDGE_SIZE),
            Length::Fill,
            mouse::Interaction::ResizingHorizontally,
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let bottom_row = row![
        resize_edge(
            window::Direction::SouthWest,
            Length::Fixed(CORNER_SIZE),
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingDiagonallyUp,
        ),
        resize_edge(
            window::Direction::South,
            Length::Fill,
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingVertically,
        ),
        resize_edge(
            window::Direction::SouthEast,
            Length::Fixed(CORNER_SIZE),
            Length::Fixed(EDGE_SIZE),
            mouse::Interaction::ResizingDiagonallyDown,
        ),
    ]
    .width(Length::Fill);

    column![top_row, mid_row, bottom_row]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn view_recent_files_dropdown<'a>(
    recent_files: &[String],
    palette: &crate::core::themes::ThemePalette,
) -> Element<'a, Message> {
    use crate::core::navbar::NavbarMessage;
    use crate::core::widgets::button::subtle_button_style;

    let palette_copy = *palette;
    let mut items = column![
        text("Open File").size(12).color(palette.text_muted),
        button(text("Browse Files... (Ctrl+O)").size(13))
            .width(Length::Fill)
            .padding(Padding::from([6, 10]))
            .style(subtle_button_style(palette_copy, true))
            .on_press(Message::Navbar(NavbarMessage::OpenFile)),
    ]
    .spacing(6)
    .width(Length::Fixed(320.0));

    if recent_files.is_empty() {
        items = items.push(
            text("No recent files")
                .size(12)
                .color(palette.text_muted)
                .boxed(),
        );
    } else {
        items = items.push(
            text("Recent Files")
                .size(12)
                .color(palette.text_muted)
                .boxed(),
        );
        for file_path in recent_files.iter().take(8) {
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path.as_str())
                .to_string();
            let path_clone = file_path.clone();
            items = items.push(
                button(
                    column![
                        text(file_name).size(13).color(palette.text),
                        text(file_path.clone()).size(11).color(palette.text_muted),
                    ]
                    .spacing(2),
                )
                .width(Length::Fill)
                .padding(Padding::from([6, 10]))
                .style(subtle_button_style(palette_copy, false))
                .on_press(Message::Navbar(NavbarMessage::OpenRecentFile(path_clone)))
                .boxed(),
            );
        }
        items = items.push(
            button(text("Clear Recent Files").size(12))
                .width(Length::Fill)
                .padding(Padding::from([4, 10]))
                .style(subtle_button_style(palette_copy, false))
                .on_press(Message::Navbar(NavbarMessage::ClearRecentFiles))
                .boxed(),
        );
    }

    let card = container(items)
        .padding(12)
        .style(move |_| container::Style {
            background: Some(palette_copy.surface.into()),
            text_color: Some(palette_copy.text),
            border: Border {
                color: palette_copy.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    let backdrop = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::Navbar(NavbarMessage::ToggleRecentDropdown));

    let positioned = container(
        column![
            Space::new().height(Length::Fixed(38.0)),
            row![Space::new().width(Length::Fixed(16.0)), card],
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    stack![backdrop, positioned]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
