<div align="center">

# CLIBOARD

**A visual CLI — terminal speed, textbook rendering.**

CLI tools have been text-only for decades. cliboard is a new kind of tool: you type in the terminal, and a live browser view renders your work as a physics textbook — updating instantly, with interactive features no terminal can offer.

When you're deriving equations — whether by hand, with Claude Code, or any AI agent — you're staring at raw LaTeX like `\frac{\hbar^2}{2m}\nabla^2`. cliboard turns that into publication-quality math you can read, select, and ask questions about.

![cliboard demo — Hydrogen Atom derivation with AI chat reply](image.png)

**Built for AI-assisted math.** Tell Claude Code "derive the hydrogen atom energy levels" and it calls `cliboard step` — you see each equation render live in the browser as the agent works. No copy-pasting LaTeX into Overleaf. No squinting at `\frac{\hbar^2}{2m}` in your terminal. The agent writes, you read a textbook.

**Ask questions about any equation.** Click an equation, ask "what is this?", and get a textbook-style explanation with its own numbered sub-equations — like having a tutor built into your derivation.

**Select any equation and send it to chat.** Highlight a term, hit "Send to terminal", and it lands in your chat input with full context — ready to ask about.

No API keys. No cloud. No cost. One 2.3MB binary. Everything runs locally.

