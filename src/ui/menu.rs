use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let items = [
        ("1", "Disk shelf map", true),
        ("2", "Disk grid", true),
        ("3", "Search", false),
        ("4", "Dump", false),
        ("9", "Setup", false),
    ];
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Select a mode",
            Style::new().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (num, label, enabled) in items {
        let style = if enabled {
            Style::new().fg(Color::White)
        } else {
            Style::new().fg(Color::DarkGray)
        };
        let suffix = if enabled { "" } else { "  (deferred)" };
        lines.push(Line::from(vec![
            Span::styled(format!("  {num}.  "), Style::new().fg(Color::Cyan)),
            Span::styled(format!("{label}{suffix}"), style),
        ]));
    }
    if let Some(notice) = &app.notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {notice}"),
            Style::new().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press number to enter. ESC quits.",
        Style::new().fg(Color::DarkGray),
    )));

    let block = Block::bordered();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [box_area] = Layout::vertical([Constraint::Length(lines.len() as u16)])
        .flex(Flex::Center)
        .areas(inner);
    frame.render_widget(Paragraph::new(lines), box_area);
}
