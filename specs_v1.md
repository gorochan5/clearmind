# ClearMind Specifications v1.0.0

> **Status:** Finalized Architecture, UX & Defect-Resolution Specification  
> **Target Stack:** Rust (Edition 2024) · `iced` `0.15.0-dev` (`markdown`, `highlighter`, `advanced`, `image`, `svg`) · `serde` + `toml` · `rfd` · `reqwest` · `tylax` + `typst` · `merman` · `ropey` · `unicode-segmentation` · `ignore` · `infer` · `opener` · `arboard` · `pulldown-cmark` · `notify` · `tokio`  
> **Platform:** Windows first. v1.0.0 is built and verified on Windows 10/11 only; OS-specific paths sit behind a small platform layer so Linux/macOS can be added later without restructuring.  
> **Supersedes:** `docs/specification_v1.md` (obsolete 2-pane split-editor draft)  
> **Reference Benchmark:** `reference/REFERENCES.md` (`Malgel`, `Scribe`, `Moeka`, `MarkText`, `Zettlr`, `Ghostwriter`, `Fresh`, `Zed`, `VS Code`, `Lapce`, `Helix`, `cosmic-text`)

---

## 1. Vision & Core UX Philosophy

ClearMind is a fast, distraction-free, **single-pane live-preview Markdown viewer and editor** built with Rust and Iced.

### Core UX Principles
1. **Pure Visualization Mode by Default (`active_line = None`):**
   - Opening a file (or restoring a session/recovery cache on startup) always starts with `active_line = None` so **100% of the document is rendered** without a single unrendered raw line breaking the reading experience.
   - In Visualization Mode, `Up`, `Down`, `PageUp`, and `PageDown` scroll the document viewport smoothly for reading. Clicking a rendered hyperlink or linked badge follows the link directly.
   - Clicking any non-link content on a line (or pressing `Enter`) enters line-edit mode on that line; pressing `Escape` returns to Pure Visualization Mode (`active_line = None`).
2. **Unified Custom `LiveMarkdownEditor` Widget (No Two-Layer Hotswapping):**
   - Instead of hotswapping between `iced::widget::markdown::view` and a separate 1-line `TextEditor`, ClearMind implements a custom Iced widget (`LiveMarkdownEditor`, built on `iced::advanced::Widget` + `cosmic-text` `Paragraph` layout).
   - It renders formatted Markdown across the entire document and reveals raw syntax on `active_line` using the user's configured `body_family` (for prose/headings/lists/quotes) and `code_family` (for code blocks/tables/frontmatter) at matching font size and `line_height`—giving **zero vertical layout jump**, native **`Paragraph::hit_test` click-to-caret**, **cross-line 2D selection & mouse drag**, and **inline `$...$` math / badge rendering**.
   - **Code Blocks:** Only the clicked `active_line` is edited raw in-place while surrounding lines remain syntax-highlighted inside the unified code block container (preserving multi-line `ParseState`).
   - **Tables, Mermaid Diagrams, & LaTeX Math Blocks:** Raw lines are edited while keeping a live rendered preview visible above/below (or surrounding table rows intact).

---

## 2. Project Structure & Architecture

### 2.1 Directory Layout

`src/main.rs` serves strictly as the application entrypoint (tracing initialization, font/window bootstrap, and `iced::application(...)` launch). **90%+ of the codebase lives in `src/core/`**, and **each of the 6 settings is its own independent module** (`models.rs`, `view.rs`, `update.rs`) corresponding to its navbar quick-access button and `_settings_` modal section.

```text
src/
├── main.rs                         # Entrypoint: logging, iced::application boot
└── core/                           # Core application & domain modules
    ├── mod.rs                      # Public module exports & ClearMind root wiring
    ├── models.rs                   # Root ClearMind state, Message enum, AppConfig
    ├── update.rs                   # Root update() dispatcher & cross-module routing
    ├── view.rs                     # Root view() shell (Stack: main layout + popovers + modals)
    ├── icons.rs                    # Global Lucide icon library (generated from lucide.ttf)
    │
    ├── widgets/                    # Cross-module reusable UI components (used by 2+ modules)
    │   ├── mod.rs
    │   ├── button.rs               # Shared styled buttons (primary, icon, toggle, danger)
    │   ├── modal.rs                # Backdrop + centered opaque modal card container
    │   ├── popover.rs              # Navbar-anchored floating popover (via selector::find bounds)
    │   ├── prompt.rs               # Save/Discard/Cancel & text input prompt widget
    │   ├── context_menu.rs         # Right-click menus (editor text & tabs)
    │   ├── toast.rs                # Bottom-right notification stack (info/warning/error)
    │   ├── stepper.rs              # Numeric "- / input / +" stepper with blur/Enter commit
    │   ├── quick_pick.rs           # Centered searchable overlay list (command palette, Go to Heading)
    │   └── dropdown/               # Stateful searchable dropdown (mini-module pattern)
    │       ├── mod.rs
    │       ├── models.rs
    │       ├── view.rs
    │       └── update.rs
    │
    ├── navbar/                     # Top window chrome: tabs, quick access, settings, window controls
    │   ├── mod.rs
    │   ├── models.rs
    │   ├── view.rs
    │   ├── update.rs
    │   └── widgets/
    │       ├── tab_strip.rs        # Scrollable/overflow-safe document tab bar
    │       └── quick_access.rs     # 6 quick-access icon buttons + Container::id anchors
    │
    ├── editor/                     # Custom LiveMarkdownEditor widget, tabs, undo/redo, multi-cursor
    │   ├── mod.rs
    │   ├── models.rs               # Tab (ropey buffer + line index), DocumentSelection (2D), Transaction undo, SearchState
    │   ├── live_widget.rs          # Custom iced::advanced::Widget + cosmic-text Paragraph hit-testing
    │   ├── view.rs                 # Document canvas, LiveMarkdownEditor host, status bar
    │   ├── update.rs               # Navigation, smart edits, 2D selection, undo/redo, search/replace
    │   ├── smart_edit.rs           # List/quote continuation, 6-rule inline toggles, table tabbing
    │   ├── links.rs                # Link & local file reference resolver/opener (§6.7)
    │   ├── rich_copy.rs            # Markdown → HTML clipboard export (§6.9)
    │   └── widgets/
    │       ├── badge.rs            # Shields.io badge renderer (cached remote SVG + pill fallback)
    │       ├── callout.rs          # Continuous blockquote & GitHub Alert ([!NOTE] etc.) card
    │       ├── code_block.rs       # Unified code container, language header, Copy button
    │       ├── diagram_block.rs    # Mermaid & LaTeX SVG renderer + live edit preview
    │       ├── frontmatter.rs      # YAML frontmatter metadata card
    │       ├── image_card.rs       # Local/remote raster/SVG image renderer + fallback card
    │       ├── table_grid.rs       # Escape-aware GFM table grid & live row/cell editor
    │       ├── conflict_banner.rs  # External file change warning bar
    │       ├── search_bar.rs       # Find & replace bar
    │       ├── sticky_headings.rs  # Pinned heading path at viewport top (§6.9)
    │       └── zoom_viewer.rs      # Full-window pan/zoom for diagrams, math & images (§6.9)
    │
    ├── markdown/                   # Pure Markdown block/inline parsers & SVG compilers
    │   ├── mod.rs
    │   ├── block_scanner.rs        # Cohesive multi-line scanner (fences, tables, quotes, lists, refs)
    │   ├── code_block.rs           # Fenced code block & unclosed fence detector
    │   ├── table.rs                # Escape- and code-span-aware GFM table parser & formatter
    │   ├── list.rs                 # Ordered/unordered/task list parser & auto-renumbering
    │   ├── callout.rs              # Blockquote & GitHub Alert ([!NOTE]..[!CAUTION]) parser
    │   ├── math.rs                 # Inline ($...$) & block ($$...$$) tylax + typst -> SVG compiler
    │   ├── mermaid.rs              # Mermaid (```mermaid) merman (ResvgSafe) -> SVG compiler
    │   ├── badge.rs                # Shields.io URL & block/inline parser
    │   └── image.rs                # Obsidian ![[...]], Markdown ![alt](url), HTML <img> parser
    │
    ├── watcher/                    # Filesystem watcher (notify + 75ms debounce stream)
    │   └── mod.rs
    │
    ├── config/                     # Persistent clearmind.toml + hot-exit recovery + _settings_ modal
    │   ├── mod.rs
    │   ├── models.rs               # ClearMindConfig, SessionState, TOML schema, clamping
    │   ├── storage.rs              # Atomic TOML I/O (.bak on corrupt) + recovery/<tab>.json cache
    │   ├── view.rs                 # _settings_ modal layout (sidebar + active module view)
    │   └── update.rs
    │
    ├── fonts/                      # Setting Module 1: Font selector & typography (_type_)
    │   ├── mod.rs
    │   ├── models.rs               # FontConfig, FontPreset (Sans/Serif/Mono), async font::list()
    │   ├── view.rs                 # Quick-popover view & full settings section view
    │   └── update.rs
    │
    ├── word_wrap/                  # Setting Module 2: Word wrap toggler (_text-wrap_)
    │   ├── mod.rs
    │   ├── models.rs               # WordWrapConfig (enabled, code_block_wrap)
    │   ├── view.rs                 # Full settings section view (navbar icon is 1-click toggle)
    │   └── update.rs
    │
    ├── margin/                     # Setting Module 3: Centered margin vs full width (_minimize_/_maximize_)
    │   ├── mod.rs
    │   ├── models.rs               # MarginConfig (full_width: bool, content_max_width: f32)
    │   ├── view.rs                 # Full settings section view (navbar icon is 1-click toggle)
    │   └── update.rs
    │
    ├── keybindings/                # Setting Module 4: Keybindings (_keyboard_) & shortcut engine
    │   ├── mod.rs
    │   ├── models.rs               # CommandId, KeyChord, KeymapConfig (overrides only)
    │   ├── engine.rs               # Modal priority router + Escape cascade + shortcut mapper
    │   ├── view.rs                 # Quick-popover cheat-sheet/search & full recorder table view
    │   └── update.rs
    │
    ├── custom_commands/            # Setting Module 5: Slash Intellisense & user macros (_terminal_)
    │   ├── mod.rs
    │   ├── models.rs               # SlashCommand, MacroConfig, SlashMenuState, inline args parser
    │   ├── builtins.rs             # Prose commands (/code, /table, ...) & Math LaTeX functions (\frac, ...)
    │   ├── variables.rs            # {{cursor}}, {{date}}, {{time}}, {{datetime}}, {{filename}}, async {{clipboard}}
    │   ├── view.rs                 # Editor '/' Intellisense popup, navbar quick-popover, & modal CRUD view
    │   └── update.rs
    │
    └── themes/                     # Setting Module 6: Themes (_palette_) + centralized color tokens
        ├── mod.rs
        ├── models.rs               # Appearance (System/Light/Dark), ThemeId, ThemePalette, CustomOverrides
        ├── presets.rs              # Built-in Light/Dark palettes (GitHub Light/Dark, etc.)
        ├── highlighter.rs          # Syntax highlighter parameterized by ThemePalette & seeded ParseState
        ├── viewer.rs               # Styling tokens for rendered Markdown elements
        ├── view.rs                 # Navbar quick-popover swatch picker & full modal theme grid
        └── update.rs
