use super::models::{CustomColorField, ThemeConfig, ThemeMessage};
use super::presets::default_theme_definition;

pub fn update(config: &mut ThemeConfig, message: ThemeMessage) {
    match message {
        ThemeMessage::SelectTheme(id) => {
            config.active_theme = id;
        }
        ThemeMessage::SetThemeIcon(icon) => {
            if let Some(def) = config.themes.get_mut(&config.active_theme) {
                def.theme_icon = icon;
            }
        }
        ThemeMessage::SetCustomHex { field, value } => {
            if let Some(def) = config.themes.get_mut(&config.active_theme) {
                match field {
                    CustomColorField::Background => def.background = value,
                    CustomColorField::Surface => def.surface = value,
                    CustomColorField::Text => def.text = value,
                    CustomColorField::Accent => def.accent = value,
                }
            }
        }
        ThemeMessage::ResetActiveTheme => {
            let default_def =
                default_theme_definition(&config.active_theme).unwrap_or_default();
            config.themes.insert(config.active_theme.clone(), default_def);
        }
    }
}
