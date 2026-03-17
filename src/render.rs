use crate::document::{Block, Document, Theme};

/// Render a single LaTeX equation to HTML using KaTeX (display mode).
///
/// Uses HTML-only output (no MathML) to stay within QuickJS's 256KB internal
/// stack limit. MathML generation is deeply recursive and unnecessary for our
/// browser-based viewer.
pub fn render_equation(latex: &str) -> Result<String, String> {
    let opts = katex::Opts::builder()
        .display_mode(true)
        .output_type(katex::OutputType::Html)
        .build()
        .unwrap();
    katex::render_with_opts(latex, opts).map_err(|e| e.to_string())
}

/// Render inline LaTeX (not display mode).
pub fn render_inline_math(latex: &str) -> Result<String, String> {
    let opts = katex::Opts::builder()
        .display_mode(false)
        .output_type(katex::OutputType::Html)
        .build()
        .unwrap();
    katex::render_with_opts(latex, opts).map_err(|e| e.to_string())
}

/// Render chat text with inline math support.
pub fn render_chat_text(text: &str) -> String {
    process_inline_math(text)
}

/// Render reply content with display equations (sub-numbered) and inline math.
/// Simple version — starts numbering from 1. Use `render_reply_content_ctx` for
/// context-aware numbering across multiple replies.
pub fn render_reply_content(text: &str, step_id: usize) -> String {
    render_reply_content_ctx(text, step_id, &std::collections::HashMap::new(), 0)
}

/// Render reply content with context-aware equation numbering.
///
/// - `known_eqs`: map of normalized LaTeX → existing equation number string (e.g., "1.2")
///   for reusing numbers when the same equation appears again.
/// - `eq_offset`: number of equations already assigned in previous replies for this step.
///   New equations start at `eq_offset + 1`.
pub fn render_reply_content_ctx(
    text: &str,
    step_id: usize,
    known_eqs: &std::collections::HashMap<String, String>,
    eq_offset: usize,
) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    result.push_str("<div class=\"reply-content\">");

    let mut next_sub = eq_offset;
    let mut rest = text;

    loop {
        match rest.find("$$") {
            Some(start) => {
                // Prose before the equation
                let prose = &rest[..start];
                let prose_trimmed = prose.trim();
                if !prose_trimmed.is_empty() {
                    result.push_str("<p class=\"reply-prose\">");
                    result.push_str(&process_inline_math(prose_trimmed));
                    result.push_str("</p>");
                }

                // Find closing $$
                let after_open = &rest[start + 2..];
                match after_open.find("$$") {
                    Some(end) => {
                        let latex = after_open[..end].trim();

                        if latex.is_empty() {
                            rest = &after_open[end + 2..];
                            continue;
                        }

                        // Check if this equation already has a number
                        let normalized = normalize_latex(latex);
                        let eq_num = if let Some(existing) = known_eqs.get(&normalized) {
                            existing.clone()
                        } else {
                            next_sub += 1;
                            format!("{}.{}", step_id, next_sub)
                        };

                        match render_equation(latex) {
                            Ok(rendered) => {
                                result.push_str(&format!(
                                    "<div class=\"equation-card reply-equation\" data-latex=\"{}\" data-eq-num=\"{}\"><div class=\"equation-content\">{}</div><span class=\"equation-number\">({})</span></div>",
                                    html_escape(latex), eq_num, rendered, eq_num
                                ));
                            }
                            Err(err_msg) => {
                                result.push_str("<div class=\"equation-card reply-equation error-card\">");
                                result.push_str("<code>");
                                result.push_str(&html_escape(latex));
                                result.push_str("</code>");
                                result.push_str("<div class=\"error-msg\">");
                                result.push_str(&html_escape(&err_msg));
                                result.push_str("</div></div>");
                            }
                        }
                        rest = &after_open[end + 2..];
                    }
                    None => {
                        let remaining = rest.trim();
                        if !remaining.is_empty() {
                            result.push_str("<p class=\"reply-prose\">");
                            result.push_str(&process_inline_math(remaining));
                            result.push_str("</p>");
                        }
                        break;
                    }
                }
            }
            None => {
                let remaining = rest.trim();
                if !remaining.is_empty() {
                    result.push_str("<p class=\"reply-prose\">");
                    result.push_str(&process_inline_math(remaining));
                    result.push_str("</p>");
                }
                break;
            }
        }
    }

    result.push_str("</div>");
    result
}

