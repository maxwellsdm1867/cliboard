use cliboard::document::{Block, Theme};
use cliboard::parser;
use cliboard::render;

// ---------------------------------------------------------------------------
// Parse → Render pipeline tests
// ---------------------------------------------------------------------------

#[test]
fn test_parse_render_simple_step() {
    let input = "---\ntitle: Test\n---\n\n## Step One\n\n$$E = mc^2$$\n";
    let doc = parser::parse(input);
    let html = render::render_blocks_html(&doc);

    assert!(html.contains("data-step-id=\"1\""));
    assert!(html.contains("Step One"));
    assert!(html.contains("katex"));
    assert!(html.contains("equation-card"));
}

#[test]
fn test_parse_render_result_step() {
    let input = "## Final {.result}\n\n$$F = ma$$\n";
    let doc = parser::parse(input);
    let html = render::render_blocks_html(&doc);

    assert!(html.contains("class=\"step result\""));
    assert!(html.contains("Final"));
}

#[test]
fn test_parse_render_prose_and_divider() {
    let input = "Some intro.\n\n---\n\n## Step\n\n$$x$$\n";
    let doc = parser::parse(input);
    let html = render::render_blocks_html(&doc);

    assert!(html.contains("class=\"prose\""));
    assert!(html.contains("Some intro."));
    assert!(html.contains("<hr class=\"divider\">"));
    assert!(html.contains("equation-card"));
}

#[test]
fn test_parse_render_full_page() {
    let input = "---\ntitle: Physics Board\ntheme: dark\n---\n\n## Newton\n\n$$F = ma$$\n";
    let doc = parser::parse(input);
    let html = render::render_full_page(&doc);

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Physics Board"));
    assert!(html.contains("data-theme=\"dark\""));
    assert!(html.contains("data-step-id=\"1\""));
    assert!(html.contains("viewer.js"));
}

#[test]
fn test_parse_render_notes_with_inline_math() {
    let input = "## Step\n\n$$x$$\n\n> Where $\\alpha$ is the constant\n";
    let doc = parser::parse(input);
    let html = render::render_blocks_html(&doc);

    // The note should contain rendered inline math (katex, no katex-display)
    assert!(html.contains("class=\"note\""));
    assert!(html.contains("katex"));
}

// ---------------------------------------------------------------------------
// Full derivation: hydrogen atom (from TRD)
// ---------------------------------------------------------------------------

#[test]
fn test_full_derivation_hydrogen_atom() {
    let input = r#"---
title: Hydrogen Atom Energy Levels
theme: dark
---

The hydrogen atom is the simplest atomic system.

## Schrödinger Equation

$$-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi$$

> The time-independent Schrödinger equation

## Coulomb Potential

$$V(r) = -\frac{e^2}{4\pi\epsilon_0 r}$$

> Electrostatic potential between proton and electron

---

## Radial Equation

$$-\frac{\hbar^2}{2m}\frac{d^2u}{dr^2} + \left[ V + \frac{\hbar^2}{2m}\frac{l(l+1)}{r^2} \right] u = Eu$$

> After separation of variables

## Energy Eigenvalues {.result}

$$E_n = -\frac{13.6 \text{ eV}}{n^2}$$

> Where $n = 1, 2, 3, \ldots$ is the principal quantum number
"#;

    let doc = parser::parse(input);

    // Verify document metadata
    assert_eq!(doc.title, "Hydrogen Atom Energy Levels");
    assert_eq!(doc.theme, Theme::Dark);

    // Count blocks: prose + 3 steps + divider + 1 step (result)
    assert_eq!(doc.step_count(), 4);

    // Verify the last step is a result
    let last_step = doc.blocks.iter().rev().find(|b| matches!(b, Block::Step { .. }));
    match last_step {
        Some(Block::Step { title, is_result, .. }) => {
            assert_eq!(title, "Energy Eigenvalues");
            assert!(is_result);
        }
        _ => panic!("Expected final result step"),
    }

    // Render all blocks — should not panic even with complex LaTeX
    let html = render::render_blocks_html(&doc);
    assert!(!html.is_empty());
    assert!(html.contains("katex"));

    // Render full page
    let full = render::render_full_page(&doc);
    assert!(full.contains("Hydrogen Atom Energy Levels"));
    assert!(full.contains("data-theme=\"dark\""));
}

// ---------------------------------------------------------------------------
// Export pipeline test
// ---------------------------------------------------------------------------

