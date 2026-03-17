# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cliboard renders math and math-related conversation in physics textbook style.
Two decoupled parts:

1. **Document format** (`.cb.md`) — markdown + LaTeX that any writer can produce
2. **Display engine** — renders the document beautifully with interactive reading features

The CLI is a convenience input layer. The canonical requirements are in `cliboard-TRD.md`.

## Three-Layer Architecture

```
INPUT LAYER (writers)              Any tool that writes .cb.md
  CLI commands | direct file edit | agent file I/O | stdin
                    │ writes
                    ▼
DOCUMENT FORMAT (.cb.md)           Markdown + LaTeX with conventions
  ## headings = steps, $$ = equations, > = notes, {.result} = highlight
                    │ watches + renders
                    ▼
DISPLAY ENGINE (viewer)            Physics textbook rendering + interaction
  KaTeX server-side rendering, selection, send-to-terminal,
  auto-scroll, error display, dark mode
```

The document format is the interface. Any tool that produces `.cb.md` gets
the full display engine for free.

## Document Format (.cb.md)

Standard markdown + LaTeX with these conventions:
- YAML frontmatter: `title`, `theme`
- `## Title` = numbered step (titles support inline math via `$...$`)
- `## Title {.result}` = highlighted result box
- `$$...$$` = display equation (KaTeX rendered, auto-numbered)
- `$...$` = inline math (supported in titles, notes, and prose)
- `> text` = annotation/note
- Plain paragraphs = unnumbered prose between steps
- `---` = section divider

Parsed into: `Document { title, theme, blocks: [Step | Prose | Divider] }`

## Technology Stack

- **Language**: Rust (single binary, no runtime dependencies)
- **CLI**: `clap` with derive macros
- **HTTP server**: `tiny_http` (synchronous, localhost-only on 127.0.0.1)
- **Math rendering**: Server-side via `katex-rs` (no client JS for math)
- **Markdown parsing**: `pulldown-cmark` with ENABLE_MATH + ENABLE_HEADING_ATTRIBUTES
- **Asset bundling**: `rust-embed` (KaTeX CSS + woff2 fonts in binary)
- **File watching**: `notify` (cross-platform fs events)
- **File locking**: `fs4` for concurrent writes
- **Distribution**: `cargo-dist`

## Build Commands

```bash
cargo build                    # debug build
cargo build --release          # release build (opt-level=z, strip, LTO)
cargo run -- <subcommand>      # run during development
cargo test                     # run tests (221 tests)
cargo clippy                   # lint
cargo fmt                      # format
```

## Display Engine

The viewer is vanilla HTML/CSS/JS (no frameworks, < 20KB excluding KaTeX assets).
Core interactive features that live in the display engine (not input layer):
- **Selection + send-to-terminal**: select equation -> server-side LaTeX to Unicode -> clipboard
  - Partial selection: `Ĥ in [Step 1] Ĥψ = Eψ` (selected text + context)
  - Full selection: `[Step 1] Ĥψ = Eψ`
