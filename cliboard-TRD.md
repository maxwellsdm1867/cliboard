# cliboard — Technical Requirements Document

> A live math rendering board for CLI agents and scientists.

**Version**: 0.2 (draft)
**Date**: 2026-03-16
**Author**: Maxwell + Wheeler

---

## 1. Product Vision

cliboard renders math and math-related conversation in physics textbook style.
It is two things:

1. **A document format** for structured math-rich content (equations, prose,
   annotations, results) — flexible enough for agents, humans, or any tool to
   write.

2. **A display engine** that renders that document beautifully in a browser —
   with interactive features like selection, send-to-terminal, and auto-scroll
   that make it a proper reading surface, not just a renderer.

The CLI is a convenience layer. Any tool that can write the document format
gets the display for free.

---

## 2. User Personas

### 2.1 Physicist with Wheeler (primary)

Maxwell is a physicist using Wheeler (Claude Code) in the VS Code terminal. He
asks Wheeler to walk him through a derivation. Wheeler adds steps to the cliboard
board as they discuss. Maxwell sees rendered equations in a VS Code tab beside
the terminal. He asks questions, Wheeler annotates steps, the derivation builds
up interactively.

**Key need**: Equations must render the way a textbook looks. Fractions stack.
Integrals have limits. Matrices have brackets. No ASCII art, no Unicode hacks.
Real rendered math.

### 2.2 Any coding agent working with math

A developer using Claude Code, Cursor, or Copilot is debugging a signal
processing algorithm. The agent needs to show the DFT formula. It calls
`cliboard render` and the equation appears in a VS Code tab. No setup, no
configuration.

### 2.3 Teacher / student

A professor uses cliboard to build up a derivation during a lecture or office
hours. Students see a live board (could be shared via screen or localhost URL).
Steps build up in order, annotations explain each transition.

### 2.4 Scientist writing directly

A physicist opens a `.cb.md` file in their editor and writes markdown + LaTeX.
cliboard watches the file and renders it live in a browser tab beside their
editor. No CLI commands needed — just write and see.

---

## 3. User Stories

### Session workflow (primary)

```
US-1: As a scientist, I want to start a derivation session with one command
      so the board appears immediately and I can start working.

US-2: As a scientist, I want Wheeler to add derivation steps while we discuss
      so I see rendered math building up without leaving my terminal.

US-3: As a scientist, I want to ask Wheeler about a specific step and have
      Wheeler annotate it, so the explanation lives with the equation.

US-4: As a scientist, I want the board to auto-scroll to the latest step
      so I always see what Wheeler just added without manual scrolling.

US-5: As a scientist, I want to see the full derivation at a glance — scroll
      up for context, scroll down for latest work.

US-6: As a scientist, I want to save a derivation session so I can reference
      it later or share it with collaborators.
```

### Display interaction

```
US-7: As a scientist, I want to select an equation on the board and send it
      to my terminal as readable Unicode math, so I can reference it while
      chatting with the agent.

US-8: As a scientist, I want to highlight a key result on the board and have
      the agent know which equation I'm referring to.
```

### Quick render (secondary)

```
US-9: As a developer, I want to render a single equation with one command
      so I can see it without starting a full session.

US-10: As an agent, I want to call cliboard from a subprocess and have the
       equation appear to the user without blocking my execution.
```

### Direct authoring

```
US-11: As a scientist, I want to write a .cb.md file in my editor and see it
       rendered live, so I can author derivations without using CLI commands.

US-12: As an agent, I want to write/append to a markdown file instead of
       calling CLI commands, so I can use standard file operations.
```

### Error handling

```
US-13: As a user, I want to see graceful error handling when LaTeX is invalid
       — show me what's wrong, don't crash or show a blank screen.

US-14: As a user, I want the server to start fast (<500ms) so there's no
       awkward wait when I begin a session.
```

---

## 4. Architecture — Three Layers

cliboard is three decoupled layers with clean contracts between them:

