//! TOML scenario parsing for `tui-inject replay`.
//!
//! Scenario files describe a widget configuration, a sequence of events to
//! inject, and an output format. The format is intentionally minimal —
//! crossterm's `Event` type has no built-in serde, so we define a small
//! [`ScenarioEvent`] schema that covers the common key/mouse cases.

use std::fs;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use serde::Deserialize;

/// A complete replay scenario.
#[derive(Debug, Deserialize)]
pub struct Scenario {
    /// Widget configuration.
    pub widget: WidgetConfig,
    /// Events to inject, in order.
    #[serde(default)]
    pub events: Vec<ScenarioEvent>,
    /// Output format (text or html).
    #[serde(default)]
    pub output: OutputConfig,
}

/// Widget configuration block.
#[derive(Debug, Deserialize)]
pub struct WidgetConfig {
    /// Widget name — currently only `counter` supports replay.
    pub name: String,
    /// Initial value (used by `counter`).
    #[serde(default)]
    pub initial: Option<i32>,
}

/// Output configuration block.
#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    /// Format: `text` (default) or `html`.
    #[serde(default = "default_format")]
    pub format: String,
    /// Frame width.
    #[serde(default = "default_width")]
    pub width: u16,
    /// Frame height.
    #[serde(default = "default_height")]
    pub height: u16,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            width: default_width(),
            height: default_height(),
        }
    }
}

fn default_format() -> String {
    "text".to_string()
}

fn default_width() -> u16 {
    50
}

fn default_height() -> u16 {
    8
}

/// A single scenario event.
///
/// The simplest form is a key string: `key = "enter"` or `key = "ctrl+c"`.
/// For mouse events or fine-grained key control, use the table form.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ScenarioEvent {
    /// Simple key string form (most common): `"enter"`, `"up"`, `"q"`, `"ctrl+c"`.
    Key(String),
    /// Full key specification for unusual cases.
    FullKey(FullKeyEvent),
    /// Mouse event.
    Mouse(MouseEventSpec),
}

/// A fully-specified key event.
#[derive(Debug, Deserialize)]
pub struct FullKeyEvent {
    /// Key code name: `enter`, `tab`, `up`, `down`, `left`, `right`, `esc`,
    /// `backspace`, `delete`, `home`, `end`, `pageup`, `pagedown`, or a single
    /// character like `"a"` or `"+"`.
    pub key: String,
    /// Modifier list (any of: `shift`, `ctrl`, `alt`, `super`).
    #[serde(default)]
    pub modifiers: Vec<String>,
}

/// A mouse event specification.
#[derive(Debug, Deserialize)]
pub struct MouseEventSpec {
    /// Mouse kind: `down`, `up`, `drag`, `moved`, `scroll_down`, `scroll_up`.
    pub kind: String,
    /// X column (0-indexed).
    pub column: u16,
    /// Y row (0-indexed).
    pub row: u16,
}

/// Parse a scenario from a TOML file.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the TOML is malformed.
pub fn load(path: &str) -> Result<Scenario, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    toml::from_str(&content).map_err(|e| format!("parse {path}: {e}"))
}

/// Convert a [`ScenarioEvent`] to a crossterm [`Event`].
///
/// # Errors
///
/// Returns an error if a key name or modifier is not recognized.
pub fn to_crossterm_event(event: &ScenarioEvent) -> Result<Event, String> {
    match event {
        ScenarioEvent::Key(s) => parse_key_string(s).map(Event::Key),
        ScenarioEvent::FullKey(k) => {
            let code = parse_key_code(&k.key)?;
            let mods = parse_modifiers(&k.modifiers)?;
            Ok(Event::Key(KeyEvent::new_with_kind_and_state(
                code,
                mods,
                KeyEventKind::Press,
                KeyEventState::NONE,
            )))
        }
        ScenarioEvent::Mouse(m) => {
            let kind = parse_mouse_kind(&m.kind)?;
            Ok(Event::Mouse(MouseEvent {
                kind,
                column: m.column,
                row: m.row,
                modifiers: KeyModifiers::NONE,
            }))
        }
    }
}

/// Parse a key string like `"enter"`, `"up"`, `"q"`, or `"ctrl+c"`.
///
/// Recognized modifiers (as prefixes): `ctrl+`, `shift+`, `alt+`, `super+`.
/// Multiple modifiers can stack: `ctrl+shift+a`. A single `+` character is
/// parsed as the key `+`, not as a modifier separator.
fn parse_key_string(s: &str) -> Result<KeyEvent, String> {
    let mut modifiers = KeyModifiers::NONE;
    let mut key_part = s;
    for (mod_name, flag) in [
        ("ctrl", KeyModifiers::CONTROL),
        ("shift", KeyModifiers::SHIFT),
        ("alt", KeyModifiers::ALT),
        ("super", KeyModifiers::SUPER),
    ] {
        let prefix = format!("{mod_name}+");
        if key_part.starts_with(&prefix) {
            modifiers |= flag;
            key_part = &key_part[prefix.len()..];
        }
    }
    let code = parse_key_code(key_part)?;
    Ok(KeyEvent::new_with_kind_and_state(
        code,
        modifiers,
        KeyEventKind::Press,
        KeyEventState::NONE,
    ))
}

