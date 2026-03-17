use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cliboard", about = "Live math rendering board for CLI agents and scientists")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new derivation session and open the board
    New {
        /// Title for the derivation
        title: String,
    },
    /// Add a titled step with an equation
    Step {
        /// Step title
        title: String,
        /// LaTeX equation
        latex: String,
    },
    /// Add an equation to the current step
    Eq {
        /// LaTeX equation
        latex: String,
    },
    /// Add a note/annotation to the current step
    Note {
        /// Note text
        text: String,
    },
    /// Add a text block (prose, no equation)
    Text {
        /// Text content
        text: String,
    },
    /// Add a highlighted result step
    Result {
        /// Result title
        title: String,
        /// LaTeX equation
        latex: String,
    },
    /// Add a section divider
    Divider,
    /// Render a single equation (one-shot, no session)
    Render {
        /// LaTeX equation (use - for stdin)
        latex: String,
        /// Output file (opens in browser if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Stop the current session server
    Stop,
    /// Export the current board as self-contained HTML
    Export {
        /// Output file path
        output: String,
    },
    /// Show current session status
    Status,
    /// Print what was last selected on the board
    Selection {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Output raw LaTeX only
        #[arg(long)]
        latex: bool,
    },
    /// Show chat messages (pending questions from viewers)
    Chat {
        /// Show all messages, not just pending
        #[arg(long)]
        all: bool,
        /// Filter by step number
        #[arg(long)]
        step: Option<usize>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reply to a question on a specific step
    Reply {
        /// Step number to reply to
        step_id: usize,
        /// Reply text (supports $...$ inline math)
        text: String,
    },
    /// Watch for new chat questions and print them to stdout (blocking)
    Listen {
        /// Output as JSON lines
        #[arg(long)]
        json: bool,
    },
    /// Update cliboard to the latest version
    Update {
        /// Just check for updates, don't install
        #[arg(long)]
        check: bool,
    },
}
