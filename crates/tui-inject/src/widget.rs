//! Widget specifications and factories for `tui-inject`.
//!
//! Each variant of [`WidgetSpec`] corresponds to a ratatui-presto widget
//! configured with simple primitive parameters (strings, ints). The
//! [`WidgetSpec::render`] method draws the widget into a ratatui `Frame`.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::Frame;
use ratatui_presto::key_help::{KeyBinding, KeyHelp};
use ratatui_presto::list::{List, ListItem};
use ratatui_presto::spinner::{Spinner, SpinnerStyle};
use ratatui_presto::style::StyleBuilder;
use ratatui_presto::table::{Column, Row, Table};
use ratatui_presto::text_input::TextInput;
use ratatui_presto::viewport::Viewport;

/// A configured widget ready to render.
///
/// Each variant carries the minimum primitive state needed to construct
/// the underlying ratatui-presto widget. Use [`WidgetSpec::render`] to
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    /// Render a widget spec into a fresh buffer for assertions.
    fn render_buffer(spec: &WidgetSpec, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| spec.render(frame, Rect::new(0, 0, width, height)))
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn row_symbols(buf: &ratatui::buffer::Buffer, y: usize, len: usize) -> String {
        (0..len)
            .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
            .collect()
    }

    // ----- from_name + NAMES -----

    #[test]
    fn names_includes_all_seven_widgets() {
        assert_eq!(WidgetSpec::NAMES.len(), 7);
        assert!(WidgetSpec::NAMES.contains(&"list"));
        assert!(WidgetSpec::NAMES.contains(&"viewport"));
        assert!(WidgetSpec::NAMES.contains(&"text-input"));
        assert!(WidgetSpec::NAMES.contains(&"spinner"));
        assert!(WidgetSpec::NAMES.contains(&"table"));
        assert!(WidgetSpec::NAMES.contains(&"key-help"));
        assert!(WidgetSpec::NAMES.contains(&"style-demo"));
    }

    #[test]
    fn from_name_returns_some_for_known_widgets() {
        for name in WidgetSpec::NAMES {
            assert!(
                WidgetSpec::from_name(name).is_some(),
                "from_name should return Some for known widget '{name}'"
            );
        }
    }

    #[test]
    fn from_name_returns_none_for_unknown() {
        assert!(WidgetSpec::from_name("does-not-exist").is_none());
        assert!(WidgetSpec::from_name("").is_none());
    }

    // ----- List -----

    #[test]
    fn list_renders_items() {
        let spec = WidgetSpec::from_name("list").unwrap();
        let buf = render_buffer(&spec, 30, 8);
        let row0 = row_symbols(&buf, 0, 30);
        assert!(row0.contains("apple"), "row 0 was: {row0:?}");
    }

    #[test]
    fn list_filter_narrows_visible_items() {
        let spec = WidgetSpec::List {
            items: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            filter: Some("am".to_string()),
        };
        let buf = render_buffer(&spec, 20, 5);
        let row0 = row_symbols(&buf, 0, 20);
        assert!(row0.contains("gamma"));
        // "alpha" should NOT appear in the first 3 rows when filter="am"
        for y in 0..3 {
            let row = row_symbols(&buf, y, 20);
            assert!(
                !row.contains("alpha"),
                "row {y} should not contain alpha: {row:?}"
            );
        }
    }

    // ----- Viewport -----

    #[test]
    fn viewport_renders_lines() {
        let spec = WidgetSpec::Viewport {
            lines: vec!["hello viewport".to_string()],
            search: None,
            height: 5,
        };
        let buf = render_buffer(&spec, 30, 6);
        let row0 = row_symbols(&buf, 0, 30);
        assert!(row0.contains("hello"));
    }

    #[test]
    fn viewport_with_search_highlights_matches() {
        let spec = WidgetSpec::Viewport {
            lines: vec!["foo line".to_string(), "no match".to_string()],
            search: Some("foo".to_string()),
            height: 5,
        };
        let buf = render_buffer(&spec, 30, 6);
        // search doesn't filter, just highlights — both lines should be present
        let combined = (0..2).map(|y| row_symbols(&buf, y, 30)).collect::<String>();
        assert!(combined.contains("foo"));
    }

    // ----- TextInput -----

    #[test]
    fn text_input_renders_initial_content() {
        let spec = WidgetSpec::TextInput {
            initial: "hello world".to_string(),
        };
        let buf = render_buffer(&spec, 30, 3);
        let row0 = row_symbols(&buf, 0, 30);
        assert!(row0.contains("hello"));
    }

    // ----- Spinner -----

    #[test]
    fn spinner_renders_first_frame_for_each_style() {
        for style in [
            SpinnerStyle::Line,
            SpinnerStyle::Dot,
            SpinnerStyle::MiniDot,
            SpinnerStyle::Jump,
            SpinnerStyle::Pulse,
            SpinnerStyle::Meter,
            SpinnerStyle::Hamburger,
            SpinnerStyle::Ellipsis,
        ] {
            let spec = WidgetSpec::Spinner { style, ticks: 0 };
            let buf = render_buffer(&spec, 5, 1);
            let frame = style.frames()[0];
            let rendered = row_symbols(&buf, 0, frame.chars().count());
            assert!(
                rendered.starts_with(frame) || rendered.contains(frame.trim()),
                "SpinnerStyle {style:?} frame 0 '{frame}' not found in '{rendered}'"
            );
        }
    }

    #[test]
    fn spinner_tick_advances_frame_in_render() {
        let spec = WidgetSpec::Spinner {
            style: SpinnerStyle::Line,
            ticks: 2,
        };
        let buf = render_buffer(&spec, 5, 1);
        let rendered = row_symbols(&buf, 0, 1);
        // Line frames: ["|", "/", "-", "\\"]; 2 ticks → "-"
        assert_eq!(rendered, "-");
    }

    // ----- Table -----

    #[test]
    fn table_renders_header_and_data() {
        let spec = WidgetSpec::from_name("table").unwrap();
        let buf = render_buffer(&spec, 30, 8);
        let header = row_symbols(&buf, 0, 30);
        assert!(header.contains("name"), "header was: {header:?}");
        // First data row
        let row1 = row_symbols(&buf, 1, 30);
        assert!(row1.contains("alice"), "row1 was: {row1:?}");
    }

    // ----- KeyHelp -----

    #[test]
    fn key_help_renders_title_and_bindings() {
        let spec = WidgetSpec::KeyHelp {
            bindings: vec![("q".to_string(), "quit".to_string())],
            title: Some("Test Title".to_string()),
        };
        let buf = render_buffer(&spec, 30, 6);
        // Title appears in top border
        let top = row_symbols(&buf, 0, 30);
        assert!(top.contains("Test Title"), "top was: {top:?}");
        // Binding appears in content area (row 1 inside border)
        let row1 = row_symbols(&buf, 1, 30);
        assert!(
            row1.contains('q') && row1.contains("quit"),
            "row1 was: {row1:?}"
        );
    }

    // ----- StyleDemo -----

    #[test]
    fn style_demo_renders_text() {
        let spec = WidgetSpec::StyleDemo {
            bold: true,
            italic: true,
            underline: true,
        };
        let buf = render_buffer(&spec, 30, 3);
        let row0 = row_symbols(&buf, 0, 30);
        assert!(row0.contains("styled text"), "row0 was: {row0:?}");
    }
}
