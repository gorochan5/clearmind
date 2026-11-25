pub mod highlighter;
pub mod models;
pub mod presets;
pub mod update;
pub mod view;
pub mod viewer;

pub use models::{ThemeConfig, ThemeMessage, ThemePalette};
pub use presets::resolve_palette;
