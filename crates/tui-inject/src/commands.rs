//! Command dispatch and implementations for `tui-inject`.

use std::fs;
use std::io::Write;
use std::process::ExitCode;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::cli::Command;
use crate::dump::{buffer_to_html, buffer_to_text, full_area};
use crate::widget::WidgetSpec;

/// Dispatch a parsed CLI command. Returns a process exit code.
pub fn run(command: Command) -> ExitCode {
    match command {
        Command::List => list(),
        Command::Render {
            name,
            items,
            filter,
            ticks,
            text,
            format,
            width,
            height,
        } => render(&name, items, filter, ticks, text, &format, width, height),
        Command::Snapshot {
            name,
            output,
            format,
            items,
            filter,
            ticks,
            text,
            width,
            height,
        } => snapshot(
            &name, &output, format, items, filter, ticks, text, width, height,
        ),
        Command::Replay { scenario } => replay(&scenario),
        Command::Record { output } => crate::record_fuzz_bench::record(&output),
        Command::Fuzz { name, events } => crate::record_fuzz_bench::fuzz(&name, events),
        Command::Bench { name, iterations } => crate::record_fuzz_bench::bench(&name, iterations),
    }
}

/// `tui-inject list` — enumerate available widgets.
fn list() -> ExitCode {
    println!("Available widgets:");
    println!();
    for name in WidgetSpec::NAMES {
        let spec = WidgetSpec::from_name(name)
            .unwrap_or_else(|| panic!("NAMES entry '{name}' missing from_name"));
        println!("  {name:14} — {}", describe(&spec));
    }
    println!();
    println!("Use `tui-inject render <name>` to render a widget.");
    ExitCode::SUCCESS
}

fn describe(spec: &WidgetSpec) -> &'static str {
    match spec {
        WidgetSpec::List { .. } => "Filterable list of items",
        WidgetSpec::Viewport { .. } => "Scrollable text viewport with search",
        WidgetSpec::TextInput { .. } => "Multi-line text input with history",
        WidgetSpec::Spinner { .. } => "Animated spinner with 8 style presets",
        WidgetSpec::Table { .. } => "Sortable table with selectable rows",
        WidgetSpec::KeyHelp { .. } => "Key binding help display",
        WidgetSpec::StyleDemo { .. } => "StyleBuilder demo showing bold/italic/underline",
    }
}

/// Build a `WidgetSpec` from CLI args.
fn build_spec(
    name: &str,
    items: Option<usize>,
    filter: Option<String>,
    ticks: Option<usize>,
    text: Option<String>,
) -> Result<WidgetSpec, String> {
    let mut spec = WidgetSpec::from_name(name).ok_or_else(|| {
        format!(
            "unknown widget '{name}'. Available: {}",
            WidgetSpec::NAMES.join(", ")
        )
    })?;
    apply_overrides(&mut spec, items, filter, ticks, text);
    Ok(spec)
}

/// Apply CLI overrides onto a default-constructed spec.
fn apply_overrides(
    spec: &mut WidgetSpec,
    items: Option<usize>,
    filter: Option<String>,
    ticks: Option<usize>,
    text: Option<String>,
) {
    // Pre-clone for reuse across multiple match arms (String doesn't impl Copy).
    let filter_clone = filter.clone();
    let text_clone = text.clone();

    if let WidgetSpec::List {
        items: ref mut spec_items,
        filter: ref mut spec_filter,
    } = spec
    {
        if let Some(n) = items {
            *spec_items = (0..n).map(|i| format!("item-{i}")).collect();
        }
        if let Some(f) = filter {
            *spec_filter = Some(f);
        }
    }
    if let WidgetSpec::Viewport {
        search: ref mut spec_search,
        ..
    } = spec
    {
        if let Some(f) = filter_clone {
            *spec_search = Some(f);
        }
    }
    if let WidgetSpec::Spinner {
        ticks: ref mut spec_ticks,
        ..
    } = spec
    {
        if let Some(t) = ticks {
            *spec_ticks = t;
        }
    }
    if let WidgetSpec::TextInput {
        initial: ref mut spec_text,
    } = spec
    {
        if let Some(t) = text_clone {
            *spec_text = t;
        }
    }
}

