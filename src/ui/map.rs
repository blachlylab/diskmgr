use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::app::App;
use crate::config::DiskOrient;
use crate::display::{dash, fmt_size};
use crate::inventory::{MappedEnclosure, MappedSlot};
use crate::ui::inner_with_status;

/// Glyph box plus 1-column gap.
fn cell_step(orient: DiskOrient) -> (u16, u16) {
    match orient {
        DiskOrient::Horizontal => (9, 3),
        DiskOrient::Vertical => (6, 5),
    }
}

fn cell_glyph_size(orient: DiskOrient) -> (u16, u16) {
    match orient {
        DiskOrient::Horizontal => (8, 3),
        DiskOrient::Vertical => (5, 5),
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let (map_area, status_area) = inner_with_status(area);
    if let Some(enc) = app.current_enclosure() {
        draw_grid(frame, map_area, enc, app.map_silk);
        draw_status(frame, status_area, enc, app.focused_slot());
    } else {
        frame.render_widget(
            Paragraph::new("no enclosure").block(Block::bordered()),
            map_area,
        );
    }
}

fn draw_grid(frame: &mut Frame, area: Rect, enc: &MappedEnclosure, focus_silk: u32) {
    let title = format!(
        " {}  {}×{} ",
        enc.config.name, enc.config.ncols, enc.config.nrows
    );
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let grid_area = chunks[0];
    let legend_area = chunks[1];

    let (step_w, step_h) = cell_step(enc.config.disk_orient);
    let (glyph_w, glyph_h) = cell_glyph_size(enc.config.disk_orient);
    let vis_cols = (grid_area.width / step_w).max(1);
    let vis_rows = (grid_area.height / step_h).max(1);
    let (origin_c, origin_r) = scroll_origin(enc, focus_silk, vis_cols as u32, vis_rows as u32);

    {
        let buf = frame.buffer_mut();
        for slot in &enc.slots {
            if slot.cell.col < origin_c || slot.cell.row < origin_r {
                continue;
            }
            let dc = slot.cell.col - origin_c;
            let dr = slot.cell.row - origin_r;
            if dc >= vis_cols as u32 || dr >= vis_rows as u32 {
                continue;
            }
            let x = grid_area.x + dc as u16 * step_w;
            let y = grid_area.y + dr as u16 * step_h;
            if x + glyph_w > grid_area.right() || y + glyph_h > grid_area.bottom() {
                continue;
            }
            paint_cell(
                buf,
                x,
                y,
                slot,
                enc.config.disk_orient,
                slot.silk == focus_silk,
            );
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("■", Style::new().fg(Color::Cyan)),
            Span::raw(" occupied  "),
            Span::styled("·", Style::new().fg(Color::DarkGray)),
            Span::raw(" empty  "),
            Span::styled("!", crate::ui::fault_style()),
            Span::raw(" fault  "),
            Span::styled("*", crate::ui::locate_style()),
            Span::raw(" locate"),
        ])),
        legend_area,
    );
}

fn scroll_origin(
    enc: &MappedEnclosure,
    focus_silk: u32,
    vis_cols: u32,
    vis_rows: u32,
) -> (u32, u32) {
    let focus = enc.slot_by_silk(focus_silk);
    let fcol = focus.map(|s| s.cell.col).unwrap_or(0);
    let frow = focus.map(|s| s.cell.row).unwrap_or(0);
    let max_c = enc
        .config
        .ncols
        .saturating_sub(vis_cols.min(enc.config.ncols));
    let max_r = enc
        .config
        .nrows
        .saturating_sub(vis_rows.min(enc.config.nrows));
    let origin_c = fcol.saturating_sub(vis_cols.saturating_sub(1)).min(max_c);
    let origin_r = frow.saturating_sub(vis_rows.saturating_sub(1)).min(max_r);
    (origin_c, origin_r)
}

