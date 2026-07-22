//! Widget specifications and factories for `tui-inject`.
//!
//! Each variant of [`WidgetSpec`] corresponds to a ratatui-bubbles widget
//! configured with simple primitive parameters (strings, ints). The
//! [`WidgetSpec::render`] method draws the widget into a ratatui `Frame`.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::Frame;
use ratatui_bubbles::key_help::{KeyBinding, KeyHelp};
use ratatui_bubbles::list::{List, ListItem};
use ratatui_bubbles::spinner::{Spinner, SpinnerStyle};
use ratatui_bubbles::style::StyleBuilder;
use ratatui_bubbles::table::{Column, Row, Table};
use ratatui_bubbles::text_input::TextInput;
use ratatui_bubbles::viewport::Viewport;

/// A configured widget ready to render.
///
/// Each variant carries the minimum primitive state needed to construct
/// the underlying ratatui-bubbles widget. Use [`WidgetSpec::render`] to
/// draw it into a `Frame`.
#[derive(Debug, Clone)]
pub enum WidgetSpec {
    /// Filterable list of string items.
    List {
        /// Items to display.
        items: Vec<String>,
        /// Optional case-insensitive substring filter.
        filter: Option<String>,
    },
    /// Scrollable viewport of text lines with optional search.
    Viewport {
        /// Lines to display.
        lines: Vec<String>,
        /// Optional search query (highlights matches).
        search: Option<String>,
        /// Height of the viewport window.
        height: u16,
    },
    /// Multi-line text input with initial content.
    TextInput {
        /// Initial text content.
        initial: String,
    },
    /// Animated spinner at a given frame.
    Spinner {
        /// Visual style preset.
        style: SpinnerStyle,
        /// Number of ticks to advance before rendering.
        ticks: usize,
    },
    /// Sortable table of string rows.
    Table {
        /// Column titles and widths.
        columns: Vec<(String, u16)>,
        /// Row data (each inner Vec is one row's cells).
        rows: Vec<Vec<String>>,
    },
    /// Key-binding help display.
    KeyHelp {
        /// Bindings as (key, description) pairs.
        bindings: Vec<(String, String)>,
        /// Optional custom title for the help block.
        title: Option<String>,
    },
    /// StyleBuilder demo — renders a styled span to show the result of `build()`.
    StyleDemo {
        /// Whether to enable bold.
        bold: bool,
        /// Whether to enable italic.
        italic: bool,
        /// Whether to enable underline.
        underline: bool,
    },
}

/// String item adapter for the [`List`] widget.
struct StringItem(String);

impl ListItem for StringItem {
    fn render(&self) -> Line<'_> {
        Line::from(self.0.as_str())
    }

    fn filterable_text(&self) -> &str {
        &self.0
    }
}

/// String row adapter for the [`Table`] widget.
#[derive(Debug, Clone)]
struct StringRow(Vec<String>);

impl Row for StringRow {
    fn cells(&self) -> Vec<String> {
        self.0.clone()
    }
}

impl WidgetSpec {
    /// All available widget names, in stable order.
    pub const NAMES: &'static [&'static str] = &[
        "list",
        "viewport",
        "text-input",
        "spinner",
        "table",
        "key-help",
        "style-demo",
    ];

    /// Construct a widget spec from its name with default parameters.
    ///
    /// Returns `None` if `name` is not a known widget.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "list" => Self::List {
                items: sample_items(),
                filter: None,
            },
            "viewport" => Self::Viewport {
                lines: sample_lines(),
                search: None,
                height: 10,
            },
            "text-input" => Self::TextInput {
                initial: String::new(),
            },
            "spinner" => Self::Spinner {
                style: SpinnerStyle::Line,
                ticks: 0,
            },
            "table" => Self::Table {
                columns: vec![("name".to_string(), 12), ("age".to_string(), 5)],
                rows: sample_table_rows(),
            },
            "key-help" => Self::KeyHelp {
                bindings: sample_bindings(),
                title: None,
            },
            "style-demo" => Self::StyleDemo {
                bold: true,
                italic: false,
                underline: false,
            },
            _ => return None,
        })
    }

    /// Render this widget into `frame` at `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match self {
            Self::List { items, filter } => {
                let list_items: Vec<StringItem> = items.iter().cloned().map(StringItem).collect();
                let mut list = List::new(list_items);
                if let Some(f) = filter {
                    list.set_filter(f);
                }
                list.render(frame, area);
            }
            Self::Viewport {
                lines,
                search,
                height,
            } => {
                let mut vp = Viewport::new(*height);
                for line in lines {
                    vp.append_line(Line::from(line.clone()));
                }
                if let Some(s) = search {
                    vp.set_search(Some(s.as_str()));
                }
                vp.render(frame, area);
            }
            Self::TextInput { initial } => {
                let mut ti = TextInput::new();
                ti.insert_str(initial.as_str());
                ti.render(frame, area);
            }
            Self::Spinner { style, ticks } => {
                let mut spinner = Spinner::new(*style);
                for _ in 0..*ticks {
                    spinner.tick();
                }
                spinner.render(frame, area);
            }
            Self::Table { columns, rows } => {
                let cols: Vec<Column> = columns
                    .iter()
                    .map(|(title, width)| Column::new(title.clone(), *width))
                    .collect();
                let mut table = Table::<StringRow>::new(cols);
                let table_rows: Vec<StringRow> = rows.iter().cloned().map(StringRow).collect();
                table.set_rows(table_rows);
                table.render(frame, area);
            }
            Self::KeyHelp { bindings, title } => {
                let mut help = if let Some(t) = title {
                    KeyHelp::new().with_title(t.clone())
                } else {
                    KeyHelp::new()
                };
                for (key, desc) in bindings {
                    help.add(KeyBinding::new(key.clone(), desc.clone()));
                }
                help.render(frame, area);
            }
            Self::StyleDemo {
                bold,
                italic,
                underline,
            } => {
                let mut builder = StyleBuilder::new().foreground(ratatui::style::Color::Cyan);
                if *bold {
                    builder = builder.bold();
                }
                if *italic {
                    builder = builder.italic();
                }
                if *underline {
                    builder = builder.underline();
                }
                let style = builder.build();
                let span = ratatui::text::Span::styled("styled text demo", style);
                let line = Line::from(span);
                let paragraph = ratatui::widgets::Paragraph::new(line);
                frame.render_widget(paragraph, area);
            }
        }
    }
}

fn sample_items() -> Vec<String> {
    vec![
        "apple".to_string(),
        "banana".to_string(),
        "cherry".to_string(),
        "date".to_string(),
        "elderberry".to_string(),
        "fig".to_string(),
        "grape".to_string(),
    ]
}

fn sample_lines() -> Vec<String> {
    vec![
        "First line of the viewport".to_string(),
        "Second line follows here".to_string(),
        "Third line continues the pattern".to_string(),
        "Fourth line wraps up the demo".to_string(),
        "Fifth line for good measure".to_string(),
    ]
}

fn sample_table_rows() -> Vec<Vec<String>> {
    vec![
        vec!["alice".to_string(), "30".to_string()],
        vec!["bob".to_string(), "25".to_string()],
        vec!["charlie".to_string(), "35".to_string()],
    ]
}

fn sample_bindings() -> Vec<(String, String)> {
    vec![
        ("q".to_string(), "quit".to_string()),
        ("↑/↓".to_string(), "navigate".to_string()),
        ("enter".to_string(), "select".to_string()),
        ("/".to_string(), "filter".to_string()),
    ]
}