/// `tui-inject render` — render and print to stdout.
#[allow(clippy::too_many_arguments)]
fn render(
    name: &str,
    items: Option<usize>,
    filter: Option<String>,
    ticks: Option<usize>,
    text: Option<String>,
    format: &str,
    width: u16,
    height: u16,
) -> ExitCode {
    let spec = match build_spec(name, items, filter, ticks, text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let output = match render_to_string(&spec, format, width, height) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("render error: {e}");
            return ExitCode::from(1);
        }
    };
    println!("{output}");
    ExitCode::SUCCESS
}

/// `tui-inject replay` — replay a TOML scenario file against a widget.
fn replay(scenario_path: &str) -> ExitCode {
    let scenario = match crate::scenario::load(scenario_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    match scenario.run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("replay error: {e}");
            ExitCode::from(1)
        }
    }
}

/// `tui-inject snapshot` — render and save to a file.
#[allow(clippy::too_many_arguments)]
fn snapshot(
    name: &str,
    output: &str,
    format: Option<String>,
    items: Option<usize>,
    filter: Option<String>,
    ticks: Option<usize>,
    text: Option<String>,
    width: u16,
    height: u16,
) -> ExitCode {
    let spec = match build_spec(name, items, filter, ticks, text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    // Infer format from extension if not given.
    let format = format.map_or_else(|| infer_format(output), String::from);
    let output_text = match render_to_string(&spec, &format, width, height) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("render error: {e}");
            return ExitCode::from(1);
        }
    };
    match fs::write(output, output_text) {
        Ok(()) => {
            eprintln!("snapshot saved to {output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("write error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Infer output format from a filename extension.
fn infer_format(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "html".to_string()
    } else {
        "text".to_string()
    }
}

/// Render a widget spec into a text or HTML string via TestBackend.
fn render_to_string(
    spec: &WidgetSpec,
    format: &str,
    width: u16,
    height: u16,
) -> Result<String, String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal setup: {e}"))?;
    terminal
        .draw(|frame| spec.render(frame, full_area(width, height)))
        .map_err(|e| format!("draw: {e}"))?;
    let buf = terminal.backend().buffer().clone();
    match format {
        "text" | "txt" => Ok(buffer_to_text(&buf, width, height)),
        "html" | "htm" => Ok(buffer_to_html(&buf, width, height)),
        other => Err(format!("unknown format '{other}' (use: text, html)")),
    }
}

/// Suppress unused import warning for std::io::Write (kept for future use
/// by the record command).
#[allow(dead_code)]
fn _write_flush(_: &mut dyn Write) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_format_recognizes_html() {
        assert_eq!(infer_format("out.html"), "html");
        assert_eq!(infer_format("out.HTM"), "html");
        assert_eq!(infer_format("out.txt"), "text");
        assert_eq!(infer_format("snapshot"), "text");
    }

    #[test]
    fn build_spec_for_known_widget() {
        let spec = build_spec("list", None, None, None, None);
        assert!(spec.is_ok());
        let WidgetSpec::List { items, .. } = spec.unwrap() else {
            panic!("expected List variant");
        };
        assert!(!items.is_empty());
    }

    #[test]
    fn build_spec_rejects_unknown_widget() {
        let spec = build_spec("does-not-exist", None, None, None, None);
        assert!(spec.is_err());
    }

    #[test]
    fn render_to_text_produces_output() {
        let spec = WidgetSpec::from_name("list").unwrap();
        let out = render_to_string(&spec, "text", 40, 8).unwrap();
        assert!(out.contains("apple"));
    }

    #[test]
    fn render_to_html_includes_doctype() {
        let spec = WidgetSpec::from_name("list").unwrap();
        let out = render_to_string(&spec, "html", 40, 8).unwrap();
        assert!(out.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn render_to_unknown_format_errors() {
        let spec = WidgetSpec::from_name("list").unwrap();
        let result = render_to_string(&spec, "pdf", 40, 8);
        assert!(result.is_err());
    }
}
