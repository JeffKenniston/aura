use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct ArtifactReviewPane {
    pub diff_content: String,
    pub is_active: bool,
}

impl ArtifactReviewPane {
    pub fn new() -> Self {
        Self {
            diff_content: String::new(),
            is_active: false,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.is_active {
            return;
        }

        let lines: Vec<Line> = self.diff_content
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

        let paragraph = Paragraph::new(lines)
            .block(Block::default().title(" Artifact Review Pane (Cedar: Forbid) ").borders(Borders::ALL));

        f.render_widget(paragraph, area);
    }
}
