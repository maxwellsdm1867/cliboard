use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::document::Selection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub step_id: usize,
    pub role: ChatRole,
    pub text: String,
    pub rendered: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ChatContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatContext {
    pub selected: Option<String>,
    pub latex: Option<String>,
    pub step_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatStore {
    pub messages: Vec<ChatMessage>,
}

pub struct Session {
    pub dir: PathBuf,
    pub board_path: PathBuf,
}

impl Session {
    /// Create a new session with the given title.
    /// Creates ~/.cliboard/sessions/<date>-<slug>/board.cb.md
    pub fn create(title: &str) -> std::io::Result<Session> {
        let base = sessions_dir();
        let slug = slugify(title);
        let timestamp = chrono::Local::now().format("%Y-%m-%d").to_string();
        let dir_name = format!("{}-{}", timestamp, slug);
        let dir = base.join(&dir_name);
        fs::create_dir_all(&dir)?;

        let board_path = dir.join("board.cb.md");
        let frontmatter = format!("---\ntitle: {}\n---\n\n", title);
        fs::write(&board_path, frontmatter)?;

        // Write the session directory path to a "current" marker file
        let current_path = base.join("current");
        fs::write(&current_path, dir.to_string_lossy().as_bytes())?;

        Ok(Session { dir, board_path })
    }

    /// Find the current active session.
    pub fn find_current() -> Option<Session> {
        let current_path = sessions_dir().join("current");
        let dir_str = fs::read_to_string(&current_path).ok()?;
        let dir = PathBuf::from(dir_str.trim());
        let board_path = dir.join("board.cb.md");
        if board_path.exists() {
            Some(Session { dir, board_path })
        } else {
            None
        }
    }

    /// Append content to the board.cb.md file with file locking.
    pub fn append(&self, content: &str) -> std::io::Result<()> {
        use fs4::fs_std::FileExt;
        let file = fs::OpenOptions::new()
            .append(true)
            .open(&self.board_path)?;
        file.lock_exclusive()?;
        let mut file = file;
        write!(file, "{}", content)?;
        file.unlock()?;
        Ok(())
    }

    /// Read the board content.
    pub fn read_board(&self) -> std::io::Result<String> {
        fs::read_to_string(&self.board_path)
    }

    /// Write selection.json to both the session dir and the global ~/.cliboard/ dir.
    pub fn write_selection(&self, selection: &Selection) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(selection)
            .map_err(std::io::Error::other)?;
        let session_path = self.dir.join("selection.json");
        fs::write(&session_path, &json)?;
        let global_path = cliboard_dir().join("selection.json");
        fs::write(&global_path, &json)?;
        Ok(())
    }

    /// Read selection.json from the session directory.
    pub fn read_selection(&self) -> Option<Selection> {
        let path = self.dir.join("selection.json");
        let json = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&json).ok()
    }

    /// Path to the server PID file.
    pub fn pid_path(&self) -> PathBuf {
        self.dir.join("server.pid")
    }

    /// Write the server PID.
    pub fn write_pid(&self, pid: u32) -> std::io::Result<()> {
        fs::write(self.pid_path(), pid.to_string())
    }

    /// Read the server PID.
    pub fn read_pid(&self) -> Option<u32> {
        fs::read_to_string(self.pid_path())
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Remove the PID file.
    pub fn remove_pid(&self) {
        let _ = fs::remove_file(self.pid_path());
    }

    /// Path to the server port file.
    pub fn port_path(&self) -> PathBuf {
        self.dir.join("server.port")
    }

    /// Write the server port.
    pub fn write_port(&self, port: u16) -> std::io::Result<()> {
        fs::write(self.port_path(), port.to_string())
    }

    /// Read the server port.
    pub fn read_port(&self) -> Option<u16> {
        fs::read_to_string(self.port_path())
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Path to the messages.json file.
    pub fn messages_path(&self) -> PathBuf {
        self.dir.join("messages.json")
    }

    /// Read all chat messages from messages.json.
    /// Returns an empty ChatStore if the file doesn't exist.
    pub fn read_messages(&self) -> std::io::Result<ChatStore> {
        let path = self.messages_path();
        if !path.exists() {
            return Ok(ChatStore::default());
        }
        let data = fs::read_to_string(&path)?;
        serde_json::from_str(&data).map_err(std::io::Error::other)
    }

    /// Append a message to messages.json atomically with file locking.
    pub fn append_message(&self, msg: ChatMessage) -> std::io::Result<()> {
        use fs4::fs_std::FileExt;
        use std::io::{Read, Seek, SeekFrom, Write};

        let path = self.messages_path();

        // Open or create, then lock (no TOCTOU race)
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        file.lock_exclusive()?;

        // Read current contents while holding lock
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut store: ChatStore = if contents.is_empty() {
            ChatStore::default()
        } else {
            serde_json::from_str(&contents).unwrap_or_default()
        };

        store.messages.push(msg);

        let json = serde_json::to_string_pretty(&store).map_err(std::io::Error::other)?;

        // Truncate and rewrite while holding lock
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        file.write_all(json.as_bytes())?;
        file.flush()?;

        file.unlock()?;
        Ok(())
    }

    /// Return messages where the last message for each step_id is from a User
    /// (i.e., unanswered questions).
    pub fn pending_messages(&self) -> std::io::Result<Vec<ChatMessage>> {
        let store = self.read_messages()?;
        // Group by step_id, find steps where last message is from User
        let mut last_by_step: std::collections::HashMap<usize, &ChatMessage> =
            std::collections::HashMap::new();
        for msg in &store.messages {
            last_by_step.insert(msg.step_id, msg);
        }
        let pending_step_ids: std::collections::HashSet<usize> = last_by_step
            .iter()
            .filter(|(_, msg)| msg.role == ChatRole::User)
            .map(|(step_id, _)| *step_id)
            .collect();
        // Return all messages from pending steps (for context)
        Ok(store
            .messages
            .into_iter()
            .filter(|m| pending_step_ids.contains(&m.step_id))
            .collect())
    }

    /// Return messages for a specific step.
    pub fn messages_for_step(&self, step_id: usize) -> std::io::Result<Vec<ChatMessage>> {
        let store = self.read_messages()?;
        Ok(store
            .messages
            .into_iter()
            .filter(|m| m.step_id == step_id)
            .collect())
    }
}

fn cliboard_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cliboard")
}

