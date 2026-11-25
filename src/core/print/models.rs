//! Print preview options, state, and messages.

use std::path::PathBuf;

use iced::widget::svg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaperSize {
    #[default]
    A4,
    Letter,
}

impl PaperSize {
    pub const ALL: [PaperSize; 2] = [PaperSize::A4, PaperSize::Letter];

    pub fn typst_name(self) -> &'static str {
        match self {
            PaperSize::A4 => "a4",
            PaperSize::Letter => "us-letter",
        }
    }

    /// Aspect ratio (height / width) in portrait orientation.
    pub fn portrait_aspect_ratio(self) -> f32 {
        match self {
            PaperSize::A4 => 297.0 / 210.0,
            PaperSize::Letter => 279.4 / 215.9,
        }
    }
}

impl std::fmt::Display for PaperSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaperSize::A4 => write!(f, "A4"),
            PaperSize::Letter => write!(f, "US Letter"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageOrientation {
    #[default]
    Portrait,
    Landscape,
}

impl PageOrientation {
    pub const ALL: [PageOrientation; 2] = [PageOrientation::Portrait, PageOrientation::Landscape];
}

impl std::fmt::Display for PageOrientation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PageOrientation::Portrait => write!(f, "Portrait"),
            PageOrientation::Landscape => write!(f, "Landscape"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarginPreset {
    Compact,
    #[default]
    Normal,
    Wide,
}

impl MarginPreset {
    pub const ALL: [MarginPreset; 3] = [
        MarginPreset::Compact,
        MarginPreset::Normal,
        MarginPreset::Wide,
    ];

    pub fn typst_mm(self) -> &'static str {
        match self {
            MarginPreset::Compact => "14mm",
            MarginPreset::Normal => "22mm",
            MarginPreset::Wide => "30mm",
        }
    }
}

impl std::fmt::Display for MarginPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarginPreset::Compact => write!(f, "Compact"),
            MarginPreset::Normal => write!(f, "Normal"),
            MarginPreset::Wide => write!(f, "Wide"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintThemeMode {
    #[default]
    CleanPaper,
    ActiveTheme,
}

impl PrintThemeMode {
    pub const ALL: [PrintThemeMode; 2] = [PrintThemeMode::CleanPaper, PrintThemeMode::ActiveTheme];
}

impl std::fmt::Display for PrintThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrintThemeMode::CleanPaper => write!(f, "Clean Paper"),
            PrintThemeMode::ActiveTheme => write!(f, "Current Theme"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintSettings {
    pub paper: PaperSize,
    pub orientation: PageOrientation,
    pub margin: MarginPreset,
    pub theme_mode: PrintThemeMode,
    pub show_page_numbers: bool,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper: PaperSize::A4,
            orientation: PageOrientation::Portrait,
            margin: MarginPreset::Normal,
            theme_mode: PrintThemeMode::CleanPaper,
            show_page_numbers: true,
        }
    }
}

impl PrintSettings {
    /// Height / width aspect ratio taking orientation into account.
    pub fn page_aspect_ratio(self) -> f32 {
        let ratio = self.paper.portrait_aspect_ratio();
        match self.orientation {
            PageOrientation::Portrait => ratio,
            PageOrientation::Landscape => 1.0 / ratio,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledPrintDoc {
    pub generation: u64,
    pub page_svgs: Vec<Vec<u8>>,
    pub pdf_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct PrintPreviewState {
    pub is_open: bool,
    pub doc_title: String,
    pub settings: PrintSettings,
    pub generation: u64,
    pub is_compiling: bool,
    pub is_exporting: bool,
    pub page_handles: Vec<svg::Handle>,
    pub pdf_bytes: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PrintMessage {
    Close,
    SetPaper(PaperSize),
    SetOrientation(PageOrientation),
    SetMargin(MarginPreset),
    SetThemeMode(PrintThemeMode),
    TogglePageNumbers,
    Compiled(Result<CompiledPrintDoc, (u64, String)>),
    ConfirmExportPdf,
    PdfExported(Option<(PathBuf, Result<(), String>)>),
}
