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
use session::{ChatMessage, ChatRole, Session};

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli.command) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
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
        Command::Stop => cmd_stop(),
        Command::Export { output } => cmd_export(&output),
        Command::Status => cmd_status(),
        Command::Selection { json, latex } => cmd_selection(json, latex),
        Command::Chat { all, step, json } => cmd_chat(all, step, json),
        Command::Reply { step_id, text } => cmd_reply(step_id, text),
        Command::Listen { json } => cmd_listen(json),
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
    server::start_server(&session, port)?;

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

fn cmd_render(latex: &str, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = render::render_equation(latex)
        .map_err(|e| format!("KaTeX error: {}", e))?;

    let html = format!(
        r#"<!DOCTYPE html>
<html><head>
<meta charset="UTF-8">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css">
<style>body {{ display:flex; justify-content:center; align-items:center; min-height:100vh; margin:0; background:#1a1a2e; color:#e0e0e0; }}</style>
</head><body>{}</body></html>"#,
        rendered
    );

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

fn cmd_export(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session = require_session()?;
    let content = session.read_board()?;
    let doc = parser::parse(&content);
    export::export_html(&doc, output)?;
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
    let rendered = render::render_chat_text(&text);
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

/// Check if a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    // kill -0 checks if process exists without sending a signal
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