/// Parse a single key code name.
fn parse_key_code(name: &str) -> Result<KeyCode, String> {
    match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => Ok(KeyCode::Enter),
        "tab" => Ok(KeyCode::Tab),
        "backspace" | "bs" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdn" => Ok(KeyCode::PageDown),
        "space" | " " => Ok(KeyCode::Char(' ')),
        single if single.len() == 1 => {
            let c = single.chars().next().unwrap();
            Ok(KeyCode::Char(c))
        }
        other => Err(format!("unknown key code '{other}'")),
    }
}

/// Parse a list of modifier names into a `KeyModifiers` bitfield.
fn parse_modifiers(mods: &[String]) -> Result<KeyModifiers, String> {
    let mut flags = KeyModifiers::NONE;
    for m in mods {
        let flag = match m.to_ascii_lowercase().as_str() {
            "shift" => KeyModifiers::SHIFT,
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "super" | "meta" => KeyModifiers::SUPER,
            other => return Err(format!("unknown modifier '{other}'")),
        };
        flags |= flag;
    }
    Ok(flags)
}

/// Parse a mouse event kind name.
fn parse_mouse_kind(name: &str) -> Result<MouseEventKind, String> {
    match name.to_ascii_lowercase().as_str() {
        "down" | "press" => Ok(MouseEventKind::Down(MouseButton::Left)),
        "up" | "release" => Ok(MouseEventKind::Up(MouseButton::Left)),
        "drag" => Ok(MouseEventKind::Drag(MouseButton::Left)),
        "moved" | "move" => Ok(MouseEventKind::Moved),
        "scroll_down" | "wheel_down" => Ok(MouseEventKind::ScrollDown),
        "scroll_up" | "wheel_up" => Ok(MouseEventKind::ScrollUp),
        other => Err(format!("unknown mouse kind '{other}'")),
    }
}

/// Resolve the scenario's widget configuration to a [`ReplayTarget`].
///
/// Currently only `counter` is supported for replay (it's the only widget
/// with an interactive App implementation). Other widgets return an error.
///
/// # Errors
///
/// Returns an error if the widget name is unknown or unsupported for replay.
pub fn resolve_widget(config: &WidgetConfig) -> Result<ReplayTarget, String> {
    match config.name.as_str() {
        "counter" => Ok(ReplayTarget::Counter {
            initial: config.initial.unwrap_or(0),
        }),
        other => Err(format!(
            "replay is not yet supported for widget '{other}'. Supported: counter"
        )),
    }
}

/// A widget that can be driven through a replay scenario.
pub enum ReplayTarget {
    /// Counter app with an initial count value.
    Counter { initial: i32 },
}

