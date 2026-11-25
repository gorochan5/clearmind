#![allow(dead_code)]

pub mod config;
pub mod custom_commands;
pub mod editor;
pub mod fonts;
pub mod icons;
pub mod keybindings;
pub mod margin;
pub mod markdown;
pub mod models;
pub mod navbar;
pub mod print;
pub mod themes;
pub mod update;
pub mod view;
pub mod watcher;
pub mod widgets;
pub mod word_wrap;

pub use models::ClearMind;

pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}
