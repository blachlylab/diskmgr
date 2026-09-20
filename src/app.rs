use crate::inventory::{Inventory, MappedEnclosure, MappedSlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Menu,
    ShelfSelect,
    ShelfMap,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridColumn {
    Enclosure,
    Face,
    Slot,
    Occupied,
    Device,
    Serial,
    Model,
    Size,
    Wwn,
    Gpt,
    Mounted,
    ZfsPool,
    ZfsVdev,
    ZfsRole,
    Fault,
    Locate,
}

impl GridColumn {
    pub const ALL: &'static [Self] = &[
        Self::Enclosure,
        Self::Face,
        Self::Slot,
        Self::Occupied,
        Self::Device,
        Self::Serial,
        Self::Model,
        Self::Size,
        Self::Wwn,
        Self::Gpt,
        Self::Mounted,
        Self::ZfsPool,
        Self::ZfsVdev,
        Self::ZfsRole,
        Self::Fault,
        Self::Locate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Enclosure => "Enclosure",
            Self::Face => "Face",
            Self::Slot => "Slot",
            Self::Occupied => "Occ",
            Self::Device => "Device",
            Self::Serial => "Serial",
            Self::Model => "Model",
            Self::Size => "Size",
            Self::Wwn => "WWN",
            Self::Gpt => "GPT",
            Self::Mounted => "Mount",
            Self::ZfsPool => "Pool",
            Self::ZfsVdev => "Vdev",
            Self::ZfsRole => "Role",
            Self::Fault => "Fault",
            Self::Locate => "Locate",
        }
    }

    fn next(self) -> Self {
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Esc,
    Enter,
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
    Backspace,
    Char(char),
    Ignored,
}

pub struct App {
    pub inventory: Inventory,
    pub mode: Mode,
    pub should_quit: bool,
    pub notice: Option<String>,
    pub select_idx: usize,
    pub map_enc: usize,
    pub map_silk: u32,
    pub jump: String,
    pub grid_row: usize,
    pub grid_sort: GridColumn,
    pub grid_rev: bool,
}

impl App {
    pub fn new(inventory: Inventory) -> Self {
        Self {
            inventory,
            mode: Mode::Menu,
            should_quit: false,
            notice: None,
            select_idx: 0,
            map_enc: 0,
            map_silk: 0,
            jump: String::new(),
            grid_row: 0,
            grid_sort: GridColumn::Enclosure,
            grid_rev: false,
        }
    }

    pub fn handle_key(&mut self, key: Key) {
        match self.mode {
            Mode::Menu => self.menu_key(key),
            Mode::ShelfSelect => self.picker_key(key),
            Mode::ShelfMap => self.map_key(key),
            Mode::Grid => self.grid_key(key),
        }
    }

    fn menu_key(&mut self, key: Key) {
        match key {
            Key::Esc => self.should_quit = true,
            Key::Char('1') => {
                self.notice = None;
                self.mode = Mode::ShelfSelect;
            }
            Key::Char('2') => {
                self.notice = None;
                self.mode = Mode::Grid;
                self.grid_row = self.grid_row.min(self.grid_rows().len().saturating_sub(1));
            }
            Key::Char('3') => self.notice = Some("Search is not implemented yet".into()),
            Key::Char('4') => self.notice = Some("Dump is not implemented yet".into()),
            Key::Char('9') => self.notice = Some("Setup is not implemented yet".into()),
            _ => {}
        }
    }

    fn picker_key(&mut self, key: Key) {
        let n = self.inventory.enclosures.len();
        if n == 0 {
            if key == Key::Esc {
                self.mode = Mode::Menu;
            }
            return;
        }
        match key {
            Key::Esc => {
                self.notice = None;
                self.mode = Mode::Menu;
            }
            Key::Enter => self.open_map(self.select_idx),
            Key::Up | Key::Char('k') => {
                self.select_idx = self.select_idx.saturating_sub(1);
            }
            Key::Down | Key::Char('j') => {
                if self.select_idx + 1 < n {
                    self.select_idx += 1;
                }
            }
            _ => {}
        }
    }