```

### 2.2 Architectural Rules & Design Patterns

1. **Fractal TEA Pattern (`models.rs` / `view.rs` / `update.rs`):**
   - Every module owns its `models.rs`, `view.rs`, and `update.rs`, plus an optional `widgets/` folder.
   - `view.rs` functions are **pure** (`&State -> Element<'_, ModuleMessage>`), mapped to root `Message` via `.map(Message::Module)`. No Markdown parsing, file I/O, or inline key-binding closures inside `view.rs`.
   - Each of the 4 popover-enabled setting modules (`fonts`, `keybindings`, `custom_commands`, `themes`) exposes both `view_quick_popover(&self)` and `view_modal_section(&self)`.
2. **The Widget Promotion Rule:**
   - A UI component starts in `<module>/widgets/` when used by only one module, and promotes to `src/core/widgets/` once used by 2+ modules.
3. **Centralized Input Priority & `Escape` Cascade (`keybindings/engine.rs`):**
   - Priority chain: `(1)` Key-Capture Recorder → `(2)` Active Modal / Prompt → `(3)` Active Navbar Quick-Popover → `(4)` Slash `/` Intellisense Menu → `(5)` Focused Find & Replace Input → `(6)` Document Editor / Visualization Viewport.
   - **`Escape` Cascade Order:** Pressing `Escape` dismisses the highest-priority active layer:
     1. Key-capture recorder
     2. Active modal (`_settings_`), quick pick (command palette / Go to Heading), zoom viewer, or prompt (`AfterConfirm`)
     3. Active navbar quick-popover
     4. Slash `/` Intellisense menu
     5. Extra multi-cursors (`extra_cursors.clear()`)
     6. Active selection (`DocumentSelection` collapses to caret)
     7. Find & Replace bar (`search_open = false`, **even if unfocused**)
     8. Exit edit mode (`active_line = None` → Pure Visualization Mode)
4. **Centralized Palette Tokens:**
   - Hardcoded `Color::from_rgb8(...)` literals are prohibited outside `src/core/themes/presets.rs`.

### 2.3 Document Buffer & Undo Model

- **Storage:** each tab holds a `ropey::Rope` plus a cached line index. Blocks (code fences, tables, quotes, lists) are derived data: after an edit, `block_scanner` re-scans only the dirty line range, widened to the enclosing block boundaries.
- **Line endings & encoding:** text is normalized to `\n` in memory. The original line ending (`LF` / `CRLF`) and UTF-8 BOM are stored on the tab and restored on save.
- **Operation-based undo** (replaces the current full-document `UndoSnapshot` clones, `src/main.rs:1129`):
  ```rust
  struct Transaction {
      edits: Vec<Edit>,                 // Edit { range, deleted: String, inserted: String }
      selection_before: DocumentSelection,
      selection_after: DocumentSelection,
      extra_cursors_before: Vec<Cursor>,
      extra_cursors_after: Vec<Cursor>,
      active_line_before: Option<usize>,
      active_line_after: Option<usize>,
      scroll_before: f32,
  }
  ```
  Undo applies the inverse edits and restores selection, cursors, `active_line` and scroll. History is capped at 1,000 transactions or 32 MB (oldest dropped first).
- **Coalescing:** consecutive single-character inserts/deletes on the same line within `750ms` merge into one transaction. A new transaction starts on a word boundary, newline, caret jump, paste, formatting command, multi-cursor edit, or save.
- **Atomic multi-line operations:** Replace All, `Tab`/`Shift+Tab` over a selection, move lines, table formatting, and slash-command/macro insertion are each a single transaction.

### 2.4 `LiveMarkdownEditor` Widget Contract

Because the custom widget replaces `TextEditor`, it must provide everything `TextEditor` used to give for free:

- **IME & text input:** handle input-method events (`iced_core::input_method`, see `docs/iced/src/iced_core/input_method.rs.html`). Enable IME while editing, draw the preedit (composition) text underlined at the caret, and report the caret rectangle so the Windows candidate window appears next to the caret. A commit inserts text as a normal edit. Dead keys on ABNT2 layouts (`´` + `a` = `á`, `~` + `a` = `ã`) and the Windows emoji picker (`Win+.`) must work. Verify the exact API against `docs/iced` before implementing.
- **Grapheme-correct editing:** caret movement, `Backspace`, `Delete` and selection step by grapheme cluster (`unicode-segmentation`), so emoji ZWJ sequences and combining marks behave as one character.
- **Viewport virtualization:** lay out and draw only the blocks inside the viewport ± one screen height. Per-block heights are cached; when an off-screen block's height changes (async SVG/image load, re-wrap), the scroll anchor is preserved so the visible content doesn't jump (Moeka/Malgel pattern). Fixes the eager rendering of every line in `src/main.rs:3225`.
- **Mouse gestures:** single click places the caret (§6.1 I-1), double-click selects a word, triple-click selects the line, drag selects across lines with auto-scroll past the viewport edges, `Shift+Click` extends the selection, and right-click opens `core/widgets/context_menu.rs` (Cut / Copy / Paste / Bold / Italic / Link / Select All).
- **Caret:** blinks at 530ms, stays solid while typing, and is hidden in Pure Visualization Mode (`active_line = None`).

---

## 3. Configuration, Session & Hot-Exit Recovery (`clearmind.toml`)

