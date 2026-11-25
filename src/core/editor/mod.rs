//! Document editor module: models, custom live-preview widget, smart editing, links, rich copy, and widgets.

pub mod links;
pub mod live_widget;
pub mod models;
pub mod rich_copy;
pub mod smart_edit;
pub mod update;
pub mod view;
pub mod widgets;

pub use models::{ClosedTabStack, SearchState, Tab};
pub use update::EditorMessage;
