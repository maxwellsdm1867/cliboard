use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::document::{Block, Document, Theme};

/// Parse a `.cb.md` file into a Document model.
pub fn parse(content: &str) -> Document {
    let (frontmatter, body) = split_frontmatter(content);

    let mut doc = if let Some(fm) = frontmatter {
        parse_frontmatter(fm)
    } else {
        Document::new("Untitled")
    };

    let options = Options::ENABLE_MATH
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let parser = Parser::new_ext(body, options);

    let mut step_count: usize = 0;
    let mut state = ParseState::TopLevel;
    let mut text_buf = String::new();
    let mut in_blockquote = false;

    // Current step being built
    let mut cur_title = String::new();
    let mut cur_equations: Vec<String> = Vec::new();
    let mut cur_notes: Vec<String> = Vec::new();
    let mut cur_is_result = false;

    for event in parser {
        match event {
            // --- Headings ---
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                classes,
                ..
            }) => {
                // Flush any previous step
                flush_step(
                    &mut doc,
                    &mut state,
                    &mut step_count,
                    &mut cur_title,
                    &mut cur_equations,
                    &mut cur_notes,
                    &mut cur_is_result,
                );
                // Flush any pending prose
                flush_prose(&mut doc, &mut state, &mut text_buf);

                cur_is_result = classes.iter().any(|c| c.as_ref() == "result");
                state = ParseState::InHeading;
                text_buf.clear();
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                cur_title = text_buf.trim().to_string();
                text_buf.clear();
                step_count += 1;
                state = ParseState::InStep;
            }

            // --- Display math ---
            Event::DisplayMath(latex) => {
                if state == ParseState::InStep {
                    cur_equations.push(latex.to_string());
                }
            }

            // --- Inline math: preserve in text ---
            Event::InlineMath(latex) => {
                text_buf.push('$');
                text_buf.push_str(&latex);
                text_buf.push('$');
            }

            // --- Blockquotes ---
            Event::Start(Tag::BlockQuote(_)) => {
                in_blockquote = true;
                text_buf.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_blockquote = false;
                let note = text_buf.trim().to_string();
                if !note.is_empty() {
                    if state == ParseState::InStep {
                        cur_notes.push(note);
                    } else {
                        // Blockquote outside a step: treat as prose
                        doc.blocks.push(Block::Prose { content: note });
                    }
                }
                text_buf.clear();
            }

            // --- Horizontal rule (divider) ---
            Event::Rule => {
                flush_step(
                    &mut doc,
                    &mut state,
                    &mut step_count,
                    &mut cur_title,
                    &mut cur_equations,
                    &mut cur_notes,
                    &mut cur_is_result,
                );
                flush_prose(&mut doc, &mut state, &mut text_buf);
                doc.blocks.push(Block::Divider);
            }

            // --- Paragraphs ---
            Event::Start(Tag::Paragraph) => {
                // Only start collecting if we're not in a blockquote (handled separately)
                if !in_blockquote {
                    text_buf.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if in_blockquote {
                    // Text already accumulated for blockquote; add newline between paragraphs
                    text_buf.push('\n');
                } else {
                    let trimmed = text_buf.trim().to_string();
                    if !trimmed.is_empty() {
                        match state {
                            ParseState::InStep => {
                                cur_notes.push(trimmed);
                            }
                            _ => {
                                doc.blocks.push(Block::Prose {
                                    content: trimmed,
                                });
                            }
                        }
                    }
                    text_buf.clear();
                }
            }

            // --- Text content ---
            Event::Text(text) => {
                text_buf.push_str(&text);
            }
            Event::Code(code) => {
                text_buf.push('`');
                text_buf.push_str(&code);
                text_buf.push('`');
            }
            Event::SoftBreak => {
                text_buf.push(' ');
            }
            Event::HardBreak => {
                text_buf.push('\n');
            }

            // --- Metadata blocks (YAML frontmatter handled by pulldown-cmark) ---
            Event::Start(Tag::MetadataBlock(_)) | Event::End(TagEnd::MetadataBlock(_)) => {
                // Already parsed frontmatter manually; skip
            }

            _ => {}
        }
    }

    // Flush any remaining step or prose
    flush_step(
        &mut doc,
        &mut state,
        &mut step_count,
        &mut cur_title,
        &mut cur_equations,
        &mut cur_notes,
        &mut cur_is_result,
    );
    flush_prose(&mut doc, &mut state, &mut text_buf);

    doc
}