- **Auto-scroll**: smooth-scroll to new steps, pause when user scrolls up
- **Equation numbers**: auto-generated, right-aligned, KaTeX_Main font
- **Inline math**: `$...$` rendered in titles, notes, and prose
- **KaTeX error display**: show raw LaTeX in red card, never crash
- **Dark mode by default**, amber/gold accent (#CA8A04)
- **DOM preservation**: skip re-render when content unchanged, defer updates during active selection

## Rendering Pipeline

```
.cb.md → parse markdown → document model → katex-rs (LaTeX→HTML) → serve HTML
```

Math is pre-rendered server-side. Viewer receives ready-to-display HTML.
Client JS only handles polling, diffing, auto-scroll, selection.

## Server Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/` | GET | Viewer HTML |
| `/board` | GET | JSON `{ version, title, blocks_html }` — supports `?v=N` for 304 short-circuit |
| `/viewer.css` | GET | Viewer stylesheet |
| `/viewer.js` | GET | Viewer JavaScript |
| `/katex/katex.min.css` | GET | Embedded KaTeX CSS |
| `/katex/fonts/*` | GET | Embedded KaTeX woff2 fonts |
| `/select` | POST | Receive selection from viewer, return `{ ok, unicode, formatted }` |

## Performance Targets

| Operation | Target |
|---|---|
| `cliboard new` to board visible | < 500ms |
| `cliboard step` to rendered | < 300ms |
| Server memory | < 10MB |
| Binary size (release) | ~2.3 MB |

## Key Design Decisions

- **Markdown, not JSON**: Document format is `.cb.md` because it's human-writable, agent-natural, portable, and diffable
- **Server-side KaTeX**: No client-side JS for math. Eliminates katex.min.js (~200KB) and client rendering latency
- **Non-blocking CLI**: Commands append to `.cb.md` and exit immediately
- **File-level locking**: fs4 for concurrent writes to `.cb.md`
- **Positional step IDs**: Steps identified by position of `##` headings (1-indexed)
- **Default port 8377**, falls back to next available
- **VS Code detection**: `$TERM_PROGRAM == "vscode"` -> Simple Browser, else system default
- **Localhost only**: Server binds to 127.0.0.1, not 0.0.0.0
- **Bounded POST body**: /select endpoint limited to 64KB
- **Version-based polling**: 304 short-circuit when client has latest version

## File Structure

```
src/
├── main.rs          # CLI entry point, command dispatch, browser opening
├── cli.rs           # clap command definitions (Cli struct + Command enum)
├── document.rs      # Document, Block, Theme, Selection types
├── parser.rs        # .cb.md → Document model (pulldown-cmark with math + heading attrs)
├── render.rs        # Document → HTML (katex-rs, equation numbers, inline math in titles/notes)
├── server.rs        # HTTP server (tiny_http), file watching, /board + /select endpoints
├── session.rs       # Session management (~/.cliboard/sessions/), file locking, PID/port
├── unicode.rs       # LaTeX → Unicode conversion (Greek, operators, scripts, accents, matrices)
├── export.rs        # Self-contained HTML export (inlined CSS, CDN font fallback)
└── lib.rs           # Module declarations

assets/
├── viewer.html      # Display engine HTML shell (links to CSS/JS + KaTeX)
├── viewer.css       # Dark OLED theme (#1C1917), equation cards, equation numbers,
│                    # result boxes (amber border), notes (continuous left border),
│                    # responsive, prefers-reduced-motion support
└── viewer.js        # Polling with 304 support, DOM diffing with selection preservation,
                     # auto-scroll, selection + send-to-terminal, clipboard, toast

katex-assets/
├── katex.min.css    # KaTeX v0.16.22 stylesheet (embedded via rust-embed)
└── fonts/           # 20 KaTeX woff2 font files (embedded via rust-embed)

tests/
└── integration.rs   # 13 integration tests (parse→render pipeline, export, edge cases)
```

## Session File Layout

```
~/.cliboard/
├── sessions/
│   ├── current              # text file pointing to active session directory
│   ├── 2026-03-16-hydrogen-atom/
│   │   ├── board.cb.md      # the document (source of truth)
│   │   ├── selection.json   # last selection from viewer
│   │   ├── server.pid       # PID of running server
│   │   └── server.port      # port of running server
│   └── ...
└── selection.json           # copy of active session's selection
```

## Module Relationships

- `main.rs` dispatches CLI commands. `cmd_new` creates a session and starts the server (blocking). Other `cmd_*` functions find the current session and append to `board.cb.md`.
- `server.rs` watches the board file via `notify`, re-parses and re-renders on change, serves via `/board` as JSON. Returns 304 when client version matches. POST `/select` does LaTeX→Unicode conversion and returns formatted text.
- `render.rs` calls `katex-rs` for each equation with auto-incrementing equation numbers. Processes inline `$...$` math in titles, notes, and prose. Errors render as red cards, never panics.
- `parser.rs` uses `pulldown-cmark` with `ENABLE_MATH` and `ENABLE_HEADING_ATTRIBUTES` to parse `.cb.md` into the `Document` model.
- `unicode.rs` converts LaTeX to terminal-friendly Unicode. Handles Greek letters, operators, fractions, scripts, accents, matrices, and preserves meaningful whitespace around operators.
- `export.rs` produces a static HTML file with inlined CSS and CDN font fallback -- no JavaScript, no server dependency.

## Writing Notes for Agents

When writing content via CLI, wrap inline math in `$...$` for proper rendering:
```bash
cliboard note 'When $d_k$ is large, softmax saturates.'     # rendered math
cliboard step 'Why $\sqrt{d_k}$?' "..."                     # math in title
```

## Testing

```bash
cargo test              # run all 221 tests
cargo test parser       # run parser tests only
cargo test unicode      # run unicode tests only
cargo test -- --nocapture  # see println output
```

Tests are co-located in each module via `#[cfg(test)] mod tests`. Integration tests in `tests/integration.rs` cover the parse→render pipeline end-to-end.
