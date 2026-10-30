mod app;
mod edit;
mod time;
mod ui;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use app::{App, Msg};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging to stderr would corrupt the screen.
    rolx_core::logging::init_file(0, "tui.log")?;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal).await;
    ratatui::restore();
    result
}

async fn run(terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();

    let key_tx = tx.clone();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            let msg = match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => Msg::Key(key),
                Event::Resize(..) => Msg::Redraw,
                _ => continue,
            };
            if key_tx.send(msg).is_err() {
                break;
            }
        }
    });

    let mut app = App::new(tx);
    app.connect();

    while !app.quit {
        app.viewport_height = terminal.size()?.height;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        match rx.recv().await {
            Some(msg) => app.update(msg),
            None => break,
        }
    }
    Ok(())
}
