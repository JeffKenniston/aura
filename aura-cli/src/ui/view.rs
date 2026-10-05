use ratatui::{
    layout::{Constraint, Layout, Direction},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::tea::model::AppModel;

pub fn view(frame: &mut Frame, model: &AppModel) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(frame.area());

    let header = Paragraph::new("Aura CLI")
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    
    frame.render_widget(header, chunks[0]);

    let main_content = Paragraph::new(format!("Running: {}", model.running))
        .block(Block::default().borders(Borders::ALL));
        
    frame.render_widget(main_content, chunks[1]);
}