/// Normalize LaTeX for comparison: collapse whitespace, trim.
fn normalize_latex(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract equation context from existing reply messages for a step.
/// Returns (known_eqs map, next offset) for use with `render_reply_content_ctx`.
pub fn reply_equation_context(
    messages: &[crate::document::ChatMessage],
    step_id: usize,
) -> (std::collections::HashMap<String, String>, usize) {
    let mut known = std::collections::HashMap::new();
    let mut max_sub = 0usize;

    for msg in messages {
        if msg.step_id != step_id || msg.role != crate::document::ChatRole::Assistant {
            continue;
        }
        // Extract $$...$$ equations and their assigned numbers from this reply
        let mut rest = msg.text.as_str();
        loop {
            match rest.find("$$") {
                Some(start) => {
                    let after = &rest[start + 2..];
                    match after.find("$$") {
                        Some(end) => {
                            let latex = after[..end].trim();
                            if !latex.is_empty() {
                                let normalized = normalize_latex(latex);
                                if !known.contains_key(&normalized) {
                                    max_sub += 1;
                                    known.insert(normalized, format!("{}.{}", step_id, max_sub));
                                }
                            }
                            rest = &after[end + 2..];
                        }
                        None => break,
                    }
                }
                None => break,
            }
        }
    }

    (known, max_sub)
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
pub(crate) fn process_inline_math(text: &str) -> String {
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

    #[test]
    fn test_render_reply_content_prose_only() {
        let html = render_reply_content("Just some text with $x^2$ inline", 1);
        assert!(html.contains("reply-content"));
        assert!(html.contains("reply-prose"));
        assert!(html.contains("katex"));
        assert!(!html.contains("equation-card"));
    }

    #[test]
    fn test_render_reply_content_single_equation() {
        let html = render_reply_content("Before $$E = mc^2$$ after", 3);
        assert!(html.contains("reply-content"));
        assert!(html.contains("reply-equation"));
        assert!(html.contains("(3.1)"));
        assert!(html.contains("Before"));
        assert!(html.contains("after"));
    }

    #[test]
    fn test_render_reply_content_multiple_equations() {
        let html = render_reply_content("Start $$a = 1$$ middle $$b = 2$$ end", 2);
        assert!(html.contains("(2.1)"));
        assert!(html.contains("(2.2)"));
        assert!(html.contains("Start"));
        assert!(html.contains("middle"));
        assert!(html.contains("end"));
    }

    #[test]
    fn test_render_reply_content_equation_only() {
        let html = render_reply_content("$$x = 1$$", 1);
        assert!(html.contains("(1.1)"));
        assert!(html.contains("equation-card"));
        // No prose paragraphs
        assert!(!html.contains("reply-prose"));
    }

    #[test]
    fn test_render_reply_content_invalid_latex() {
        let html = render_reply_content("Bad: $$\\frac{$$ ok", 1);
        assert!(html.contains("error-card"));
        assert!(html.contains("ok"));
    }

    #[test]
    fn test_render_reply_content_empty() {
        let html = render_reply_content("", 1);
        assert!(html.contains("reply-content"));
        assert!(!html.contains("reply-prose"));
        assert!(!html.contains("equation-card"));
    }

    #[test]
    fn test_render_reply_content_inline_math_in_prose() {
        let html = render_reply_content("Where $n$ is the quantum number $$E_n = -13.6/n^2$$", 1);
        assert!(html.contains("(1.1)"));
        // Inline math in prose should be rendered
        assert!(html.contains("katex"));
    }

    #[test]
    fn test_render_reply_content_unclosed_display_math() {
        let html = render_reply_content("Text $$ unclosed", 1);
        // Should treat as prose, not crash
        assert!(html.contains("reply-prose"));
        assert!(!html.contains("equation-card"));
    }
}
