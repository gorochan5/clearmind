use iced::Widget;
use iced::advanced::text::Highlighter;
use iced::widget::markdown;
use iced::{Code, Element, Theme};

use super::models::ThemePalette;

pub struct PaletteViewer<'a, M> {
    pub palette: &'a ThemePalette,
    pub theme: &'a Theme,
    pub highlighter: &'a dyn Highlighter<Code, Theme>,
    pub on_link_click: fn(markdown::Uri) -> M,
}

impl<'a, M> PaletteViewer<'a, M> {
    pub fn new(
        palette: &'a ThemePalette,
        theme: &'a Theme,
        highlighter: &'a dyn Highlighter<Code, Theme>,
        on_link_click: fn(markdown::Uri) -> M,
    ) -> Self {
        Self {
            palette,
            theme,
            highlighter,
            on_link_click,
        }
    }
}

impl<'a, M: 'a> markdown::Viewer<'a, M> for PaletteViewer<'a, M> {
    fn theme(&self) -> &Theme {
        self.theme
    }

    fn highlighter(&self) -> &dyn Highlighter<Code, Theme> {
        self.highlighter
    }

    fn on_link_click(url: markdown::Uri) -> M {
        let _ = url;
        unreachable!("Use PaletteViewer::view_with or pass on_link_click directly")
    }
}

impl<'a, M: 'a> PaletteViewer<'a, M> {
    pub fn view(
        &self,
        items: &'a [markdown::Item],
        settings: markdown::Settings,
    ) -> Element<'a, M> {
        markdown::view_with(items, settings, self).boxed()
    }

    pub fn link_message(&self, url: markdown::Uri) -> M {
        (self.on_link_click)(url)
    }
}
