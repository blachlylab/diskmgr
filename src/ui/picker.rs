use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .inventory
        .enclosures
        .iter()
        .map(|enc| {
            let ses = enc.ses.as_ref();
            let unit_owned = enc.config.ses_unit();
            let unit = ses
                .map(|s| s.enc.as_str())
                .or(unit_owned.as_deref())
                .unwrap_or("—");
            let id = enc.config.enclosure_id.as_deref().unwrap_or("—");
            let status = ses
                .map(|s| s.status.join(","))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "—".into());
            let unmatched = if enc.unmatched { "  UNMATCHED" } else { "" };
            let line1 = Line::from(vec![
                Span::styled(
                    enc.config.name.clone(),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  [{}]", enc.config.face.label())),
            ]);
            let line2 = Line::from(Span::styled(
                format!(
                    "  {}×{}  {}/{} occupied  {unit}  {id}  {status}{unmatched}",
                    enc.config.ncols,
                    enc.config.nrows,
                    enc.occupied_count(),
                    enc.slots.len()
                ),
                Style::new().fg(Color::DarkGray),
            ));
            ListItem::new(vec![line1, line2])
        })
        .collect();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.select_idx));
    }
    let list = List::new(items)
        .block(Block::bordered().title(" Enclosures "))
        .highlight_style(
            Style::new()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, area, &mut state);
}