fn sessions_dir() -> PathBuf {
    cliboard_dir().join("sessions")
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("E = mc^2"), "e-mc-2");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("Already-Slugged"), "already-slugged");
    }

    #[test]
    fn test_cliboard_dir() {
        let dir = cliboard_dir();
        assert!(dir.to_string_lossy().contains(".cliboard"));
    }

    #[test]
    fn test_sessions_dir() {
        let dir = sessions_dir();
        assert!(dir.to_string_lossy().contains("sessions"));
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("a/b\\c"), "a-b-c");
        assert_eq!(slugify("---dashes---"), "dashes");
        assert_eq!(slugify("MiXeD CaSe"), "mixed-case");
        assert_eq!(slugify("123 Numbers"), "123-numbers");
    }

    #[test]
    fn test_slugify_unicode() {
        // Unicode alphanumeric chars are kept by is_alphanumeric()
        assert_eq!(slugify("café"), "café");
        // ï is alphanumeric in Unicode, so it's kept
        assert_eq!(slugify("naïve"), "naïve");
    }

    #[test]
    fn test_create_session() {
        // Use a temp directory to avoid polluting ~/.cliboard
        let tmp = tempfile::tempdir().unwrap();
        let tmp_path = tmp.path().to_path_buf();

        // We can't easily override sessions_dir(), so test the
        // primitives directly: create dir, write board, read it back
        let session_dir = tmp_path.join("test-session");
        fs::create_dir_all(&session_dir).unwrap();

        let board_path = session_dir.join("board.cb.md");
        let frontmatter = "---\ntitle: Test\n---\n\n";
        fs::write(&board_path, frontmatter).unwrap();

        let session = Session {
            dir: session_dir.clone(),
            board_path: board_path.clone(),
        };

        // Test read_board
        let content = session.read_board().unwrap();
        assert!(content.contains("title: Test"));

        // Test append
        session.append("\n## Step 1\n\n$$x = 1$$\n").unwrap();
        let content = session.read_board().unwrap();
        assert!(content.contains("## Step 1"));
        assert!(content.contains("$$x = 1$$"));
    }

    #[test]
    fn test_pid_write_read_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let session = Session {
            dir: tmp.path().to_path_buf(),
            board_path: tmp.path().join("board.cb.md"),
        };

        // No PID initially
        assert!(session.read_pid().is_none());

        // Write and read PID
        session.write_pid(12345).unwrap();
        assert_eq!(session.read_pid(), Some(12345));

        // Remove PID
        session.remove_pid();
        assert!(session.read_pid().is_none());
    }

    #[test]
    fn test_port_write_read() {
        let tmp = tempfile::tempdir().unwrap();
        let session = Session {
            dir: tmp.path().to_path_buf(),
            board_path: tmp.path().join("board.cb.md"),
        };

        assert!(session.read_port().is_none());

        session.write_port(8377).unwrap();
        assert_eq!(session.read_port(), Some(8377));
    }

    #[test]
    fn test_selection_write_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();

        // Also create the global .cliboard dir in tmp (won't work with real
        // cliboard_dir, but we can test session-local selection)
        let session_dir = tmp.path().to_path_buf();
        let board_path = session_dir.join("board.cb.md");
        fs::write(&board_path, "").unwrap();

        let session = Session {
            dir: session_dir.clone(),
            board_path,
        };

        // Write selection JSON directly (bypassing write_selection which
        // also writes to ~/.cliboard)
        let selection = Selection {
            step_id: 1,
            title: "Test Step".to_string(),
            latex: "E = mc^2".to_string(),
            unicode: "E = mc\u{00B2}".to_string(),
            formatted: "E = mc^2".to_string(),
            notes: vec!["Famous equation".to_string()],
            selected_at: "2026-03-16T00:00:00".to_string(),
        };
        let json = serde_json::to_string_pretty(&selection).unwrap();
        fs::write(session_dir.join("selection.json"), &json).unwrap();

        // Read it back
        let read_back = session.read_selection().unwrap();
        assert_eq!(read_back.step_id, 1);
        assert_eq!(read_back.title, "Test Step");
        assert_eq!(read_back.latex, "E = mc^2");
        assert_eq!(read_back.unicode, "E = mc\u{00B2}");
    }

    #[test]
    fn test_find_current_returns_none_without_session() {
        // This tests the real find_current — if no session has been created
        // in the current test environment, it should return None or Some
        // depending on whether a real session exists. We just verify it
        // doesn't panic.
        let _result = Session::find_current();
    }

    #[test]
    fn test_append_multiple_times() {
        let tmp = tempfile::tempdir().unwrap();
        let board_path = tmp.path().join("board.cb.md");
        fs::write(&board_path, "---\ntitle: T\n---\n").unwrap();

        let session = Session {
            dir: tmp.path().to_path_buf(),
            board_path: board_path.clone(),
        };

        session.append("\n## Step 1\n\n$$a$$\n").unwrap();
        session.append("\n## Step 2\n\n$$b$$\n").unwrap();
        session.append("\n## Step 3\n\n$$c$$\n").unwrap();

        let content = session.read_board().unwrap();
        assert!(content.contains("## Step 1"));
        assert!(content.contains("## Step 2"));
        assert!(content.contains("## Step 3"));
    }
}