### 3.1 File Locations
- **Config File:** `%APPDATA%\clearmind\clearmind.toml` (Windows), `~/Library/Application Support/clearmind/clearmind.toml` (macOS), `$XDG_CONFIG_HOME/clearmind/clearmind.toml` (Linux).
- **Hot-Exit Buffer Recovery Cache:** `%APPDATA%\clearmind\recovery\<tab_uuid>.json` (sibling `recovery/` folder in the same app-data directory).
- **Logs:** `%APPDATA%\clearmind\logs\clearmind.log`, rotated (5 files × 2 MB) via `tracing-subscriber`. The panic hook writes here too.

### 3.2 Resilience, Corrupt Backup & Hot-Exit Rules
- **Partial/Missing Configs:** All structs derive `#[serde(default)]` with numeric bounds clamping (`font_size`, `code_size`, `line_height`, `content_max_width`).
- **Corrupt `clearmind.toml` Protection:** If `clearmind.toml` fails TOML parsing on startup, back up the broken file to `clearmind.toml.bak`, write clean defaults, and surface a non-blocking warning notice.
- **Hot-Exit & Session Restoration:**
  - `[session]` in `clearmind.toml` stores `restore_session` (default `true`), `open_files`, and `active_file`.
  - On `CloseWindow` (requiring `iced::window::Settings { exit_on_close_request: false, .. }` + `window::close_requests()`), all dirty and untitled buffers are automatically flushed to `recovery/<tab_uuid>.json` without blocking the user with save prompts (VS Code-style hot exit).
  - **Crash safety (same cache, written earlier):** each dirty tab's recovery file is also written after 2s of idle typing, and a `std::panic::set_hook` handler flushes all dirty tabs and logs the panic. A crash or power loss therefore restores exactly like a normal close.
  - **Restore conflict:** if the file on disk changed after the recovery snapshot was taken, the recovered buffer opens as dirty and `conflict_banner` offers **Keep Recovered** / **Load Disk Version**.
  - A tab's recovery file is deleted on successful save, on Discard, or when its content matches the file on disk again.
