# Editor references

These projects are here to inform ClearMind, a Rust/Iced Markdown editor with a rendered document view and a raw editor for the active line. Each entry gives the main implementation stack, a few defining features, and ideas worth studying. Toolkit names describe the primary UI layer, not every dependency.

## Markdown and writing apps

### [MarkText](https://github.com/marktext/marktext) · [local clone](./marktext/README.md)

- **Stack:** JavaScript/TypeScript, Vue, Electron; CodeMirror is used for text editing.
- **Features:** Live WYSIWYG Markdown, source mode, themes, focus and typewriter modes, math, and export.
- **Distinctive idea:** Keeps Markdown as the source while presenting formatted text as the editing surface.
- **ClearMind angle:** Study how syntax marks are revealed during editing, and how focus modes and formatting shortcuts feel.

### [Scribe](https://github.com/gnacho/scribe) · [local clone](./scribe/README.md)

- **Stack:** Rust, GTK 4/libadwaita, GtkSourceView 5, and `pulldown-cmark`.
- **Features:** Live Markdown styling in one buffer, rendered tables and code blocks, focus mode, spell checking, and optional split preview.
- **Distinctive idea:** Renders with text tags and drawn decorations over the source buffer, without a separate web preview.
- **ClearMind angle:** A close reference for source-preserving live rendering and cursor-line editing. Compare its buffer decorations with ClearMind’s rendered inactive lines.

### [Malgel](https://github.com/rcawston/Malgel) · [local clone](./Malgel/README.md)

- **Stack:** Rust, GPUI, and GPUI Kit.
- **Features:** Side-by-side Markdown editor and preview, in-place Live Preview, two-way scroll sync, tables, math, diagrams, search, and recovery.
- **Distinctive idea:** Lets users edit formatted Markdown directly while exposing source syntax near the caret; maps scrolling between source and preview.
- **ClearMind angle:** Study source-to-render mapping, table editing, and how to keep two views synchronized without fighting the user.

### [Moeka](https://github.com/MoekaProject/moeka) · [local clone](./moeka/README.md)

- **Stack:** Rust and GPUI.
- **Features:** Edit, Reading, and Source modes; tabs and split panes; outline and search; workspaces, wiki links, recovery, math, and diagrams.
- **Distinctive idea:** Combines Markdown editing with local-first workspace and knowledge-management features.
- **ClearMind angle:** Look at mode switching, workspace navigation, session restoration, and how a focused editor can grow into a multi-document app.

### [Zettlr](https://github.com/Zettlr/Zettlr) · [local clone](./Zettlr/README.md)

- **Stack:** JavaScript/TypeScript, Electron, Vue, and CodeMirror 6.
- **Features:** Markdown writing, full-text search, citations, knowledge linking, multilingual UI, and Pandoc/LaTeX/Word-oriented publishing.
- **Distinctive idea:** Treats plain-text documents as a research and publication workspace, including citation workflows.
- **ClearMind angle:** Useful for ideas around document organization, outline/search, metadata, and export workflows if ClearMind expands beyond single-document editing.

### [Ghostwriter](https://github.com/KDE/ghostwriter) · [local clone](./ghostwriter/README.md)

- **Stack:** C++ and Qt 6, with KDE Frameworks integrations.
- **Features:** Markdown editing and preview, themes, distraction-free writing, and export through Markdown processors such as cmark-gfm and Pandoc.
- **Distinctive idea:** Keeps the writing environment focused and configurable instead of centering an IDE-style workbench.
- **ClearMind angle:** Study typography, focus mode, preview presentation, and how much application chrome a writing tool needs.

## Code editors and workbenches

### [Fresh](https://github.com/sinelaw/fresh) · [local clone](./fresh/README.md)

- **Stack:** Rust core and terminal UI using crossterm. Fresh Desktop wraps its web UI in Electron.
- **Features:** Tabs, file explorer, command palette, multi-cursor editing, search/replace, splits, Markdown preview, LSP, Git tools, and plugins.
- **Distinctive idea:** Brings familiar graphical-editor interactions to a terminal editor; the desktop shell reuses the same editor session.
- **ClearMind angle:** Study command discovery, keyboard shortcuts, file workflows, search/replace, and robust session behavior. The terminal rendering itself is less relevant to Iced.

### [Zed](https://github.com/zed-industries/zed) · [local clone](./zed/README.md)

- **Stack:** Rust and GPUI, Zed’s GPU-accelerated UI framework.
- **Features:** Native code editing, language tooling, project navigation, split views, themes, and collaborative editing.
- **Distinctive idea:** Builds the editor and its UI framework together around responsive, GPU-accelerated rendering.
- **ClearMind angle:** Use it as a visual and interaction reference for a polished native editor: spacing, panel hierarchy, tabs, command flows, and responsive feedback.

### [Visual Studio Code (Code OSS)](https://github.com/microsoft/vscode) · [local clone](./vscode/README.md)

- **Stack:** TypeScript, Electron, and the Monaco editor, with a web-based workbench UI.
- **Features:** Editor groups, tabs, command palette, file explorer, search, themes, extensions, debugging, and integrated tools.
- **Distinctive idea:** A highly extensible workbench where commands and extensions add capabilities across the same interface.
- **ClearMind angle:** Study navigation, command palette, keyboard-first discoverability, and how to organize many tools without crowding the document. Code OSS is MIT-licensed; Microsoft’s VS Code distribution includes additional customizations.

### [Lapce](https://github.com/lapce/lapce) · [local clone](./lapce/README.md)

- **Stack:** Rust, Floem UI, and `wgpu` rendering.
- **Features:** LSP support, modal editing, remote development, built-in terminal, and WASI-based plugins.
- **Distinctive idea:** Uses a rope-based text model and GPU rendering to support fast editing and a native-feeling interface.
- **ClearMind angle:** Study editor responsiveness, text-model boundaries, and the layout of panels around the main document.

### [Helix](https://github.com/helix-editor/helix) · [local clone](./helix/README.md)

- **Stack:** Rust and a custom terminal UI (`helix-tui`, using crossterm).
- **Features:** Modal editing, multiple selections, Tree-sitter syntax highlighting, and built-in language-server support.
- **Distinctive idea:** Uses a selection-first editing model inspired by Kakoune, alongside incremental syntax highlighting.
- **ClearMind angle:** Borrow ideas for keyboard editing, selection behavior, and precise cursor motions; its terminal presentation is not a desktop UI reference.

## Text engine

### [COSMIC Text](https://github.com/pop-os/cosmic-text) · [local clone](./cosmic-text/README.md)

- **Stack:** Pure Rust text-handling library; it is not a standalone editor and has no application UI toolkit.
- **Features:** Text shaping, layout, rendering, font fallback, bidirectional text, and editor examples.
- **Distinctive idea:** Handles complex scripts and Unicode in a custom layout system, with large multilingual editing tests.
- **ClearMind angle:** A reference for text correctness—wrapping, cursor/selection geometry, font fallback, and bidirectional text—rather than app layout.
