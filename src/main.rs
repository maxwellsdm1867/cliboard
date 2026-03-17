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
use session::Session;

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

    // Open browser: VS Code Simple Browser or system default
    if std::env::var("TERM_PROGRAM")
        .map(|v| v == "vscode")
        .unwrap_or(false)
    {
        let _ = std::process::Command::new("code")
            .args(["--command", "simpleBrowser.show", &url])
            .spawn();
    } else {
        let _ = open::that(&url);
    }

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

/// Check if a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    // kill -0 checks if process exists without sending a signal
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
