// aura-cli/src/app.rs

#[derive(Debug, Clone)]
pub enum Message {
    TerminalEvent(crossterm::event::Event),
    Tick,
    Quit,
}

pub struct AppModel {
    pub running: bool,
    pub chat_history: Vec<String>,
    pub artifact_tracking: Vec<String>,
    pub system_metrics: SystemMetrics,
}

#[derive(Default)]
pub struct SystemMetrics {
    pub memory_usage: u64,
    pub cpu_usage: f32,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            running: true,
            chat_history: Vec::new(),
            artifact_tracking: Vec::new(),
            system_metrics: SystemMetrics::default(),
        }
    }
}

impl AppModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::Quit => {
                self.running = false;
            }
            Message::TerminalEvent(event) => {
                use crossterm::event::{Event, KeyCode, KeyEventKind};
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.running = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Message::Tick => {
                // Update system metrics or background tasks
            }
        }
    }
}
