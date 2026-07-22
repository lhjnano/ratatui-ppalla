//! Buffer dump utilities — convert a ratatui `Buffer` to plain text or HTML.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// Render the visible portion of `buf` as plain text.
///
/// Each row is the concatenation of cell symbols from x=0 to x=width.
/// Trailing whitespace is preserved so the output has consistent shape.
#[must_use]
pub fn buffer_to_text(buf: &Buffer, width: u16, height: u16) -> String {
    let mut out = String::with_capacity(usize::from(width) * usize::from(height) * 2);
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            out.push_str(cell.symbol());
        }
        if y + 1 < height {
            out.push('\n');
        }
    }
    out
}

/// Render the visible portion of `buf` as a standalone HTML document.
///
/// Each cell becomes a `<span>` with inline CSS reflecting its style
/// (foreground/background color, modifier flags). The result is a complete
/// `<!DOCTYPE html>` document with a dark theme and monospace font.
#[must_use]
pub fn buffer_to_html(buf: &Buffer, width: u16, height: u16) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>tui-inject render</title>\n");
    html.push_str("<style>\n");
    html.push_str("  body { background: #0d1117; color: #e6edf3; margin: 0; padding: 24px; ");
    html.push_str("font-family: 'Cascadia Code','Fira Code','SF Mono',Consolas,monospace; }\n");
    html.push_str("  .frame { display: inline-block; ");
    html.push_str("border: 1px solid #30363d; background: #161b22; padding: 8px; ");
    html.push_str("line-height: 1.0; font-size: 14px; white-space: pre; }\n");
    html.push_str("  .frame div { min-height: 1em; }\n");
    html.push_str("  span { display: inline-block; }\n");
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<div class=\"frame\">\n");

    for y in 0..height {
        html.push_str("<div>");
        for x in 0..width {
            let cell = &buf[(x, y)];
            let css = style_to_css(&cell.style());
            let symbol = html_escape(cell.symbol());
            if css.is_empty() {
                html.push_str(&symbol);
            } else {
                html.push_str("<span style=\"");
                html.push_str(&css);
                html.push_str("\">");
                html.push_str(&symbol);
                html.push_str("</span>");
            }
        }
        html.push_str("</div>\n");
    }

    html.push_str("</div>\n</body>\n</html>\n");
    html
}

/// Convert a ratatui `Style` to inline CSS properties.
fn style_to_css(style: &ratatui::style::Style) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(fg) = style.fg {
        if let Some(color) = color_to_css(fg) {
            parts.push(format!("color: {color}"));
        }
    }
    if let Some(bg) = style.bg {
        if let Some(color) = color_to_css(bg) {
            parts.push(format!("background-color: {color}"));
        }
    }

    let modifier = style.add_modifier;
    if modifier.contains(Modifier::BOLD) {
        parts.push("font-weight: bold".to_string());
    }
    if modifier.contains(Modifier::ITALIC) {
        parts.push("font-style: italic".to_string());
    }
    if modifier.contains(Modifier::UNDERLINED) {
        parts.push("text-decoration: underline".to_string());
    }
    if modifier.contains(Modifier::REVERSED) {
        parts.push("filter: invert(1)".to_string());
    }

    parts.join("; ")
}

/// Convert a ratatui `Color` to a CSS color string.
fn color_to_css(color: Color) -> Option<String> {
    match color {
        Color::Reset => None,
        Color::Black => Some("#000000".to_string()),
        Color::Red => Some("#ff5555".to_string()),
        Color::Green => Some("#50fa7b".to_string()),
        Color::Yellow => Some("#f1fa8c".to_string()),
        Color::Blue => Some("#bd93f9".to_string()),
        Color::Magenta => Some("#ff79c6".to_string()),
        Color::Cyan => Some("#8be9fd".to_string()),
        Color::Gray => Some("#bbbbbb".to_string()),
        Color::DarkGray => Some("#555555".to_string()),
        Color::LightRed => Some("#ff7777".to_string()),
        Color::LightGreen => Some("#88ff99".to_string()),
        Color::LightYellow => Some("#ffffaa".to_string()),
        Color::LightBlue => Some("#ccbbff".to_string()),
        Color::LightMagenta => Some("#ffaadd".to_string()),
        Color::LightCyan => Some("#aaeeff".to_string()),
        Color::White => Some("#ffffff".to_string()),
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Indexed(i) => Some(format!("rgb({i}, {i}, {i})")),
    }
}

/// Escape special HTML characters.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            ' ' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render area dimensions as a `Rect` for convenience.
#[must_use]
pub fn full_area(width: u16, height: u16) -> Rect {
    Rect::new(0, 0, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn text_dump_concatenates_symbols() {
        let buf = Buffer::with_lines(["ab", "cd"]);
        let text = buffer_to_text(&buf, 2, 2);
        assert_eq!(text, "ab\ncd");
    }

    #[test]
    fn html_dump_wraps_in_doctype() {
        let buf = Buffer::with_lines(["hi"]);
        let html = buffer_to_html(&buf, 2, 1);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("hi"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn html_escape_handles_special_chars() {
        assert_eq!(html_escape("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(html_escape(" "), "&nbsp;");
    }

    #[test]
    fn style_to_css_handles_basic_colors() {
        let style = ratatui::style::Style::default()
            .fg(Color::Red)
            .bg(Color::Blue);
        let css = style_to_css(&style);
        assert!(css.contains("color: #ff5555"));
        assert!(css.contains("background-color: #bd93f9"));
    }
}
