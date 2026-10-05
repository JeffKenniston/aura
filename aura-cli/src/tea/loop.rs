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
use crate::ipc::zenoh::ZenohIpcClient;

pub async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    model: &mut AppModel,
) -> std::io::Result<()> {
    let mut reader = EventStream::new();
    let mut tick_interval = time::interval(Duration::from_millis(16)); // ~60fps

    // Dummy WASI channel
    let (wasi_tx, mut wasi_rx) = tokio::sync::mpsc::channel::<()>(32);

    // Dummy Zenoh SHM IPC channel (Normally we'd use ZenohIpcClient's subscriber)
    let mut ipc_rx = Box::pin(tokio_stream::iter(std::iter::empty::<()>()));

    while model.running {
        terminal.draw(|f| {
            view(f, model);
        })?;

        // Multiplex three distinct streams using work-stealing tokio scheduler to prevent starvation
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
            Some(_) = ipc_rx.next() => {
                // Process Zero-Copy Zenoh IPC event
            }
            Some(_) = wasi_rx.recv() => {
                // Process WASI 0.3 asynchronous channel event
            }
        }
    }

    Ok(())
}