fn paint_cell(
    buf: &mut ratatui::buffer::Buffer,
    x: u16,
    y: u16,
    slot: &MappedSlot,
    orient: DiskOrient,
    focused: bool,
) {
    let border = if focused {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    let num_style = if focused {
        Style::new()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if slot.occupied() {
        Style::new().fg(Color::White)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    let (occ_ch, occ_style) = if slot.occupied() {
        ('■', Style::new().fg(Color::Cyan))
    } else {
        ('·', Style::new().fg(Color::DarkGray))
    };
    let num = format!("{:02}", slot.silk);

    match orient {
        DiskOrient::Horizontal => {
            // ┌──────┐
            // │01 ■!*│
            // └──────┘
            put_str(buf, x, y, "┌──────┐", border);
            put_ch(buf, x, y + 1, '│', border);
            put_str(buf, x + 1, y + 1, &num, num_style);
            put_ch(buf, x + 3, y + 1, ' ', Style::new());
            put_ch(buf, x + 4, y + 1, occ_ch, occ_style);
            put_led(buf, x + 5, y + 1, slot.fault(), '!');
            put_led(buf, x + 6, y + 1, slot.locate(), '*');
            put_ch(buf, x + 7, y + 1, '│', border);
            put_str(buf, x, y + 2, "└──────┘", border);
        }
        DiskOrient::Vertical => {
            // ┌───┐
            // │01 │
            // │ ■ │
            // │!* │
            // └───┘
            put_str(buf, x, y, "┌───┐", border);
            put_ch(buf, x, y + 1, '│', border);
            put_str(buf, x + 1, y + 1, &num, num_style);
            put_ch(buf, x + 3, y + 1, ' ', Style::new());
            put_ch(buf, x + 4, y + 1, '│', border);
            put_ch(buf, x, y + 2, '│', border);
            put_ch(buf, x + 1, y + 2, ' ', Style::new());
            put_ch(buf, x + 2, y + 2, occ_ch, occ_style);
            put_ch(buf, x + 3, y + 2, ' ', Style::new());
            put_ch(buf, x + 4, y + 2, '│', border);
            put_ch(buf, x, y + 3, '│', border);
            put_led(buf, x + 1, y + 3, slot.fault(), '!');
            put_led(buf, x + 2, y + 3, slot.locate(), '*');
            put_ch(buf, x + 3, y + 3, ' ', Style::new());
            put_ch(buf, x + 4, y + 3, '│', border);
            put_str(buf, x, y + 4, "└───┘", border);
        }
    }
}

fn put_str(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, s: &str, style: Style) {
    buf.set_stringn(x, y, s, s.chars().count(), style);
}

fn put_ch(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, ch: char, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch);
        cell.set_style(style);
    }
}

fn put_led(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, on: bool, ch: char) {
    if on {
        let style = if ch == '!' {
            crate::ui::fault_style()
        } else {
            crate::ui::locate_style()
        };
        put_ch(buf, x, y, ch, style);
    } else {
        put_ch(buf, x, y, ' ', Style::new());
    }
}

fn draw_status(frame: &mut Frame, area: Rect, enc: &MappedEnclosure, slot: Option<&MappedSlot>) {
    let Some(slot) = slot else {
        frame.render_widget(Block::bordered().title(" Slot "), area);
        return;
    };
    let mut lines = vec![
        kv("enclosure", &enc.config.name),
        kv("face", enc.config.face.label()),
        kv("slot", &slot.silk.to_string()),
        kv(
            "cell",
            &format!("col {}  row {}", slot.cell.col, slot.cell.row),
        ),
    ];
    let unit_owned = enc.config.ses_unit();
    let unit = enc
        .ses
        .as_ref()
        .map(|s| s.enc.as_str())
        .or(unit_owned.as_deref())
        .unwrap_or("—");
    match &slot.bay {
        None => {
            lines.push(kv("SES", unit));
            lines.push(kv("occupied", "no"));
            lines.push(kv("locate", "off"));
            lines.push(kv("fault", "off"));
            lines.push(kv("status", "—"));
        }
        Some(bay) => {
            lines.push(kv("SES", &format!("{unit}  element {}", bay.element_id)));
            lines.push(kv("occupied", if slot.occupied() { "yes" } else { "no" }));
            lines.push(kv_led("locate", bay.locate, crate::ui::locate_style()));
            lines.push(kv_led("fault", bay.fault, crate::ui::fault_style()));
            let mut status = bay.status.clone();
            if bay.swapped {
                status.push_str("  swapped");
            }
            lines.push(kv("status", &status));
        }
    }
    lines.push(Line::from(""));
    lines.push(section("Disk"));
    match slot.bay.as_ref().filter(|_| slot.occupied()) {
        None => {
            lines.push(kv("device", "—"));
            lines.push(Line::from(Span::styled(
                "  (empty bay)",
                Style::new().fg(Color::DarkGray),
            )));
        }
        Some(bay) => {
            let dev = dash(bay.kernel_disk.as_deref());
            lines.push(Line::from(vec![
                key_span("device"),
                Span::raw(dev.to_string()),
                Span::styled("  (unstable)", Style::new().fg(Color::Yellow)),
            ]));
            lines.push(kv("mfr", dash(bay.manufacturer())));
            lines.push(kv("model", dash(bay.model.as_deref())));
            lines.push(kv("serial", dash(bay.serial.as_deref())));
            let size = match bay.size_bytes {
                Some(b) => format!("{}  ({b} bytes)", fmt_size(b)),
                None => "—".into(),
            };
            lines.push(kv("size", &size));
            lines.push(kv("WWN", "—"));
            if bay.serial.is_none() {
                lines.push(Line::from(Span::styled(
                    "  no stable identity (serial/WWN missing)",
                    Style::new().fg(Color::Yellow),
                )));
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(section("Usage"));
    lines.push(kv("GPT", "unavailable (not probed)"));
    lines.push(kv("mounted", "unavailable (not probed)"));
    lines.push(kv("ZFS", "unavailable (not probed)"));

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(format!(" Slot {} ", slot.silk)))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    ))
}

fn key_span(key: &str) -> Span<'static> {
    Span::styled(format!("  {key:<10} "), Style::new().fg(Color::DarkGray))
}

fn kv(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![key_span(key), Span::raw(value.to_string())])
}

fn kv_led(key: &str, on: bool, on_style: Style) -> Line<'static> {
    if on {
        Line::from(vec![key_span(key), Span::styled("on", on_style)])
    } else {
        kv(key, "off")
    }
}