```
┌─────────────────────────────────────────────────────────────┐
│                     INPUT LAYER (writers)                     │
│                                                               │
│  CLI commands    Direct file edit    Agent file I/O    stdin  │
│  (cliboard step) (vim/VS Code)      (write/append)    (pipe) │
│                                                               │
└──────────────────────────┬──────────────────────────────────┘
                           │ writes
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                  DOCUMENT FORMAT (.cb.md)                     │
│                                                               │
│  Markdown + LaTeX with lightweight conventions.               │
│  Human-readable, human-writable, version-controllable.        │
│  The contract between any writer and the display engine.      │
│                                                               │
└──────────────────────────┬──────────────────────────────────┘
                           │ watches + renders
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    DISPLAY ENGINE (viewer)                    │
│                                                               │
│  Renders document as a physics textbook page.                 │
│  KaTeX math rendering (server-side).                          │
│  Interactive reading: selection, send-to-terminal,            │
│  auto-scroll, error display, dark mode.                       │
│                                                               │
│  Served via built-in HTTP server on localhost.                 │
│  Displayed in any browser (VS Code Simple Browser, etc.)      │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

The key insight: **the document format is the interface**. Any tool that can
produce a `.cb.md` file gets the full display engine for free. The CLI is
just the most convenient writer for agents.

---

## 5. Document Format

### 5.1 Overview

The cliboard document format is **markdown with LaTeX**. File extension: `.cb.md`
(but `.md` works too — the extension is a convention, not a requirement).

This format was chosen because:
- Scientists already know markdown + LaTeX (papers, Jupyter, overleaf)
- Agents naturally produce markdown + LaTeX
- Human-readable and human-writable in any editor
- Version-controllable (clean git diffs)
- No special tooling needed to create or edit

### 5.2 Format specification

```markdown
---
title: Hydrogen Atom Energy Levels
---

## Time-independent Schrödinger equation

$$\hat{H}\psi = E\psi$$

## Expand the Hamiltonian

$$-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi$$

> The kinetic energy operator plus potential energy.

Now substitute the Coulomb potential for hydrogen.

## Coulomb potential

$$V(r) = -\frac{e^2}{4\pi\epsilon_0 r}$$

> Central force → separable in spherical coordinates.

---

## Separation of variables

$$\psi(r,\theta,\phi) = R(r) \cdot Y_l^m(\theta,\phi)$$

$$-\frac{\hbar^2}{2m}\left[\frac{1}{r^2}\frac{d}{dr}\left(r^2\frac{dR}{dr}\right)\right] + V(r)R = ER$$

> The angular part gives spherical harmonics. The radial part gives
> the energy eigenvalues.

## Hydrogen energy levels {.result}

$$E_n = -\frac{m_e e^4}{2\hbar^2} \cdot \frac{1}{n^2} = -\frac{13.6 \text{ eV}}{n^2}$$

> The $1/n^2$ dependence matches the Balmer series.
```

### 5.3 Conventions

The format uses standard markdown with minimal conventions that the display
engine interprets for textbook-style rendering:

| Markdown construct | Display rendering |
|---|---|
| `---` (YAML frontmatter) | Document metadata: `title`, `theme` |
| `## Title` | Numbered step with title |
| `$$...$$` | Display equation (KaTeX rendered, in a card) |
| `$...$` | Inline math within prose or notes |
| `> text` | Annotation/note (muted style below equation) |
| Plain paragraph | Prose block between steps (unnumbered context) |
| `---` (horizontal rule) | Section divider with optional label |
| `## Title {.result}` | Highlighted result box — the punchline |

**Key rules:**
- Every `## heading` starts a new numbered step
- `$$equations$$` within a step are grouped into that step's equation card
- Multiple `$$` blocks under the same `##` become multiple equations in the
  same step (vertically stacked)
- `> blockquotes` are annotations attached to the preceding equation
- Paragraphs without `##` headings are unnumbered prose blocks (context between
  steps, like "Now we substitute..." transitions)