#[derive(Debug, PartialEq)]
enum ParseState {
    TopLevel,
    InHeading,
    InStep,
}

fn flush_step(
    doc: &mut Document,
    state: &mut ParseState,
    step_count: &mut usize,
    title: &mut String,
    equations: &mut Vec<String>,
    notes: &mut Vec<String>,
    is_result: &mut bool,
) {
    if *state == ParseState::InStep {
        doc.blocks.push(Block::Step {
            id: *step_count,
            title: std::mem::take(title),
            equations: std::mem::take(equations),
            notes: std::mem::take(notes),
            is_result: *is_result,
        });
        *is_result = false;
        *state = ParseState::TopLevel;
    }
}

fn flush_prose(doc: &mut Document, state: &mut ParseState, text_buf: &mut String) {
    if *state == ParseState::TopLevel {
        let trimmed = text_buf.trim().to_string();
        if !trimmed.is_empty() {
            doc.blocks.push(Block::Prose { content: trimmed });
        }
        text_buf.clear();
    }
}

/// Split content into optional YAML frontmatter and body.
/// Frontmatter is delimited by `---` at the very start of the file.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    // Find the opening ---
    let after_open = &trimmed[3..];
    // Must be followed by newline or be at end
    let after_open = if let Some(rest) = after_open.strip_prefix("\r\n") {
        rest
    } else if let Some(rest) = after_open.strip_prefix('\n') {
        rest
    } else {
        return (None, content);
    };

    // Find closing ---
    if let Some(end_pos) = find_closing_fence(after_open) {
        let frontmatter = &after_open[..end_pos];
        let rest_start = end_pos + 3; // skip the ---
        let rest = &after_open[rest_start..];
        // Skip the newline after closing ---
        let rest = if let Some(r) = rest.strip_prefix("\r\n") {
            r
        } else if let Some(r) = rest.strip_prefix('\n') {
            r
        } else {
            rest
        };
        (Some(frontmatter), rest)
    } else {
        (None, content)
    }
}

/// Find byte position of closing `---` or `...` that starts at the beginning of a line.
fn find_closing_fence(s: &str) -> Option<usize> {
    let mut pos = 0;
    for line in s.lines() {
        if line.trim() == "---" || line.trim() == "..." {
            return Some(pos);
        }
        pos += line.len();
        // Account for the line ending
        if s.as_bytes().get(pos) == Some(&b'\r') {
            pos += 1;
        }
        if s.as_bytes().get(pos) == Some(&b'\n') {
            pos += 1;
        }
    }
    None
}

