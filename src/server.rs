#[allow(unused_imports)]
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use rust_embed::Embed;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::parser;
use crate::render;
use crate::session::Session;

#[derive(Embed)]
#[folder = "katex-assets/"]
struct KatexAssets;

#[derive(Embed)]
#[folder = "assets/"]
struct ViewerAssets;

struct RenderedState {
    title: String,
    blocks_html: String,
}

/// Start the HTTP server in the foreground (blocking).
///
/// Serves the viewer, KaTeX assets, and the /board polling endpoint.
/// Watches the board file for changes and re-renders automatically.
pub fn start_server(
    session: &Session,
    preferred_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = find_available_port(preferred_port)?;
    let addr = format!("127.0.0.1:{}", port);
    let server =
        Server::http(&addr).map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    session.write_pid(std::process::id())?;
    session.write_port(port)?;

    let board_path = session.board_path.clone();
    let state = initial_render(&board_path);
    let state = Arc::new(Mutex::new(state));
    let version = Arc::new(AtomicU64::new(1));

    start_file_watcher(board_path, Arc::clone(&state), Arc::clone(&version));

    let session_dir = session.dir.clone();

    eprintln!("cliboard server listening on http://localhost:{}", port);

    for request in server.incoming_requests() {
        handle_request(request, &state, &version, &session_dir);
    }

    Ok(())
}

fn initial_render(board_path: &Path) -> RenderedState {
    let content = std::fs::read_to_string(board_path).unwrap_or_default();
    let doc = parser::parse(&content);
    let blocks_html = render::render_blocks_html(&doc);
    RenderedState {
        title: doc.title,
        blocks_html,
    }
}

fn handle_request(
    request: tiny_http::Request,
    state: &Arc<Mutex<RenderedState>>,
    version: &Arc<AtomicU64>,
    session_dir: &Path,
) {
    let url = request.url().to_string();

    match request.method() {
        Method::Get => handle_get(request, &url, state, version),
        Method::Post if url == "/select" => handle_select(request, session_dir),
        _ => respond_not_found(request),
    }
}

fn handle_get(
    request: tiny_http::Request,
    url: &str,
    state: &Arc<Mutex<RenderedState>>,
    version: &Arc<AtomicU64>,
) {
    match url {
        "/" => serve_embedded::<ViewerAssets>(request, "viewer.html", "text/html; charset=utf-8"),
        "/viewer.css" => serve_embedded::<ViewerAssets>(request, "viewer.css", "text/css"),
        "/viewer.js" => {
            serve_embedded::<ViewerAssets>(request, "viewer.js", "application/javascript")
        }
        _ if url == "/board" || url.starts_with("/board?") => {
            serve_board(request, url, state, version)
        }
        "/katex/katex.min.css" => serve_embedded::<KatexAssets>(request, "katex.min.css", "text/css"),
        _ if url.starts_with("/katex/fonts/") => {
            let font_name = &url["/katex/fonts/".len()..];
            serve_embedded::<KatexAssets>(
                request,
                &format!("fonts/{}", font_name),
                "font/woff2",
            );
        }
        _ => respond_not_found(request),
    }
}

/// Try to find an available port starting from `preferred`.
pub fn find_available_port(preferred: u16) -> Result<u16, Box<dyn std::error::Error>> {
    for port in preferred..=preferred + 10 {
        if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return Ok(port);
        }
    }
    Err(format!(
        "No available port in range {}-{}",
        preferred,
        preferred + 10
    )
    .into())
}

fn respond_not_found(request: tiny_http::Request) {
    let resp = Response::from_string("Not Found").with_status_code(StatusCode(404));
    let _ = request.respond(resp);
}

/// Serve an embedded asset file.
fn serve_embedded<E: Embed>(request: tiny_http::Request, name: &str, content_type: &str) {
    match E::get(name) {
        Some(file) => {
            let data = file.data.to_vec();
            let header = Header::from_bytes("Content-Type", content_type).unwrap();
            let resp = Response::from_data(data).with_header(header);
            let _ = request.respond(resp);
        }
        None => respond_not_found(request),
    }
}

/// Serve the /board endpoint: JSON with version, title, and pre-rendered blocks HTML.
/// Supports `?v=<version>` query param -- returns 304 if the client is already up to date.
fn serve_board(
    request: tiny_http::Request,
    url: &str,
    state: &Arc<Mutex<RenderedState>>,
    version: &Arc<AtomicU64>,
) {
    let ver = version.load(Ordering::Relaxed);

    // Short-circuit: if client already has this version, skip serialization + cloning
    if let Some(client_ver) = parse_version_param(url) {
        if client_ver >= ver {
            let resp = Response::from_string("")
                .with_status_code(StatusCode(304));
            let _ = request.respond(resp);
            return;
        }
    }

    let (title, blocks_html) = {
        let st = state.lock().unwrap();
        (st.title.clone(), st.blocks_html.clone())
    };
    let json = serde_json::json!({
        "version": ver,
        "title": title,
        "blocks_html": blocks_html,
    });
    let body = json.to_string();
    let header = Header::from_bytes("Content-Type", "application/json").unwrap();
    let resp = Response::from_string(body).with_header(header);
    let _ = request.respond(resp);
}

