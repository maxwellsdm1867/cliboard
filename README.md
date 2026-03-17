# cliboard

**Live math whiteboard for your terminal.** Type LaTeX, see beautiful equations in the browser — with per-step AI chat built in.

cliboard renders math derivations in physics textbook style. It pairs a plain markdown+LaTeX document format (`.cb.md`) with a display engine that renders interactive, publication-quality math in the browser — all from a single Rust binary with zero runtime dependencies.

Built for physicists, mathematicians, and AI agents who work in the terminal.

## Demo

```bash
cliboard new "Hydrogen Atom Energy Levels"
cliboard step "Schrödinger equation" "\hat{H}\psi = E\psi"
cliboard note 'Time-independent form: $\hat{H}$ is the Hamiltonian operator'
cliboard step "Expand the Hamiltonian" "-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi"
cliboard result "Energy levels" "E_n = -\frac{13.6 \text{ eV}}{n^2}"
```

Steps appear live in the browser as you type them. Equations are server-side rendered with KaTeX — no client-side JavaScript needed for math.

## Features

- **Live rendering** — equations appear in the browser as you type CLI commands
- **Per-step AI chat** — click the chat icon on any equation, ask a question, get an AI-powered answer right on the board
- **Selection + send-to-terminal** — select an equation, copy LaTeX-to-Unicode to clipboard (`\hat{H}\psi = E\psi` → `Ĥψ = Eψ`)
- **Server-side KaTeX** — math is pre-rendered on the server, browser just displays HTML. No katex.min.js (~200KB) needed
- **Single binary** — Rust binary with embedded fonts. No Node, no Python, no Docker
- **WebSocket + polling** — instant updates via WebSocket, with HTTP polling fallback
- **Dark/warm/light themes** — toggle in the viewer, respects system preference
- **Auto-scroll** — smooth-scroll to new steps, pauses when you scroll up
- **Equation numbering** — automatic, sequential, right-aligned, textbook style
- **Self-contained export** — `cliboard export output.html` produces a single HTML file with inlined CSS

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at target/release/cliboard (~2.3MB, self-contained)
```

## Quick Start

```bash
# Terminal 1: Start a session (opens browser)
cliboard new "My Derivation"

# Terminal 2: Add content
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
3. AI answers right in the thread (with rendered math)

**From the terminal:**
```bash
cliboard chat              # see pending questions
cliboard chat --all        # see all messages
cliboard chat --step 3     # messages for step 3
cliboard reply 1 '$\hat{H}$ is the Hamiltonian operator'
cliboard listen --json     # stream new questions to stdout
```

**Auto-reply with AI:**

Set `CLIBOARD_REPLY_HOOK` to a script that receives questions and calls `cliboard reply`:

```bash
export CLIBOARD_REPLY_HOOK="./chat-hook.sh"
cliboard new "My Derivation"
```

The included `chat-hook.sh` pipes questions to the `claude` CLI. Replace it with any LLM or custom logic:

```bash
#!/bin/bash
# Env vars: CLIBOARD_STEP_ID, CLIBOARD_QUESTION, CLIBOARD_CONTEXT
ANSWER=$(echo "$CLIBOARD_QUESTION" | your-llm-cli --print)
cliboard reply "$CLIBOARD_STEP_ID" "$ANSWER"
```

**Selection → Chat flow:**
1. Highlight text on an equation
2. Click "? Ask about this" (appears alongside "Send to terminal")
3. Chat opens pre-filled with your selection as context
4. The AI sees what you highlighted, which equation, and which step

## CLI Reference

| Command | Description |
|---------|-------------|
| `cliboard new "Title"` | Start a session and open the board |
| `cliboard step "Title" "\latex"` | Add a numbered step with an equation |
| `cliboard eq "\latex"` | Add an equation to the current step |
| `cliboard note "text"` | Add an annotation (supports `$inline$` math) |
| `cliboard text "text"` | Add a prose paragraph |
| `cliboard result "Title" "\latex"` | Add a highlighted result box |
| `cliboard divider` | Add a section divider |
| `cliboard render "\latex"` | Quick one-shot render (no session) |
| `cliboard export file.html` | Export as self-contained HTML |
| `cliboard chat` | Show pending chat questions |
| `cliboard reply N "text"` | Reply to a question on step N |
| `cliboard listen` | Watch for new questions (blocking) |
| `cliboard selection` | Read last selection from the board |
| `cliboard status` | Show session status |
| `cliboard stop` | Stop the server |

All content commands (`step`, `eq`, `note`, `text`, `result`, `divider`) append to the `.cb.md` file and exit immediately. The display engine watches the file and re-renders automatically.

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

## Selection and Send-to-Terminal

Select text on the board. Two buttons appear:

- **Send to terminal** — converts LaTeX to Unicode (`\hat{H}\psi = E\psi` → `Ĥψ = Eψ`), copies to clipboard with step context
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

## License

MIT
