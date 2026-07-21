//! Integration test exercising the Elm architecture's [`ratatui_bubbles::elm::Model`] trait
//! via a tiny counter example, rendered into a TestBackend buffer.

use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use ratatui_bubbles::elm::{flatten, Command, Model};

/// A minimal [`Model`] that counts increment/decrement messages.
#[derive(Default)]
struct Counter {
    count: i32,
}

/// Messages processed by [`Counter`].
enum CounterMsg {
    Increment,
    Decrement,
}

impl Model<CounterMsg> for Counter {
    fn update(&mut self, msg: CounterMsg) -> Command<CounterMsg> {
        match msg {
            CounterMsg::Increment => self.count += 1,
            CounterMsg::Decrement => self.count -= 1,
        }
        Command::none()
    }

    fn view(&self, frame: &mut Frame<'_>, area: Rect) {
        let text = format!("count = {}", self.count);
        frame.render_widget(Paragraph::new(text), area);
    }
}

/// Render `model` into a fresh [`TestBackend`] buffer of `width` x `height` cells.
fn render_model(model: &Counter, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame<'_>| model.view(frame, Rect::new(0, 0, width, height)))
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// Collect the cell symbols in `buf` at row `y`, columns `0..len`, joined into a String.
fn row_symbols(buf: &Buffer, y: u16, len: u16) -> String {
    (0..len)
        .map(|x| buf.cell((x, y)).map(Cell::symbol).unwrap_or("").to_string())
        .collect()
}

#[test]
fn counter_updates_and_renders() {
    let mut counter = Counter::default();
    counter.update(CounterMsg::Increment);
    counter.update(CounterMsg::Increment);
    counter.update(CounterMsg::Increment);

    let buf = render_model(&counter, 20, 3);
    // "count = 3" is 9 characters at row 0, column 0.
    assert_eq!(row_symbols(&buf, 0, 9), "count = 3");
}

#[test]
fn command_flatten_via_model_update() {
    let mut counter = Counter::default();

    // Each update returns a `Command::None` leaf; collect them straight from the model.
    let cmd1 = counter.update(CounterMsg::Increment);
    let cmd2 = counter.update(CounterMsg::Increment);
    let cmd3 = counter.update(CounterMsg::Decrement);

    // Batch the three leaf commands and flatten end-to-end via `elm::flatten`.
    let batch = Command::batch(vec![cmd1, cmd2, cmd3]);
    let flattened = flatten(batch);

    // Three non-Batch leaves survive flattening unchanged.
    assert_eq!(flattened.len(), 3);
    // And the model state reflects the net effect (+1 +1 -1 = +1).
    assert_eq!(counter.count, 1);
}
