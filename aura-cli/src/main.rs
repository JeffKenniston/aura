mod app;

use app::{AppModel, Message};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use miette::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    miette::set_hook(Box::new(|_| {
        Box::new(miette::MietteHandlerOpts::new().build())
    }))?;

    // Setup terminal
    enable_raw_mode().expect("can run in raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .expect("can setup terminal");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("can create terminal");

    let mut app = AppModel::new();

    run_app(&mut terminal, &mut app).await?;

    // Restore terminal
    disable_raw_mode().expect("can disable raw mode");
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).expect("can restore terminal");
    terminal.show_cursor().expect("can show cursor");

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppModel,
) -> Result<()> {
    let mut reader = EventStream::new();
    let mut tick_rate = interval(Duration::from_millis(250));

    // Dummy channels for future phases (WASI background tasks, Zenoh)
    let (mut _tx, mut rx) = mpsc::channel::<()>(100);

    while app.running {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(10),
                        Constraint::Percentage(80),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(f.area());

            let header = Paragraph::new("Aura CLI - Phase 1 & 2")
                .block(Block::default().title("Header").borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let history = Paragraph::new(format!(
                "Chat History: {}\nArtifacts: {}\nCPU: {}%, Mem: {}",
                app.chat_history.len(),
                app.artifact_tracking.len(),
                app.system_metrics.cpu_usage,
                app.system_metrics.memory_usage,
            ))
            .block(Block::default().title("Main Context").borders(Borders::ALL));
            f.render_widget(history, chunks[1]);

            let footer = Paragraph::new("Press 'q' or 'Esc' to quit.")
                .block(Block::default().title("Footer").borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        }).expect("can draw");

        tokio::select! {
            _ = tick_rate.tick() => {
                app.update(Message::Tick);
            }
            Some(Ok(event)) = reader.next() => {
                app.update(Message::TerminalEvent(event));
            }
            Some(_) = rx.recv() => {
                // Background tasks placeholder
            }
        }
    }

    Ok(())
}
