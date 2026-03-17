use crate::document::{Block, Document};
use crate::render;
use crate::session::{ChatContext, ChatMessage, ChatRole, ChatStore};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
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

/// Export the session as a structured JSON report.
///
/// Combines the derivation (steps, equations, notes) with the full
/// conversation history, grouped by step. Designed to be read by
/// downstream agents, used for reports, or fed into other tools.
pub fn export_json(
    doc: &Document,
    messages: &ChatStore,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut steps = Vec::new();

    for block in &doc.blocks {
        if let Block::Step {
            id,
            title,
            equations,
            notes,
            is_result,
        } = block
        {
            let chat: Vec<ChatEntry> = messages
                .messages
                .iter()
                .filter(|m| m.step_id == *id)
                .map(|m| ChatEntry {
                    role: m.role.to_string(),
                    text: m.text.clone(),
                    timestamp: m.timestamp.clone(),
                    context: m.context.as_ref().map(|c| ChatEntryContext {
                        selected: c.selected.clone(),
                        latex: c.latex.clone(),
                        step_title: c.step_title.clone(),
                    }),
                })
                .collect();

            steps.push(StepExport {
                id: *id,
                title: title.clone(),
                equations: equations.clone(),
                notes: notes.clone(),
                is_result: *is_result,
                chat,
            });
        }
    }

    let report = SessionReport {
        title: doc.title.clone(),
        step_count: doc.step_count(),
        message_count: messages.messages.len(),
        steps,
        // Include any messages not tied to a step (step_id=0 or orphaned)
        unattached_messages: messages
            .messages
            .iter()
            .filter(|m| !doc.blocks.iter().any(|b| matches!(b, Block::Step { id, .. } if *id == m.step_id)))
            .map(|m| ChatEntry {
                role: m.role.to_string(),
                text: m.text.clone(),
                timestamp: m.timestamp.clone(),
                context: m.context.as_ref().map(|c| ChatEntryContext {
                    selected: c.selected.clone(),
                    latex: c.latex.clone(),
                    step_title: c.step_title.clone(),
                }),
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, json)?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct SessionReport {
    title: String,
    #[serde(default)]
    step_count: usize,
    #[serde(default)]
    message_count: usize,
    steps: Vec<StepExport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unattached_messages: Vec<ChatEntry>,
}

#[derive(Serialize, Deserialize)]
struct StepExport {
    id: usize,
    title: String,
    #[serde(default)]
    equations: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    is_result: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chat: Vec<ChatEntry>,
}

#[derive(Serialize, Deserialize)]
struct ChatEntry {
    role: String,
    text: String,
    #[serde(default)]
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<ChatEntryContext>,
}

#[derive(Serialize, Deserialize)]
struct ChatEntryContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step_title: Option<String>,
}

/// Import a session from a JSON report.
///
/// Reconstructs the .cb.md board file and messages.json from a previously
/// exported (or hand-crafted) JSON report. Returns the title and generated
/// board content + messages for the caller to create the session.
pub fn import_json(
    input_path: &str,
) -> Result<(String, String, ChatStore), Box<dyn std::error::Error>> {
    let json = fs::read_to_string(input_path)?;
    let report: SessionReport = serde_json::from_str(&json)?;

    // Reconstruct .cb.md content
    let mut board = format!("---\ntitle: {}\n---\n", report.title);

    for step in &report.steps {
        let result_attr = if step.is_result { " {.result}" } else { "" };
        board.push_str(&format!("\n## {}{}\n", step.title, result_attr));

        for eq in &step.equations {
            board.push_str(&format!("\n$${}$$\n", eq));
        }

        for note in &step.notes {
            board.push_str(&format!("\n> {}\n", note));
        }
    }

    // Reconstruct messages
    let mut messages = Vec::new();
    for step in &report.steps {
        for entry in &step.chat {
            let role = match entry.role.as_str() {
                "assistant" => ChatRole::Assistant,
                _ => ChatRole::User,
            };

            let rendered = if role == ChatRole::Assistant {
                render::render_reply_content(&entry.text, step.id)
            } else {
                render::render_chat_text(&entry.text)
            };

            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            messages.push(ChatMessage {
                id: format!("{:x}", timestamp_ms + messages.len() as u128),
                step_id: step.id,
                role,
                text: entry.text.clone(),
                rendered,
                timestamp: if entry.timestamp.is_empty() {
                    chrono::Local::now().to_rfc3339()
                } else {
                    entry.timestamp.clone()
                },
                context: entry.context.as_ref().map(|c| ChatContext {
                    selected: c.selected.clone(),
                    latex: c.latex.clone(),
                    step_title: c.step_title.clone(),
                }),
            });
        }
    }

    let store = ChatStore { messages };
    Ok((report.title, board, store))
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
