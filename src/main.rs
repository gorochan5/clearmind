#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;

use iced::{Size, window};

fn main() -> iced::Result {
    core::init_logging();

    let cfg = match core::config::load_config_from_path(&core::config::config_file_path()) {
        core::config::LoadConfigResult::Loaded { config, .. } => config,
        _ => core::config::ClearMindConfig::default(),
    };

    let window_settings = window::Settings {
        size: Size::new(
            cfg.session.window.width as f32,
            cfg.session.window.height as f32,
        ),
        min_size: Some(Size::new(640.0, 420.0)),
        decorations: false,
        transparent: false,
        ..Default::default()
    };

    iced::application(
        core::ClearMind::boot,
        core::ClearMind::update,
        core::ClearMind::view,
    )
    .title("ClearMind")
    .subscription(core::ClearMind::subscription)
    .theme(core::ClearMind::theme)
    .style(core::ClearMind::style)
    .window(window_settings)
    .fonts([core::icons::FONT_BYTES])
    .run()
}
