//! Top navbar module: scrollable tab strip, 6 quick-access controls, drag area, settings, and window controls.

pub mod models;
pub mod update;
pub mod view;
pub mod widgets;

pub use models::NavbarState;
pub use update::NavbarMessage;