/// Parse simple YAML frontmatter (title + theme) without a YAML library.
fn parse_frontmatter(fm: &str) -> Document {
    let mut title = "Untitled".to_string();
    let mut theme = Theme::default();

    for line in fm.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("title:") {
            title = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = line.strip_prefix("theme:") {
            let val = val.trim().trim_matches('"').trim_matches('\'').to_lowercase();
            if val == "light" {
                theme = Theme::Light;
            }
        }
    }

    Document {
        title,
        theme,
        blocks: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Block;

    #[test]
    fn test_empty_document() {
        let doc = parse("");
        assert_eq!(doc.title, "Untitled");
        assert_eq!(doc.theme, Theme::Dark);
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn test_frontmatter_parsing() {
        let input = "---\ntitle: My Board\ntheme: light\n---\n";
        let doc = parse(input);
        assert_eq!(doc.title, "My Board");
        assert_eq!(doc.theme, Theme::Light);
    }

    #[test]
    fn test_frontmatter_quoted_title() {
        let input = "---\ntitle: \"Quoted Title\"\ntheme: dark\n---\n";
        let doc = parse(input);
        assert_eq!(doc.title, "Quoted Title");
        assert_eq!(doc.theme, Theme::Dark);
    }

    #[test]
    fn test_single_step() {
        let input = "---\ntitle: Test\n---\n\n## Step One\n\n$$E = mc^2$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Step {
                id,
                title,
                equations,
                is_result,
                ..
            } => {
                assert_eq!(*id, 1);
                assert_eq!(title, "Step One");
                assert_eq!(equations, &["E = mc^2"]);
                assert!(!is_result);
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_result_step() {
        let input = "## Final Answer {.result}\n\n$$x = 42$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Step {
                title, is_result, ..
            } => {
                assert_eq!(title, "Final Answer");
                assert!(is_result);
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_blockquote_note() {
        let input = "## Step 1\n\n$$a + b$$\n\n> This is a note\n";
        let doc = parse(input);
        match &doc.blocks[0] {
            Block::Step { notes, .. } => {
                assert_eq!(notes.len(), 1);
                assert_eq!(notes[0], "This is a note");
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_divider() {
        let input = "## Step 1\n\n$$x$$\n\n---\n\n## Step 2\n\n$$y$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 3);
        assert!(matches!(doc.blocks[1], Block::Divider));
    }

    #[test]
    fn test_prose_block() {
        let input = "Some introductory text.\n\n## Step 1\n\n$$x$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 2);
        match &doc.blocks[0] {
            Block::Prose { content } => {
                assert_eq!(content, "Some introductory text.");
            }
            _ => panic!("Expected Prose block"),
        }
    }

    #[test]
    fn test_step_counting() {
        let input = "## First\n\n$$a$$\n\n## Second\n\n$$b$$\n\n## Third\n\n$$c$$\n";
        let doc = parse(input);
        assert_eq!(doc.step_count(), 3);
        match &doc.blocks[0] {
            Block::Step { id, .. } => assert_eq!(*id, 1),
            _ => panic!("Expected Step"),
        }
        match &doc.blocks[1] {
            Block::Step { id, .. } => assert_eq!(*id, 2),
            _ => panic!("Expected Step"),
        }
        match &doc.blocks[2] {
            Block::Step { id, .. } => assert_eq!(*id, 3),
            _ => panic!("Expected Step"),
        }
    }

    #[test]
    fn test_inline_math_in_note() {
        let input = "## Step 1\n\nWhere $x$ is the variable\n";
        let doc = parse(input);
        match &doc.blocks[0] {
            Block::Step { notes, .. } => {
                assert!(notes[0].contains("$x$"));
            }
            _ => panic!("Expected Step"),
        }
    }

    #[test]
    fn test_no_frontmatter() {
        let input = "## Step 1\n\n$$y = mx + b$$\n";
        let doc = parse(input);
        assert_eq!(doc.title, "Untitled");
        assert_eq!(doc.theme, Theme::Dark);
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn test_frontmatter_title_only() {
        let input = "---\ntitle: Just a Title\n---\n";
        let doc = parse(input);
        assert_eq!(doc.title, "Just a Title");
        assert_eq!(doc.theme, Theme::Dark); // default
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn test_frontmatter_single_quoted_values() {
        let input = "---\ntitle: 'Single Quoted'\ntheme: 'light'\n---\n";
        let doc = parse(input);
        assert_eq!(doc.title, "Single Quoted");
        assert_eq!(doc.theme, Theme::Light);
    }

    #[test]
    fn test_step_with_multiple_equations() {
        let input = "## Multi Eq Step\n\n$$a = b$$\n\n$$c = d$$\n\n$$e = f$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Step { equations, title, .. } => {
                assert_eq!(title, "Multi Eq Step");
                assert_eq!(equations.len(), 3);
                assert_eq!(equations[0], "a = b");
                assert_eq!(equations[1], "c = d");
                assert_eq!(equations[2], "e = f");
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_step_with_notes() {
        let input = "## Step With Notes\n\n$$x = 1$$\n\n> First note\n\n> Second note\n";
        let doc = parse(input);
        match &doc.blocks[0] {
            Block::Step { notes, .. } => {
                assert_eq!(notes.len(), 2);
                assert_eq!(notes[0], "First note");
                assert_eq!(notes[1], "Second note");
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_prose_between_steps() {
        // Text paragraphs inside a step (after heading) are treated as notes,
        // not as separate Prose blocks. Prose blocks only appear at top level.
        let input = "Some prose before.\n\n## Step 1\n\n$$a$$\n\n## Step 2\n\n$$b$$\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 3);
        match &doc.blocks[0] {
            Block::Prose { content } => {
                assert_eq!(content, "Some prose before.");
            }
            _ => panic!("Expected Prose block, got {:?}", doc.blocks[0]),
        }
        assert!(matches!(&doc.blocks[1], Block::Step { .. }));
        assert!(matches!(&doc.blocks[2], Block::Step { .. }));
    }

    #[test]
    fn test_multiple_dividers() {
        let input = "---\ntitle: T\n---\n\n## S1\n\n$$a$$\n\n---\n\n## S2\n\n$$b$$\n\n---\n";
        let doc = parse(input);
        // S1, divider, S2, divider
        assert_eq!(doc.blocks.len(), 4);
        assert!(matches!(doc.blocks[0], Block::Step { .. }));
        assert!(matches!(doc.blocks[1], Block::Divider));
        assert!(matches!(doc.blocks[2], Block::Step { .. }));
        assert!(matches!(doc.blocks[3], Block::Divider));
    }

    #[test]
    fn test_inline_math_in_blockquote_note() {
        let input = "## Step\n\n$$x$$\n\n> Where $\\alpha$ is the coefficient\n";
        let doc = parse(input);
        match &doc.blocks[0] {
            Block::Step { notes, .. } => {
                assert!(notes[0].contains("$\\alpha$"));
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_mixed_content() {
        let input = "---\ntitle: Mixed\n---\n\nIntro text.\n\n## Step 1\n\n$$E = mc^2$$\n\n> Famous equation\n\n---\n\nMiddle prose.\n\n## Result {.result}\n\n$$F = ma$$\n";
        let doc = parse(input);
        assert_eq!(doc.title, "Mixed");
        // Intro prose, Step1, Divider, Middle prose, Result step
        assert_eq!(doc.blocks.len(), 5);
        match &doc.blocks[0] {
            Block::Prose { content } => assert_eq!(content, "Intro text."),
            _ => panic!("Expected Prose"),
        }
        match &doc.blocks[1] {
            Block::Step { id, title, equations, notes, is_result } => {
                assert_eq!(*id, 1);
                assert_eq!(title, "Step 1");
                assert_eq!(equations, &["E = mc^2"]);
                assert_eq!(notes, &["Famous equation"]);
                assert!(!is_result);
            }
            _ => panic!("Expected Step"),
        }
        assert!(matches!(doc.blocks[2], Block::Divider));
        match &doc.blocks[3] {
            Block::Prose { content } => assert_eq!(content, "Middle prose."),
            _ => panic!("Expected Prose"),
        }
        match &doc.blocks[4] {
            Block::Step { id, title, is_result, .. } => {
                assert_eq!(*id, 2);
                assert_eq!(title, "Result");
                assert!(is_result);
            }
            _ => panic!("Expected Step"),
        }
    }

    #[test]
    fn test_hydrogen_atom_derivation() {
        // A realistic multi-step derivation similar to TRD examples
        let input = r#"---
title: Hydrogen Atom Energy Levels
---

## Schrödinger Equation

$$-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi$$

> The time-independent Schrödinger equation for the hydrogen atom

## Coulomb Potential

$$V(r) = -\frac{e^2}{4\pi\epsilon_0 r}$$

> The electrostatic potential between electron and proton

## Energy Eigenvalues {.result}

$$E_n = -\frac{m_e e^4}{2\hbar^2} \cdot \frac{1}{n^2}$$

> Where $n = 1, 2, 3, \ldots$ is the principal quantum number
"#;
        let doc = parse(input);
        assert_eq!(doc.title, "Hydrogen Atom Energy Levels");
        assert_eq!(doc.step_count(), 3);

        // Check step IDs are sequential
        let mut ids: Vec<usize> = Vec::new();
        for block in &doc.blocks {
            if let Block::Step { id, .. } = block {
                ids.push(*id);
            }
        }
        assert_eq!(ids, vec![1, 2, 3]);

        // Last step should be a result
        match &doc.blocks[2] {
            Block::Step { is_result, title, .. } => {
                assert!(is_result);
                assert_eq!(title, "Energy Eigenvalues");
            }
            _ => panic!("Expected result step"),
        }
    }

    #[test]
    fn test_step_without_equation() {
        let input = "## Empty Step\n\nJust some text, no equation.\n";
        let doc = parse(input);
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Step { equations, notes, .. } => {
                assert!(equations.is_empty());
                assert_eq!(notes.len(), 1);
                assert_eq!(notes[0], "Just some text, no equation.");
            }
            _ => panic!("Expected Step block"),
        }
    }

    #[test]
    fn test_sequential_step_ids() {
        let input = "## A\n\n$$x$$\n\n## B\n\n$$y$$\n\n## C\n\n$$z$$\n\n## D\n\n$$w$$\n";
        let doc = parse(input);
        assert_eq!(doc.step_count(), 4);
        for (i, block) in doc.blocks.iter().enumerate() {
            match block {
                Block::Step { id, .. } => assert_eq!(*id, i + 1),
                _ => panic!("Expected Step"),
            }
        }
    }
}