    fn open_map(&mut self, enc: usize) {
        let Some(e) = self.inventory.enclosures.get(enc) else {
            return;
        };
        self.map_enc = enc;
        self.map_silk = e.config.slot_index_base;
        self.jump.clear();
        self.notice = None;
        self.mode = Mode::ShelfMap;
    }

    fn map_key(&mut self, key: Key) {
        match key {
            Key::Esc => {
                if !self.jump.is_empty() {
                    self.jump.clear();
                    self.notice = None;
                    return;
                }
                self.notice = None;
                self.mode = Mode::ShelfSelect;
            }
            Key::Up | Key::Char('k') => self.move_cell(0, -1),
            Key::Down | Key::Char('j') => self.move_cell(0, 1),
            Key::Left | Key::Char('h') => self.move_cell(-1, 0),
            Key::Right | Key::Char('l') => self.move_cell(1, 0),
            Key::Tab => self.step_silk(1),
            Key::BackTab => self.step_silk(-1),
            Key::Backspace => {
                self.jump.pop();
            }
            Key::Enter => self.commit_jump(),
            Key::Char(c) if c.is_ascii_digit() => {
                if self.jump.len() < 4 {
                    self.jump.push(c);
                }
            }
            _ => {}
        }
    }

    fn move_cell(&mut self, dcol: i32, drow: i32) {
        let Some(enc) = self.current_enclosure() else {
            return;
        };
        let Some(cur) = enc.slot_by_silk(self.map_silk) else {
            return;
        };
        let nc = cur.cell.col as i32 + dcol;
        let nr = cur.cell.row as i32 + drow;
        if nc < 0 || nr < 0 || nc >= enc.config.ncols as i32 || nr >= enc.config.nrows as i32 {
            return;
        }
        if let Some(next) = enc.slot_at_cell(nc as u32, nr as u32) {
            self.map_silk = next.silk;
            self.jump.clear();
            self.notice = None;
        }
    }

    fn step_silk(&mut self, dir: i32) {
        let Some(enc) = self.current_enclosure() else {
            return;
        };
        if enc.slots.is_empty() {
            return;
        }
        let i = enc
            .slots
            .iter()
            .position(|s| s.silk == self.map_silk)
            .unwrap_or(0);
        let n = enc.slots.len() as i32;
        let next = (i as i32 + dir).rem_euclid(n) as usize;
        self.map_silk = enc.slots[next].silk;
        self.jump.clear();
        self.notice = None;
    }

    fn commit_jump(&mut self) {
        if self.jump.is_empty() {
            return;
        }
        let Ok(silk) = self.jump.parse::<u32>() else {
            self.notice = Some(format!("invalid slot {}", self.jump));
            self.jump.clear();
            return;
        };
        let exists = self
            .current_enclosure()
            .is_some_and(|e| e.slot_by_silk(silk).is_some());
        if exists {
            self.map_silk = silk;
            self.notice = None;
        } else {
            self.notice = Some(format!("no slot {silk}"));
        }
        self.jump.clear();
    }

    fn grid_key(&mut self, key: Key) {
        let n = self.grid_rows().len();
        match key {
            Key::Esc => {
                self.notice = None;
                self.mode = Mode::Menu;
            }
            Key::Up | Key::Char('k') => {
                self.grid_row = self.grid_row.saturating_sub(1);
            }
            Key::Down | Key::Char('j') => {
                if n > 0 {
                    self.grid_row = (self.grid_row + 1).min(n - 1);
                }
            }
            Key::Char('s') => {
                let keep = self.grid_rows().get(self.grid_row).copied();
                self.grid_sort = self.grid_sort.next();
                self.reselect_grid(keep);
            }
            Key::Char('r') => {
                let keep = self.grid_rows().get(self.grid_row).copied();
                self.grid_rev = !self.grid_rev;
                self.reselect_grid(keep);
            }
            _ => {}
        }
    }

