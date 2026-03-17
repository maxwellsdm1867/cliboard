#[allow(unused_imports)]
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use rust_embed::Embed;
use tiny_http::{Header, Method, Response, Server, StatusCode};
use tungstenite::WebSocket;

use crate::parser;
use crate::render;
use crate::session::{ChatContext, ChatMessage, ChatRole, ChatStore, Session};

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

/// A list of connected WebSocket clients, protected by a mutex.
type WsClients = Arc<Mutex<Vec<WebSocket<TcpStream>>>>;

/// Start the HTTP server in the foreground (blocking).
///
/// Serves the viewer, KaTeX assets, and the /board polling endpoint.
/// Also starts a WebSocket server on port+1 for instant push updates.
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

    // Start WebSocket server on port+1
    let ws_port = find_available_port(port + 1)?;
    let ws_clients: WsClients = Arc::new(Mutex::new(Vec::new()));

    start_ws_server(ws_port, Arc::clone(&ws_clients));

    start_file_watcher(
        board_path,
        Arc::clone(&state),
        Arc::clone(&version),
        Arc::clone(&ws_clients),
    );

    let session_dir = session.dir.clone();

    eprintln!("cliboard server listening on http://localhost:{}", port);
    eprintln!("cliboard WebSocket server on ws://localhost:{}", ws_port);

    for request in server.incoming_requests() {
        handle_request(request, &state, &version, &session_dir, ws_port, &ws_clients);
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
    ws_port: u16,
    ws_clients: &WsClients,
) {
    let url = request.url().to_string();

    match request.method() {
        Method::Get if url == "/chat" || url.starts_with("/chat?") => {
            handle_get_chat(request, session_dir)
        }
        Method::Get => handle_get(request, &url, state, version, ws_port),
        Method::Post if url == "/select" => handle_select(request, session_dir),
        Method::Post if url == "/chat" => handle_post_chat(request, session_dir, ws_clients),
        _ => respond_not_found(request),
    }
}

