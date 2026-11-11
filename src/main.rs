use iced::alignment;
use iced::mouse;
use iced::widget::{
    button, column, container, markdown, mouse_area, row, scrollable, space, text, text_editor,
    Widget,
};
use iced::{Background, Border, Color, Font, Length, Padding, Size, Task, Theme};

/// Initial sample markdown for Tab 0 (Fiscal rules note).
const REGRAS_BI_MARKDOWN: &str = r#"# Regras BI Saldo Fiscal

## Visão Geral

Regras de negócio para o cálculo de saldo fiscal e consolidação no BI.

### Regras Principais

1. **Validação de Entradas**: Todas as notas fiscais de entrada devem conter CFOP válido.
2. **Apuração de Impostos**: ICMS, PIS e COFINS calculados na data de emissão da nota.
3. **Consolidação Mensal**: Fechamento contábil no último dia útil de cada mês corrente.

> "A precisão dos dados fiscais garante a conformidade tributária e a integridade do balanço patrimonial."

### Parâmetros do Sistema

- Alíquota padrão: `18%`
- Regime tributário: **Lucro Real**
- Verificação automática: [Portal Sped Fiscal](https://sped.rfb.gov.br)
"#;

/// Initial sample markdown for Tab 1 (Lorem Ipsum - matching Obsidian screenshot).
const LOREM_IPSUM_MARKDOWN: &str = r#"# Lorem Ipsum

## What is Lorem Ipsum?

Lorem Ipsum is simply dummy text of the printing and typesetting industry. Lorem Ipsum has been the industry's standard dummy text ever since 1966, when designers at Letraset and James Mosley, the librarian at St Bride Printing Library in London, took a 1914 Cicero translation and scrambled it to make dummy text for Letraset's Body Type sheets. It has survived not only many decades, but also the leap into electronic typesetting, remaining essentially unchanged. It was popularised thanks to these sheets and more recently with desktop publishing software like Aldus PageMaker and Microsoft Word including versions of Lorem Ipsum.

## Why do we use it?

It is a long established fact that a reader will be distracted by the readable content of a page when looking at its layout. The point of using Lorem Ipsum is that it has a more-or-less normal distribution of letters, as opposed to using 'Content here, content here', making it look like readable English. Many desktop publishing packages and web page editors now use Lorem Ipsum as their default model text, and a search for 'lorem ipsum' will uncover many web sites still in their infancy. Various versions have evolved over the years, sometimes by accident, sometimes on purpose (injected humour and the like).

## Where does it come from?

Contrary to popular belief, Lorem Ipsum is not simply random text. It has roots in a piece of classical Latin literature from 45 BC, making it over 2000 years old. Richard McClintock, a Latin professor at Hampden-Sydney College in Virginia, looked up one of the more obscure Latin words, consectetur, from a Lorem Ipsum passage, and going through the cites of the word in classical literature, discovered the undoubtable source. Lorem Ipsum comes from sections 1.10.32 and 1.10.33 of "de Finibus Bonorum et Malorum" (The Extremes of Good and Evil) by Cicero, written in 45 BC. This book is a treatise on the theory of ethics, very popular during the Renaissance. The first line of Lorem Ipsum, "Lorem ipsum dolor sit amet..", comes from a line in section 1.10.32.

The standard chunk of Lorem Ipsum used since 1966 is reproduced below for those interested. Sections 1.10.32 and 1.10.33 from "de Finibus Bonorum et Malorum" by Cicero are also reproduced in their exact original form, accompanied by English versions from the 1914 translation by H. Rackham.
"#;

/// Generates the GitHub Light mode custom theme.
fn custom_theme() -> Theme {
    let seed = iced::theme::palette::Seed {
        background: Color::from_rgb8(255, 255, 255), // Main canvas (#ffffff)
        text: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
        primary: Color::from_rgb8(9, 105, 218),      // Links and accents (#0969da)
        success: Color::from_rgb8(26, 127, 55),      // Green (#1a7f37)
        warning: Color::from_rgb8(154, 103, 0),      // Yellow/Orange (#9a6700)
        danger: Color::from_rgb8(207, 34, 46),       // Red (#cf222e)
    };

    Theme::custom("GitHub Light", seed)
}

/// A discrete line in the document with cached parsed Markdown items.
#[derive(Debug, Clone)]
pub struct Line {
    /// Raw text of the line.
    pub raw: String,
    /// Pre-parsed Markdown items for zero-cost view rendering.
    pub items: Vec<markdown::Item>,
}

impl Line {
    /// Creates a new line and parses its Markdown items.
    pub fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let items = markdown::parse(&raw).collect();
        Self { raw, items }
    }

    /// Updates the line text and regenerates pre-parsed items.
    pub fn update(&mut self, new_raw: String) {
        self.items = markdown::parse(&new_raw).collect();
        self.raw = new_raw;
    }
}

