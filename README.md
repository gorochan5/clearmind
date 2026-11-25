# ClearMind

ClearMind is a desktop Markdown editor built in Rust with [iced](https://github.com/iced-rs/iced). It combines a live, rendered Markdown editing surface with keyboard-focused writing tools, local file tabs, and configurable reading and editor settings.

## Features

- Live Markdown preview while editing, with support for headings, lists, tables, task lists, blockquotes and callouts, links, images, and syntax-highlighted code blocks.
- Rendered LaTeX math and Mermaid diagrams.
- Context-aware suggestions: type `/` in prose for snippets such as `/table`, `/code`, `/callout`, and `/mermaid`; use `\` in math or press `Ctrl+Space` for completions.
- Multiple document tabs, recent files, session restore, undo and redo, find and replace, and external file change detection.
- Editor tools including multi-cursor editing, smart Markdown shortcuts, word wrap, focus and typewriter modes, adjustable content width, and word targets.
- Configurable fonts, themes, and keybindings, plus user-defined slash commands.
- Print preview and PDF export.

## Run

Install the Rust toolchain with Cargo, then run ClearMind from the repository:

```sh
cargo run
```

Open one or more files on startup by passing their paths:

```sh
cargo run -- notes.md draft.md
```

Build an optimized executable with:

```sh
cargo build --release
```

The executable is written under `target/release/` (`clearmind.exe` on Windows).

## Keyboard shortcuts

Shortcuts can be changed in **Settings → Keybindings**. Some defaults:

| Action | Shortcut |
| --- | --- |
| Open file | `Ctrl+O` |
| Save | `Ctrl+S` |
| Find / replace | `Ctrl+F` / `Ctrl+H` |
| Command palette | `Ctrl+Shift+P` |
| Open settings | `Ctrl+,` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |

## Settings and session data

ClearMind saves its settings and session state in `clearmind.toml` inside its application data directory:

- Windows: `%APPDATA%\clearmind\clearmind.toml`
- macOS: `~/Library/Application Support/clearmind/clearmind.toml`
- Linux and other Unix-like systems: `$XDG_CONFIG_HOME/clearmind/clearmind.toml`, or `~/.config/clearmind/clearmind.toml` when `XDG_CONFIG_HOME` is unset

Settings are also available in the app. Open them with `Ctrl+,` to adjust themes, typography, word wrap, layout width, keybindings, or custom commands.

## Development

ClearMind uses Rust edition 2024 and iced `0.15.0-dev`. The project keeps a pinned iced API documentation snapshot under [`docs/iced/`](docs/iced/); consult it before changing iced code. The snapshot is read-only and must not be edited by hand.

```sh
cargo check
```

To open a Markdown file from the development build, pass its path after `--`:

```sh
cargo run -- path/to/notes.md
```

## Project layout

- `src/main.rs` starts the iced application.
- `src/core/` contains editor, Markdown rendering, configuration, themes, keybindings, and UI modules.
- `resources/` contains the app icon and icon font assets.
- `specs_v1.md` documents the v1 product and implementation specification.