#[test]
fn test_export_pipeline() {
    let input = "---\ntitle: Export Test\n---\n\n## Step 1\n\n$$a + b = c$$\n\n## Result {.result}\n\n$$x = 42$$\n";
    let doc = parser::parse(input);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("export_test.html");
    let path_str = path.to_str().unwrap();

    cliboard::export::export_html(&doc, path_str).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();

    // Verify it's a complete HTML page
    assert!(content.contains("<!DOCTYPE html>"));
    assert!(content.contains("Export Test"));

    // Verify steps are rendered
    assert!(content.contains("equation-card"));
    assert!(content.contains("katex"));

    // Verify it's static (no scripts)
    assert!(!content.contains("<script"));

    // Verify step count
    assert!(content.contains("2 steps"));

    // Verify CDN font references
    assert!(content.contains("cdn.jsdelivr.net"));
}

// ---------------------------------------------------------------------------
// Round-trip: parse → step_count matches line counting
// ---------------------------------------------------------------------------

#[test]
fn test_step_count_matches_heading_count() {
    let input = "## A\n\n$$x$$\n\n## B\n\n$$y$$\n\n## C {.result}\n\n$$z$$\n";
    let doc = parser::parse(input);

    let heading_count = input.lines().filter(|l| l.starts_with("## ")).count();
    assert_eq!(doc.step_count(), heading_count);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_empty_input_parse_render() {
    let doc = parser::parse("");
    assert_eq!(doc.title, "Untitled");
    assert!(doc.blocks.is_empty());

    let html = render::render_blocks_html(&doc);
    assert!(html.is_empty());

    let full = render::render_full_page(&doc);
    assert!(full.contains("Untitled"));
}

#[test]
fn test_only_frontmatter_no_content() {
    let input = "---\ntitle: Empty Board\ntheme: light\n---\n";
    let doc = parser::parse(input);
    assert_eq!(doc.title, "Empty Board");
    assert_eq!(doc.theme, Theme::Light);
    assert!(doc.blocks.is_empty());
    assert_eq!(doc.step_count(), 0);
}

#[test]
fn test_invalid_latex_does_not_panic() {
    // Use a known-invalid LaTeX command that KaTeX will reject.
    // We construct the Document directly to avoid markdown parser interference.
    let doc = cliboard::document::Document {
        title: "Bad".to_string(),
        theme: Theme::Dark,
        blocks: vec![Block::Step {
            id: 1,
            title: "Bad Math".to_string(),
            equations: vec!["\\frac{".to_string()],
            notes: vec![],
            is_result: false,
        }],
    };

    // Rendering should produce error card, not panic
    let html = render::render_blocks_html(&doc);
    assert!(html.contains("error-card"));
    assert!(html.contains("error-msg"));
}

#[test]
fn test_multiple_result_steps() {
    let input = "## R1 {.result}\n\n$$a$$\n\n## R2 {.result}\n\n$$b$$\n";
    let doc = parser::parse(input);

    for block in &doc.blocks {
        match block {
            Block::Step { is_result, .. } => assert!(is_result),
            _ => panic!("Expected Step"),
        }
    }
}

#[test]
fn test_parse_render_preserves_latex_in_data_attr() {
    let input = "## Step\n\n$$E = mc^2$$\n";
    let doc = parser::parse(input);
    let html = render::render_blocks_html(&doc);

    assert!(html.contains("data-latex=\"E = mc^2\""));
}

// Test KaTeX rendering from a thread simulating the file watcher
#[test]
fn test_katex_rendering_from_spawned_thread() {
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .name("test-watcher".into())
        .spawn(|| {
            let input = r#"---
title: Test
---

## Schrödinger Equation

$$\hat{H}\psi = E\psi$$

> Time-independent form

## Expand the Hamiltonian

$$-\frac{\hbar^2}{2m}\nabla^2\psi + V(r)\psi = E\psi$$

## Energy Levels {.result}

$$E_n = -\frac{13.6 \text{ eV}}{n^2}$$
"#;
            let doc = parser::parse(input);
            let html = render::render_blocks_html(&doc);
            // Should render successfully (no error cards)
            assert!(!html.contains("error-card"), "KaTeX rendering failed in spawned thread: {}", 
                html.chars().take(500).collect::<String>());
            assert!(html.contains("katex-display"), "Missing katex-display in output");
        })
        .unwrap();
    handle.join().unwrap();
}