/// A document tab in the editor.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Unique tab identifier.
    pub id: usize,
    /// Tab display title.
    pub title: String,
    /// Lines of the document.
    pub lines: Vec<Line>,
    /// Index of the currently active line showing raw Markdown.
    pub active_line: Option<usize>,
    /// Buffer for the active line editor.
    pub active_editor: text_editor::Content,
}

impl Tab {
    /// Creates a new document tab from title and Markdown content.
    pub fn new(id: usize, title: String, content: &str) -> Self {
        let lines: Vec<Line> = content.lines().map(Line::new).collect();
        let lines = if lines.is_empty() {
            vec![Line::new("")]
        } else {
            lines
        };

        Self {
            id,
            title,
            lines,
            active_line: None,
            active_editor: text_editor::Content::new(),
        }
    }

    /// Selects a line to edit, committing any previously active line.
    pub fn select_line(&mut self, idx: usize) {
        if self.active_line == Some(idx) {
            return;
        }
        self.commit_active_line();
        if idx < self.lines.len() {
            self.active_line = Some(idx);
            self.active_editor = text_editor::Content::with_text(&self.lines[idx].raw);
        }
    }

    /// Commits the active line editor back into the lines list.
    pub fn commit_active_line(&mut self) {
        if let Some(idx) = self.active_line.take()
            && idx < self.lines.len()
        {
            let text = self.active_editor.text();
            if text.contains('\n') {
                let split: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                if split.is_empty() {
                    self.lines[idx].update(String::new());
                } else {
                    self.lines.remove(idx);
                    for (offset, l) in split.into_iter().enumerate() {
                        self.lines.insert(idx + offset, Line::new(l));
                    }
                }
            } else {
                self.lines[idx].update(text);
            }
        }
    }

    /// Returns the full reconstructed document text.
    pub fn full_text(&self) -> String {
        let mut raws: Vec<String> = self.lines.iter().map(|l| l.raw.clone()).collect();
        if let Some(idx) = self.active_line
            && idx < raws.len()
        {
            raws[idx] = self.active_editor.text();
        }
        raws.join("\n")
    }

    /// Computes the word count of the document.
    pub fn word_count(&self) -> usize {
        self.full_text().split_whitespace().count()
    }

    /// Computes the character count of the document.
    pub fn char_count(&self) -> usize {
        self.full_text().chars().count()
    }
}

/// The Doggy application state.
pub struct Doggy {
    /// Open tabs.
    tabs: Vec<Tab>,
    /// Index of the active tab.
    active_tab_index: usize,
    /// Next ID for new tabs.
    next_tab_id: usize,
    /// Whether the editor is in pure reading view mode.
    is_reading_mode: bool,
}

/// Messages handled by Doggy.
#[derive(Debug, Clone)]
pub enum Message {
    /// Select a tab by index.
    SelectTab(usize),
    /// Close a tab by index.
    CloseTab(usize),
    /// Create a new tab.
    NewTab,
    /// Select a line within the active document to show raw Markdown.
    SelectLine(usize),
    /// Deselect the active line and render all lines.
    DeselectLine,
    /// An action within the active line editor.
    EditorAction(text_editor::Action),
    /// Insert a new empty line below the current active line.
    InsertLineBelow,
    /// Clicked in the empty area below the document text.
    ClickBottomArea,
    /// Toggle between live preview edit and pure reading view.
    ToggleReadingMode,
    /// Clicked an interactive link in the rendered preview.
    LinkClicked(markdown::Uri),
}

