use crate::document::Document;
use crate::render;
use rust_embed::Embed;
use std::fs;

#[derive(Embed)]
#[folder = "katex-assets/"]
struct KatexAssets;

#[derive(Embed)]
#[folder = "assets/"]
struct ViewerAssets;

/// Export the document as a self-contained HTML file.
///
/// Inlines KaTeX CSS (with CDN font fallback), viewer CSS, and all
/// pre-rendered math HTML. No JavaScript — the output is a static snapshot.
pub fn export_html(doc: &Document, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let blocks_html = render::render_blocks_html(doc);

    // Load KaTeX CSS and rewrite font URLs to use CDN fallback
    let katex_css = KatexAssets::get("katex.min.css")
        .map(|f| {
            let css = String::from_utf8_lossy(&f.data).to_string();
            // Replace relative font paths with CDN URLs so the export
            // renders perfectly when online, and degrades gracefully offline.
            css.replace(
                "url(fonts/",
                "url(https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/fonts/",
            )
        })
        .unwrap_or_default();

    // Load viewer CSS
    let viewer_css = ViewerAssets::get("viewer.css")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    let title_escaped = html_escape(&doc.title);
    let step_count = doc.step_count();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title_escaped} — cliboard</title>
    <style>{katex_css}</style>
    <style>{viewer_css}</style>
</head>
<body>
    <div id="board">
        <header id="board-header">
            <h1 id="board-title">{title_escaped}</h1>
            <span id="step-count">{step_count} steps</span>
        </header>
        <div id="board-content">
            {blocks_html}
        </div>
    </div>
</body>
</html>"#
    );

    fs::write(output_path, html)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Block, Theme};

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<b>\"a&b\"</b>"), "&lt;b&gt;&quot;a&amp;b&quot;&lt;/b&gt;");
    }

    #[test]
    fn test_export_html_creates_file() {
        let doc = Document {
            title: "Test Export".to_string(),
            theme: Theme::Dark,
            blocks: vec![
                Block::Step {
                    id: 1,
                    title: "First Step".to_string(),
                    equations: vec!["E = mc^2".to_string()],
                    notes: vec!["Famous equation".to_string()],
                    is_result: false,
                },
                Block::Divider,
                Block::Step {
                    id: 2,
                    title: "Result".to_string(),
                    equations: vec!["F = ma".to_string()],
                    notes: vec![],
                    is_result: true,
                },
            ],
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_export.html");
        let path_str = path.to_str().unwrap();

        export_html(&doc, path_str).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Test Export"));
        assert!(content.contains("2 steps"));
        assert!(content.contains("katex"));
        assert!(content.contains("equation-card"));
        // No JavaScript in static export
        assert!(!content.contains("<script"));
        // CDN font URLs present
        assert!(content.contains("cdn.jsdelivr.net/npm/katex"));
    }

    #[test]
    fn test_export_html_empty_document() {
        let doc = Document::new("Empty");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.html");
        let path_str = path.to_str().unwrap();

        export_html(&doc, path_str).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Empty"));
        assert!(content.contains("0 steps"));
    }

    #[test]
    fn test_export_escapes_title() {
        let doc = Document::new("<script>alert('xss')</script>");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("xss.html");
        let path_str = path.to_str().unwrap();

        export_html(&doc, path_str).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("<script>alert"));
        assert!(content.contains("&lt;script&gt;"));
    }
}
