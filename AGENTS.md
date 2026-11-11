# AGENTS.md

## What this repository is

`doggy` is a Rust desktop application built with **iced 0.15.0-dev**.

`docs/iced/` is a read-only Markdown snapshot of the iced API documentation,
generated from <https://docs.iced.rs/iced/> at the commit pinned in
`Cargo.lock` (`cd04b74ca7073766f654535d6e72c452ef749d07`). It covers iced and its
15 companion crates, with exact signatures, trait implementations and source
pages.

Treat `docs/iced/` as the authoritative API reference for this project, but
never edit it. See [Keeping the docs current](#keeping-the-docs-current).

## Rule: consult the docs before writing iced code

Before using an iced type, widget, trait or function, look it up in
`docs/iced/`. Do not rely on memory, blog posts, or examples for other iced
versions: this project tracks `master` and the API changes frequently.

Start narrow and expand only as needed:

1. `docs/iced/llms.txt` — compact map of crates and major modules.
2. `docs/iced/index.md` — full table of contents.
3. `docs/iced/<crate>/index.md` and `docs/iced/<crate>/<module>/index.md` —
   items with one-line summaries.
4. `docs/iced/<crate>/<module>/<kind>.<Name>.md` — the full item page.

Use `docs/iced/llms-full.txt` (~9 MB) only as a last resort for full-text
search. In normal searches exclude it with `--glob '!llms-full.txt'` so every
hit does not appear twice.

### Mapping Rust paths to files

A path maps almost directly to the folder structure:

| Rust path | Documentation |
| --- | --- |
| `iced::widget::Button` | `docs/iced/iced/widget/struct.Button.md` |
| `iced::widget::button::Button` | `docs/iced/iced/widget/button/struct.Button.md` (stub) |
| `iced::Task` | `docs/iced/iced/struct.Task.md` (stub) |
| `iced::widget::text_input` (module) | `docs/iced/iced/widget/text_input/index.md` |

Re-export pages are **stubs**. They carry a `canonical:` field in the
frontmatter and a short body pointing to the real page. Always follow that
link and read the canonical page. For example,
`iced::widget::button::Button` points to `iced/widget/struct.Button`.

### Page format

- The first heading is `` # `crate::path::Item` ``.
- The exact declaration follows in a fenced `rust` block.
- `## Methods` groups inherent methods by `impl` block.
- A method heading is a `####` heading whose text is the method name in
  backticks; its signature is the next `rust` block.
- Other sections: `## Fields`, `## Variants`, `## Trait Implementations`,
  `## Auto Trait Implementations`, `## Blanket Implementations`,
  `## Required Methods`, `## Provided Methods`, `## Implementors`.
- Frontmatter provides `path`, `kind`, `module`, `source_file`,
  `source_lines`, `docs_url` and, for stubs, `canonical`.

## Searching

Use `rg` (or the agent's search tool) instead of opening large files:

```sh
# Find a page from an exact Rust path. Keep patterns free of embedded double
# quotes: PowerShell may strip them when passing arguments to rg.
rg -l --glob '!llms-full.txt' '^path: .*iced::widget::Button.$' docs/iced

# Find pages by name or file name
rg --files docs/iced/iced/widget | rg 'Button\.md$'

# Look up an item in the manifest: file, kind and canonical link
rg -n 'iced::widget::button::Button' docs/iced/manifest.json

# Full-text search, excluding the duplicate bundle
rg -n --glob '!llms-full.txt' 'on_press_maybe' docs/iced

# Read a specific method on a page
rg -n -A 12 '^#### `on_press`' docs/iced/iced/widget/struct.Button.md
```

If a page is a stub, look up its `canonical` value in the manifest (for example
`iced::widget::Button`) instead of guessing the target path.

## Using the docs correctly

- Copy signatures exactly; do not modernize or simplify them.
- Read the trait page before implementing `Application`, `Widget`, `Theme`,
  `Catalog` or any other iced trait. Required and provided methods are listed
  separately.
- The docs also cover `iced_core`, `iced_widget`, `iced_runtime`,
  `iced_winit` and the other companion crates. Use them when the top-level
  `iced` re-export is not enough.
- The docs do **not** cover `std`, `tokio`, `serde`, `sqlx` or other
  dependencies. Use their official documentation for those.
- If the docs are silent about runtime behavior, open the source page for the
  `source_file` in the frontmatter, e.g.
  `docs/iced/src/iced_widget/button.rs.md` (one fenced Rust block with the
  pinned source), or the upstream source at that commit. Do not invent
  behavior.
- After changing iced code, run `cargo check` and fix the errors. Compilation
  against the pinned dependency is the final check that the API usage is
  correct.
- Do not add dependencies or change the iced pin unless explicitly asked.
- If PowerShell shows UTF-8 punctuation (em dashes, arrows) as mojibake, the
  console encoding is at fault, not the docs. The files are valid UTF-8.
- When explaining iced APIs to the user, cite the documentation path you used.

## Keeping the docs current

`docs/iced/` is a generated artifact:

- Do not hand-edit any file under `docs/iced/`, including `README.md`,
  `index.md`, `llms.txt`, `llms-full.txt` and `manifest.json`.
- The version in `docs/iced/README.md` must match `Cargo.lock`. Check with:

  ```sh
  rg -n -m 1 'iced-rs/iced\?branch=master#' Cargo.lock
  rg -n 'Upstream commit' docs/iced/README.md
  ```

- If the commits differ, the docs are stale: update them from the upstream
  documentation build instead of guessing the new API.
- Regeneration is done in the documentation repository, not here; replace the
  whole folder with the new build and keep its internal relative links intact.
