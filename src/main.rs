mod cli;
mod document;
mod export;
mod parser;
mod render;
mod server;
mod session;
mod unicode;

use clap::Parser;
use cli::{Cli, Command};
use document::{ChatMessage, ChatRole};
use session::Session;

fn main() {
    // KaTeX's embedded JS engine (QuickJS) has a 256KB internal stack limit.
    // Use a large thread stack as defense-in-depth alongside the HTML-only
    // KaTeX output mode (the primary fix in render.rs).
    let builder = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .name("cliboard-main".into());

    let handler = builder
        .spawn(|| {
            let cli = Cli::parse();
            if let Err(e) = run(cli.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        })
        .expect("Failed to spawn main thread");

    handler.join().unwrap();
}

fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::New { title } => cmd_new(&title),
        Command::Step { title, latex } => cmd_step(&title, &latex),
        Command::Eq { latex } => cmd_eq(&latex),
        Command::Note { text } => cmd_note(&text),
        Command::Text { text } => cmd_text(&text),
        Command::Result { title, latex } => cmd_result(&title, &latex),
        Command::Divider => cmd_divider(),
        Command::Render { latex, output } => cmd_render(&latex, output.as_deref()),
        Command::Serve { file, port } => cmd_serve(&file, port),
        Command::Stop => cmd_stop(),
        Command::Export { output } => cmd_export(&output),
        Command::Status => cmd_status(),
        Command::Selection { json, latex } => cmd_selection(json, latex),
        Command::Chat { all, step, json } => cmd_chat(all, step, json),
        Command::Reply { step_id, text } => cmd_reply(step_id, text),
        Command::Listen { json } => cmd_listen(json),
        Command::Agent => cmd_agent(),
        Command::Import { input } => cmd_import(&input),
        Command::Update { check } => cmd_update(check),
    }
}

fn require_session() -> Result<Session, Box<dyn std::error::Error>> {
    Session::find_current()
        .ok_or_else(|| "No active session. Run `cliboard new \"title\"` first.".into())
}

fn count_steps(content: &str) -> usize {
    content
        .lines()
        .filter(|line| line.starts_with("## "))
        .count()
}

fn cmd_new(title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = Session::create(title)?;

    let port = server::find_available_port(8377)?;
    let url = format!("http://localhost:{}", port);

    // Always open in the system default browser
    let _ = open::that(&url);

    println!("Board live at {}", url);

    // Block the main thread so the server stays alive until Ctrl+C
    server::start_server_for_session(&session, port)?;

    Ok(())
}

fn cmd_step(title: &str, latex: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = format!("\n## {}\n\n$${}$$\n", title, latex);
    session.append(&content)?;
    let board = session.read_board()?;
    let n = count_steps(&board);
    println!("Step {} added: \"{}\"", n, title);
    Ok(())
}

fn cmd_eq(latex: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = format!("\n$${}$$\n", latex);
    session.append(&content)?;
    println!("Equation added");
    Ok(())
}

fn cmd_note(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = format!("\n> {}\n", text);
    session.append(&content)?;
    println!("Note added");
    Ok(())
}

fn cmd_text(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = format!("\n{}\n", text);
    session.append(&content)?;
    println!("Text added");
    Ok(())
}

fn cmd_result(title: &str, latex: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = format!("\n## {} {{.result}}\n\n$${}$$\n", title, latex);
    session.append(&content)?;
    println!("Result added: \"{}\"", title);
    Ok(())
}

fn cmd_divider() -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    session.append("\n---\n")?;
    println!("Divider added");
    Ok(())
}

