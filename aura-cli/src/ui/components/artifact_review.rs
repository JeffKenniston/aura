use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct ArtifactReviewPane {
    pub diff_text: String,
}

impl ArtifactReviewPane {
    pub fn new(diff_text: String) -> Self {
        Self { diff_text }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Artifact Review - Zero-Copy Diff")
            .borders(Borders::ALL);

        let lines: Vec<Line> = self.diff_text
            .lines()
            .map(|line| {
                if line.starts_with('+') {
                    Line::from(Span::styled(line, Style::default().fg(Color::Green)))
                } else if line.starts_with('-') {
                    Line::from(Span::styled(line, Style::default().fg(Color::Red)))
                } else {
                    Line::from(Span::raw(line))
                }
            })
            .collect();

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
