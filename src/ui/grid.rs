use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Cell, HighlightSpacing, Row, Table, TableState};

use crate::app::{App, GridColumn};
use crate::display::{dash, fmt_size};
use crate::inventory::MappedSlot;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let rows = app.grid_rows();
    let header = GridColumn::ALL.iter().map(|c| {
        let mut label = c.label().to_string();
        if *c == app.grid_sort {
            label.push(if app.grid_rev { '↓' } else { '↑' });
        }
        Cell::from(label)
    });
    let table_rows = rows.iter().map(|&(ei, si)| {
        let enc = &app.inventory.enclosures[ei];
        let slot = &enc.slots[si];
        Row::new(vec![
            cell(&enc.config.name),
            cell(enc.config.face.label()),
            cell(&slot.silk.to_string()),
            cell(if slot.occupied() { "yes" } else { "no" }),
            cell(disk_name(slot)),
            cell(dash(slot.bay.as_ref().and_then(|b| b.serial.as_deref()))),
            cell(dash(slot.bay.as_ref().and_then(|b| b.model.as_deref()))),
            cell(
                &slot
                    .bay
                    .as_ref()
                    .and_then(|b| b.size_bytes)
                    .map(fmt_size)
                    .unwrap_or_else(|| "—".into()),
            ),
            cell("—"),
            cell("—"),
            cell("—"),
            cell("—"),
            cell("—"),
            cell("—"),
            cell(if slot.fault() { "on" } else { "off" }),
            cell(if slot.locate() { "on" } else { "off" }),
        ])
    });

    let mut state = TableState::default();
    if !rows.is_empty() {
        state.select(Some(app.grid_row));
    }
    let widths = [
        Constraint::Min(16),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(4),
        Constraint::Length(7),
        Constraint::Min(10),
        Constraint::Min(14),
        Constraint::Length(8),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(7),
    ];
    let table = Table::new(table_rows, widths)
        .header(Row::new(header).style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .block(Block::bordered().title(" Disks (one row per slot) "))
        .row_highlight_style(
            Style::new()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ")
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(table, area, &mut state);
}

fn cell(text: &str) -> Cell<'static> {
    Cell::from(text.to_string())
}

fn disk_name(slot: &MappedSlot) -> &str {
    dash(slot.bay.as_ref().and_then(|b| b.kernel_disk.as_deref()))
}
