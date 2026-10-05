use std::time::Duration;
use tokio::time;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use tokio_stream::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;

use super::model::AppModel;
use super::message::Message;
use super::update::update;
use crate::ui::view::view;

pub async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    model: &mut AppModel,
) -> std::io::Result<()> {
    let mut reader = EventStream::new();
    let mut tick_interval = time::interval(Duration::from_millis(16)); // ~60fps

    while model.running {
        terminal.draw(|f| {
            view(f, model);
        })?;

        tokio::select! {
            _ = tick_interval.tick() => {
                update(model, Message::Tick);
            }
            maybe_event = reader.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(KeyEvent { code: KeyCode::Char('q'), .. }))) |
                    Some(Ok(Event::Key(KeyEvent { code: KeyCode::Esc, .. }))) => {
                        update(model, Message::Quit);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
