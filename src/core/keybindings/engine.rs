use super::models::{CommandId, KeybindingsConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput<'a> {
    pub key: &'a str,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub text: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityLevel {
    ModalCapture,
    QuickPickOrPalette,
    SlashIntellisense,
    FindReplace,
    GlobalShortcut,
    EditorText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterContext {
    pub recording_shortcut: bool,
    pub modal_open: bool,
    pub popover_open: bool,
    pub quick_pick_open: bool,
    pub slash_open: bool,
    pub find_replace_open: bool,
    pub has_multi_cursors: bool,
    pub has_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeTarget {
    CloseRecorder,
    CloseModal,
    ClosePopover,
    CloseQuickPick,
    CloseSlashIntellisense,
    CloseFindReplace,
    CollapseMultiCursors,
    ClearSelection,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyDispatch {
    TypedText(String),
    Command(CommandId),
    Escape(EscapeTarget),
    Unhandled,
}

pub fn is_windows_altgr_text(input: &KeyInput<'_>) -> bool {
    input.ctrl && input.alt && !input.shift && input.text.is_some_and(|t| !t.is_empty())
}

pub fn format_chord(input: &KeyInput<'_>) -> String {
    let mut parts = Vec::new();
    if input.ctrl {
        parts.push("Ctrl");
    }
    if input.alt {
        parts.push("Alt");
    }
    if input.shift {
        parts.push("Shift");
    }
    parts.push(input.key);
    parts.join("+")
}

pub fn resolve_escape_cascade(ctx: &RouterContext) -> EscapeTarget {
    if ctx.recording_shortcut {
        EscapeTarget::CloseRecorder
    } else if ctx.modal_open {
        EscapeTarget::CloseModal
    } else if ctx.popover_open {
        EscapeTarget::ClosePopover
    } else if ctx.quick_pick_open {
        EscapeTarget::CloseQuickPick
    } else if ctx.slash_open {
        EscapeTarget::CloseSlashIntellisense
    } else if ctx.find_replace_open {
        EscapeTarget::CloseFindReplace
    } else if ctx.has_multi_cursors {
        EscapeTarget::CollapseMultiCursors
    } else if ctx.has_selection {
        EscapeTarget::ClearSelection
    } else {
        EscapeTarget::None
    }
}

pub fn active_priority_level(ctx: &RouterContext) -> PriorityLevel {
    if ctx.recording_shortcut || ctx.modal_open {
        PriorityLevel::ModalCapture
    } else if ctx.quick_pick_open {
        PriorityLevel::QuickPickOrPalette
    } else if ctx.slash_open {
        PriorityLevel::SlashIntellisense
    } else if ctx.find_replace_open {
        PriorityLevel::FindReplace
    } else {
        PriorityLevel::GlobalShortcut
    }
}

pub fn route_key_event(
    config: &KeybindingsConfig,
    ctx: &RouterContext,
    input: &KeyInput<'_>,
) -> KeyDispatch {
    if input.key.eq_ignore_ascii_case("Escape") && !input.ctrl && !input.alt && !input.shift {
        return KeyDispatch::Escape(resolve_escape_cascade(ctx));
    }

    if is_windows_altgr_text(input) {
        return KeyDispatch::TypedText(input.text.unwrap_or_default().to_string());
    }

    let chord = format_chord(input);
    for &cmd in CommandId::all() {
        let bound = config.shortcut_for(cmd);
        if !bound.is_empty() && bound.eq_ignore_ascii_case(&chord) {
            return KeyDispatch::Command(cmd);
        }
    }

    if let Some(txt) = input.text
        && !txt.is_empty()
        && !input.ctrl
        && !input.alt
    {
        return KeyDispatch::TypedText(txt.to_string());
    }

    KeyDispatch::Unhandled
}

pub fn find_conflicts(
    config: &KeybindingsConfig,
    target_cmd: CommandId,
    chord: &str,
) -> Vec<CommandId> {
    if chord.trim().is_empty() {
        return Vec::new();
    }
    CommandId::all()
        .iter()
        .copied()
        .filter(|&cmd| {
            cmd != target_cmd && config.shortcut_for(cmd).eq_ignore_ascii_case(chord.trim())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altgr_produces_typed_text_not_shortcut() {
        let cfg = KeybindingsConfig::default();
        let ctx = RouterContext {
            recording_shortcut: false,
            modal_open: false,
            popover_open: false,
            quick_pick_open: false,
            slash_open: false,
            find_replace_open: false,
            has_multi_cursors: false,
            has_selection: false,
        };
        let input = KeyInput {
            key: "Q",
            ctrl: true,
            alt: true,
            shift: false,
            text: Some("/"),
        };
        assert_eq!(
            route_key_event(&cfg, &ctx, &input),
            KeyDispatch::TypedText("/".to_string())
        );
    }

    #[test]
    fn escape_cascade_follows_8_step_priority() {
        let mut ctx = RouterContext {
            recording_shortcut: true,
            modal_open: true,
            popover_open: true,
            quick_pick_open: true,
            slash_open: true,
            find_replace_open: true,
            has_multi_cursors: true,
            has_selection: true,
        };
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::CloseRecorder);
        ctx.recording_shortcut = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::CloseModal);
        ctx.modal_open = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::ClosePopover);
        ctx.popover_open = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::CloseQuickPick);
        ctx.quick_pick_open = false;
        assert_eq!(
            resolve_escape_cascade(&ctx),
            EscapeTarget::CloseSlashIntellisense
        );
        ctx.slash_open = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::CloseFindReplace);
        ctx.find_replace_open = false;
        assert_eq!(
            resolve_escape_cascade(&ctx),
            EscapeTarget::CollapseMultiCursors
        );
        ctx.has_multi_cursors = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::ClearSelection);
        ctx.has_selection = false;
        assert_eq!(resolve_escape_cascade(&ctx), EscapeTarget::None);
    }

    #[test]
    fn conflict_detection_finds_duplicate_bindings() {
        let cfg = KeybindingsConfig::default();
        let conflicts = find_conflicts(&cfg, CommandId::FileNewTab, "Ctrl+S");
        assert_eq!(conflicts, vec![CommandId::FileSave]);
    }
}