fn cmd_render(input: &str, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let html = if input == "-" {
        // Read from stdin
        let mut content = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut content)?;
        render_content_to_html(&content)
    } else if input.ends_with(".cb.md") || input.ends_with(".md") {
        // Render a .cb.md file
        let content = std::fs::read_to_string(input)
            .map_err(|_| format!("Could not read file: {}", input))?;
        render_content_to_html(&content)
    } else {
        // Single LaTeX equation (original behavior)
        let rendered = render::render_equation(input)
            .map_err(|e| format!("KaTeX error: {}", e))?;
        format!(
            r#"<!DOCTYPE html>
<html><head>
<meta charset="UTF-8">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css">
<style>body {{ display:flex; justify-content:center; align-items:center; min-height:100vh; margin:0; background:#1a1a2e; color:#e0e0e0; }}</style>
</head><body>{}</body></html>"#,
            rendered
        )
    };

    match output {
        Some(path) => {
            std::fs::write(path, &html)?;
            println!("Rendered to {}", path);
        }
        None => {
            let tmp = tempfile::Builder::new()
                .suffix(".html")
                .tempfile()?;
            let tmp_path = tmp.into_temp_path();
            std::fs::write(&tmp_path, &html)?;
            let _ = open::that(tmp_path.to_str().unwrap_or(""));
            // Give the browser time to open the file before it's cleaned up
            std::thread::sleep(std::time::Duration::from_secs(2));
            println!("Opened in browser");
        }
    }

    Ok(())
}

fn render_content_to_html(content: &str) -> String {
    let doc = parser::parse(content);
    let blocks_html = render::render_blocks_html(&doc);
    let title = &doc.title;

    format!(
        r#"<!DOCTYPE html>
<html><head>
<meta charset="UTF-8">
<title>{title} — cliboard</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css">
<style>
body {{ margin:0; padding:2rem; background:#1C1917; color:#e0e0e0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }}
h1 {{ color: #CA8A04; font-size: 1.5rem; margin-bottom: 2rem; }}
.step {{ margin: 1.5rem 0; padding: 1.5rem; background: #292524; border-radius: 8px; }}
.step.result {{ border-left: 3px solid #CA8A04; }}
.step-header {{ margin-bottom: 1rem; }}
.step-number {{ color: #CA8A04; font-weight: bold; margin-right: 0.5rem; }}
.step-title {{ font-size: 1.1rem; font-weight: 600; }}
.equation-card {{ text-align: center; margin: 1rem 0; padding: 1rem; position: relative; }}
.equation-number {{ position: absolute; right: 1rem; top: 50%; transform: translateY(-50%); color: #78716C; font-family: KaTeX_Main, serif; }}
.note {{ color: #A8A29E; border-left: 2px solid #44403C; padding-left: 1rem; margin: 0.75rem 0; }}
.prose {{ margin: 1rem 0; color: #D6D3D1; }}
.divider {{ border: none; border-top: 1px solid #44403C; margin: 2rem 0; }}
.error-card {{ background: #451a1a; border: 1px solid #dc2626; padding: 1rem; border-radius: 4px; }}
.error-card code {{ color: #fca5a5; }}
.error-msg {{ color: #f87171; font-size: 0.85rem; margin-top: 0.5rem; }}
.error-inline {{ color: #f87171; background: #451a1a; padding: 0 4px; border-radius: 2px; }}
</style>
</head><body>
<h1>{title}</h1>
{blocks_html}
</body></html>"#
    )
}

fn cmd_serve(file: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(file)
        .canonicalize()
        .map_err(|_| format!("File not found: {}", file))?;

    let port = server::find_available_port(port)?;
    let url = format!("http://localhost:{}", port);
    let _ = open::that(&url);
    println!("Serving {} at {}", file, url);

    server::start_server(server::ServerConfig {
        board_path: path,
        port,
        session_dir: None,
    })?;
    Ok(())
}

fn cmd_stop() -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;

    if let Some(pid) = session.read_pid() {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
        session.remove_pid();
        println!("Server stopped (PID {})", pid);
    } else {
        println!("No server PID found");
    }

    Ok(())
}

fn cmd_import(input: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (title, board_content, store) = export::import_json(input)?;

    let session = Session::create(&title)?;

    // Write the board content
    std::fs::write(&session.board_path, &board_content)?;

    // Write messages if any
    if !store.messages.is_empty() {
        let json = serde_json::to_string_pretty(&store)?;
        std::fs::write(session.messages_path(), json)?;
    }

    let step_count = count_steps(&board_content);
    let msg_count = store.messages.len();
    println!(
        "Imported \"{}\" ({} steps, {} messages)",
        title, step_count, msg_count
    );

    let port = server::find_available_port(8377)?;
    let url = format!("http://localhost:{}", port);

    let _ = open::that(&url);

    println!("Board live at {}", url);

    server::start_server_for_session(&session, port)?;

    Ok(())
}

fn cmd_export(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = session.read_board()?;
    let doc = parser::parse(&content);

    if output.ends_with(".json") {
        let store = session.read_messages().unwrap_or_default();
        export::export_json(&doc, &store, output)?;
    } else {
        export::export_html(&doc, output)?;
    }

    println!("Exported to {}", output);
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    match Session::find_current() {
        Some(session) => {
            let port = session.read_port();
            let pid = session.read_pid();
            let content = session.read_board().unwrap_or_default();
            let steps = count_steps(&content);

            let alive = pid
                .map(is_pid_alive)
                .unwrap_or(false);

            if alive {
                let port_str = port
                    .map(|p| format!(":{}", p))
                    .unwrap_or_else(|| "unknown port".to_string());
                println!("Running on {}, {} steps", port_str, steps);
            } else {
                println!("Not running ({} steps in board)", steps);
            }
        }
        None => {
            println!("No active session");
        }
    }
    Ok(())
}

fn cmd_selection(json: bool, latex: bool) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;

    match session.read_selection() {
        Some(selection) => {
            if json {
                let json_str = serde_json::to_string_pretty(&selection)?;
                println!("{}", json_str);
            } else if latex {
                println!("{}", selection.latex);
            } else {
                println!("Step {}: {}", selection.step_id, selection.title);
                println!("LaTeX: {}", selection.latex);
                println!("Unicode: {}", selection.unicode);
            }
        }
        None => {
            println!("No selection yet. Click an equation on the board first.");
        }
    }

    Ok(())
}

fn cmd_chat(all: bool, step: Option<usize>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let store = session.read_messages()?;

    let messages = if let Some(step_id) = step {
        store
            .messages
            .into_iter()
            .filter(|m| m.step_id == step_id)
            .collect()
    } else if all {
        store.messages
    } else {
        // Show pending (unanswered) questions
        session.pending_messages()?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&messages)?);
        return Ok(());
    }

    if messages.is_empty() {
        println!("No messages.");
        return Ok(());
    }

    for msg in &messages {
        let role_prefix = match msg.role {
            ChatRole::User => "Q",
            ChatRole::Assistant => "A",
        };
        println!("[Step {}] {}: {}", msg.step_id, role_prefix, msg.text);
        if let Some(ctx) = &msg.context {
            if let (Some(selected), Some(latex)) = (&ctx.selected, &ctx.latex) {
                let title_part = ctx
                    .step_title
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map(|t| format!(" ({})", t))
                    .unwrap_or_default();
                println!(
                    "  -> selected: \"{}\" in {}{}",
                    selected, latex, title_part
                );
            }
        }
    }
    Ok(())
}

fn cmd_reply(step_id: usize, text: String) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let existing = session.read_messages().map(|s| s.messages).unwrap_or_default();
    let (known_eqs, eq_offset) = render::reply_equation_context(&existing, step_id);
    let rendered = render::render_reply_content_ctx(&text, step_id, &known_eqs, eq_offset);
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let msg = ChatMessage {
        id: format!("{:x}", timestamp_ms),
        step_id,
        role: ChatRole::Assistant,
        text,
        rendered,
        timestamp: chrono::Local::now().to_rfc3339(),
        context: None,
    };
    session.append_message(msg)?;
    println!("Reply sent to step {}.", step_id);
    Ok(())
}

fn cmd_listen(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use notify::{EventKind, RecursiveMode, Watcher};
    use std::collections::HashSet;

    let session = require_session()?;
    let messages_path = session.messages_path();

    // Track which message IDs we've already printed
    let mut seen: HashSet<String> = {
        let store = session.read_messages()?;
        store.messages.iter().map(|m| m.id.clone()).collect()
    };

    eprintln!("Listening for chat questions... (Ctrl+C to stop)");

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;

    let watch_dir = messages_path
        .parent()
        .unwrap_or(std::path::Path::new("."));
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;

    for event_result in rx {
        match event_result {
            Ok(event) => {
                let dominated = matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_)
                );
                let affects = event.paths.iter().any(|p| p == &messages_path);

                if dominated && affects {
                    if let Ok(store) = session.read_messages() {
                        for msg in &store.messages {
                            if msg.role == ChatRole::User && !seen.contains(&msg.id) {
                                seen.insert(msg.id.clone());
                                if json {
                                    println!("{}", serde_json::to_string(msg)?);
                                } else {
                                    print!("[Step {}] {}", msg.step_id, msg.text);
                                    if let Some(ctx) = &msg.context {
                                        if let Some(sel) = &ctx.selected {
                                            print!("  (re: \"{}\")", sel);
                                        }
                                    }
                                    println!();
                                }
                                // Flush immediately so piped consumers see it
                                use std::io::Write;
                                let _ = std::io::stdout().flush();
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {}", e),
        }
    }

    Ok(())
}

fn cmd_agent() -> Result<(), Box<dyn std::error::Error>> {
    use notify::{EventKind, RecursiveMode, Watcher};
    use std::collections::HashSet;

    let session = require_session()?;
    let messages_path = session.messages_path();

    // Register this agent via PID file so the server skips inline replies
    session.write_agent_pid(std::process::id())?;

    // Clean up agent.pid on exit
    let session_dir_for_cleanup = session.dir.clone();
    let cleanup = move || {
        let _ = std::fs::remove_file(session_dir_for_cleanup.join("agent.pid"));
    };
    let cleanup_clone = {
        let dir = session.dir.clone();
        move || {
            let _ = std::fs::remove_file(dir.join("agent.pid"));
        }
    };

    // Handle Ctrl+C gracefully
    ctrlc::set_handler(move || {
        cleanup_clone();
        eprintln!("\n[agent] Stopped.");
        std::process::exit(0);
    })?;

    // Track which message IDs we've already seen
    let mut seen: HashSet<String> = {
        let store = session.read_messages()?;
        store.messages.iter().map(|m| m.id.clone()).collect()
    };

    eprintln!("[agent] Board agent running (PID {})", std::process::id());
    eprintln!("[agent] Watching for chat questions... (Ctrl+C to stop)");

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;

    let watch_dir = messages_path
        .parent()
        .unwrap_or(std::path::Path::new("."));
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;

    for event_result in rx {
        match event_result {
            Ok(event) => {
                let dominated = matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_)
                );
                let affects = event.paths.iter().any(|p| {
                    p == &messages_path
                        || p.file_name().and_then(|n| n.to_str()) == Some("messages.json")
                        || p.file_name().and_then(|n| n.to_str()) == Some("messages.json.tmp")
                });

                if dominated && affects {
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    if let Ok(store) = session.read_messages() {
                        for msg in &store.messages {
                            if msg.role == ChatRole::User && !seen.contains(&msg.id) {
                                seen.insert(msg.id.clone());

                                let step_id = msg.step_id;
                                let question = msg.text.clone();
                                let context = msg.context.clone();

                                eprintln!(
                                    "[agent] New question on step {}: \"{}\"",
                                    step_id,
                                    if question.len() > 60 {
                                        format!("{}...", &question[..57])
                                    } else {
                                        question.clone()
                                    }
                                );

                                // Build prompt with FULL context
                                let board = session.read_board().unwrap_or_default();
                                let all_messages = session
                                    .read_messages()
                                    .map(|s| s.messages)
                                    .unwrap_or_default();

                                let step_history: Vec<_> = all_messages
                                    .iter()
                                    .filter(|m| m.step_id == step_id)
                                    .collect();

                                let prompt = build_agent_prompt(
                                    &board,
                                    &step_history,
                                    step_id,
                                    &question,
                                    &context,
                                );

                                eprintln!("[agent] Generating reply for step {}...", step_id);
                                let output = std::process::Command::new("claude")
                                    .args(["-p", &prompt])
                                    .output();

                                match output {
                                    Ok(out) if out.status.success() => {
                                        let reply =
                                            String::from_utf8_lossy(&out.stdout).trim().to_string();
                                        if !reply.is_empty() {
                                            let existing = session
                                                .read_messages()
                                                .map(|s| s.messages)
                                                .unwrap_or_default();
                                            let (known_eqs, eq_offset) =
                                                render::reply_equation_context(&existing, step_id);
                                            let rendered = render::render_reply_content_ctx(
                                                &reply,
                                                step_id,
                                                &known_eqs,
                                                eq_offset,
                                            );
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis();
                                            let reply_msg = ChatMessage {
                                                id: format!("{:x}", ts),
                                                step_id,
                                                role: ChatRole::Assistant,
                                                text: reply,
                                                rendered,
                                                timestamp: chrono::Local::now().to_rfc3339(),
                                                context: None,
                                            };
                                            match session.append_message(reply_msg) {
                                                Ok(()) => {
                                                    seen.insert(format!("{:x}", ts));
                                                    eprintln!(
                                                        "[agent] Reply posted for step {}",
                                                        step_id
                                                    );
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "[agent] Failed to save reply: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Ok(out) => {
                                        eprintln!(
                                            "[agent] claude CLI error: {}",
                                            String::from_utf8_lossy(&out.stderr)
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!("[agent] Failed to run claude: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("[agent] Watch error: {}", e),
        }
    }

    cleanup();
    Ok(())
}

/// Build a prompt for the board agent with full conversation context.
fn build_agent_prompt(
    board: &str,
    step_history: &[&ChatMessage],
    step_id: usize,
    question: &str,
    context: &Option<crate::document::ChatContext>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a board agent answering questions about a math/physics derivation. \
         You have the full derivation and conversation history for context.\n\
         Use LaTeX: $$...$$ for display equations, $...$ for inline math.\n\
         Be concise, precise, and pedagogical.\n\n",
    );

    prompt.push_str("## Derivation\n\n");
    prompt.push_str(board);
    prompt.push_str("\n\n");

    // Include conversation history for this step (multi-turn context)
    if step_history.len() > 1 {
        prompt.push_str("## Previous conversation on this step\n\n");
        for msg in &step_history[..step_history.len() - 1] {
            let role = match msg.role {
                ChatRole::User => "User",
                ChatRole::Assistant => "Assistant",
            };
            prompt.push_str(&format!("{}: {}\n\n", role, msg.text));
        }
    }

    if let Some(ctx) = context {
        if let Some(selected) = &ctx.selected {
            prompt.push_str(&format!("The user selected the text: \"{}\"\n", selected));
        }
        if let Some(latex) = &ctx.latex {
            prompt.push_str(&format!("From the LaTeX: {}\n", latex));
        }
        if let Some(title) = &ctx.step_title {
            prompt.push_str(&format!("In the step titled: \"{}\"\n", title));
        }
        prompt.push('\n');
    }

    prompt.push_str(&format!(
        "## Current question (about step {})\n\n{}\n",
        step_id, question
    ));

    prompt
}

const GITHUB_REPO: &str = "maxwellsdm1867/cliboard";

fn cmd_update(check_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let current = env!("CARGO_PKG_VERSION");
    println!("cliboard v{}", current);
    println!("Checking for updates...");

    // Fetch latest release from GitHub API
    let output = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            &format!(
                "https://api.github.com/repos/{}/releases/latest",
                GITHUB_REPO
            ),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") || output.status.code() == Some(22) {
            println!("No releases published yet.");
            println!(
                "Build from source: cargo install --path . (or cargo build --release)"
            );
            return Ok(());
        }
        return Err("Failed to check for updates. Check your internet connection.".into());
    }

    let body = String::from_utf8(output.stdout)?;
    let release: serde_json::Value = serde_json::from_str(&body)?;

    let latest = release["tag_name"]
        .as_str()
        .ok_or("Could not parse latest version")?
        .trim_start_matches('v');

    if latest == current {
        println!("Already on the latest version.");
        return Ok(());
    }

    println!("New version available: v{} -> v{}", current, latest);

    if check_only {
        println!(
            "Run `cliboard update` to install, or visit:\n  https://github.com/{}/releases/tag/v{}",
            GITHUB_REPO, latest
        );
        return Ok(());
    }

    // Determine platform target triple
    let target = target_triple();

    // Find the right asset in the release
    let asset_name = format!("cliboard-{}.tar.gz", target);
    let assets = release["assets"]
        .as_array()
        .ok_or("No assets in release")?;

    // cargo-dist may use slightly different naming, try both patterns
    let download_url = assets
        .iter()
        .find(|a| {
            let name = a["name"].as_str().unwrap_or("");
            name == asset_name || name.contains(target)
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| {
            format!(
                "No prebuilt binary for your platform ({}).\n\
                 Install from source: cargo install --path .",
                target
            )
        })?;

    println!("Downloading...");

    let tmp_dir = tempfile::tempdir()?;
    let tmp_archive = tmp_dir.path().join("cliboard.tar.gz");

    let status = std::process::Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&tmp_archive)
        .arg(download_url)
        .status()?;

    if !status.success() {
        return Err("Download failed.".into());
    }

    // Extract
    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(&tmp_archive)
        .arg("-C")
        .arg(tmp_dir.path())
        .status()?;

    if !status.success() {
        return Err("Failed to extract archive.".into());
    }

    // Find the cliboard binary in extracted files (cargo-dist may nest it)
    let new_bin = find_binary_in_dir(tmp_dir.path())
        .ok_or("Could not find cliboard binary in downloaded archive.")?;

    // Replace current binary
    let current_exe = std::env::current_exe()?;
    let backup = current_exe.with_extension("old");

    // Backup current binary
    std::fs::rename(&current_exe, &backup)?;

    match std::fs::copy(&new_bin, &current_exe) {
        Ok(_) => {
            // Set executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    &current_exe,
                    std::fs::Permissions::from_mode(0o755),
                )?;
            }
            let _ = std::fs::remove_file(&backup);
            println!("Updated to v{}!", latest);
        }
        Err(e) => {
            // Restore from backup
            let _ = std::fs::rename(&backup, &current_exe);
            return Err(format!("Install failed: {}. Restored previous version.", e).into());
        }
    }

    Ok(())
}

/// Find the cliboard binary in a directory tree (handles cargo-dist nesting).
fn find_binary_in_dir(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let bin_name = if cfg!(windows) {
        "cliboard.exe"
    } else {
        "cliboard"
    };

    // Check top level
    let direct = dir.join(bin_name);
    if direct.exists() {
        return Some(direct);
    }

    // Check one level deep (cargo-dist puts it in a subdirectory)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let nested = entry.path().join(bin_name);
            if nested.exists() {
                return Some(nested);
            }
        }
    }

    None
}

/// Get the target triple for the current platform.
fn target_triple() -> &'static str {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    { "aarch64-apple-darwin" }
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    { "x86_64-apple-darwin" }
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    { "x86_64-unknown-linux-gnu" }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    { "aarch64-unknown-linux-gnu" }
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    { "x86_64-pc-windows-msvc" }
    #[cfg(all(target_arch = "aarch64", target_os = "windows"))]
    { "aarch64-pc-windows-msvc" }
    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "windows"),
        all(target_arch = "aarch64", target_os = "windows"),
    )))]
    { "unknown" }
}

/// Check if a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    // kill -0 checks if process exists without sending a signal
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_serve_missing_file() {
        let result = cmd_serve("/nonexistent/file.cb.md", 8377);
        assert!(result.is_err());
    }

    #[test]
    fn test_render_content_to_html_basic() {
        let content = "---\ntitle: Test\n---\n\n## Step 1\n\n$$E = mc^2$$\n";
        let html = render_content_to_html(content);
        assert!(html.contains("Test"));
        assert!(html.contains("katex"));
        assert!(html.contains("Step 1"));
    }

    #[test]
    fn test_render_content_to_html_empty() {
        let html = render_content_to_html("");
        assert!(html.contains("Untitled"));
    }

    #[test]
    fn test_cmd_render_missing_file() {
        let result = cmd_render("/nonexistent/file.cb.md", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_agent_prompt_basic() {
        let board = "## Step 1\n\n$$E = mc^2$$\n";
        let prompt = build_agent_prompt(board, &[], 1, "What is E?", &None);
        assert!(prompt.contains("Derivation"));
        assert!(prompt.contains("E = mc^2"));
        assert!(prompt.contains("What is E?"));
        assert!(prompt.contains("step 1"));
        // No conversation history for empty step_history
        assert!(!prompt.contains("Previous conversation"));
    }

    #[test]
    fn test_build_agent_prompt_with_history() {
        use crate::document::{ChatContext, ChatMessage, ChatRole};

        let board = "## Step 1\n\n$$x = 1$$\n";
        let msg1 = ChatMessage {
            id: "1".into(),
            step_id: 1,
            role: ChatRole::User,
            text: "What is x?".into(),
            rendered: String::new(),
            timestamp: String::new(),
            context: None,
        };
        let msg2 = ChatMessage {
            id: "2".into(),
            step_id: 1,
            role: ChatRole::Assistant,
            text: "x is a variable".into(),
            rendered: String::new(),
            timestamp: String::new(),
            context: None,
        };
        let msg3 = ChatMessage {
            id: "3".into(),
            step_id: 1,
            role: ChatRole::User,
            text: "But why 1?".into(),
            rendered: String::new(),
            timestamp: String::new(),
            context: None,
        };
        let history = vec![&msg1, &msg2, &msg3];
        let prompt = build_agent_prompt(board, &history, 1, "But why 1?", &None);

        // Should include previous conversation (first 2 messages, not the current question)
        assert!(prompt.contains("Previous conversation"));
        assert!(prompt.contains("User: What is x?"));
        assert!(prompt.contains("Assistant: x is a variable"));
        assert!(prompt.contains("But why 1?"));
    }

    #[test]
    fn test_build_agent_prompt_with_context() {
        use crate::document::ChatContext;

        let board = "## Step 1\n\n$$\\psi$$\n";
        let ctx = Some(ChatContext {
            selected: Some("ψ".into()),
            latex: Some("\\psi".into()),
            step_title: Some("Wave Function".into()),
        });
        let prompt = build_agent_prompt(board, &[], 1, "What is this?", &ctx);

        assert!(prompt.contains("selected the text: \"ψ\""));
        assert!(prompt.contains("From the LaTeX: \\psi"));
        assert!(prompt.contains("step titled: \"Wave Function\""));
    }
}
