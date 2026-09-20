use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use diskmgr::app::Key;
use diskmgr::{App, Config, FixtureProbe, Inventory};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

#[derive(Parser, Debug)]
#[command(name = "diskmgr", about = "Map disk shelves to occupancy and usage")]
struct Args {
    /// Path to diskmgr.toml
    #[arg(long)]
    config: Option<PathBuf>,
    /// Directory of recorded sesutil JSON (map.json, show.json, status.json)
    #[arg(long)]
    fixture: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (_config_path, config) =
        Config::find_and_load(args.config.as_deref()).with_context(|| "loading config")?;
    let probe = if let Some(fixture_dir) = args.fixture {
        FixtureProbe::load(&fixture_dir)
            .with_context(|| format!("loading fixture {}", fixture_dir.display()))?
    } else {
        FixtureProbe::live().context("live FreeBSD probe")?
    };
    let inventory = Inventory::from_fixture(&config, &probe)?;
    let mut app = App::new(inventory);
    if !probe.warnings.is_empty() {
        app.notice = Some(probe.warnings.join(" · "));
    }
    ratatui::run(|terminal| run_app(terminal, &mut app)).context("running TUI")?;
    Ok(())
}

fn run_app(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| draw(frame, app))?;
        if let Some(key) = read_key()? {
            app.handle_key(key);
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    diskmgr::ui::draw(frame, app);
}

fn read_key() -> std::io::Result<Option<Key>> {
    loop {
        match event::read()? {
            Event::Key(ev) if ev.kind == KeyEventKind::Press => {
                return Ok(Some(map_key(ev.code, ev.modifiers)));
            }
            Event::Resize(_, _) => return Ok(None),
            Event::Key(_) => continue,
            _ => continue,
        }
    }
}

fn map_key(code: KeyCode, mods: KeyModifiers) -> Key {
    match code {
        KeyCode::Esc => Key::Esc,
        KeyCode::Enter => Key::Enter,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Tab if mods.contains(KeyModifiers::SHIFT) => Key::BackTab,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Char(c) => Key::Char(c),
        _ => Key::Ignored,
    }
}