fn handle_get(
    request: tiny_http::Request,
    url: &str,
    state: &Arc<Mutex<RenderedState>>,
    version: &Arc<AtomicU64>,
    ws_port: u16,
) {
    match url {
        "/" => serve_embedded::<ViewerAssets>(request, "viewer.html", "text/html; charset=utf-8"),
        "/viewer.css" => serve_embedded::<ViewerAssets>(request, "viewer.css", "text/css"),
        "/viewer.js" => {
            serve_embedded::<ViewerAssets>(request, "viewer.js", "application/javascript")
        }
        _ if url == "/board" || url.starts_with("/board?") => {
            serve_board(request, url, state, version, ws_port)
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
        if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
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

/// Serve the /board endpoint: JSON with version, title, pre-rendered blocks HTML, and ws_port.
/// Supports `?v=<version>` query param -- returns 304 if the client is already up to date.
fn serve_board(
    request: tiny_http::Request,
    url: &str,
    state: &Arc<Mutex<RenderedState>>,
    version: &Arc<AtomicU64>,
    ws_port: u16,
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
        "type": "board_update",
        "version": ver,
        "title": title,
        "blocks_html": blocks_html,
        "ws_port": ws_port,
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
        #[serde(default)]
        reply_context: Option<String>,
        #[serde(default)]
        eq_num: Option<String>,
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

            // Format the clipboard text with context
            let full_eq = &selection.unicode;
            let sel_chars = selected_text.chars().count();
            let eq_chars = full_eq.chars().count();
            let is_partial = !selected_text.is_empty()
                && sel_chars < eq_chars
                && sel_chars <= eq_chars * 3 / 4;

            // Build step label: "[Step N]" or "[Step N → (N.M)]" for reply equations
            let step_label = match &sel_req.eq_num {
                Some(num) => format!("[Step {} \u{2192} ({})]", selection.step_id, num),
                None => format!("[Step {}]", selection.step_id),
            };

            let eq_text = if is_partial {
                format!("{} in {} {}", selected_text, step_label, full_eq)
            } else {
                format!("{} {}", step_label, full_eq)
            };

            // Prepend reply context (user question) if present
            let formatted = match &sel_req.reply_context {
                Some(ctx) if !ctx.is_empty() => format!("Q: \"{}\"\n{}", ctx, eq_text),
                _ => eq_text,
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

/// Handle GET /chat: return all messages as JSON.
fn handle_get_chat(request: tiny_http::Request, session_dir: &Path) {
    let session = Session {
        dir: session_dir.to_path_buf(),
        board_path: session_dir.join("board.cb.md"),
    };
    match session.read_messages() {
        Ok(store) => {
            let json = serde_json::json!({ "messages": store.messages });
            let header = Header::from_bytes("Content-Type", "application/json").unwrap();
            let resp = Response::from_string(json.to_string()).with_header(header);
            let _ = request.respond(resp);
        }
        Err(_) => {
            let json = serde_json::json!({ "messages": [] });
            let header = Header::from_bytes("Content-Type", "application/json").unwrap();
            let resp = Response::from_string(json.to_string()).with_header(header);
            let _ = request.respond(resp);
        }
    }
}

/// Handle POST /chat: receive a chat message from the viewer.
fn handle_post_chat(mut request: tiny_http::Request, session_dir: &Path, ws_clients: &WsClients) {
    const MAX_BODY_SIZE: usize = 64 * 1024; // 64KB limit

    let content_length = request.body_length().unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        let resp = Response::from_string("Payload Too Large").with_status_code(StatusCode(413));
        let _ = request.respond(resp);
        return;
    }

    let mut body = String::new();
    if request
        .as_reader()
        .take(MAX_BODY_SIZE as u64 + 1)
        .read_to_string(&mut body)
        .is_err()
    {
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
    struct ChatRequest {
        step_id: usize,
        text: String,
        #[serde(default)]
        context: Option<ChatContext>,
    }

    match serde_json::from_str::<ChatRequest>(&body) {
        Ok(chat_req) => {
            // Validate
            let text = chat_req.text.trim().to_string();
            if text.is_empty() {
                let resp = Response::from_string("Message cannot be empty")
                    .with_status_code(StatusCode(400));
                let _ = request.respond(resp);
                return;
            }
            if text.len() > 4096 {
                let resp = Response::from_string("Message too long")
                    .with_status_code(StatusCode(400));
                let _ = request.respond(resp);
                return;
            }

            let rendered = crate::render::render_chat_text(&text);
            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            // Capture values for the reply hook before msg is moved
            let hook_step_id = chat_req.step_id;
            let hook_text = text.clone();
            let hook_context = chat_req.context.clone();

            let msg = ChatMessage {
                id: format!("{:x}", timestamp_ms),
                step_id: chat_req.step_id,
                role: ChatRole::User,
                text,
                rendered,
                timestamp: chrono::Local::now().to_rfc3339(),
                context: chat_req.context,
            };

            let session = Session {
                dir: session_dir.to_path_buf(),
                board_path: session_dir.join("board.cb.md"),
            };

            if let Err(e) = session.append_message(msg) {
                eprintln!("Failed to append chat message: {}", e);
                let resp =
                    Response::from_string("Internal Server Error").with_status_code(StatusCode(500));
                let _ = request.respond(resp);
                return;
            }

            // Broadcast chat update via WebSocket
            eprintln!("[chat] POST /chat received: step={} text=\"{}\"", hook_step_id, &hook_text);
            if let Ok(store) = session.read_messages() {
                eprintln!("[chat] Broadcasting user msg via WebSocket ({} total messages)", store.messages.len());
                let ws_json = serde_json::json!({
                    "type": "chat_update",
                    "messages": store.messages,
                });
                broadcast_to_ws_clients(ws_clients, &ws_json.to_string());
            }

            // Fire reply hook: CLIBOARD_REPLY_HOOK env var, or auto-detect claude CLI
            let hook = std::env::var("CLIBOARD_REPLY_HOOK").ok();
            let use_claude = hook.is_none()
                && std::process::Command::new("which")
                    .arg("claude")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);

            if hook.is_some() || use_claude {
                let step_id = hook_step_id;
                let text = hook_text;
                let context_json = serde_json::to_string(&hook_context).unwrap_or_default();
                let session_dir_owned = session_dir.to_path_buf();

                if let Some(hook_cmd) = hook {
                    eprintln!("[chat] Firing reply hook: {}", &hook_cmd);
                    thread::spawn(move || {
                        let status = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&hook_cmd)
                            .env("CLIBOARD_STEP_ID", step_id.to_string())
                            .env("CLIBOARD_QUESTION", &text)
                            .env("CLIBOARD_CONTEXT", &context_json)
                            .status();
                        match &status {
                            Ok(s) => eprintln!("[chat] Reply hook exited: {}", s),
                            Err(e) => eprintln!("[chat] Reply hook failed: {}", e),
                        }
                    });
                } else {
                    // Default: use claude CLI
                    eprintln!("[chat] Auto-replying with claude CLI for step {}", step_id);
                    thread::spawn(move || {
                        // Read board for context
                        let board_path = session_dir_owned.join("board.cb.md");
                        let board = std::fs::read_to_string(&board_path).unwrap_or_default();

                        let prompt = format!(
                            "You are answering a question about a math derivation. \
                             Use LaTeX: $$...$$ for display equations, $...$ for inline math. \
                             Be concise and precise.\n\n\
                             Derivation:\n{}\n\n\
                             Question about step {}: {}",
                            board, step_id, text
                        );

                        let output = std::process::Command::new("claude")
                            .args(["-p", &prompt])
                            .output();

                        match output {
                            Ok(out) if out.status.success() => {
                                let reply = String::from_utf8_lossy(&out.stdout).trim().to_string();
                                if !reply.is_empty() {
                                    let rendered = crate::render::render_reply_content(&reply, step_id);
                                    let ts = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis();
                                    let msg = ChatMessage {
                                        id: format!("{:x}", ts),
                                        step_id,
                                        role: ChatRole::Assistant,
                                        text: reply,
                                        rendered,
                                        timestamp: chrono::Local::now().to_rfc3339(),
                                        context: None,
                                    };
                                    let session = Session {
                                        dir: session_dir_owned.clone(),
                                        board_path: board_path.clone(),
                                    };
                                    if let Err(e) = session.append_message(msg) {
                                        eprintln!("[chat] Failed to save claude reply: {}", e);
                                    } else {
                                        eprintln!("[chat] Claude reply saved for step {}", step_id);
                                    }
                                }
                            }
                            Ok(out) => {
                                eprintln!("[chat] claude CLI failed: {}", String::from_utf8_lossy(&out.stderr));
                            }
                            Err(e) => {
                                eprintln!("[chat] Failed to run claude: {}", e);
                            }
                        }
                    });
                }
            }

            let resp_json = serde_json::json!({ "ok": true });
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

/// Start the WebSocket server on a dedicated TCP port.
/// Accepts incoming connections and adds them to the shared client list.
fn start_ws_server(ws_port: u16, ws_clients: WsClients) {
    let addr = format!("127.0.0.1:{}", ws_port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind WebSocket server to {}: {}", addr, e);
            return;
        }
    };

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let clients = Arc::clone(&ws_clients);
                    thread::spawn(move || {
                        handle_ws_connection(stream, clients);
                    });
                }
                Err(e) => {
                    eprintln!("WebSocket accept error: {}", e);
                }
            }
        }
    });
}

