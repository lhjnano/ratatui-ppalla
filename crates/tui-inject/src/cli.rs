//! Command-line interface definitions for `tui-inject`.

use clap::{Parser, Subcommand};

/// CLI tool for testing ratatui-bubbles widgets via event injection.
#[derive(Parser, Debug)]
#[command(name = "tui-inject", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Available commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// List all available widgets with their parameter schemas.
    List,

    /// Render a widget to stdout (text or HTML).
    Render {
        /// Widget name (run `list` to see options).
        name: String,

        /// Number of items (used by `list`, `table`).
        #[arg(long)]
        items: Option<usize>,

        /// Filter query (used by `list`).
        #[arg(long)]
        filter: Option<String>,

        /// Number of spinner ticks before rendering.
        #[arg(long)]
        ticks: Option<usize>,

        /// Initial text content (used by `text-input`).
        #[arg(long)]
        text: Option<String>,

        /// Output format: `text` (default) or `html`.
        #[arg(long, default_value = "text")]
        format: String,

        /// Frame width in terminal cells.
        #[arg(long, default_value_t = 60)]
        width: u16,

        /// Frame height in terminal cells.
        #[arg(long, default_value_t = 16)]
        height: u16,
    },

    /// Render a widget and save the output to a file.
    Snapshot {
        /// Widget name.
        name: String,

        /// Output file path. Extensions `.html`/`.htm` switch to HTML format.
        #[arg(short, long)]
        output: String,

        /// Output format override (otherwise inferred from extension).
        #[arg(long)]
        format: Option<String>,

        #[arg(long)]
        items: Option<usize>,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        ticks: Option<usize>,
        #[arg(long)]
        text: Option<String>,

        #[arg(long, default_value_t = 60)]
        width: u16,
        #[arg(long, default_value_t = 16)]
        height: u16,
    },

    /// Replay a TOML scenario file against a widget.
    Replay {
        /// Path to the scenario file (.toml).
        scenario: String,
    },

    /// Record keyboard events interactively and save as a TOML scenario.
    Record {
        /// Output scenario file path.
        output: String,
    },

    /// Fuzz a widget with N random events.
    Fuzz {
        /// Widget name.
        name: String,

        /// Number of random events to generate.
        #[arg(short, long, default_value_t = 100)]
        events: usize,
    },

    /// Benchmark widget render performance.
    Bench {
        /// Widget name.
        name: String,

        /// Number of render iterations.
        #[arg(short, long, default_value_t = 1000)]
        iterations: usize,
    },
}
