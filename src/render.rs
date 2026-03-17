use crate::document::{Block, Document, Theme};

/// Render a single LaTeX equation to HTML using KaTeX (display mode).
pub fn render_equation(latex: &str) -> Result<String, String> {
    let opts = katex::Opts::builder()
        .display_mode(true)
        .build()
        .unwrap();
    katex::render_with_opts(latex, opts).map_err(|e| e.to_string())
}

/// Render inline LaTeX (not display mode).
pub fn render_inline_math(latex: &str) -> Result<String, String> {
    let opts = katex::Opts::builder()
        .display_mode(false)
        .build()
        .unwrap();
    katex::render_with_opts(latex, opts).map_err(|e| e.to_string())
}

/// HTML-escape a string for use in attributes.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Process inline math (`$...$`) in text, rendering each with KaTeX.
/// Non-math text is HTML-escaped to prevent XSS.
/// Does not match `$$...$$` (display math).
fn process_inline_math(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // Collect runs of non-math text and escape them together
    let mut text_buf = String::new();

    let flush_text = |buf: &mut String, out: &mut String| {
        if !buf.is_empty() {
            out.push_str(&html_escape(buf));
            buf.clear();
        }
    };

    while i < len {
        // Skip $$ (not inline math)
        if i + 1 < len && chars[i] == '$' && chars[i + 1] == '$' {
            text_buf.push('$');
            text_buf.push('$');
            i += 2;
            continue;
        }

        if chars[i] == '$' {
            // Look for closing $
            let start = i + 1;
            let mut end = None;
            let mut j = start;
            while j < len {
                if j + 1 < len && chars[j] == '$' && chars[j + 1] == '$' {
                    j += 2;
                    continue;
                }
                if chars[j] == '$' {
                    end = Some(j);
                    break;
                }
                j += 1;
            }

            if let Some(end_pos) = end {
                let latex: String = chars[start..end_pos].iter().collect();
                if latex.is_empty() {
                    text_buf.push('$');
                    i += 1;
                    continue;
                }
                flush_text(&mut text_buf, &mut result);
                match render_inline_math(&latex) {
                    Ok(html) => result.push_str(&html),
                    Err(_) => {
                        result.push_str("<code class=\"error-inline\">");
                        result.push('$');
                        result.push_str(&html_escape(&latex));
                        result.push('$');
                        result.push_str("</code>");
                    }
                }
                i = end_pos + 1;
            } else {
                text_buf.push('$');
                i += 1;
            }
        } else {
            text_buf.push(chars[i]);
            i += 1;
        }
    }

    flush_text(&mut text_buf, &mut result);
    result
}

/// Render a single block to HTML (used by tests that don't need equation numbers).
fn render_block(block: &Block) -> String {
    let mut eq_num: usize = 0;
    render_block_numbered(block, &mut eq_num)
}

/// Render a single block to HTML with sequential equation numbering.
fn render_block_numbered(block: &Block, eq_number: &mut usize) -> String {
    match block {
        Block::Step {
            id,
            title,
            equations,
            notes,
            is_result,
        } => {
            let class = if *is_result { "step result" } else { "step" };
            let data_latex = equations
                .first()
                .map(|eq| html_escape(eq))
                .unwrap_or_default();

            let mut html = format!(
                "<div class=\"{}\" data-step-id=\"{}\" data-step-title=\"{}\" data-latex=\"{}\">",
                class,
                id,
                html_escape(title),
                data_latex
            );

            // Step header (titles support inline math via $...$)
            html.push_str(&format!(
                "<div class=\"step-header\"><span class=\"step-number\">{}.</span><span class=\"step-title\">{}</span></div>",
                id,
                process_inline_math(title)
            ));

            // Equations with numbers
            for eq in equations {
                *eq_number += 1;
                match render_equation(eq) {
                    Ok(rendered) => {
                        html.push_str(&format!(
                            "<div class=\"equation-card\"><div class=\"equation-content\">{}</div><span class=\"equation-number\">({})</span></div>",
                            rendered, eq_number
                        ));
                    }
                    Err(err_msg) => {
                        html.push_str("<div class=\"equation-card error-card\">");
                        html.push_str("<code>");
                        html.push_str(&html_escape(eq));
                        html.push_str("</code>");
                        html.push_str("<div class=\"error-msg\">");
                        html.push_str(&html_escape(&err_msg));
                        html.push_str("</div></div>");
                    }
                }
            }

            // Notes (with inline math)
            for note in notes {
                html.push_str("<div class=\"note\">");
                html.push_str(&process_inline_math(note));
                html.push_str("</div>");
            }

            html.push_str("</div>");
            html
        }
        Block::Prose { content } => {
            format!(
                "<div class=\"prose\">{}</div>",
                process_inline_math(content)
            )
        }
        Block::Divider => "<hr class=\"divider\">".to_string(),
    }
}