[![Rust](https://img.shields.io/badge/Rust-1.70+-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-242_passing-2ea44f)](#)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Binary](https://img.shields.io/badge/binary-2.3MB-8B5CF6)](#)
[![KaTeX](https://img.shields.io/badge/KaTeX-server--side-329DAA?logo=latex&logoColor=white)](#)

</div>

---

```bash
curl -fsSL https://github.com/maxwellsdm1867/cliboard/releases/latest/download/cliboard-installer.sh | sh
```

<div align="center">

Works on Mac, Windows, and Linux. Single binary, no runtime dependencies.

</div>

Or install via Cargo:
```bash
cargo install cliboard
```

<div align="center">

```
 ┌─────────────────────────────────────────────────────────────────────┐
 │ ● ● ●                        Terminal                              │
 ├─────────────────────────────────────────────────────────────────────┤
 │                                                                     │
 │  ~ $ cliboard new "Hydrogen Atom Energy Levels"                     │
 │  Board live at http://localhost:8377                                 │
 │                                                                     │
 │  ~ $ cliboard step "Schrödinger equation" "\hat{H}\psi = E\psi"    │
 │  Step 1 added: "Schrödinger equation"                               │
 │                                                                     │
 │  ~ $ cliboard step "Expand the Hamiltonian" \                       │
 │        "-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi"        │
 │  Step 2 added: "Expand the Hamiltonian"                             │
 │                                                                     │
 │  ~ $ cliboard result "Energy levels" "E_n = -\frac{13.6}{n^2}"     │
 │  Step 3 added: "Energy levels"                                      │
 │                                                                     │
 │  ~ $ █                                                              │
 │                                                                     │
 └─────────────────────────────────────────────────────────────────────┘
```

</div>

Steps appear live in the browser as you type them. Equations are server-side rendered with KaTeX — no client-side JavaScript needed for math.

---

<div align="center">

*"It's like having a physics textbook that writes itself as you think."*

*"Finally, a tool that lets me derive in the terminal and present in the browser."*

*"The AI chat on each equation is a game changer for teaching."*

</div>

---

## How It Works

```
Terminal                           Browser
────────                           ───────

cliboard step "..." "LaTeX"   ──>  .cb.md file
                                      │
                                   file watcher
                                      │
                                   pulldown-cmark → Document model → katex-rs
                                      │
                                   server-side rendered HTML
                                      │
                                   WebSocket push to viewer
                                      │
                                   Beautiful equations appear instantly
```

**Three layers, fully decoupled:**

| Layer | What it does |
|-------|-------------|
| **Input** | CLI commands, direct file edit, AI agent file I/O — anything that writes `.cb.md` |
| **Document** | Markdown + LaTeX with conventions: `##` = steps, `$$` = equations, `>` = notes |
| **Display** | KaTeX server-side rendering, selection, AI chat, auto-scroll, dark mode |

The document format is the interface. Any tool that produces `.cb.md` gets the full display engine for free.

## Features

**Core rendering**
- Server-side KaTeX — math is pre-rendered, browser just displays HTML
- Auto-numbered equations, right-aligned, textbook style
- Inline math (`$...$`) in titles, notes, and prose
- Dark/warm/light themes with toggle
- Self-contained HTML export

**Interactive**
- Select an equation → "Send to terminal" → LaTeX-to-Unicode on clipboard (`Ĥψ = Eψ`)
- Auto-scroll to new steps, pauses when you scroll up
- KaTeX error display — shows raw LaTeX in red card, never crashes

**Per-step AI chat**
- Chat icon on every equation — ask about any step
- AI replies render as textbook continuations with sub-numbered equations `(1.1)`, `(1.2)`
- Selection → "Send to terminal" → text lands in chat input with context
- Hook system: plug in any LLM via `CLIBOARD_REPLY_HOOK`

**Technical**
- Single ~2.3MB Rust binary, no runtime dependencies
- WebSocket for instant updates, HTTP polling fallback
- < 500ms to first board visible, < 300ms per step render
- < 10MB server memory
- Localhost-only (127.0.0.1)

## Quick Start

cliboard is a toolkit — pick the level of commitment you need:

```bash
# One-shot: render a single equation or file
cliboard render "E = mc^2"
cliboard render derivation.cb.md
echo '$$\int_0^\infty e^{-x^2} dx$$' | cliboard render -

# Live document: watch a file, live-reload in browser (no sessions)
cliboard serve notes.cb.md

# Full interactive session with chat, selection, send-to-terminal
cliboard new "My Derivation"
cliboard step "Title" "\latex"
cliboard note "Annotation text with $inline$ math"
cliboard result "Final Answer" "\latex"

# Export when done
cliboard export derivation.html
```

## Per-Step AI Chat

Each step has a collapsible chat thread. Click the speech bubble icon to ask about any equation.

**From the browser:**
1. Click the chat icon on a step
2. Type your question and hit Send
3. AI answers right in the thread — with rendered equations numbered `(1.1)`, `(1.2)`, ...

**From the terminal:**
```bash
cliboard chat              # see pending questions
cliboard reply 1 'The eigenvalues satisfy $$E_n = -\frac{13.6}{n^2}$$ for $n = 1, 2, 3, ...$'
cliboard listen --json     # stream new questions to stdout
```

**Auto-reply with any LLM:**

```bash
export CLIBOARD_REPLY_HOOK="./chat-hook.sh"
cliboard new "My Derivation"
```

```bash
#!/bin/bash
# chat-hook.sh — receives CLIBOARD_STEP_ID, CLIBOARD_QUESTION, CLIBOARD_CONTEXT
ANSWER=$(echo "$CLIBOARD_QUESTION" | claude --print)
cliboard reply "$CLIBOARD_STEP_ID" "$ANSWER"
```

## Document Format (.cb.md)

Standard markdown + LaTeX. Write it by hand, with CLI commands, or with any agent:

```markdown
---
title: Hydrogen Atom Energy Levels
---

## Time-independent Schrödinger equation

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
| `$$...$$` | Display equation (auto-numbered) |
| `$...$` | Inline math (in titles, notes, prose) |
| `> text` | Annotation/note |
| Plain paragraph | Unnumbered prose |
| `---` | Section divider |

## CLI Reference

**Core (no session needed)**

| Command | Description |
|---------|-------------|
| `cliboard render "\latex"` | Render a single equation and open in browser |
| `cliboard render file.cb.md` | Render a `.cb.md` file as self-contained HTML |
| `cliboard render -` | Render `.cb.md` content from stdin |
| `cliboard serve file.cb.md` | Watch a file and live-render in browser (no session) |

**Interactive (full session)**

| Command | Description |
|---------|-------------|
| `cliboard new "Title"` | Start a session and open the board |
| `cliboard step "Title" "\latex"` | Add a numbered step with an equation |
| `cliboard eq "\latex"` | Add an equation to the current step |
| `cliboard note "text"` | Add an annotation (supports `$inline$` math) |
| `cliboard text "text"` | Add a prose paragraph |
| `cliboard result "Title" "\latex"` | Add a highlighted result box |
| `cliboard divider` | Add a section divider |
| `cliboard export file.html` | Export as self-contained HTML |
| `cliboard chat` | Show pending chat questions |
| `cliboard reply N "text"` | Reply to a question on step N |
| `cliboard listen` | Watch for new questions (blocking) |
| `cliboard selection` | Read last selection from the board |
| `cliboard status` | Show session status |
| `cliboard stop` | Stop the server |
| `cliboard update` | Update to the latest version |

## Selection and Send-to-Terminal

Select text on the board. Two buttons appear:

- **Send to terminal** — converts LaTeX to Unicode, pastes into chat input with step context
- **Ask about this** — opens the chat for that step, pre-filled with your selection

Read the current selection programmatically:
```bash
cliboard selection          # human-readable
cliboard selection --json   # full JSON
cliboard selection --latex  # raw LaTeX
```

## Architecture

```
.cb.md  →  pulldown-cmark  →  Document model  →  katex-rs  →  HTML  →  browser
                                                                ↑
                                                        server-side rendering
```

- **Language**: Rust — single binary, no runtime dependencies
- **Math**: Server-side KaTeX via `katex-rs` (no client JS for math)
- **Server**: `tiny_http` (synchronous, localhost-only on 127.0.0.1)
- **Updates**: WebSocket via `tungstenite`, with HTTP polling fallback
- **Markdown**: `pulldown-cmark` with math + heading attributes
- **Assets**: `rust-embed` (KaTeX CSS + 20 woff2 fonts compiled into binary)
- **File watching**: `notify` (cross-platform fs events)
- **Viewer**: Vanilla HTML/CSS/JS, no framework (< 20KB)

## Performance

| Metric | Target |
|--------|--------|
| `cliboard new` to board visible | < 500ms |
| `cliboard step` to rendered | < 300ms |
| Server memory | < 10MB |
| Binary size (release) | ~2.3MB |

---

<div align="center">

[Quick Start](#quick-start) · [AI Chat](#per-step-ai-chat) · [Document Format](#document-format-cbmd) · [CLI Reference](#cli-reference) · [Architecture](#architecture)

MIT License

</div>