    fn reselect_grid(&mut self, keep: Option<(usize, usize)>) {
        let rows = self.grid_rows();
        self.grid_row = keep
            .and_then(|id| rows.iter().position(|r| *r == id))
            .unwrap_or(0);
    }

    pub fn current_enclosure(&self) -> Option<&MappedEnclosure> {
        self.inventory.enclosures.get(self.map_enc)
    }

    pub fn focused_slot(&self) -> Option<&MappedSlot> {
        self.current_enclosure()?.slot_by_silk(self.map_silk)
    }

    /// `(enclosure_index, slot_index)` in current sort order.
    pub fn grid_rows(&self) -> Vec<(usize, usize)> {
        let mut rows: Vec<(usize, usize)> = self
            .inventory
            .enclosures
            .iter()
            .enumerate()
            .flat_map(|(ei, enc)| (0..enc.slots.len()).map(move |si| (ei, si)))
            .collect();
        rows.sort_by(|a, b| {
            let cmp = self.cmp_grid(*a, *b);
            if self.grid_rev { cmp.reverse() } else { cmp }
        });
        rows
    }

    fn cmp_grid(&self, a: (usize, usize), b: (usize, usize)) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let slot =
            |p: (usize, usize)| -> &MappedSlot { &self.inventory.enclosures[p.0].slots[p.1] };
        let enc = |p: (usize, usize)| -> &MappedEnclosure { &self.inventory.enclosures[p.0] };
        let sa = slot(a);
        let sb = slot(b);
        let ea = enc(a);
        let eb = enc(b);
        let primary = match self.grid_sort {
            GridColumn::Enclosure => ea.config.name.cmp(&eb.config.name),
            GridColumn::Face => ea.config.face.label().cmp(eb.config.face.label()),
            GridColumn::Slot => sa.silk.cmp(&sb.silk),
            GridColumn::Occupied => sa.occupied().cmp(&sb.occupied()),
            GridColumn::Device => opt_str(sa.bay.as_ref().and_then(|b| b.kernel_disk.as_deref()))
                .cmp(&opt_str(
                    sb.bay.as_ref().and_then(|b| b.kernel_disk.as_deref()),
                )),
            GridColumn::Serial => opt_str(sa.bay.as_ref().and_then(|b| b.serial.as_deref()))
                .cmp(&opt_str(sb.bay.as_ref().and_then(|b| b.serial.as_deref()))),
            GridColumn::Model => opt_str(sa.bay.as_ref().and_then(|b| b.model.as_deref()))
                .cmp(&opt_str(sb.bay.as_ref().and_then(|b| b.model.as_deref()))),
            GridColumn::Size => sa
                .bay
                .as_ref()
                .and_then(|b| b.size_bytes)
                .cmp(&sb.bay.as_ref().and_then(|b| b.size_bytes)),
            GridColumn::Wwn => opt_str(sa.wwn()).cmp(&opt_str(sb.wwn())),
            GridColumn::Gpt => {
                opt_str(sa.gpt_summary().as_deref()).cmp(&opt_str(sb.gpt_summary().as_deref()))
            }
            GridColumn::Fault => sa.fault().cmp(&sb.fault()),
            GridColumn::Locate => sa.locate().cmp(&sb.locate()),
            GridColumn::ZfsPool => opt_str(sa.zfs_pool()).cmp(&opt_str(sb.zfs_pool())),
            GridColumn::ZfsVdev => opt_str(sa.zfs_vdev()).cmp(&opt_str(sb.zfs_vdev())),
            GridColumn::ZfsRole => opt_str(sa.zfs_role()).cmp(&opt_str(sb.zfs_role())),
            GridColumn::Mounted => Ordering::Equal,
        };
        primary
            .then_with(|| ea.config.name.cmp(&eb.config.name))
            .then_with(|| sa.silk.cmp(&sb.silk))
    }
}

