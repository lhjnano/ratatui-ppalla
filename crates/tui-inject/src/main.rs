//! tui-inject — CLI tool for testing ratatui-ppalla widgets.
//!
//! Inspect, render, snapshot, replay, record, fuzz, and benchmark any
//! ratatui-ppalla widget without touching a real terminal.
//!
//! Run `tui-inject --help` for the list of commands.

mod cli;
mod commands;
mod dump;
mod record_fuzz_bench;
mod scenario;
mod widget;

use clap::Parser;

use crate::cli::Cli;
use crate::commands::run;

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    run(cli.command)
}
