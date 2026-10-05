pub mod tea {
    pub mod r#loop;
    pub mod message;
    pub mod model;
    pub mod update;
}
pub mod ui {
    pub mod graphics;
    pub mod view;
    pub mod components {
        pub mod artifact_review;
    }
}
pub mod auth {
    pub mod cedar;
}
pub mod headless {
    pub mod pipeline;
}
pub mod ipc {
    pub mod zenoh;
}

use clap::Parser;
use std::io;
use ratatui::{backend::CrosstermBackend, Terminal};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in headless mode
    #[arg(long, default_value_t = false)]
    headless: bool,

    /// Subagent pipeline payload
    #[arg(short, long)]
    p: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.headless {
        if let Some(payload) = args.p {
            if let Err(e) = headless::pipeline::run_headless_pipeline(&payload) {
                eprintln!("Pipeline error: {}", e);
            }
        } else {
            eprintln!("Error: Headless mode requires a payload (-p)");
        }
        return Ok(());
    }

    // Initialize immediate-mode terminal rendering (ratatui + crossterm)
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Initialize state model
    let mut model = tea::model::AppModel { running: true };

    // Run the Asynchronous TEA event loop
    let res = tea::r#loop::run_loop(&mut terminal, &mut model).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}
