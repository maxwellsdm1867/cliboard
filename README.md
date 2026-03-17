# cliboard

Live math rendering board for CLI agents and scientists.

cliboard renders math and math-related derivations in physics textbook style. It pairs a plain markdown+LaTeX document format (`.cb.md`) with a display engine that renders beautiful, interactive math in the browser -- all from a single Rust binary with zero runtime dependencies.

## How It Works

```
INPUT LAYER (writers)              Any tool that writes .cb.md
  CLI commands | direct file edit | agent file I/O
                    |
                    v
DOCUMENT FORMAT (.cb.md)           Markdown + LaTeX with conventions
  ## headings = steps, $$ = equations, > = notes, {.result} = highlight
                    |
                    v
DISPLAY ENGINE (viewer)            Physics textbook rendering + interaction
  KaTeX server-side rendering, equation numbers, selection,
  send-to-terminal, auto-scroll, error display, dark mode
```

The document format is the interface. Any tool that produces `.cb.md` gets the full display engine for free.

## Quick Start

```bash
# Install
cargo install --path .

# Start a derivation
cliboard new "Hydrogen Atom Energy Levels"

# Add steps (in a second terminal, or from an agent)
cliboard step "Schrodinger equation" "\hat{H}\psi = E\psi"
cliboard note 'Time-independent form: $\hat{H}$ is the Hamiltonian operator'
cliboard result "Energy levels" "E_n = -\frac{13.6}{n^2}"

# Export to a self-contained HTML file
cliboard export derivation.html
```

`cliboard new` starts a local server and opens the board in your browser (or VS Code Simple Browser). Steps appear live as you add them.

## CLI Commands

| Command | Description | Example |
|---------|-------------|---------|
| `new` | Start a new session and open the board | `cliboard new "Title"` |
| `step` | Add a titled step with an equation | `cliboard step "Name" "\latex"` |
| `eq` | Add an equation to the current step | `cliboard eq "\latex"` |
| `note` | Add an annotation to the current step | `cliboard note "text"` |
| `text` | Add a prose block | `cliboard text "paragraph"` |
| `result` | Add a highlighted result step | `cliboard result "Name" "\latex"` |
| `divider` | Add a section divider | `cliboard divider` |
| `render` | Quick one-shot render (no session needed) | `cliboard render "\latex"` |
| `export` | Export the board as self-contained HTML | `cliboard export output.html` |
| `stop` | Stop the current session server | `cliboard stop` |
| `status` | Show session status | `cliboard status` |
| `selection` | Read what was last selected on the board | `cliboard selection --json` |

All mutation commands (`step`, `eq`, `note`, `text`, `result`, `divider`) append to the `.cb.md` file and exit immediately. The display engine watches the file and re-renders automatically.

## Inline Math

Inline math is supported everywhere -- titles, notes, and prose blocks. Wrap LaTeX in `$...$`:

```bash
cliboard step 'Why scale by $\sqrt{d_k}$?' "\text{Var}(q \cdot k) = d_k"
cliboard note 'When $d_k$ is large, softmax saturates. Dividing by $\sqrt{d_k}$ fixes this.'
```

## Equation Numbers

Display equations are automatically numbered sequentially across the document, like a textbook. Numbers appear on the right side of each equation card in matching KaTeX font. No manual numbering needed.

## Document Format (.cb.md)

The document format is standard markdown with LaTeX, using a few conventions:

```markdown
---
title: Hydrogen Atom Energy Levels
---

## Time-independent Schrodinger equation

$$\hat{H}\psi = E\psi$$

> The starting point for any quantum mechanics problem.

## Expand the Hamiltonian

$$-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi$$

> Kinetic energy operator plus potential energy.

---

## Energy levels {.result}

$$E_n = -\frac{13.6 \text{ eV}}{n^2}$$

> The $1/n^2$ dependence matches the Balmer series.
```

| Syntax | Meaning |
|--------|---------|
| `---` (YAML) | Frontmatter with `title` and optional `theme` |
| `## Title` | Numbered step |
| `## Title {.result}` | Highlighted result box |
| `$$...$$` | Display equation (KaTeX rendered, auto-numbered) |
| `$...$` | Inline math (in titles, notes, and prose) |
| `> text` | Annotation/note |
| Plain paragraph | Unnumbered prose between steps |
| `---` | Section divider |

You can write `.cb.md` files by hand or with any tool -- the display engine does not care how the file was produced.

## Selection and Send-to-Terminal

Select text on the board. A floating "Send to terminal" button appears. Clicking it:

1. Converts LaTeX to Unicode math via server-side conversion (`\frac{e^2}{4\pi\epsilon_0 r}` becomes `e²/4πε₀r`)
2. Formats with context based on selection size:
   - **Partial selection** (one symbol): `Ĥ in [Step 1] Ĥψ = Eψ`
   - **Full equation**: `[Step 1] Ĥψ = Eψ`
3. Copies to system clipboard
4. Writes full context to `~/.cliboard/selection.json` for agents to read programmatically

Paste into your terminal to reference equations while chatting with an agent:

```
what is Ĥ in [Step 1] Ĥψ = Eψ
```

Read the current selection programmatically:

```bash
cliboard selection          # human-readable summary
cliboard selection --json   # full JSON (step_id, title, latex, unicode, formatted)
cliboard selection --latex  # raw LaTeX only
```

## Architecture

**Technology stack:**

- **Language**: Rust -- single binary, no runtime dependencies
- **CLI**: `clap` with derive macros
- **HTTP server**: `tiny_http` (synchronous, localhost-only)
- **Math rendering**: Server-side via `katex-rs` (no client-side JS for math)
- **Markdown parsing**: `pulldown-cmark` with math and heading attributes enabled
- **Asset bundling**: `rust-embed` (KaTeX CSS + woff2 fonts compiled into the binary)
- **File watching**: `notify` (cross-platform filesystem events)
- **File locking**: `fs4` for concurrent write safety

**Server-side KaTeX rendering** is the key architectural decision. LaTeX is rendered to HTML on the server by `katex-rs`. The browser receives ready-to-display HTML and only needs KaTeX CSS and fonts for glyph display. This eliminates `katex.min.js` (~200KB) and all client-side rendering latency.

**Rendering pipeline:**

```
.cb.md --> parse markdown --> Document model --> katex-rs (LaTeX to HTML) --> serve HTML
```

**Server endpoints:**

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/` | GET | Viewer HTML |
| `/board` | GET | JSON `{ version, title, blocks_html }` (supports `?v=N` for 304) |
| `/katex/*` | GET | Embedded KaTeX CSS and fonts |
| `/select` | POST | Receive selection, return unicode conversion |

The viewer is vanilla HTML/CSS/JS with no framework dependencies (< 20KB excluding KaTeX assets). Client JS handles polling (500ms), DOM diffing, auto-scroll, and selection.

## Building from Source

```bash
cargo build --release
```

The release profile is configured for minimum binary size: `opt-level = "z"`, `strip = true`, `lto = true`, `codegen-units = 1`, `panic = "abort"`.

The resulting binary (~2.3MB) is self-contained -- KaTeX CSS and all 20 woff2 font files are embedded at compile time.

## Session Storage

Sessions are stored in `~/.cliboard/sessions/`. Each session is a directory containing the `board.cb.md` file and metadata (PID, port, selection state). The current active session is tracked via `~/.cliboard/sessions/current`.

## License

TBD