/// Parse `?v=<number>` from a URL path.
fn parse_version_param(url: &str) -> Option<u64> {
    let query = url.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(val) = param.strip_prefix("v=") {
            return val.parse().ok();
        }
    }
    None
}

/// Handle POST /select: receive selection JSON from the viewer and write to selection.json.
fn handle_select(mut request: tiny_http::Request, session_dir: &Path) {
    const MAX_BODY_SIZE: usize = 64 * 1024; // 64KB limit

    let content_length = request.body_length().unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        let resp = Response::from_string("Payload Too Large").with_status_code(StatusCode(413));
        let _ = request.respond(resp);
        return;
    }

    let mut body = String::new();
    if request.as_reader().take(MAX_BODY_SIZE as u64 + 1).read_to_string(&mut body).is_err() {
        let resp = Response::from_string("Bad Request").with_status_code(StatusCode(400));
        let _ = request.respond(resp);
        return;
    }

    if body.len() > MAX_BODY_SIZE {
        let resp = Response::from_string("Payload Too Large").with_status_code(StatusCode(413));
        let _ = request.respond(resp);
        return;
    }

    #[derive(serde::Deserialize)]
    struct SelectRequest {
        step_id: usize,
        title: String,
        latex: String,
        text: String,
    }

    match serde_json::from_str::<SelectRequest>(&body) {
        Ok(sel_req) => {
            let unicode = crate::unicode::latex_to_unicode(&sel_req.latex);
            let selected_text = sel_req.text.trim().to_string();
            let selection = crate::document::Selection {
                step_id: sel_req.step_id,
                title: sel_req.title,
                latex: sel_req.latex,
                unicode,
                formatted: selected_text.clone(),
                notes: vec![],
                selected_at: chrono::Local::now().to_rfc3339(),
            };
            let json = serde_json::to_string_pretty(&selection).unwrap_or_default();
            let _ = std::fs::write(session_dir.join("selection.json"), &json);
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let global = PathBuf::from(home).join(".cliboard").join("selection.json");
            let _ = std::fs::write(&global, &json);

            // If user selected a partial snippet, format as:
            //   {selected} in [Step N] {full equation}
            // If full selection or empty, just:
            //   [Step N] {full equation}
            let full_eq = &selection.unicode;
            let sel_chars = selected_text.chars().count();
            let eq_chars = full_eq.chars().count();
            let is_partial = !selected_text.is_empty()
                && sel_chars < eq_chars
                && sel_chars <= eq_chars * 3 / 4; // selection is less than 75% of equation
            let formatted = if is_partial {
                format!("{} in [Step {}] {}", selected_text, selection.step_id, full_eq)
            } else {
                format!("[Step {}] {}", selection.step_id, full_eq)
            };
            let resp_json = serde_json::json!({
                "ok": true,
                "unicode": selection.unicode,
                "formatted": formatted,
            });
            let header = Header::from_bytes("Content-Type", "application/json").unwrap();
            let resp = Response::from_string(resp_json.to_string()).with_header(header);
            let _ = request.respond(resp);
        }
        Err(_) => {
            let resp = Response::from_string("Bad Request").with_status_code(StatusCode(400));
            let _ = request.respond(resp);
        }
    }
}

/// Start a file watcher thread that re-parses and re-renders when the board file changes.
fn start_file_watcher(
    board_path: PathBuf,
    state: Arc<Mutex<RenderedState>>,
    version: Arc<AtomicU64>,
) {
    use notify::{EventKind, RecursiveMode, Watcher};

    let watch_path = board_path.clone();

    thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create file watcher: {}", e);
                return;
            }
        };

        // Watch the parent directory (some editors do atomic rename)
        let watch_dir = watch_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
            eprintln!("Failed to watch directory: {}", e);
            return;
        }

        for event_result in rx {
            match event_result {
                Ok(event) => {
                    let dominated = matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    );
                    let affects_board = event.paths.iter().any(|p| p == &board_path);

                    if dominated && affects_board {
                        if let Ok(content) = std::fs::read_to_string(&board_path) {
                            let doc = parser::parse(&content);
                            let blocks_html = render::render_blocks_html(&doc);
                            let mut st = state.lock().unwrap();
                            st.title = doc.title;
                            st.blocks_html = blocks_html;
                            drop(st);
                            version.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("File watch error: {}", e);
                }
            }
        }
    });
}