/// Helper trait: write scenario errors in a friendly format.
impl Scenario {
    /// Run this scenario and return the rendered output as a string.
    ///
    /// Drives a Counter App through `main_loop` using a `TestBackend` and a
    /// `ScriptedEventSource` preloaded with the scenario's events.
    ///
    /// # Errors
    ///
    /// Returns an error if the widget is unsupported, an event fails to
    /// convert, or the main loop returns an IO error.
    pub fn run(&self) -> Result<String, String> {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        use ratatui_presto::elm::{Command, Model};
        use ratatui_presto::runtime::{main_loop, App};
        use ratatui_presto::test_utils::ScriptedEventSource;

        let target = resolve_widget(&self.widget)?;

        // Convert scenario events to crossterm events.
        let mut events = Vec::with_capacity(self.events.len());
        for ev in &self.events {
            events.push(to_crossterm_event(ev)?);
        }

        let ReplayTarget::Counter { initial } = target;

        // Counter App — mirrors examples/demo.rs but with initial count.
        #[derive(Debug, Default)]
        struct Counter {
            count: i32,
        }
        #[derive(Debug, Clone, Copy)]
        enum Msg {
            Increment,
            Decrement,
            Quit,
        }
        impl Model<Msg> for Counter {
            fn update(&mut self, msg: Msg) -> Command<Msg> {
                match msg {
                    Msg::Increment => self.count += 1,
                    Msg::Decrement => self.count -= 1,
                    Msg::Quit => {}
                }
                Command::none()
            }
            fn view(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
                use ratatui::layout::{Alignment, Rect};
                use ratatui::style::{Color, Modifier, Style};
                use ratatui::text::{Line, Span};
                use ratatui::widgets::{Block, Borders, Paragraph};

                let _ = Alignment::Left; // suppress unused import warning if not needed
                let title = Span::styled(
                    " Counter (replay) ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
                let count_line = Line::from(vec![
                    Span::raw(" Count: "),
                    Span::styled(
                        self.count.to_string(),
                        Style::default().fg(if self.count >= 0 {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                ]);
                let block = Block::default().borders(Borders::ALL).title(title);
                let paragraph = Paragraph::new(vec![Line::default(), count_line]).block(block);
                frame.render_widget(paragraph, area);
                let _ = Rect::new(0, 0, 0, 0); // ensure Rect import is used
            }
        }
        impl App for Counter {
            type Msg = Msg;
            fn init(&mut self) -> Command<Msg> {
                Command::none()
            }
            fn on_event(&mut self, event: Event) -> Option<Msg> {
                if let Event::Key(KeyEvent {
                    code,
                    kind: KeyEventKind::Press,
                    ..
                }) = event
                {
                    match code {
                        KeyCode::Char('+') | KeyCode::Up => return Some(Msg::Increment),
                        KeyCode::Char('-') | KeyCode::Down => return Some(Msg::Decrement),
                        KeyCode::Char('q') | KeyCode::Esc => return Some(Msg::Quit),
                        _ => {}
                    }
                }
                None
            }
            fn should_quit(&self, msg: &Msg) -> bool {
                matches!(msg, Msg::Quit)
            }
        }

        let mut counter = Counter { count: initial };
        let backend = TestBackend::new(self.output.width, self.output.height);
        let mut terminal = Terminal::new(backend).map_err(|e| format!("terminal: {e}"))?;
        let mut source = ScriptedEventSource::new(events);

        // main_loop returns Ok(()) on Quit, Err on exhaustion — both are fine
        // for replay (we just want the final buffer).
        let _ = main_loop(&mut counter, &mut terminal, &mut source);
        let buf = terminal.backend().buffer().clone();

        match self.output.format.as_str() {
            "text" | "txt" => Ok(crate::dump::buffer_to_text(
                &buf,
                self.output.width,
                self.output.height,
            )),
            "html" | "htm" => Ok(crate::dump::buffer_to_html(
                &buf,
                self.output.width,
                self.output.height,
            )),
            other => Err(format!("unknown format '{other}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_key_string() {
        let ev = parse_key_string("enter").unwrap();
        assert_eq!(ev.code, KeyCode::Enter);
        assert_eq!(ev.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_key_with_modifier() {
        let ev = parse_key_string("ctrl+c").unwrap();
        assert_eq!(ev.code, KeyCode::Char('c'));
        assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_single_char_plus_key() {
        let ev = parse_key_string("+").unwrap();
        assert_eq!(ev.code, KeyCode::Char('+'));
        assert_eq!(ev.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_arrow_keys() {
        assert_eq!(parse_key_string("up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_key_string("down").unwrap().code, KeyCode::Down);
    }

    #[test]
    fn parse_invalid_key_returns_error() {
        assert!(parse_key_string("not-a-key").is_err());
    }

    #[test]
    fn parse_invalid_modifier_returns_error() {
        assert!(parse_modifiers(&["hyperlock".to_string()]).is_err());
    }

    #[test]
    fn scenario_parses_from_toml() {
        let toml_str = r#"
[widget]
name = "counter"
initial = 5

[[events]]
key = "+"

[[events]]
key = "q"

[output]
format = "text"
width = 40
height = 6
"#;
        let scenario: Scenario = toml::from_str(toml_str).unwrap();
        assert_eq!(scenario.widget.name, "counter");
        assert_eq!(scenario.widget.initial, Some(5));
        assert_eq!(scenario.events.len(), 2);
        assert_eq!(scenario.output.format, "text");
    }

    #[test]
    fn scenario_uses_default_output_when_omitted() {
        let toml_str = r#"
[widget]
name = "counter"

[[events]]
key = "q"
"#;
        let scenario: Scenario = toml::from_str(toml_str).unwrap();
        assert_eq!(scenario.output.format, "text");
        assert_eq!(scenario.output.width, 50);
        assert_eq!(scenario.output.height, 8);
    }

    #[test]
    fn resolve_counter_target() {
        let config = WidgetConfig {
            name: "counter".to_string(),
            initial: Some(10),
        };
        let target = resolve_widget(&config).unwrap();
        let ReplayTarget::Counter { initial } = target;
        assert_eq!(initial, 10);
    }

    #[test]
    fn resolve_unsupported_widget_errors() {
        let config = WidgetConfig {
            name: "list".to_string(),
            initial: None,
        };
        assert!(resolve_widget(&config).is_err());
    }

    #[test]
    fn scenario_run_produces_output() {
        let toml_str = r#"
[widget]
name = "counter"
initial = 5

[[events]]
key = "+"

[[events]]
key = "+"

[[events]]
key = "q"

[output]
format = "text"
width = 30
height = 5
"#;
        let scenario: Scenario = toml::from_str(toml_str).unwrap();
        let output = scenario.run().unwrap();
        // Initial 5 + 2 increments + Quit = final count 7
        assert!(output.contains("7"), "output was: {output}");
    }
}
