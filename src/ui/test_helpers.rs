use ratatui::{backend::TestBackend, Frame, Terminal};

/// Render a single frame to a flat string for snapshot-style assertions.
///
/// Creates a [`TestBackend`] of the given dimensions, draws one frame using
/// `render_fn`, then concatenates every cell symbol row-by-row with a newline
/// after each row.
pub fn render_to_string<F>(width: u16, height: u16, render_fn: F) -> String
where
    F: FnOnce(&mut Frame),
{
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(render_fn).unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for row in 0..buf.area.height {
        for col in 0..buf.area.width {
            out.push_str(buf[(col, row)].symbol());
        }
        out.push('\n');
    }
    out
}