- **Config versioning:** `clearmind.toml` carries `config_version = 1` so future releases can migrate old configs.
- **Live reload:** the existing `notify` watcher also watches `clearmind.toml`. When it's edited outside ClearMind:
  - Valid file: the new settings apply immediately and a toast says "Settings reloaded".
  - Invalid file: the current settings stay active in memory, an error toast shows the TOML line/column, and the file is **not** overwritten or backed up (the `.bak` rule applies only at startup).
  - ClearMind ignores change events caused by its own saves (compares the content hash of the last write).
  - On launch (both with and without CLI file arguments), ClearMind restores any cached dirty/untitled buffers from `recovery/` (marked `is_dirty = true`, `active_line = None`) alongside `session.open_files` (skipping missing disk paths with a notice) and any newly passed CLI files.
  - Explicit `CloseTab` (`Ctrl+W` / `×`) on a dirty tab still prompts `AfterConfirm` (**Save** / **Don't Save** / **Cancel**), clearing the recovery file on Save or Discard.

### 3.3 `clearmind.toml` Schema

```toml
config_version = 1

[session]
restore_session = true
open_files = ["D:/notes/roadmap.md", "D:/notes/ideas.md"]
active_file = "D:/notes/roadmap.md"
recent_files = ["D:/notes/roadmap.md", "D:/notes/ideas.md"]   # most recent first, max 10
window = { width = 960, height = 720, x = 120, y = 80, maximized = false }

[view]
zoom = 1.0                       # clamped: 0.5..=2.0, step 0.1
sticky_headings = true
code_line_numbers = false
focus_mode = false
typewriter_mode = false

[writing_targets]
# Per-document word targets, keyed by file path
"D:/notes/novel.md" = 2000

[fonts]
preset = "sans"                  # "sans" | "serif" | "mono" | "custom"
body_family = "Segoe UI"
code_family = "Cascadia Mono"
font_size = 16.0                 # clamped: 12.0..=28.0
code_size = 14.0                 # clamped: 10.0..=24.0
line_height = 1.5                # clamped: 1.2..=2.0

[word_wrap]
enabled = true

[margin]
full_width = false               # false = centered margin mode, true = full window width
content_max_width = 820.0        # clamped: 560.0..=1400.0 (used when full_width = false)

[themes]
appearance = "system"            # "system" | "light" | "dark"
light_theme = "github-light"
dark_theme = "github-dark"

[themes.custom]
# Optional user hex overrides applied on top of the active theme preset
# background = "#ffffff"
# surface = "#f6f8fa"
# text = "#1f2328"
# accent = "#0969da"

[keybindings]
# Only overrides from defaults; empty string "" = unbound
"edit.move_line_up" = "Alt+Up"
"view.toggle_word_wrap" = "Alt+Z"

[[commands]]
trigger = "meeting"
label = "Meeting Notes"
description = "Insert timestamped meeting template"
template = "# Meeting Notes — {{date}} ({{time}})\n**File:** {{filename}}\n\n- {{cursor}}"
```

---

## 4. Navbar, Quick Access Popovers & Config Modal (`_settings_`)

### 4.1 Navbar Layout (Height: `36px`, borderless window)

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ [📄 Untitled.md ×] [+] [📂] │ [T] [↩] [⤢] [⌨] [>_] [🎨] │ ░░░░ drag area ░░░░ [⚙] [─ □ ×] │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
  └─ Left: Scrollable Tabs ─┘   └─ Center: Quick Access ─┘   └─ Right: Drag + Config + Win ─┘
```

| Zone | Controls | Behavior |
| :--- | :--- | :--- |
| **Left** | Scrollable tab strip + `+` (New) + `folder-open` (Open) | Horizontal `scrollable` with max tab width and ellipsis truncation so opening many tabs never crushes `drag_area` or `window_controls`. Shows dirty (`•`) and conflict (`⚠`) indicators. |
| **Center (Quick Access)** | 6 Lucide icon buttons (`type`, `text-wrap`, `minimize`/`maximize`, `keyboard`, `terminal`, `palette`) | Instant access to each of the 6 settings modules. |
| **Right** | Reserved drag area (min-width `80px`), **Config (`settings`)**, Window controls (`minus`, `square`, `x`) | `settings` sits immediately to the left of the window controls. Dragging moves window; double-click toggles maximize. |

### 4.2 Quick Access Interaction Model (1-Click Toggles + Anchored Popovers)

- **2 Instant 1-Click Toggles:**
  - **`text-wrap` (`word_wrap/`):** Immediately toggles `word_wrap.enabled`.
  - **`minimize` / `maximize` (`margin/`):** Immediately toggles `margin.full_width` (`maximize` when centered, `minimize` when full-width).
- **4 Anchored Floating Popovers (`core/widgets/popover.rs`):**
  - Each of the 4 buttons (`type`, `keyboard`, `terminal`, `palette`) is wrapped in `Container::id(btn_id)`. On click, `iced::widget::selector::find(selector::id(btn_id))` resolves the button's exact `Rectangle` bounds so the popover renders inside `Stack` + `Float`/`Pin` anchored directly beneath the icon (clamped to viewport bounds):
    1. **`type` (`fonts/`):** Quick `Sans` / `Serif` / `Mono` presets, searchable body/code font picker, `font_size` stepper.
    2. **`keyboard` (`keybindings/`):** Search filter + scrollable shortcut list with 1-click inline re-recording.
    3. **`terminal` (`custom_commands/`):** Filterable `/` commands & macros + quick `"+ Add Macro"` form.
    4. **`palette` (`themes/`):** `System` / `Light` / `Dark` toggle + compact clickable theme swatch grid.

### 4.3 Main Config Modal (`_settings_`)
- Clicking `settings` or pressing `Ctrl+,` opens the centered Config Modal (`core/widgets/modal.rs` via `Stack` + `opaque` backdrop) with left sidebar for all 6 modules and full advanced controls on the right.

### 4.4 Borderless Window Chrome (Windows)
- The window uses `decorations: false` (`src/main.rs:4365`) but today **cannot be resized from its edges**. v1.0.0 adds invisible 6px resize edges and 12px corners that call `iced::window::drag_resize(id, Direction)` and show matching resize cursors (verify the exact signature against `docs/iced`).
- Double-click on the drag area toggles maximize. Minimum window size stays `500×400`, and window controls remain reachable at that size.
- **Window state persistence:** size, position and maximized state are saved in `[session].window` and restored on launch, clamped to the visible monitors.

### 4.5 Notifications (Toasts)
- `core/widgets/toast.rs`: bottom-right stack, `info | warning | error` levels, auto-dismiss after 4s (errors stay until dismissed), optional action button.
- Used for: corrupt `clearmind.toml` backed up to `.bak`, session files missing on restore, recovered buffers restored, remote image/badge fetch failures, "File not found" on local references, and save errors.

### 4.6 Status Bar
Left to right: mode pill (`Viewing` / `Editing`), file path, save state (`Saved` / `Modified` / `Unsaved`), `Ln, Col`, selection size, word count + reading time (click for the stats popover, §6.9.8), word-target progress (when a target is set), unclosed-fence warning (when present, §6.9.4), focus/typewriter indicators (when on), zoom (`100%`, click to reset), line ending (`LF` / `CRLF`, click to convert), encoding (`UTF-8` / `UTF-8 BOM`).

### 4.7 Tab & Mouse Ergonomics
- Middle-click closes a tab; tabs can be reordered by dragging.
- Tab context menu: Close / Close Others / Close to the Right / Copy Path / Reveal in Explorer.
- `Ctrl+Wheel` zooms the document (`50%..=200%`, step 10%), saved as `[view].zoom`.

### 4.8 Quick Navigation
- **Reopen closed tab (`Ctrl+Shift+T`):** ClearMind keeps an in-memory stack of the last 20 closed tabs (path, caret line, scroll position). Reopening restores them in reverse order. Untitled tabs closed with **Don't Save** aren't kept.
- **Open Recent:** the navbar `folder-open` button gets a dropdown arrow listing the last 10 opened files (`[session].recent_files`, most recent first). Missing files are greyed out and removed when clicked, with a toast.
- **Go to Heading (`Ctrl+Shift+O`):** opens `core/widgets/quick_pick.rs` with the active document's headings (indented by level). Typing filters them; `Enter` scrolls to the heading **without** leaving Pure Visualization Mode.
- **Command palette (`Ctrl+Shift+P`):** opens the same quick pick with every command from §7 (name, category, current shortcut). `Enter` runs the selected command. Commands that don't make sense in the current state (e.g. Save with no tabs) are shown disabled.
- Both quick picks close on `Esc` or a click outside, and sit at priority 2 of the input chain (§2.2.3).

---

## 5. Detailed Specification of the 6 Configuration Modules

### 5.1 Fonts & Typography (`src/core/fonts/` — `_type_`)
- **Async System Font Discovery:** Runs `iced::font::list()` at startup, handling the initial loading state and mapping `Family::Name` to `String`.
- **Role Separation:** `body_family` for prose/headings/lists/quotes/tables; `code_family` for code blocks and frontmatter. Both use matching `font_size` / `line_height` inside `LiveMarkdownEditor` for zero-jump editing.
- **Steppers:** `font_size` (`12..=28`, default `16`), `code_size` (`10..=24`, default `14`), `line_height` (`1.2..=2.0`, default `1.5`).

### 5.2 Word Wrap (`src/core/word_wrap/` — `_text-wrap_`)
- Controls `Wrapping::Word` vs `Wrapping::None` across `LiveMarkdownEditor` and code blocks.

### 5.3 Margin & Layout Width (`src/core/margin/` — `_minimize_` / `_maximize_`)
- **Centered Margin (`full_width = false`):** Clamps document column to `content_max_width` (`560..=1400px`, default `820px`) via `Responsive` / tracked window width inside `container(...).width(Fill).center_x(Fill)`.
- **Full Width (`full_width = true`):** Spans `Length::Fill` with `16px` horizontal padding.

### 5.4 Keybindings (`src/core/keybindings/` — `_keyboard_`)
- Categorized, searchable table (`File`, `Edit`, `Format`, `View`, `Navigation`) with **Record**, **Unbind**, **Reset**, and **Reset All**.
- Key-capture recorder ignores lone modifiers, cancels on unmodified `Esc`, and runs conflict detection before committing.

### 5.5 Custom Commands & Context-Aware `/` Intellisense (`src/core/custom_commands/` — `_terminal_`)
- **Context-Aware Trigger Rules:**
  - **Suppressed (Disabled):** Inside fenced code blocks, inline code spans (`` `...` ``), and URLs (`https://...`, `http://...`, file paths).
  - **Prose Mode:** Opens at line start or after whitespace; supports optional inline arguments before `Enter`/`Tab` (`/code rust`, `/table 3x4`, `/callout warning`). Keeps open across a space **only** when the token before the space matches an argument-taking command; closes on 0 matches.
  - **Math Mode (inside `$...$` or `$$...$$`):** Automatically switches the `/` Intellisense list to **LaTeX functions** (`\frac{{}}{{}}`, `\sum_{}^{}`, `\int_{}^{}`, `\sqrt{}`, `\begin{matrix}...\end{matrix}`, Greek symbols `\alpha`, `\beta`, `\theta`, etc.)!
- **Built-in Prose Commands:** `/code [lang]`, `/table [RxC]`, `/latex`, `/mermaid [type]`, `/task`, `/callout [kind]`, `/badge`.
- **User Macros & Async Variables:** `[[commands]]` in `clearmind.toml` support multi-line templates with `{{cursor}}`, `{{date}}`, `{{time}}`, `{{datetime}}`, `{{filename}}`, and async `{{clipboard}}` (via `iced::clipboard::read` `Task` with stale-edit guard if tab/line changes).
- **Keyboard vs. Mouse Arbitration:** Keyboard navigation (`Up`/`Down`/`Enter`/`Tab`/`Esc`) locks out hover highlight changes until the mouse physically moves (`MarkText` pattern).

### 5.6 Themes (`src/core/themes/` — `_palette_`)
- `Appearance` (`System`, `Light`, `Dark`) with separate `light_theme` and `dark_theme` slots.
- Built-in presets (`GitHub Light`, `Catppuccin Latte`, `Solarized Light`, `GitHub Dark`, `Catppuccin Mocha`, `Tokyo Night`, `Gruvbox Dark`) + optional `[themes.custom]` hex overrides.

---

## 6. Comprehensive Defect Resolution & Reference Alignment (`src/main.rs` vs. `reference/REFERENCES.md`)

### 6.1 Editor Interaction & Selection Fixes

| # | Current Defect in `src/main.rs` | v1.0.0 Required Fix (Reference Pattern) |
| :--- | :--- | :--- |
| **I-1** | **Clicking a rendered line always places caret at column 0 (`1484-1495`, `2124-2131`) & link clicks conflict with line selection (`974`, `3462`):** | **Direct Link Following + `Paragraph::hit_test` Click-to-Caret (`Malgel`, `cosmic-text`):**<br>• Clicking a rendered link or linked badge follows it directly (`https://` in browser, relative `.md` in tab, `#anchor` scrolls).<br>• Clicking any non-link region hit-tests `(x, y)` via `LiveMarkdownEditor`'s `Paragraph` + `DisplayMap` (rendered↔raw offset map) and places the caret at the exact clicked character. |
| **I-2** | **Clicking a collapsed Code Block (`3287`), Table (`3409`), or Badge Block (`3462`) jumps to a hardcoded line:** | **Per-Line / Per-Cell Click Targets (`Malgel`, `Scribe`):** Clicking inside a code block, table row/cell, or badge block activates the exact clicked line/cell. |
| **I-3** | **`Ctrl+A` (`SelectAllMarkdown`, `2718-2729`) destroys `doc.lines`:** Collapses the document into a 1-element `vec![Line::new(&full)]`. | **Non-Destructive Multi-Step `Ctrl+A`:** Never collapse `doc.lines`. In edit mode, first `Ctrl+A` selects the active line (or current code block/table cell); second `Ctrl+A` (or `Ctrl+A` in Visualization Mode) selects the full document `DocumentSelection` `(0, 0)..(last_line, last_len)`. In Visualization Mode, `Ctrl+C` copies the selection while destructive edits require edit mode. |
| **I-4** | **Dual, incompatible selection models (`selected_lines` vs `active_editor` selection, `1798`, `2884`, `3527`):** | **Unified 2D `DocumentSelection { anchor: (line, col), head: (line, col) }` (`cosmic-text`, `Moeka`):** Supports intra-line, cross-line `Shift+Arrow`, `Shift+Click`, mouse drag selection across lines (with edge auto-scroll), and `Backspace`/`Delete`/`Tab`/`Alt+Up/Down`/`Ctrl+D` over the 2D range. Never clears anchor on `ModifiersChanged`. |
| **I-5** | **Multi-cursor (`extra_cursors`) shifts layout with `"│"` (`3675`), lost on `Ctrl+Z` (`1129`), 4 undo steps on `Tab` (`3562`), cannot shrink (`2743`), `Escape` exits editing (`2132`):** | **Multi-Cursor Integrity (`Helix`, `Moeka`):** Render secondary cursors as zero-width overlays inside `LiveMarkdownEditor`; store `extra_cursors` in each undo `Transaction` (§2.3); process multi-cursor edits in descending `(line, col)` order and run `dedup_cursors()` after every motion/edit; shrink stack on opposite `Ctrl+Alt+Arrow`; follow the 8-step `Escape` cascade (`§2.2.3`). |
| **I-6** | **No auto-scroll on keyboard navigation (`1912`) & inaccurate linear jump on search (`2998`):** | **Minimal-Margin Auto-Scroll & Viewport Anchoring (`Malgel`, `Moeka`):** Track cumulative block heights and `scrollable::Viewport`; scroll minimally (1-line margin) only when `active_line` or active search match falls outside the viewport. |

### 6.2 Keyboard Commands & Smart Editing Ergonomics Fixes

| # | Current Defect in `src/main.rs` | v1.0.0 Required Fix (Reference Pattern) |
| :--- | :--- | :--- |
| **K-1** | **Inconsistent Visualization vs. Edit state (`1206`, `1988`, `2116`):** `boot()` calls dead `focus(ACTIVE_EDITOR_ID)` while `active_line == None`. | **First-Class Pure Visualization Mode (`active_line = None`):** Always open files with `active_line = None`. `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End` scroll the viewport; `Enter` or line click enters edit mode. |
| **K-2** | **`search_open == true` disables `Ctrl+Z/Y/A/D` (`4232`), while `Alt+Up/Down` mutates doc from search box (`4189`):** | **Focus-Aware Shortcut Routing (`Fresh`):** Gate shortcuts by whether search/replace input actually holds focus (`§2.2.3`). |
| **K-3** | **Broken cursor navigation (`1912`, `3492`):** `Left`/`Right` don't wrap across lines; `Up`/`Down` loses column; `Ctrl+Home/End` & `PageUp/PageDown` trapped in 1 line. | **Cross-Line Motions & Pixel `goal_x: Option<f32>` (`cosmic-text`, `Helix`, `Malgel`):** Wrap `Left` at col 0 and `Right` at line end; step `Up`/`Down` by visual wrapped rows preserving pixel `goal_x` + `Affinity`; route `Ctrl+Home`/`Ctrl+End` and `PageUp`/`PageDown` across the full document. |
| **K-4** | **Dumb `Enter`, `Tab`, `Shift+Tab`, `Home`, and `Backspace` (`2222`, `3553`):** | **Smart Markdown Editing (`Malgel`, `Moeka`, `Zettlr`, `Scribe`):**<br>• **`Enter`:** Continue indent, bullet, ordered (`N+1.` + auto-renumber), task (`- [ ]`), and quote (`> `) prefixes; outdent empty nested items; clear empty top-level markers. Inside tables, insert a new empty row below (or exit on an empty row).<br>• **`Tab` / `Shift+Tab`:** Never lose editor focus. Indent/outdent lists (+ renumber), jump table cells (+ append row on `Tab` at last cell), indent/outdent multi-line selections, or insert 4 spaces on plain lines.<br>• **Smart `Home` (`SoftHome`):** Toggle between start of content (after indent/prefix) and col 0.<br>• **Smart `Backspace`:** Delete full list/task/quote prefix in 1 step at prefix end; delete both chars inside empty auto-paired delimiters. |
| **K-5** | **Missing Markdown formatting & app shortcuts (`4181-4245`):** | **6-Rule Smart Inline & Block Formatting (`Malgel`, `Zettlr`, `Ghostwriter`):** Add `Ctrl+B`, `Ctrl+I`, `Ctrl+E`, `Ctrl+Shift+X`, `Ctrl+K`, `Ctrl+1..6`, `Ctrl+0`, `Ctrl+Shift+.`, `Ctrl+L`, `Ctrl+Alt+T`. |
| **K-6** | **Clipboard & Undo flaws (`1310`, `2471-2557`):** | **Smart Clipboard & Undo Coalescing (`Malgel`, `Moeka`):** Place caret at end of paste; Smart URL Paste (`[selection](url)`); copy/cut whole line when selection is empty; coalesce single-char typing (`750ms` + word boundaries) and restore `(active_line, selection, extra_cursors, scroll_offset)` on Undo/Redo. |

### 6.3 Markdown Parsing & Live-Preview Rendering Fixes

| # | Current Defect in `src/main.rs` | v1.0.0 Required Fix (Reference Pattern) |
| :--- | :--- | :--- |
| **M-1** | **Per-line isolated `markdown::parse(trimmed)` breaks ordered lists, nested bullets, tab alignment (`8px` vs `24px`), reference links, Setext headings, & frontmatter (`1060`, `3882`):** | **Contextual Block & List Scanner (`MarkText`, `Scribe`, `Malgel`):** Unify tab width (`1 tab = 4 spaces = 24px`), track ordered list sequence & depth-styled bullets (`•`, `◦`, `▪`), maintain document-wide `ReferenceDefinitionIndex`, and support Setext headings & YAML frontmatter cards. |
| **M-2** | **Disconnected Blockquotes & broken multi-line GitHub Alerts (`3233`, `3882`):** | **Continuous Blockquote & GitHub Alert Containers (`Scribe`, `Malgel`):** Render consecutive `>` lines with a continuous quote bar/fill, and `> [!NOTE]`..`[!CAUTION]` as themed callout cards. |
| **M-3** | **Code blocks lose syntax highlighting & shift layout when clicked (`3262`, `3707`):** | **Seeded In-Place Highlighted Code Blocks (`Scribe`, `Moeka`):** Keep the unified code block container (language badge + Copy button) intact when editing inside it; seed `active_line`'s highlighter with `(ParseState, ScopeStack)` from `block_start + 1 .. active_line` and re-highlight lines below live. |
| **M-4** | **Table parsing & explosion bugs (`455`, `3305`, `3759`) + No visual LaTeX/Mermaid rendering:** | **Escape-Aware Tables & Visual SVG LaTeX/Mermaid (`Malgel`, `Scribe`):**<br>• **Tables:** Escape/code-span-aware pipe splitter, no `Length::Fixed` vertical clipping, bold headers, surrounding rows kept as grid while editing.<br>• **LaTeX & Mermaid:** Pure-Rust `tylax` + `typst` (`$...$` inline with baseline alignment + `$$...$$` display) and `merman` (`ResvgSafe` + `viewBox` sizing) rendered to `iced::widget::svg`, with live preview kept visible during editing and diagnostic fallback on syntax error. |
| **M-5** | **Dead links (`2872`), placeholder-only images (`985`), unstyled checked tasks (`3796`), dropped inline badges (`3842`):** | **Rich Media, Direct Links & Tasks (`Malgel`, `Scribe`):** Direct click follows links (`https://`, `#anchor`, and local files per §6.7); `reqwest` async HTTP fetch + memory cache for remote images/Shields.io badges + local raster/SVG rendering; strikethrough + muted text on checked tasks; inline badges rendered inside `LiveMarkdownEditor` lines. |

### 6.4 File, Tab Lifecycle & Search Fixes

| # | Current Defect in `src/main.rs` | v1.0.0 Required Fix (Reference Pattern) |
| :--- | :--- | :--- |
| **F-1** | **Untitled tabs cannot be saved (`1737`), no Open File UI (`1942`), no session/crash recovery:** | **Native `rfd::AsyncFileDialog` + Hot-Exit Recovery (`Malgel`, `Moeka`):** `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` via `rfd::AsyncFileDialog`, `FileDropped`, pristine untitled tab reuse, atomic BOM/CRLF save, and hot-exit `recovery/` caching on `CloseWindow` (auto-restored on next launch). |
| **F-2** | **Closing dirty tabs (`CloseTab`, `2079`) discards unsaved work without warning:** | **Stateful `AfterConfirm` Prompt on `CloseTab` (`Malgel`, `VS Code`):** Explicit `Ctrl+W` / `×` on a dirty tab shows Save / Don't Save / Cancel (`core/widgets/prompt.rs`) and clears the recovery cache on Save/Discard. |
| **F-3** | **Search only tints whole lines (`3648`), omits tables/code blocks (`3282`), doesn't select match (`2996`):** | **Substring Search Selection & Universal Block Highlight:** Select `char_col..(char_col + char_len)` on active match navigation and highlight matches inside tables, code blocks, and callouts. Toggles: `Aa` case-sensitive, `ab` whole word, `.*` regex. Match counter (`3 / 17`); `Enter` / `Shift+Enter` = next / previous. Search runs over the raw Markdown source; Replace All is one undo transaction. |
| **F-4** | **No clear rules for external changes or failed saves:** the current conflict banner (`3974-4014`) offers Overwrite / Cancel / Reload / Keep but makes no distinction between clean and dirty tabs, deleted files, or save errors. | **Smart External Change Handling (`VS Code`):**<br>• **Clean tab** (no unsaved edits): reload silently from disk, keeping the caret line and scroll position.<br>• **Dirty tab:** show `conflict_banner` (**Reload from Disk** / **Keep My Changes**).<br>• **File deleted or moved on disk:** keep the content, mark the tab title `(deleted)`, and let Save recreate the file.<br>• **Save fails** (read-only, permission denied, disk full): error toast with a **Save As…** action; the tab stays dirty and its recovery file is kept. |

### 6.5 Markdown Support Matrix

| Feature | v1.0.0 Status |
| :--- | :--- |
| CommonMark core: ATX & Setext headings, paragraphs, emphasis, code spans, links, images, lists, blockquotes, thematic breaks, hard breaks | Rendered |
| GFM: tables, task lists, strikethrough, autolinks | Rendered |
| GitHub Alerts (`> [!NOTE]` … `> [!CAUTION]`) | Rendered (callout cards) |
| Footnotes (`[^1]` + `[^1]: …`) | Rendered (superscript reference + footnote list at document end) |
| Highlight (`==text==`) | Rendered |
| Math (`$...$`, `$$...$$`) & Mermaid (```` ```mermaid ````) | Rendered as SVG (`tylax` + `typst`, `merman`) |
| YAML frontmatter | Rendered (metadata card) |
| Shields.io badges | Rendered (remote SVG via `reqwest`, pill fallback) |
| Wiki-links `[[note]]` and embeds `![[file]]` | Rendered (resolved per §6.7) |
| Raw HTML `<img>`, `<br>`, `<kbd>`, `<sub>`, `<sup>`, `<details>`/`<summary>` | Rendered |
| Any other raw HTML | Shown as literal text |
| Emoji shortcodes (`:smile:`) | Not supported in v1.0.0 (shown as typed) |

### 6.6 Security & Resource Limits

- **Remote media (`reqwest`):** always enabled (no toggle). `http`/`https` only, 10s timeout, 10 MB max body, 64 MB LRU memory cache, fetched off the UI thread.
- **Typst:** compiles in a sandboxed `World` with no filesystem or network access beyond bundled fonts; the Typst world initializes lazily on the first math block.
- **Mermaid:** `merman` with a 10s deadline.
- Math and Mermaid compile off the UI thread and are cached by content hash.
- **Local files:** never executed unless they are a known image or document type (§6.7).

### 6.7 Local File References

Markdown often points at other files: `[spec](./docs/api.md)`, `![diagram](assets/flow.png)`, `![[report.pdf]]`, `[[meeting notes]]`.

- **Resolution (`ignore` crate, the walker used by ripgrep):**
  1. Relative paths resolve against the current document's folder.
  2. Absolute paths and `file://` URLs are used as given.
  3. Obsidian-style `[[name]]` / `![[name]]` without a folder are looked up by file name under the document's root folder (nearest `.git` / `.obsidian` folder, otherwise the document's folder), respecting `.gitignore`. The lookup runs off the UI thread and builds a name→path index that the existing `notify` watcher keeps up to date.
- **Type detection:** the file extension picks the category from the allowlists below. `infer` also reads the file's magic bytes: a file that is really an executable or script under an allowed extension (for example `invoice.pdf` that is actually an `.exe`) is treated as **Other**.
- **Rendering & click behaviour (allowlist, safe by default):**

  | Referenced file | Rendering | Click |
  | :--- | :--- | :--- |
  | **Image**: png, jpg, jpeg, gif, webp, bmp, ico, tiff, svg | Embeds (`![](…)`, `![[…]]`) render inline (`iced` `image` / `svg`) | Embedded image: enters edit mode on that line. Plain link to an image: opens in the default Windows viewer (`opener::open`) |
  | **Markdown**: md, markdown | Link text | Opens in a ClearMind tab (switches to it if already open); `#heading` anchors scroll there |
  | **Document**: pdf, txt, rtf, doc, docx, odt, xls, xlsx, ods, csv, ppt, pptx, odp, epub | Link text; `![[file]]` shows a file chip (icon, name, size) | Opens with the default Windows app (`opener::open`) |
  | **Other** (any other extension, incl. exe, bat, cmd, ps1, sh, vbs, msi, lnk, js, zip, …) | Link text / file chip | **Never executed.** Opens Explorer with the file selected (`explorer /select,<path>` via `opener::reveal`) |
  | **Folder** | Link text | Opens the folder in Explorer |
  | **Missing file** | Red dashed underline | Toast "File not found: <path>" |

- `Ctrl+Click` on any local reference shows it in Explorer.

### 6.8 Pasting & Dropping Images

- **Paste (`Ctrl+V`) with image data on the clipboard** (e.g. a screenshot): ClearMind saves it as PNG to an `assets/` folder next to the document, named `<document-stem>-<YYYYMMDD-HHMMSS>.png`, and inserts `![](assets/<file>.png)` at the caret. Check `docs/iced` for clipboard image support first; if iced can't read clipboard images, use `arboard` (already a dependency for rich-text copy, §6.9.7).
- **Drop an image file onto the document:** if the image already lives inside the document's folder tree, insert a relative link to it where it is; otherwise copy it into `assets/` first (adding `-1`, `-2`… on name clashes). The link goes on a new line below the line under the pointer, and that line enters edit mode.
- **Untitled documents** have no folder yet, so pasting or dropping an image first opens Save As. Cancelling Save As cancels the insert.
- Dropping a `.md` file still opens it in a tab (§6.4 F-1). Dropping any other file type does nothing except a toast.
- Each paste or drop is a single undo transaction. Undo removes the link but leaves the saved file in `assets/`.

### 6.9 Editing & Reading Enhancements (from reference projects)

1. **Auto-pair delimiters (`moeka` `editor/handlers.rs`):**
   - Typing `(`, `[`, `{`, `"`, `` ` `` or `$` inserts the matching closer with the caret between them. Typing `**` or `~~` inserts `****` / `~~~~` with the caret in the middle.
   - With a selection, typing an opener wraps the selection instead (`(selection)`, `**selection**`).
   - Typing a closer when the next character is that same closer steps over it.
   - `Backspace` inside an empty pair removes both characters (K-4). One `Ctrl+Z` undoes the pairing.
   - Exceptions: `'` never pairs (apostrophes); `"` doesn't pair right after a letter or digit; `$` doesn't pair before a digit (currency like `$5`). Inside fenced code blocks only brackets and quotes pair.
2. **Sticky headings (VS Code `stickyScroll`):** when the current section's heading scrolls out of view, a thin bar pinned to the top of the document viewport shows the heading path (`H1 › H2 › H3`, up to 3 levels, truncated with ellipsis). Clicking a segment jumps to that heading. Works in both Viewing and Editing modes. Setting: `[view] sticky_headings`.
3. **Zoom viewer (`Malgel` `diagram_viewer.rs`):** hovering a rendered Mermaid diagram, display-math block or image shows a small zoom button in its top-right corner. Clicking it, or double-clicking the element, opens a full-window overlay:
   - `Ctrl+Wheel` / `+` / `-` zoom, drag to pan, `0` fits to window, `Esc` closes.
   - SVG content is re-rendered at the current zoom so it stays sharp.
4. **Unclosed fence detector (`Scribe` `UnclosedFence`):** if a ```` ``` ````, `~~~` or `$$` opening fence has no closing fence (so it would swallow the rest of the document), the status bar shows "Unclosed code block at line N" with **Go to line** and **Close it** actions. **Close it** inserts the matching closing fence after the last non-blank line of that block's first paragraph (one undo transaction). The warning only appears when the caret isn't on the opening fence line, so it doesn't flash while you type. Also available in the command palette.
5. **Interactive table controls (`Malgel` `live_editor.rs`):** while the caret is inside a table (Editing mode only, so reading is never disturbed):
   - `+` buttons on the table's right and bottom edges add a column or row.
   - Hovering a row or column header shows a `⋯` menu: Insert above/below/left/right, Move up/down/left/right, Delete, and (for columns) Align left/center/right.
   - Each action rewrites the Markdown table, re-aligned like `format.table_align`, as one undo transaction.
6. **Code block language picker & line numbers (`MarkText` muya):**
   - In Editing mode, clicking a code block's language badge opens a searchable list (`core/widgets/dropdown/`) of the languages the highlighter supports. Picking one rewrites the opening fence's info string (one undo transaction). Blocks without a language show a "Plain text" badge.
   - `[view] code_line_numbers = true` shows a muted, right-aligned line-number gutter in code blocks. Line numbers are never included by the Copy button.
7. **Copy as rich text (`Malgel`, `Ctrl+Alt+Shift+C`):** converts the selection (or the active line if nothing is selected) from Markdown to HTML with `pulldown-cmark` (tables, task lists, strikethrough enabled) and puts both HTML and the original Markdown on the clipboard via `arboard`. Pasting into Word, Outlook, Gmail, Teams or Slack keeps headings, bold, links, lists and tables. Styling is theme-independent (no dark backgrounds). In v1.0.0, math and Mermaid are copied as their source inside `<pre>` blocks; images use absolute `file:///` or remote URLs. Also in the editor context menu.
8. **Writing stats & targets (Ghostwriter session stats, Zettlr `writing-targets-store`):**
   - The status bar shows the word count plus a reading-time estimate (`~4 min read`, 230 words per minute).
   - Clicking it opens a small popover: words, characters (with and without spaces), paragraphs, reading time, and words written this session (since the tab was opened).
   - **Per-document word target:** set via the command palette ("Set Word Target…"), stored in `[writing_targets]` keyed by file path. When set, the status bar shows a progress bar (`1,240 / 2,000`), and a toast appears when the target is reached.
   - Counts use rendered text and exclude Markdown syntax, code blocks and frontmatter.
9. **Focus & typewriter modes (Ghostwriter):**
   - **Focus mode** (`[view] focus_mode`, `Ctrl+Shift+F`): in Editing mode, every block except the one containing the caret is drawn at 35% opacity. A list item or table row counts as one block. Viewing mode is unaffected.
   - **Typewriter mode** (`[view] typewriter_mode`, command palette): keeps the active line vertically centered while typing and navigating. It replaces the minimal-margin auto-scroll (I-6) and adds half a viewport of padding below the last line so the end of the document can reach the center.

---

## 7. Complete Keyboard Shortcuts Specification

All shortcuts below are registered in `src/core/keybindings/models.rs` and customizable via `clearmind.toml` (`[keybindings]`), the `_keyboard_` popover, and the `_settings_` Keybindings tab:

| Category | Command ID | Default Shortcut | Action |
| :--- | :--- | :--- | :--- |
| **File** | `file.new_tab` | `Ctrl+N` | Create new untitled tab |
| | `file.open` | `Ctrl+O` | Open file via `rfd::AsyncFileDialog` |
| | `file.save` | `Ctrl+S` | Save active tab (triggers Save As if untitled) |
| | `file.save_as` | `Ctrl+Shift+S` | Save active tab to a new path |
| | `file.close_tab` | `Ctrl+W` | Close active tab (prompts with `AfterConfirm` if dirty) |
| | `file.reopen_closed_tab` | `Ctrl+Shift+T` | Reopen the most recently closed tab |
| | `file.open_settings` | `Ctrl+,` | Toggle Config modal (`_settings_`) |
| **Edit** | `edit.undo` | `Ctrl+Z` | Undo last coalesced edit transaction |
| | `edit.redo` | `Ctrl+Y` / `Ctrl+Shift+Z` | Redo last undone transaction |
| | `edit.cut` | `Ctrl+X` | Cut selection (or entire current line if empty) |
| | `edit.copy` | `Ctrl+C` | Copy selection (or entire current line if empty) |
| | `edit.copy_rich_text` | `Ctrl+Alt+Shift+C` | Copy selection as rich text (HTML + Markdown), §6.9.7 |
| | `edit.paste` | `Ctrl+V` | Paste (smart `[selection](url)` if pasting URL over text) |
| | `edit.select_all` | `Ctrl+A` | Select line/block first, then full document (non-destructive) |
| | `edit.duplicate` | `Ctrl+D` | Duplicate selection or active line(s) |
| | `edit.move_line_up` | `Alt+Up` | Move active line or selection up |
| | `edit.move_line_down` | `Alt+Down` | Move active line or selection down |
| | `edit.add_cursor_above` | `Ctrl+Alt+Up` | Add multi-cursor above (or shrink bottom cursor) |
| | `edit.add_cursor_below` | `Ctrl+Alt+Down` | Add multi-cursor below (or shrink top cursor) |
| | `edit.find` | `Ctrl+F` | Open/focus Find bar |
| | `edit.replace` | `Ctrl+H` | Open/focus Find & Replace bar |
| **Format** | `format.bold` | `Ctrl+B` | 6-rule smart toggle Bold (`**`) |
| | `format.italic` | `Ctrl+I` | 6-rule smart toggle Italic (`*`) |
| | `format.strikethrough` | `Ctrl+Shift+X` | 6-rule smart toggle Strikethrough (`~~`) |
| | `format.inline_code` | `Ctrl+E` | 6-rule smart toggle Inline Code (`` ` ``) |
| | `format.link` | `Ctrl+K` | Wrap selection in `[text](url)` and select `url` |
| | `format.code_block` | `Ctrl+Shift+C` | Wrap/insert fenced code block |
| | `format.heading_1..6` | `Ctrl+1` .. `Ctrl+6` | Set/toggle Heading 1–6 (replaces existing `#` prefix) |
| | `format.paragraph` | `Ctrl+0` | Strip block prefix (return to plain paragraph) |
| | `format.blockquote` | `Ctrl+Shift+.` | Toggle blockquote prefix (`> `) |
| | `format.task_toggle` | `Ctrl+L` | Toggle task list item / checkbox `[ ]` ↔ `[x]` |
| | `format.table_align` | `Ctrl+Alt+T` | Format and align GFM table pipes |
| **View & Nav** | `view.toggle_word_wrap` | `Alt+Z` | Toggle word wrap (`_text-wrap_`) |
| | `view.toggle_full_width` | `Ctrl+Shift+M` | Toggle centered margin vs. full width (`_minimize_`/`_maximize_`) |
| | `view.toggle_focus_mode` | `Ctrl+Shift+F` | Toggle focus mode (§6.9.9) |
| | `view.toggle_typewriter_mode` | *(unbound; command palette)* | Toggle typewriter mode (§6.9.9) |
| | `view.zoom_in` | `Ctrl+=` / `Ctrl++` / `Ctrl+NumpadAdd` / `Ctrl+Wheel Up` | Zoom in (10% steps, max 200%) |
| | `view.zoom_out` | `Ctrl+-` / `Ctrl+NumpadSubtract` / `Ctrl+Wheel Down` | Zoom out (10% steps, min 50%) |
| | `view.zoom_reset` | *(unbound; click the zoom % in the status bar)* | Reset zoom to 100% |
| | `nav.next_tab` | `Ctrl+Tab` / `Ctrl+PgDn` | Switch to next open tab |
| | `nav.prev_tab` | `Ctrl+Shift+Tab` / `Ctrl+PgUp` | Switch to previous open tab |
| | `nav.go_to_heading` | `Ctrl+Shift+O` | Open Go to Heading quick pick |
| | `view.command_palette` | `Ctrl+Shift+P` | Open command palette |
| | `slash.trigger` | `/` (prose or math) | Open Slash-Command / LaTeX Intellisense (`_terminal_`) |

> [!IMPORTANT]
> **AltGr on Windows:** Windows reports AltGr as `Ctrl+Alt`. On ABNT2 keyboards AltGr types characters such as `/` (AltGr+Q), `?` (AltGr+W) and `₢` (AltGr+C). The keymap engine must treat any `Ctrl+Alt` key press that produces text (`KeyPress.text` is `Some`) as **typed text, not a shortcut**. That's why rich-text copy uses `Ctrl+Alt+Shift+C` instead of Malgel's `Ctrl+Alt+C`. The key-capture recorder (§5.4) must warn when you try to bind a `Ctrl+Alt` chord that produces a character.

---

## 8. Phased Implementation Roadmap

### Phase 0 — Prerequisites
- Regenerate `docs/iced/` from the upstream documentation build: `Cargo.lock` pins iced `c3d2cfd` while `docs/iced/README.md` was generated at `cd04b74` (AGENTS.md rule).
- Archive or delete the obsolete `docs/specification_v1.md`.

### Phase 1 — Structural Decomposition into `src/core/`
- Enable `iced` features `["markdown", "highlighter", "advanced", "image", "svg"]` and add `toml`, `rfd`, `reqwest`, `tylax`, `typst`, `merman`, `ropey`, `unicode-segmentation`, `ignore`, `infer`, `opener`, `arboard`, and `pulldown-cmark` to `Cargo.toml`.
- Extract `markdown/`, `watcher/`, `themes/`, `keybindings/`, `editor/`, `navbar/`, `core/widgets/`, and the 6 independent config modules (`fonts/`, `word_wrap/`, `margin/`, `keybindings/`, `custom_commands/`, `themes/`).
- Reduce `src/main.rs` to application entrypoint only.

### Phase 2 — Custom `LiveMarkdownEditor` Widget, Selection, Keyboard & File Lifecycle
- Build `LiveMarkdownEditor` (`iced::advanced::Widget` + `cosmic-text` `Paragraph`) with Pure Visualization Mode (`active_line = None`), `Paragraph::hit_test` click-to-caret, direct link following, unified 2D `DocumentSelection` + mouse drag, multi-cursor zero-width overlays + `dedup_cursors()`, pixel `goal_x` vertical navigation, and minimal-margin auto-scroll.
- Implement `Escape` cascade, smart `Enter`/`Tab`/`Shift+Tab`/`Home`/`Backspace`, 6-rule formatting shortcuts, smart clipboard + undo coalescing, `rfd::AsyncFileDialog` Open/Save/Save As, `AfterConfirm` on `CloseTab`, and hot-exit `recovery/` caching on `CloseWindow`.
- Implement the rope buffer + transaction undo (§2.3), the widget contract (§2.4: IME, graphemes, virtualization, mouse gestures, context menu), borderless resize edges + window state persistence (§4.4), crash-safe recovery (§3.2), toasts (§4.5), status bar (§4.6), and tab ergonomics + zoom (§4.7).
- Implement smart external change handling (F-4), reopen closed tab, and Open Recent (§4.8).
- Implement auto-pair delimiters, copy as rich text, writing stats & targets, and focus & typewriter modes (§6.9 items 1, 7, 8, 9).

### Phase 3 — Live-Preview Markdown, SVG & Media Pipeline
- Implement contextual `block_scanner.rs` (ordered lists, nested bullets, unified `24px` tabs, reference definitions, Setext headings, YAML frontmatter card, continuous blockquotes, GitHub Alert callouts).
- Implement seeded in-place highlighted code blocks, escape-aware tables with live row/cell editing, `tylax`+`typst` LaTeX SVG (`$...$` & `$$...$$`), `merman` (`ResvgSafe`) Mermaid SVG, and `reqwest`-cached remote images & Shields.io badges.
- Implement the Markdown support matrix (§6.5), security & resource limits (§6.6), and local file references with the safe-open allowlist (§6.7).
- Implement image paste & drop into `assets/` (§6.8).
- Implement sticky headings, the zoom viewer, the unclosed fence detector, interactive table controls, and the code block language picker + line numbers (§6.9 items 2–6).

### Phase 4 — Config Persistence, Navbar Popovers, `_settings_` Modal & `/` Intellisense
- Implement `config/` (`clearmind.toml` load/save, `.bak` corrupt backup, session restore, clamping).
- Wire the 2 instant navbar toggles, 4 `selector::find`-anchored popovers (`_type_`, `_keyboard_`, `_terminal_`, `_palette_`), and `_settings_` modal.
- Implement context-aware `/` Intellisense (prose commands + inline args, math-block LaTeX functions, user macros with `{{cursor}}` / `{{date}}` / `{{time}}` / `{{datetime}}` / `{{filename}}` / async `{{clipboard}}`).

---

## 9. Engineering Invariants & Quality Gates

- **Validation Discipline:** Every phase must pass `cargo check` and `cargo clippy` cleanly without warnings.
- **No Duplicated Logic:** Symmetric operations (`move_lines(Direction)`, `add_cursor(Direction)`, `cycle_tab(Direction)`) must be parameterized.
- **Pure Views:** `view.rs` modules never parse Markdown, perform I/O, embed key-binding business logic, or use raw `Color::from_rgb8(...)` literals.
- **Unit Test Coverage:** Required for `markdown/` (code fences, escape-aware tables, ordered list renumbering, GitHub alerts, reference definitions, LaTeX/Mermaid SVG compilation), `editor/smart_edit.rs`, `config/` (TOML round-trip, `.bak` corrupt recovery, clamping), `keybindings/` (conflict detection, `Escape` cascade), and `custom_commands/` (prose vs math trigger context, inline args, variable expansion, `{{cursor}}` placement), `editor/links.rs` (path resolution, allowlist categories, magic-byte override), and §2.3 undo (inverse edits, coalescing).
- **Who runs what:** tests are written as part of every phase but **run by the user**. Per the project rules, agents only run `cargo check` and `cargo clippy` (never `cargo build`, `cargo run`, or test runners).

---

## 10. Non-Goals for v1.0.0

Split panes, file tree sidebar, export (HTML/PDF), spell check, Vim mode, plugin system, real-time collaboration, git integration, localization (i18n), emoji shortcodes, Hemingway mode, Pomodoro timer, markup visibility modes (Scribe-style Hidden / Dim), single-instance IPC / Windows file association, Linux/macOS builds, auto-save, and non-UTF-8 encodings (a file that isn't valid UTF-8, or is binary, is refused with an error toast instead of being opened). These may be considered for later versions.

---

## 11. Performance Budgets & Definition of Done

### 11.1 Performance Budgets
| Metric | Budget |
| :--- | :--- |
| Cold start to first paint (1 MB file) | < 300 ms (Typst world initialized lazily) |
| Keystroke to repaint | < 16 ms on a 10,000-line document |
| Scrolling | 60 fps with viewport virtualization |
| Maximum supported file | 20 MB; above 5 MB a toast offers raw-only mode (no SVG/media rendering) |

### 11.2 Definition of Done per Phase
- **Phase 0:** `docs/iced/README.md` upstream commit matches `Cargo.lock`; obsolete spec archived.
- **Phase 1:** behaviour identical to the current build; `cargo check` and `cargo clippy` clean; `src/main.rs` under 40 lines; no single source file over ~800 lines.
- **Phase 2:** every I-*, K-* and F-* item in §6 checked manually; IME verified with ABNT2 dead keys and a Japanese IME; crash recovery verified (kill the process with dirty tabs, relaunch, content restored); window resizable from all edges and corners; §6.9 items 1 and 7–9 work; typing AltGr characters on ABNT2 (`/`, `?`) never triggers shortcuts.
- **Phase 3:** every M-* item, the §6.5 support matrix, and the §6.7 allowlist (a linked `.ps1`/`.exe` only opens Explorer) verified, plus §6.9 items 2–6.
- **Phase 4:** `clearmind.toml` round-trips (including `.bak` on a corrupt file); all navbar toggles, popovers and the `_settings_` modal work; `/` Intellisense works in prose and math contexts and is suppressed in code and URLs; command palette and Go to Heading work; editing `clearmind.toml` by hand reloads settings live.