impl Doggy {
    /// Creates a new `Doggy` instance initialized with the tabs from the Obsidian view.
    pub fn new() -> Self {
        let tab0 = Tab::new(1, "Regras BI Saldo Fiscal".to_string(), REGRAS_BI_MARKDOWN);
        let mut tab1 = Tab::new(2, "Lorem Ipsum".to_string(), LOREM_IPSUM_MARKDOWN);
        // Pre-select line 6 ("## Why do we use it?") to match Obsidian screenshot
        tab1.select_line(6);

        Self {
            tabs: vec![tab0, tab1],
            active_tab_index: 1, // Start on Lorem Ipsum tab
            next_tab_id: 3,
            is_reading_mode: false,
        }
    }

    /// Handles application messages and updates editor/tabs state.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTab(idx) => {
                if idx < self.tabs.len() && idx != self.active_tab_index {
                    if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
                        current.commit_active_line();
                    }
                    self.active_tab_index = idx;
                }
            }
            Message::CloseTab(idx) => {
                if self.tabs.len() <= 1 {
                    let next_id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs[0] = Tab::new(
                        next_id,
                        "Untitled".to_string(),
                        "# Untitled\n\nStart typing here...",
                    );
                    self.active_tab_index = 0;
                } else {
                    self.tabs.remove(idx);
                    if self.active_tab_index >= self.tabs.len() {
                        self.active_tab_index = self.tabs.len() - 1;
                    }
                }
            }
            Message::NewTab => {
                if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
                    current.commit_active_line();
                }
                let tab_id = self.next_tab_id;
                self.next_tab_id += 1;
                let title = format!("Untitled {}", tab_id);
                let new_tab = Tab::new(tab_id, title, "# New Document\n\nType something here...");
                self.tabs.push(new_tab);
                self.active_tab_index = self.tabs.len() - 1;
            }
            Message::SelectLine(idx) => {
                if !self.is_reading_mode
                    && let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                {
                    doc.select_line(idx);
                }
            }
            Message::DeselectLine => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    doc.commit_active_line();
                }
            }
            Message::EditorAction(action) => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index) {
                    let is_edit = action.is_edit();
                    doc.active_editor.perform(action);

                    if is_edit
                        && let Some(idx) = doc.active_line
                        && idx < doc.lines.len()
                    {
                        doc.lines[idx].update(doc.active_editor.text());
                    }
                }
            }
            Message::InsertLineBelow => {
                if let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                    && let Some(idx) = doc.active_line
                {
                    doc.lines[idx].update(doc.active_editor.text());
                    let new_idx = idx + 1;
                    doc.lines.insert(new_idx, Line::new(""));
                    doc.active_line = Some(new_idx);
                    doc.active_editor = text_editor::Content::new();
                }
            }
            Message::ClickBottomArea => {
                if !self.is_reading_mode
                    && let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                {
                    doc.commit_active_line();
                    let last_idx = doc.lines.len().saturating_sub(1);
                    if !doc.lines.is_empty() && !doc.lines[last_idx].raw.is_empty() {
                        doc.lines.push(Line::new(""));
                        let new_idx = doc.lines.len() - 1;
                        doc.select_line(new_idx);
                    } else {
                        doc.select_line(last_idx);
                    }
                }
            }
            Message::ToggleReadingMode => {
                self.is_reading_mode = !self.is_reading_mode;
                if self.is_reading_mode
                    && let Some(doc) = self.tabs.get_mut(self.active_tab_index)
                {
                    doc.commit_active_line();
                }
            }
            Message::LinkClicked(url) => {
                tracing::info!("Markdown link clicked: {url}");
            }
        }

        Task::none()
    }

    /// Renders the Obsidian-style live edit user interface with tabs and in-place line editing.
    pub fn view(&self) -> impl Widget<Message> + '_ {
        let theme_val = custom_theme();

        // 1. Top Tab Bar
        let mut tab_row: iced::widget::Row<iced::Element<'_, Message>> =
            iced::widget::Row::new().spacing(4).align_y(alignment::Vertical::Center);

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = idx == self.active_tab_index;

            let tab_title_click = mouse_area(
                text(&tab.title)
                    .size(12)
                    .color(if is_active {
                        Color::from_rgb8(31, 35, 40)
                    } else {
                        Color::from_rgb8(101, 109, 118)
                    }),
            )
            .interaction(mouse::Interaction::Pointer)
            .on_press(Message::SelectTab(idx));

            let close_click = mouse_area(
                container(
                    text("✕")
                        .size(10)
                        .color(Color::from_rgb8(140, 149, 159)),
                )
                .padding([2, 4]),
            )
            .interaction(mouse::Interaction::Pointer)
            .on_press(Message::CloseTab(idx));

            let tab_widget = container(
                row![tab_title_click, close_click]
                    .spacing(8)
                    .align_y(alignment::Vertical::Center),
            )
            .padding([5, 12])
            .style(move |_theme| container::Style {
                background: Some(Background::Color(if is_active {
                    Color::from_rgb8(255, 255, 255)
                } else {
                    Color::from_rgb8(240, 242, 245)
                })),
                border: Border::default()
                    .rounded(6.0)
                    .color(if is_active {
                        Color::from_rgb8(208, 215, 222)
                    } else {
                        Color::TRANSPARENT
                    })
                    .width(1.0),
                ..Default::default()
            });

            tab_row = tab_row.push(tab_widget.boxed());
        }

        let new_tab_btn = button(
            text("+")
                .size(14)
                .color(Color::from_rgb8(101, 109, 118)),
        )
        .style(button::subtle)
        .padding([3, 8])
        .on_press(Message::NewTab);

        tab_row = tab_row.push(new_tab_btn.boxed());

        let top_tab_bar = container(
            row![tab_row, space().width(Length::Fill)]
                .width(Length::Fill)
                .align_y(alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .padding([6, 12])
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
            border: Border::default()
                .color(Color::from_rgb8(208, 215, 222))
                .width(0.5),
            ..Default::default()
        });

        // 2. Breadcrumb / Navigation Bar
        let active_tab = self.tabs.get(self.active_tab_index);
        let tab_title = active_tab.map_or("Untitled", |t| t.title.as_str());

        let nav_back = button(text("←").size(12).color(Color::from_rgb8(140, 149, 159)))
            .style(button::subtle)
            .padding([2, 6]);

        let nav_forward = button(text("→").size(12).color(Color::from_rgb8(140, 149, 159)))
            .style(button::subtle)
            .padding([2, 6]);

        let breadcrumb_text = text(tab_title)
            .size(12)
            .color(Color::from_rgb8(101, 109, 118));

        let mode_toggle_btn = button(
            text(if self.is_reading_mode {
                "📖 Reading View"
            } else {
                "✏️ Live Preview"
            })
            .size(11)
            .color(Color::from_rgb8(9, 105, 218)),
        )
        .style(button::subtle)
        .padding([2, 8])
        .on_press(Message::ToggleReadingMode);

        let breadcrumb_bar = container(
            row![
                row![nav_back, nav_forward]
                    .spacing(2)
                    .align_y(alignment::Vertical::Center),
                breadcrumb_text,
                space().width(Length::Fill),
                mode_toggle_btn,
            ]
            .width(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .spacing(10),
        )
        .width(Length::Fill)
        .padding([4, 16])
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(255, 255, 255))),
            border: Border::default()
                .color(Color::from_rgb8(225, 228, 232))
                .width(0.5),
            ..Default::default()
        });

        // 3. Document Lines (Live Preview with raw markdown on line click)
        let mut lines_column: iced::widget::Column<iced::Element<'_, Message>> =
            iced::widget::Column::new().spacing(3).width(820);

        if let Some(doc) = active_tab {
            for (idx, line) in doc.lines.iter().enumerate() {
                if doc.active_line == Some(idx) && !self.is_reading_mode {
                    // Active line: RAW MARKDOWN text editor!
                    let active_input = text_editor(&doc.active_editor)
                        .font(Font::MONOSPACE)
                        .on_action(Message::EditorAction)
                        .padding([2, 4])
                        .key_binding(|event| {
                            if event.key
                                == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                            {
                                Some(text_editor::Binding::Custom(Message::DeselectLine))
                            } else if event.key
                                == iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                                && !event.modifiers.shift()
                            {
                                Some(text_editor::Binding::Custom(Message::InsertLineBelow))
                            } else {
                                text_editor::Binding::from_key_press(event)
                            }
                        })
                        .style(|_theme, _status| text_editor::Style {
                            background: Background::Color(Color::TRANSPARENT),
                            border: Border::default(),
                            placeholder: Color::from_rgb8(140, 149, 159),
                            value: Color::from_rgb8(31, 35, 40),
                            selection: Color::from_rgba8(9, 105, 218, 0.25),
                        });

                    let active_line_widget = container(
                        row![
                            text("v")
                                .size(11)
                                .color(Color::from_rgb8(140, 149, 159)),
                            active_input,
                        ]
                        .spacing(6)
                        .align_y(alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba8(9, 105, 218, 0.06))),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    });

                    lines_column = lines_column.push(active_line_widget.boxed());
                } else if line.raw.trim().is_empty() {
                    // Empty line: clickable space for cursor placement
                    let empty_line = mouse_area(space().width(Length::Fill).height(18))
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(empty_line.boxed());
                } else {
                    // Inactive line: formatted Markdown rendering
                    let rendered = markdown::view(
                        &line.items,
                        markdown::Settings::default(),
                        theme_val.clone(),
                    )
                    .map(Message::LinkClicked);

                    let line_container = container(rendered)
                        .width(Length::Fill)
                        .padding([1, 4]);

                    let interactive_line = mouse_area(line_container)
                        .interaction(mouse::Interaction::Text)
                        .on_press(Message::SelectLine(idx));

                    lines_column = lines_column.push(interactive_line.boxed());
                }
            }
        }

        // Empty bottom area: clicking here continues writing at the end of the document
        let bottom_spacer = mouse_area(
            container(space().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(160),
        )
        .interaction(mouse::Interaction::Text)
        .on_press(Message::ClickBottomArea);

        lines_column = lines_column.push(bottom_spacer.boxed());

        let centered_doc = container(lines_column)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(Padding {
                top: 24.0,
                right: 32.0,
                bottom: 32.0,
                left: 32.0,
            });

        let main_doc_scroll = scrollable(centered_doc)
            .width(Length::Fill)
            .height(Length::Fill);

        // 4. Status Bar
        let (word_count, char_count) = if let Some(doc) = active_tab {
            (doc.word_count(), doc.char_count())
        } else {
            (0, 0)
        };

        let status_bar = container(
            row![
                text("0 backlinks")
                    .size(11)
                    .color(Color::from_rgb8(101, 109, 118)),
                space().width(Length::Fill),
                text(format!("{} words", word_count))
                    .size(11)
                    .color(Color::from_rgb8(101, 109, 118)),
                text("•")
                    .size(11)
                    .color(Color::from_rgb8(175, 184, 193)),
                text(format!("{} characters", char_count))
                    .size(11)
                    .color(Color::from_rgb8(101, 109, 118)),
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .padding([4, 16])
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(246, 248, 250))),
            border: Border::default()
                .color(Color::from_rgb8(208, 215, 222))
                .width(1.0),
            ..Default::default()
        });

        column![
            top_tab_bar,
            breadcrumb_bar,
            main_doc_scroll,
            status_bar,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    }
}

impl Default for Doggy {
    fn default() -> Self {
        Self::new()
    }
}

fn theme(_state: &Doggy) -> Theme {
    custom_theme()
}

fn style(_state: &Doggy, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::from_rgb8(255, 255, 255), // GitHub Light canvas (#ffffff)
        text_color: Color::from_rgb8(31, 35, 40),          // Primary text (#1f2328)
    }
}

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt::try_init();

    iced::application(Doggy::default, Doggy::update, Doggy::view)
        .title("Doggy")
        .theme(theme)
        .style(style)
        .window(iced::window::Settings {
            size: Size::new(960.0, 720.0),
            min_size: Some(Size::new(500.0, 400.0)),
            ..Default::default()
        })
        .run()
}