fn opt_str(v: Option<&str>) -> &str {
    v.unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::inventory::Inventory;
    use crate::probe::FixtureProbe;
    use std::path::Path;

    fn lab_app() -> App {
        let cfg = Config::load_from_path(Path::new("examples/diskmgr.toml")).unwrap();
        let probe = FixtureProbe::load(Path::new("examples/sesutil")).unwrap();
        App::new(Inventory::from_fixture(&cfg, &probe).unwrap())
    }

    #[test]
    fn esc_quits_only_from_menu() {
        let mut app = lab_app();
        app.handle_key(Key::Char('1'));
        assert_eq!(app.mode, Mode::ShelfSelect);
        app.handle_key(Key::Esc);
        assert_eq!(app.mode, Mode::Menu);
        assert!(!app.should_quit);
        app.handle_key(Key::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn map_esc_stack() {
        let mut app = lab_app();
        app.handle_key(Key::Char('1'));
        app.handle_key(Key::Enter);
        assert_eq!(app.mode, Mode::ShelfMap);
        app.handle_key(Key::Esc);
        assert_eq!(app.mode, Mode::ShelfSelect);
        app.handle_key(Key::Esc);
        assert_eq!(app.mode, Mode::Menu);
    }

    #[test]
    fn deferred_modes_stay_on_menu() {
        let mut app = lab_app();
        app.handle_key(Key::Char('3'));
        assert_eq!(app.mode, Mode::Menu);
        assert!(app.notice.as_deref().unwrap().contains("Search"));
        app.handle_key(Key::Char('4'));
        assert!(app.notice.as_deref().unwrap().contains("Dump"));
    }

    #[test]
    fn map_arrows_follow_visual_grid() {
        let mut app = lab_app();
        app.handle_key(Key::Char('1'));
        app.handle_key(Key::Enter);
        assert_eq!(app.map_silk, 0);
        // slot 0 is bottom-left; up decreases display row -> next silk in the column
        app.handle_key(Key::Up);
        assert_eq!(app.map_silk, 1);
        let right = {
            let enc = app.current_enclosure().unwrap();
            let cur = enc.slot_by_silk(1).unwrap();
            enc.slot_at_cell(cur.cell.col + 1, cur.cell.row)
                .unwrap()
                .silk
        };
        app.handle_key(Key::Right);
        assert_eq!(app.map_silk, right);
        app.handle_key(Key::Down);
        let below = {
            let enc = app.current_enclosure().unwrap();
            let cur = enc.slot_by_silk(right).unwrap();
            enc.slot_at_cell(cur.cell.col, cur.cell.row + 1)
                .unwrap()
                .silk
        };
        assert_eq!(app.map_silk, below);
        // down from the bottom row does not wrap
        app.handle_key(Key::Down);
        assert_eq!(app.map_silk, below);
    }

    #[test]
    fn tab_walks_silk_order() {
        let mut app = lab_app();
        app.handle_key(Key::Char('1'));
        app.handle_key(Key::Enter);
        app.handle_key(Key::Tab);
        assert_eq!(app.map_silk, 1);
        app.handle_key(Key::BackTab);
        assert_eq!(app.map_silk, 0);
    }

    #[test]
    fn jump_to_slot_number() {
        let mut app = lab_app();
        app.handle_key(Key::Char('1'));
        app.handle_key(Key::Down); // rear 12-bay
        app.handle_key(Key::Enter);
        app.handle_key(Key::Char('6'));
        app.handle_key(Key::Enter);
        assert_eq!(app.map_silk, 6);
        assert!(!app.focused_slot().unwrap().occupied());
    }

    #[test]
    fn grid_sort_keeps_row_identity() {
        let mut app = lab_app();
        app.handle_key(Key::Char('2'));
        let before = app.grid_rows()[app.grid_row];
        app.handle_key(Key::Char('s'));
        assert_eq!(app.grid_sort, GridColumn::Face);
        assert_eq!(app.grid_rows()[app.grid_row], before);
    }
}
