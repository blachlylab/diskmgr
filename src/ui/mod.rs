mod grid;
mod map;
mod menu;
mod picker;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Mode};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    frame.render_widget(title_bar(app), chunks[0]);
    match app.mode {
        Mode::Menu => menu::draw(frame, app, chunks[1]),
        Mode::ShelfSelect => picker::draw(frame, app, chunks[1]),
        Mode::ShelfMap => map::draw(frame, app, chunks[1]),
        Mode::Grid => grid::draw(frame, app, chunks[1]),
    }
    frame.render_widget(footer(app), chunks[2]);
}

fn title_bar(app: &App) -> Paragraph<'static> {
    let host = app
        .inventory
        .host_name
        .clone()
        .unwrap_or_else(|| "diskmgr".into());
    let subtitle = match app.mode {
        Mode::Menu => "main menu".into(),
        Mode::ShelfSelect => "disk shelf map — select enclosure".into(),
        Mode::ShelfMap => app
            .current_enclosure()
            .map(|e| format!("{} [{}]", e.config.name, e.config.face.label()))
            .unwrap_or_else(|| "disk shelf map".into()),
        Mode::Grid => "disk grid".into(),
    };
    Paragraph::new(Line::from(vec![
        Span::styled(" diskmgr ", Style::new().fg(Color::Black).bg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(host, Style::new().fg(Color::Cyan)),
        Span::raw("  ·  "),
        Span::raw(subtitle),
    ]))
    .block(Block::bordered())
}

fn footer(app: &App) -> Paragraph<'static> {
    let keys = match app.mode {
        Mode::Menu => "1 map   2 grid   ESC quit",
        Mode::ShelfSelect => "↑↓ select   Enter open   ESC back",
        Mode::ShelfMap => "←↑↓→ move   Tab cycle   0-9+Enter jump   ESC back",
        Mode::Grid => "↑↓ row   s sort column   r reverse   ESC back",
    };
    let mut spans = vec![Span::styled(
        format!(" {keys} "),
        Style::new().add_modifier(Modifier::REVERSED),
    )];
    if let Some(notice) = &app.notice {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(notice.clone(), Style::new().fg(Color::Yellow)));
    } else if app.mode == Mode::ShelfMap && !app.jump.is_empty() {
        spans.push(Span::raw("  jump: "));
        spans.push(Span::styled(
            app.jump.clone(),
            Style::new().fg(Color::Yellow),
        ));
    }
    Paragraph::new(Line::from(spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::inventory::Inventory;
    use crate::probe::FixtureProbe;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::Path;

    fn lab_app() -> App {
        let cfg = Config::load_from_path(Path::new("examples/diskmgr.toml")).unwrap();
        let probe = FixtureProbe::load(Path::new("examples/sesutil")).unwrap();
        App::new(Inventory::from_fixture(&cfg, &probe).unwrap())
    }

    fn view(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        format!("{}", term.backend())
    }

    #[test]
    fn menu_lists_modes() {
        let app = lab_app();
        let text = view(&app, 80, 24);
        assert!(text.contains("Disk shelf map"));
        assert!(text.contains("Disk grid"));
        assert!(text.contains("deferred"));
        assert!(text.contains("ESC quits"));
    }

    #[test]
    fn map_shows_slot_and_status() {
        let mut app = lab_app();
        app.handle_key(crate::app::Key::Char('1'));
        app.handle_key(crate::app::Key::Enter);
        let text = view(&app, 120, 30);
        assert!(text.contains("SMC SC846P"));
        assert!(text.contains("serial"));
        assert!(text.contains("unstable"));
    }
}

pub(crate) fn inner_with_status(area: Rect) -> (Rect, Rect) {
    if area.width >= 88 {
        let cols = Layout::horizontal([Constraint::Min(40), Constraint::Length(38)]).split(area);
        (cols[0], cols[1])
    } else {
        let rows = Layout::vertical([Constraint::Min(8), Constraint::Length(12)]).split(area);
        (rows[0], rows[1])
    }
}