- `{.result}` attribute on a heading marks it as a key result (highlighted box)

### 5.4 Frontmatter

```yaml
---
title: Hydrogen Atom Energy Levels    # displayed as board title
theme: dark                           # dark (default) or light
---
```

Minimal. Most settings are display-engine concerns, not document concerns.

### 5.5 Why markdown, not JSON

The original design used `state.json` as the document format. Markdown is
better because:

- **Human-writable**: A scientist can open a `.cb.md` in vim and just write.
  Nobody wants to write JSON by hand.
- **Agent-natural**: When agents produce math explanations, they already output
  markdown + LaTeX. The document format matches what they'd naturally produce.
- **Portable**: A `.cb.md` file is useful even without cliboard — it renders
  in any markdown viewer, GitHub, Jupyter, etc. The LaTeX is standard.
- **Diffable**: Markdown diffs are clean. JSON diffs are noisy.
- **Multiple input paths**: The CLI appends markdown. An agent can write the
  whole file. A human can edit it. All use the same format.

The display engine parses markdown into an internal model for rendering. This
is a one-way transformation: document → display. The document file is the
source of truth.

### 5.6 Step identity

Steps are identified by their position (1-indexed order of `##` headings).
When the scientist selects "Step 3" on the board, that means the third `##`
heading in the document. If steps are reordered or deleted, IDs shift — the
viewer re-renders from the full document.