/// Render all blocks to HTML string (just the blocks, not the full page).
pub fn render_blocks_html(doc: &Document) -> String {
    // Pre-allocate: ~1KB per block is a reasonable estimate
    let mut html = String::with_capacity(doc.blocks.len() * 1024);
    let mut eq_number: usize = 0;
    for block in &doc.blocks {
        html.push_str(&render_block_numbered(block, &mut eq_number));
    }
    html
}

/// Render the full HTML page (viewer shell + rendered blocks).
pub fn render_full_page(doc: &Document) -> String {
    let theme_class = match doc.theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    };
    let blocks_html = render_blocks_html(doc);
    let title_escaped = html_escape(&doc.title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="{theme_class}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title_escaped} — cliboard</title>
    <link rel="stylesheet" href="/katex.min.css">
    <link rel="stylesheet" href="/viewer.css">
</head>
<body>
    <div id="board">
        <header id="board-header">
            <h1>{title_escaped}</h1>
        </header>
        <main id="board-content">
            {blocks_html}
        </main>
    </div>
    <script src="/viewer.js"></script>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn test_render_equation_success() {
        let result = render_equation("E = mc^2");
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("katex"));
    }

    #[test]
    fn test_render_equation_error() {
        let result = render_equation(r"\");
        assert!(result.is_err());
    }

    #[test]
    fn test_render_inline_math() {
        let result = render_inline_math("x^2");
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("katex"));
        // Inline mode should NOT contain katex-display
        assert!(!html.contains("katex-display"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<div>"), "&lt;div&gt;");
        assert_eq!(html_escape("a&b"), "a&amp;b");
        assert_eq!(html_escape("x=\"y\""), "x=&quot;y&quot;");
    }

    #[test]
    fn test_process_inline_math() {
        let result = process_inline_math("The value $x^2$ is positive");
        assert!(result.contains("katex"));
        assert!(result.contains("The value "));
        assert!(result.contains(" is positive"));
    }

    #[test]
    fn test_process_inline_math_no_math() {
        let result = process_inline_math("No math here");
        assert_eq!(result, "No math here");
    }

    #[test]
    fn test_process_inline_math_error() {
        let result = process_inline_math("Bad: $\\$ end");
        assert!(result.contains("error-inline"));
    }

    #[test]
    fn test_render_blocks_html_step() {
        let doc = Document {
            title: "Test".to_string(),
            theme: Theme::Dark,
            blocks: vec![Block::Step {
                id: 1,
                title: "First Step".to_string(),
                equations: vec!["E = mc^2".to_string()],
                notes: vec!["Energy equation".to_string()],
                is_result: false,
            }],
        };
        let html = render_blocks_html(&doc);
        assert!(html.contains("data-step-id=\"1\""));
        assert!(html.contains("First Step"));
        assert!(html.contains("equation-card"));
        assert!(html.contains("note"));
    }

    #[test]
    fn test_render_blocks_html_result_step() {
        let doc = Document {
            title: "Test".to_string(),
            theme: Theme::Dark,
            blocks: vec![Block::Step {
                id: 2,
                title: "Result".to_string(),
                equations: vec!["F = ma".to_string()],
                notes: vec![],
                is_result: true,
            }],
        };
        let html = render_blocks_html(&doc);
        assert!(html.contains("class=\"step result\""));
    }

    #[test]
    fn test_render_blocks_html_prose() {
        let doc = Document {
            title: "Test".to_string(),
            theme: Theme::Dark,
            blocks: vec![Block::Prose {
                content: "Some text with $x$ inline".to_string(),
            }],
        };
        let html = render_blocks_html(&doc);
        assert!(html.contains("class=\"prose\""));
        assert!(html.contains("katex"));
    }

    #[test]
    fn test_render_blocks_html_divider() {
        let doc = Document {
            title: "Test".to_string(),
            theme: Theme::Dark,
            blocks: vec![Block::Divider],
        };
        let html = render_blocks_html(&doc);
        assert!(html.contains("<hr class=\"divider\">"));
    }

    #[test]
    fn test_render_full_page() {
        let doc = Document {
            title: "Physics".to_string(),
            theme: Theme::Dark,
            blocks: vec![Block::Divider],
        };
        let html = render_full_page(&doc);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Physics"));
        assert!(html.contains("data-theme=\"dark\""));
        assert!(html.contains("viewer.js"));
    }

    #[test]
    fn test_render_full_page_light_theme() {
        let doc = Document {
            title: "Math".to_string(),
            theme: Theme::Light,
            blocks: vec![],
        };
        let html = render_full_page(&doc);
        assert!(html.contains("data-theme=\"light\""));
    }

    #[test]
    fn test_render_equation_simple_display() {
        let result = render_equation("a + b = c");
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("katex-display"));
    }

    #[test]
    fn test_render_equation_invalid_latex_error_card() {
        // Completely invalid LaTeX should produce an error
        let result = render_equation(r"\invalidcommand{");
        assert!(result.is_err());
    }

    #[test]
    fn test_render_block_step_html_structure() {
        let block = Block::Step {
            id: 3,
            title: "Test Step".to_string(),
            equations: vec!["x = 1".to_string()],
            notes: vec!["A note".to_string()],
            is_result: false,
        };
        let html = render_block(&block);
        assert!(html.contains("class=\"step\""));
        assert!(html.contains("data-step-id=\"3\""));
        assert!(html.contains("data-step-title=\"Test Step\""));
        assert!(html.contains("step-number"));
        assert!(html.contains("3."));
        assert!(html.contains("step-title"));
        assert!(html.contains("equation-card"));
        assert!(html.contains("class=\"note\""));
        assert!(html.contains("A note"));
    }

    #[test]
    fn test_render_block_prose() {
        let block = Block::Prose {
            content: "Simple paragraph".to_string(),
        };
        let html = render_block(&block);
        assert!(html.contains("class=\"prose\""));
        assert!(html.contains("Simple paragraph"));
    }

    #[test]
    fn test_render_block_divider() {
        let block = Block::Divider;
        let html = render_block(&block);
        assert_eq!(html, "<hr class=\"divider\">");
    }

    #[test]
    fn test_render_block_result_class() {
        let block = Block::Step {
            id: 1,
            title: "Final".to_string(),
            equations: vec!["y = 42".to_string()],
            notes: vec![],
            is_result: true,
        };
        let html = render_block(&block);
        assert!(html.contains("class=\"step result\""));
    }

    #[test]
    fn test_render_block_multiple_equations() {
        let block = Block::Step {
            id: 1,
            title: "Multi".to_string(),
            equations: vec!["a = 1".to_string(), "b = 2".to_string(), "c = 3".to_string()],
            notes: vec![],
            is_result: false,
        };
        let html = render_block(&block);
        // Should have 3 equation-card divs
        let count = html.matches("equation-card").count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_render_block_notes_with_inline_math() {
        let block = Block::Step {
            id: 1,
            title: "Notes".to_string(),
            equations: vec!["x = 1".to_string()],
            notes: vec!["Where $x$ is a variable".to_string()],
            is_result: false,
        };
        let html = render_block(&block);
        // Inline math should be rendered via KaTeX
        assert!(html.contains("katex"));
        assert!(html.contains("class=\"note\""));
    }

    #[test]
    fn test_html_escape_single_quote() {
        assert_eq!(html_escape("it's"), "it&#x27;s");
    }

    #[test]
    fn test_render_block_data_latex_escaped() {
        let block = Block::Step {
            id: 1,
            title: "Test".to_string(),
            equations: vec!["a < b & c > d".to_string()],
            notes: vec![],
            is_result: false,
        };
        let html = render_block(&block);
        assert!(html.contains("data-latex=\"a &lt; b &amp; c &gt; d\""));
    }

    #[test]
    fn test_render_empty_document() {
        let doc = Document {
            title: "Empty".to_string(),
            theme: Theme::Dark,
            blocks: vec![],
        };
        let html = render_blocks_html(&doc);
        assert!(html.is_empty());

        let full = render_full_page(&doc);
        assert!(full.contains("Empty"));
        assert!(full.contains("board-content"));
    }

    #[test]
    fn test_render_equation_error_card_in_step() {
        // Use invalid LaTeX that will trigger error-card rendering within a step
        let block = Block::Step {
            id: 1,
            title: "Bad".to_string(),
            equations: vec!["\\frac{".to_string()],
            notes: vec![],
            is_result: false,
        };
        let html = render_block(&block);
        assert!(html.contains("error-card"));
        assert!(html.contains("error-msg"));
    }
}