/// Handle a single WebSocket connection.
/// Upgrades the TCP stream to a WebSocket, adds it to the client list,
/// then reads (and discards) incoming messages to detect disconnection.
fn handle_ws_connection(stream: TcpStream, clients: WsClients) {
    // Set a read timeout so we can periodically check for disconnection
    let _ = stream.set_nonblocking(false);

    let ws = match tungstenite::accept(stream) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    // Add to client list
    {
        let mut list = clients.lock().unwrap();
        list.push(ws);
    }

    // We don't need to read from the client in a loop here because:
    // - The client list is used by the file watcher to broadcast
    // - Disconnection is detected when broadcast fails (write returns error)
    // - The WebSocket object is now owned by the clients Vec
    //
    // This thread's job is done after handshake + registration.
}

/// Broadcast a JSON message to all connected WebSocket clients.
/// Removes disconnected clients from the list.
fn broadcast_to_ws_clients(ws_clients: &WsClients, message: &str) {
    let mut clients = ws_clients.lock().unwrap();
    eprintln!("[ws] Broadcasting to {} client(s)", clients.len());

    // Iterate backwards so we can remove by index without invalidating indices
    let mut i = clients.len();
    while i > 0 {
        i -= 1;
        let send_result =
            clients[i].send(tungstenite::Message::Text(message.to_string()));
        if send_result.is_err() {
            // Client disconnected; remove it
            clients.swap_remove(i);
        }
    }
}

/// Start a file watcher thread that re-parses and re-renders when the board file changes.
/// On change, broadcasts the new state to all WebSocket clients.
fn start_file_watcher(
    board_path: PathBuf,
    state: Arc<Mutex<RenderedState>>,
    version: Arc<AtomicU64>,
    ws_clients: WsClients,
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
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    );
                    let affects_board = event.paths.iter().any(|p| p == &board_path);
                    let messages_path = board_path.with_file_name("messages.json");
                    let affects_messages = event.paths.iter().any(|p| {
                        p == &messages_path
                            || p.file_name().and_then(|n| n.to_str()) == Some("messages.json")
                            || p.file_name().and_then(|n| n.to_str()) == Some("messages.json.tmp")
                    });

                    if dominated && affects_board {
                        if let Ok(content) = std::fs::read_to_string(&board_path) {
                            let doc = parser::parse(&content);
                            let blocks_html = render::render_blocks_html(&doc);

                            let mut st = state.lock().unwrap();
                            st.title = doc.title;
                            st.blocks_html = blocks_html;

                            let new_ver = version.fetch_add(1, Ordering::Relaxed) + 1;

                            // Build the JSON payload for WebSocket broadcast
                            let json = serde_json::json!({
                                "type": "board_update",
                                "version": new_ver,
                                "title": st.title,
                                "blocks_html": st.blocks_html,
                            });
                            drop(st);

                            broadcast_to_ws_clients(&ws_clients, &json.to_string());
                        }
                    }

                    if dominated && affects_messages {
                        // Broadcast chat update when messages.json changes
                        // (e.g., from CLI `cliboard reply` or reply hook)
                        eprintln!("[watcher] messages.json changed (event: {:?})", event.kind);
                        // Small delay to let atomic rename complete
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        if let Ok(data) = std::fs::read_to_string(&messages_path) {
                            if let Ok(store) = serde_json::from_str::<ChatStore>(&data) {
                                eprintln!("[watcher] Broadcasting chat_update via WebSocket ({} messages)", store.messages.len());
                                let json = serde_json::json!({
                                    "type": "chat_update",
                                    "messages": store.messages,
                                });
                                broadcast_to_ws_clients(&ws_clients, &json.to_string());
                            } else {
                                eprintln!("[watcher] Failed to parse messages.json");
                            }
                        } else {
                            eprintln!("[watcher] Failed to read messages.json");
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