This is deliberate: the document is a linear narrative (like a textbook), and
positional identity matches how scientists think ("the third step in the
derivation").

---

## 6. Display Engine

The display engine is the core of cliboard. It renders the document as a
physics textbook page with interactive reading features.

### 6.1 The viewer (HTML board)

**Layout**:
```
┌──────────────────────────────────────────────────────────────┐
│  Hydrogen Atom Energy Levels                      [5 steps]  │
│──────────────────────────────────────────────────────────────│
│                                                              │
│  1. Time-independent Schrödinger equation                    │
│  ┌──────────────────────────────────────────────────────┐    │
│  │         Ĥψ = Eψ                                     │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  2. Expand the Hamiltonian                                   │
│  ┌──────────────────────────────────────────────────────┐    │
│  │      ℏ²                                              │    │
│  │   - ─── ∇²ψ + V(r)ψ = Eψ                           │    │
│  │      2m                                              │    │
│  └──────────────────────────────────────────────────────┘    │
│  The kinetic energy operator plus potential.                  │
│                                                              │
│  Now substitute the Coulomb potential for hydrogen.           │
│                                                              │
│  3. Coulomb potential                                        │
│  ┌──────────────────────────────────────────────────────┐    │
│  │           e²                                         │    │
│  │  V(r) = ────                                         │    │
│  │         4πε₀r                                        │    │
│  └──────────────────────────────────────────────────────┘    │
│  Central force → separable in spherical coordinates.         │
│                                                              │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   │
│                                                              │
│  4. Separation of variables                                  │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  ψ(r,θ,φ) = R(r) · Yₗᵐ(θ,φ)                       │    │
│  │                                                      │    │
│  │      ℏ²   1  d      dR                               │    │
│  │   - ─── [── ──(r² ──)] + V(r)R = ER                 │    │
│  │      2m   r² dr     dr                               │    │
│  └──────────────────────────────────────────────────────┘    │
│  The angular part gives spherical harmonics.                 │
│                                                              │
│  ╔══════════════════════════════════════════════════════╗     │
│  ║  5. Hydrogen energy levels                    ★     ║     │
│  ║  ┌──────────────────────────────────────────────┐   ║     │
│  ║  │         mₑe⁴    1        13.6 eV            │   ║     │
│  ║  │  Eₙ = - ──── · ─── = - ─────────            │   ║     │
│  ║  │         2ℏ²     n²         n²               │   ║     │
│  ║  └──────────────────────────────────────────────┘   ║     │
│  ║  The 1/n² dependence matches the Balmer series.     ║     │
│  ╚══════════════════════════════════════════════════════╝     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

(The equations above are plaintext mockups — in the actual viewer, KaTeX renders
them as proper typeset math with real fraction bars, sized delimiters, etc.)

**Visual design principles**:

- **Dark mode by default.** Scientists work late. Light mode available via toggle.
- **Generous whitespace.** Each step is visually separated. Equations breathe.
- **Clear hierarchy.** Step number + title in a readable font. Equation in a
  lightly bordered card. Notes in a softer style below.
- **Typography.** KaTeX's default Computer Modern for math. System sans-serif
  (Inter, SF Pro, or system-ui) for titles and notes. High contrast.
- **Accent color.** Minimal. One highlight color for key results (★ marker)
  and annotations. Default: amber/gold on dark, blue on light.
- **No chrome.** No toolbar, no sidebar, no settings panel. The board is
  content-only. All configuration happens via CLI or frontmatter.
- **Result boxes.** Steps marked `{.result}` get a distinct highlighted border —
  the visual punchline of the derivation.
- **Prose blocks.** Unnumbered paragraphs between steps render as transitional
  text — lighter weight than steps, bridging one idea to the next.

### 6.2 Auto-scroll

When the document changes and new steps appear, the board smooth-scrolls to
the latest step. If the user has manually scrolled up (reading earlier steps),
auto-scroll pauses until they scroll back to the bottom.

### 6.3 Animations

New steps fade in (200ms ease-in). No bouncing, no sliding, no
attention-grabbing effects. Subtle and professional.

### 6.4 KaTeX error display

If LaTeX is invalid, the step shows the raw LaTeX in a monospace red-tinted
card with the KaTeX error message below. The board does NOT crash or hide the
broken step — the user needs to see what went wrong so they can tell the agent
to fix it.

### 6.5 Selection and send-to-terminal

This is a core display engine feature. The scientist needs to reference
specific equations from the board while chatting in the terminal.

**How it works:**

1. **Select**: Highlight any portion of a step — a single equation, a note, or
   the entire step. Standard browser text selection (click + drag, or click a
   step to select it entirely).

2. **Send to terminal button**: A small floating button appears near the
   selection: `→ Send to terminal`. Clicking it:

   - Converts LaTeX to **Unicode math** for terminal readability:
     `\frac{e^2}{4\pi\epsilon_0 r}` → `e²/4πε₀r`
   - Wraps with a step reference: `[Step 3] V(r) = -e²/4πε₀r`
   - Copies to **system clipboard** in this terminal-ready format
   - Writes full context to **selection file** (`~/.cliboard/selection.json`)
     for agents to read programmatically
   - Toast: "Step 3 → clipboard" (fades after 1s)

3. **Paste into terminal**: The scientist pastes (Cmd+V) into the terminal.
   The text is already formatted — Unicode math, step reference, readable.

**What the scientist pastes into the terminal:**

```
[Step 3] V(r) = -e²/4πε₀r
```

They can type around it naturally:

```
why does [Step 3] V(r) = -e²/4πε₀r imply separability?
```

**Partial selection**: If the scientist highlights just one equation from a
multi-equation step, only that equation is sent (not the whole step). If they
highlight a note, it sends the note text with its step reference.

**Selection file format** (`~/.cliboard/selection.json`):

```json
{
  "step_id": 3,
  "title": "Coulomb potential",
  "latex": "V(r) = -\\frac{e^2}{4\\pi\\epsilon_0 r}",
  "unicode": "V(r) = -e²/4πε₀r",
  "formatted": "[Step 3] V(r) = -e²/4πε₀r",
  "notes": ["Central force → separable in spherical coordinates"],
  "selected_at": "2026-03-16T14:33:00Z"
}
```

**LaTeX → Unicode conversion**: The server handles conversion using a built-in
mapping table (Greek letters, operators, super/subscripts, common symbols).
Fractions fall back to inline `/` notation.

**Example conversions:**

| LaTeX | Unicode terminal format |
| --- | --- |
| `\frac{\hbar^2}{2m}` | `ℏ²/2m` |
| `\nabla^2 \psi` | `∇²ψ` |
| `\int_0^{\infty} e^{-x^2} dx` | `∫₀^∞ e^{-x²} dx` |
| `\begin{pmatrix} a & b \\ c & d \end{pmatrix}` | `(a b; c d)` |
| `\partial \psi / \partial t` | `∂ψ/∂t` |

The viewer writes the selection via a POST to the local server, which writes the
file. No external network access, everything stays on localhost.

### 6.6 Viewer implementation

A single `index.html` file containing:

- **KaTeX CSS + fonts**: served from the binary via `rust-embed`. No CDN, no
  external dependencies. Fully offline.
- **Math HTML**: pre-rendered server-side by `katex-rs`. No client-side JS
  needed for math rendering.
- **App CSS**: inlined. Two themes (dark/light). Responsive. ~200 lines.
- **App JS**: inlined. Polls for document changes. Diffs against last known
  state to avoid full re-renders. Handles auto-scroll, selection, and
  send-to-terminal. ~300 lines.
- **No framework dependencies.** Vanilla HTML/CSS/JS. The viewer must be
  self-contained and trivially auditable.

Total viewer size target: < 20KB (excluding KaTeX CSS/fonts served separately).

---

## 7. Input Layer

The input layer is anything that writes the document format. The display engine
doesn't care how the document was produced — it watches the file and renders
whatever it finds.

### 7.1 CLI commands (primary for agents)

The CLI is the most convenient input method for agents. Commands append to
the current session's `.cb.md` file and exit immediately (non-blocking).

**Starting a session:**

```bash
$ cliboard new "Hydrogen Atom Energy Levels"
```

What happens:
1. Creates session directory: `~/.cliboard/sessions/<timestamp>-hydrogen-atom/`
2. Creates `board.cb.md` with frontmatter (title)
3. Starts local HTTP server watching the file (default port: 8377)
4. Opens board in browser (VS Code Simple Browser if in VS Code, default
   browser otherwise)
5. Prints to stdout: `Board live at http://localhost:8377`

Time budget: < 500ms from command to board visible.

**Adding content:**

```bash
# Add a titled step with one equation
$ cliboard step "Coulomb potential" "V(r) = -\frac{e^2}{4\pi\epsilon_0 r}"
# → appends: ## Coulomb potential\n\n$$V(r) = ...$$\n

# Add an equation to the current (most recent) step
$ cliboard eq "\psi(r,\theta,\phi) = R(r) \cdot Y_l^m(\theta,\phi)"
# → appends: \n$$\psi(r,...) = ...$$\n  (under last ## heading)

# Add a note/annotation to the current step
$ cliboard note "Separation of variables works because V(r) is spherically symmetric"
# → appends: \n> Separation of variables...\n

# Add a text-only block (context, no equation)
$ cliboard text "Now we apply the boundary conditions."
# → appends: \nNow we apply the boundary conditions.\n

# Add a highlighted result
$ cliboard result "Hydrogen energy levels" "E_n = -\frac{13.6 \text{ eV}}{n^2}"
# → appends: ## Hydrogen energy levels {.result}\n\n$$E_n = ...$$\n

# Add a section divider
$ cliboard divider
# → appends: \n---\n
```

Every mutation command prints a one-line confirmation:
```
Step 4 added: "Coulomb potential"
```

**Session management:**

```bash
$ cliboard stop                  # stop server
$ cliboard export output.html    # self-contained export
$ cliboard status                # "running on :8377, 5 steps" or "not running"
$ cliboard state                 # prints rendered state to stdout
$ cliboard selection             # prints what scientist selected on the board
$ cliboard selection --json      # full selection.json
$ cliboard selection --latex     # raw LaTeX only
```

**Quick render (no session):**

```bash
# Render a single equation, open in browser
$ cliboard render "\oint \mathbf{B} \cdot d\mathbf{l} = \mu_0 I_{enc}"

# Render to file
$ cliboard render -o gauss.html "\nabla \cdot \mathbf{E} = \frac{\rho}{\epsilon_0}"

# Render from stdin
$ echo "\sum_{n=0}^{\infty} \frac{x^n}{n!} = e^x" | cliboard render -
```

Quick render creates a minimal self-contained HTML file (KaTeX inlined) and
opens it. No server, no session. Fire and forget.

### 7.2 Direct file editing

A scientist or agent can write/edit the `.cb.md` file directly:

```bash
# Watch any markdown file and render it live
$ cliboard watch derivation.md
```

This starts the server, opens the viewer, and watches the file for changes.
The scientist edits the file in vim, VS Code, or any editor. The board updates
live as they save.

`cliboard new "title"` is essentially syntactic sugar for: create a new
`.cb.md` file with frontmatter, then `cliboard watch` it.

### 7.3 Agent file I/O

An agent that prefers file operations over CLI commands can simply write or
append to the `.cb.md` file:

```bash
# Agent appends a step by writing to the file directly
echo '
## Fourier Transform

$$\hat{f}(\xi) = \int_{-\infty}^{\infty} f(x) e^{-2\pi i x \xi} dx$$

> Decomposes a function into its frequency components.
' >> ~/.cliboard/sessions/current/board.cb.md
```

The display engine watches the file — it doesn't care whether the file was
modified by the CLI, an editor, or `echo >>`.

### 7.4 Stdin piping

```bash
# Pipe markdown + LaTeX content into the current session
cat derivation-fragment.md | cliboard append

# Or pipe into a new render
cat full-derivation.md | cliboard watch -
```

---

## 8. Technical Implementation

### 8.1 Server architecture

The HTTP server is built into the Rust binary. It:

1. **Watches** the session's `.cb.md` file for changes (fs events or polling)
2. **Parses** the markdown into an internal document model (steps, equations,
   notes, prose, results)
3. **Renders** LaTeX to HTML via `katex-rs` (server-side — no client JS needed)
4. **Serves** the rendered HTML to the viewer
5. **Handles** selection POSTs from the viewer (writes `selection.json`)

The viewer polls a lightweight endpoint to check for document changes. When the
document changes, the server sends the updated rendered content. The viewer
diffs and updates only what changed.

### 8.2 Markdown parsing

The server parses `.cb.md` into a structured document model:

```
Document
├── title: String (from frontmatter)
├── theme: dark | light (from frontmatter)
└── blocks: Vec<Block>
    ├── Step { id, title, equations, notes, is_result }
    ├── Prose { content }
    └── Divider
```

Parsing rules:
- `## Heading` → new Step (id = position among headings)
- `## Heading {.result}` → new Step with `is_result = true`
- `$$...$$` after a `##` → equation added to current Step
- `> text` after a `$$` → note added to current Step
- Paragraphs outside any `##` context → Prose block
- `---` (not frontmatter) → Divider

The parser is intentionally simple — it's not a full markdown AST. It only
needs to recognize these few constructs to produce the document model.

### 8.3 Rendering pipeline

```
.cb.md file
    │
    ▼ (parse)
Document model (steps, prose, dividers)
    │
    ▼ (render math)
katex-rs: LaTeX strings → KaTeX HTML strings
    │
    ▼ (template)
Full HTML page (KaTeX HTML embedded in page structure)
    │
    ▼ (serve)
HTTP response → browser
```

Math rendering happens server-side. The viewer receives ready-to-display HTML.
Client-side JS only handles: polling for updates, diffing, auto-scroll,
selection, and send-to-terminal.

### 8.4 Browser integration

On `cliboard new` or `cliboard watch`, the CLI opens the board in a browser:

**Detection priority:**
1. If in VS Code terminal (`$TERM_PROGRAM == "vscode"`): open VS Code Simple
   Browser via `code --command simpleBrowser.show "http://localhost:8377"`
   (keeps board in a VS Code tab beside the terminal)
2. Otherwise: open default browser via `open` crate (macOS `open`, Linux
   `xdg-open`, Windows `start`)

Default port: 8377, falls back to next available.

### 8.5 Export

`cliboard export output.html` produces a **self-contained** HTML file:
- KaTeX CSS inlined (not CDN)
- All rendered math HTML inlined
- No server dependency — opens in any browser, forever
- Shareable, archivable, printable

This is important for scientists: derivations become permanent artifacts.

### 8.6 File structure

```
~/.cliboard/
├── sessions/
│   ├── 2026-03-16-hydrogen-atom/
│   │   ├── board.cb.md         # the document (source of truth)
│   │   └── selection.json      # last selection from viewer
│   └── 2026-03-16-maxwell/
│       ├── board.cb.md
│       └── selection.json
└── selection.json              # symlink to active session's selection
```

---

## 9. Non-Functional Requirements

### 9.1 Performance

| Operation | Target |
|---|---|
| `cliboard new` (to board visible) | < 500ms |
| `cliboard step` (to rendered in viewer) | < 300ms |
| `cliboard render` (one-shot) | < 1s |
| Server memory footprint | < 10MB |
| Viewer memory (50-step derivation) | < 50MB |
| File write (append to .cb.md) | < 5ms |

### 9.2 Reliability

- **Crash recovery**: If the server dies, `cliboard new` or `cliboard watch`
  restarts it. Session state is in a markdown file on disk — never lost.
- **Concurrent writes**: File-level locking on `.cb.md`. Two agents writing
  simultaneously should not corrupt the document.
- **Invalid LaTeX**: Never crashes. Shows error inline in the viewer. The
  user/agent can fix the LaTeX in the file or via CLI.

### 9.3 Distribution

```bash
# macOS (via Homebrew, auto-generated by cargo-dist)
brew install maxwellsdm/tap/cliboard

# From source
cargo install cliboard

# Binary download (GitHub Releases)
```

Zero runtime dependencies. Single binary. No Node.js, no Python, no TeX
installation required.

---

## 10. Technology Stack

### 10.1 Chosen stack

| Component | Choice | Rationale |
|---|---|---|
| **Language** | Rust | Single binary, fast startup, distributable via brew/cargo |
| **CLI framework** | `clap` (derive) | Ecosystem standard, enum subcommands, shell completions |
| **HTTP server** | `tiny_http` | ~300KB binary impact, no async runtime |
| **Math rendering** | KaTeX (server-side via `katex-rs`) | Pure Rust KaTeX port, no JS engine needed |
| **Markdown parsing** | `pulldown-cmark` | Standard Rust markdown parser, lightweight |
| **Asset bundling** | `rust-embed` | Embeds CSS + woff2 fonts into binary at compile time |
| **Serialization** | `serde` + `serde_json` | For selection.json and internal state |
| **File locking** | `fs4` | Maintained fork of fs2, pure Rust |
| **File watching** | `notify` | Cross-platform fs events (fallback to polling) |
| **Atomic writes** | `tempfile` | `NamedTempFile::persist()` for write-then-rename |
| **Distribution** | `cargo-dist` | Auto GitHub Actions + Homebrew formula from git tag |

### 10.2 Cargo.toml dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
tiny_http = "0.12"
katex-rs = "0.1"
pulldown-cmark = "0.12"
rust-embed = { version = "8", features = ["compression"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
fs4 = { version = "0.13", features = ["sync"] }
notify = { version = "7", features = ["macos_fsevent"] }
tempfile = "3"
chrono = { version = "0.4", features = ["serde"] }
open = "5"

[profile.release]
opt-level = "z"
strip = true
lto = true
codegen-units = 1
panic = "abort"
```

### 10.3 KaTeX rendering strategy

**Server-side rendering** — the key architectural decision:

1. `katex-rs` renders LaTeX to HTML strings in Rust (no JS engine, no browser)
2. The viewer receives pre-rendered HTML — **no JavaScript needed for math**
3. Client only loads `katex.min.css` + woff2 fonts (for glyph display)
4. Both CSS and fonts are embedded in the binary via `rust-embed`
5. Served from the built-in HTTP server on localhost

This eliminates the largest asset (katex.min.js at ~200KB) and removes any
client-side rendering latency. Equations arrive as ready-to-display HTML.

---

## 11. MVP Scope

**Ship this first:**

**Document format:**
1. `.cb.md` format: frontmatter + `##` steps + `$$` equations + `>` notes +
   prose + `{.result}` + `---` dividers

**Display engine:**
2. Viewer with dark mode, auto-scroll, KaTeX rendering, error display
3. Selection and send-to-terminal (LaTeX → Unicode → clipboard)
4. Built-in LaTeX → Unicode conversion table (Greek, operators, sub/superscripts)

**CLI (input layer):**
5. `cliboard new "title"` — create session, start server, open browser
6. `cliboard step "title" "latex"` — add a step
7. `cliboard eq "latex"` — add equation to current step
8. `cliboard note "text"` — annotate current step
9. `cliboard text "prose"` — add text block
10. `cliboard result "title" "latex"` — highlighted result box
11. `cliboard render "latex"` — quick one-shot render
12. `cliboard stop` — stop server
13. `cliboard export output.html` — self-contained export
14. `cliboard selection` — read what scientist selected

**Explicitly NOT in MVP**: `cliboard watch` (direct file editing mode), PDF
export, keyboard shortcuts, light mode toggle, session management (ls/open),
edit/rm commands, WebSocket, config file, offline KaTeX caching, stdin piping.

---

## 12. Future Considerations (not MVP)

These are explicitly deferred. Noted here so the architecture doesn't
accidentally block them.

- **`cliboard watch`**: Watch any `.cb.md` file for live rendering. The
  architecture fully supports this — it's just the server watching a different
  file path.
- **WebSocket**: Replace polling with WebSocket for instant updates.
- **Collaborative**: Multiple users viewing the same board.
- **Terminal graphics**: When kitty/sixel protocols are available, render math
  directly in the terminal via `cliboard inline "latex"`.
- **Deep interactive steps**: Click a step to send structured messages back to
  the agent ("explain this", "expand this", "what if?").
- **Branching derivations**: Fork at a step to explore "what if?" paths.
- **Multi-panel**: Side-by-side derivations for comparison.
- **LaTeX document export**: Export to `.tex` for inclusion in papers.
- **Keyboard shortcuts**: Step-by-step scrolling, fullscreen, theme toggle.
- **Session management**: `cliboard ls`, `cliboard open <name>`.

---

## 13. Resolved Decisions

| Question | Decision | Rationale |
|---|---|---|
| Language | Rust | Standalone binary, fast, any agent can shell out to it |
| Name | `cliboard` | CLI + board, memorable, no unfortunate substrings |
| Document format | Markdown + LaTeX (`.cb.md`) | Human-writable, agent-natural, portable, diffable |
| KaTeX bundling | Embed CSS + fonts in binary | Offline by default, ~355KB cost, no CDN dependency |
| Rendering | Server-side via katex-rs | No client-side JS needed, faster display |
| Port | Fixed 8377, fall back to next available | Simple for agents to hardcode |
| Session persistence | Keep forever, manual cleanup | Scientists want to reference old derivations |
| Selection/send-to-terminal | Core display engine feature | Reading interaction, not input — lives in the viewer |

---

## 14. Open Questions

1. **Font subsetting**: Ship all 52 woff2 files (~330KB) or subset to Main + Math
   + Size + AMS (~150KB)? Full set is safer but larger.

2. **File watching strategy**: Use `notify` crate for fs events, or simple
   polling? `notify` is more responsive but adds a dependency and platform
   complexity. Polling at 500ms is simpler and probably fine for MVP.

3. **Markdown parser**: Use `pulldown-cmark` for full CommonMark compliance, or
   write a minimal custom parser that only recognizes the few constructs we
   need? Custom is faster and smaller but may have edge cases.
