//! Silk-screen slot number ↔ grid cell.
//!
//! `Cell::row` 0 is the **top** of the rectangle (terminal drawing order).
//! `Cell::col` 0 is the left edge.

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Origin {
    Tl,
    Tr,
    Bl,
    Br,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    Column,
    Row,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub ncols: u32,
    pub nrows: u32,
    pub origin: Origin,
    pub fill: Fill,
    pub slot_index_base: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub col: u32,
    pub row: u32,
}

impl Geometry {
    pub fn slot_count(&self) -> u32 {
        self.ncols * self.nrows
    }

    pub fn first_slot(&self) -> u32 {
        self.slot_index_base
    }

    pub fn last_slot(&self) -> u32 {
        self.slot_index_base + self.slot_count() - 1
    }

    pub fn slots(&self) -> impl Iterator<Item = u32> {
        self.first_slot()..=self.last_slot()
    }

    /// Map a silk-screen slot number onto a cell in display coordinates.
    pub fn slot_to_cell(&self, slot: u32) -> Result<Cell> {
        self.check()?;
        if slot < self.first_slot() || slot > self.last_slot() {
            return Err(Error::Layout(format!(
                "slot {slot} out of range {}–{}",
                self.first_slot(),
                self.last_slot()
            )));
        }
        let i = slot - self.first_slot();
        let (x0, y0, dx, dy) = origin_walk(self.origin, self.ncols, self.nrows);
        let (col, row) = match self.fill {
            Fill::Column => {
                let major = i / self.nrows;
                let minor = i % self.nrows;
                (add_delta(x0, dx, major), add_delta(y0, dy, minor))
            }
            Fill::Row => {
                let major = i / self.ncols;
                let minor = i % self.ncols;
                (add_delta(x0, dx, minor), add_delta(y0, dy, major))
            }
        };
        Ok(Cell { col, row })
    }

    pub fn cell_to_slot(&self, cell: Cell) -> Result<u32> {
        self.check()?;
        if cell.col >= self.ncols || cell.row >= self.nrows {
            return Err(Error::Layout(format!(
                "cell ({}, {}) outside {}×{} grid",
                cell.col, cell.row, self.ncols, self.nrows
            )));
        }
        for slot in self.slots() {
            if self.slot_to_cell(slot)? == cell {
                return Ok(slot);
            }
        }
        Err(Error::Layout(format!(
            "no slot for cell ({}, {})",
            cell.col, cell.row
        )))
    }

    fn check(&self) -> Result<()> {
        if self.ncols < 1 || self.nrows < 1 {
            return Err(Error::Layout("ncols and nrows must be >= 1".into()));
        }
        if self.slot_index_base > 1 {
            return Err(Error::Layout("slot_index_base must be 0 or 1".into()));
        }
        Ok(())
    }
}

fn origin_walk(origin: Origin, ncols: u32, nrows: u32) -> (u32, u32, i32, i32) {
    match origin {
        Origin::Tl => (0, 0, 1, 1),
        Origin::Tr => (ncols - 1, 0, -1, 1),
        Origin::Bl => (0, nrows - 1, 1, -1),
        Origin::Br => (ncols - 1, nrows - 1, -1, -1),
    }
}

fn add_delta(start: u32, dir: i32, steps: u32) -> u32 {
    (start as i32 + dir * steps as i32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(ncols: u32, nrows: u32, origin: Origin, fill: Fill, base: u32) -> Geometry {
        Geometry {
            ncols,
            nrows,
            origin,
            fill,
            slot_index_base: base,
        }
    }

    fn grid(geom: Geometry) -> Vec<Vec<u32>> {
        let mut rows = vec![vec![u32::MAX; geom.ncols as usize]; geom.nrows as usize];
        for slot in geom.slots() {
            let cell = geom.slot_to_cell(slot).unwrap();
            rows[cell.row as usize][cell.col as usize] = slot;
        }
        rows
    }

    #[test]
    fn supermicro_24bay_bl_column_0based() {
        let geom = g(6, 4, Origin::Bl, Fill::Column, 0);
        assert_eq!(
            grid(geom),
            vec![
                vec![3, 7, 11, 15, 19, 23],
                vec![2, 6, 10, 14, 18, 22],
                vec![1, 5, 9, 13, 17, 21],
                vec![0, 4, 8, 12, 16, 20],
            ]
        );
    }

    #[test]
    fn one_based_adds_one() {
        let geom = g(6, 4, Origin::Bl, Fill::Column, 1);
        assert_eq!(geom.slot_to_cell(1).unwrap(), Cell { col: 0, row: 3 });
        assert_eq!(geom.slot_to_cell(24).unwrap(), Cell { col: 5, row: 0 });
        assert!(geom.slot_to_cell(0).is_err());
    }

    #[test]
    fn lsi_12bay_4x3_bl_column() {
        let geom = g(4, 3, Origin::Bl, Fill::Column, 0);
        assert_eq!(
            grid(geom),
            vec![vec![2, 5, 8, 11], vec![1, 4, 7, 10], vec![0, 3, 6, 9],]
        );
    }

    #[test]
    fn tl_row_is_english_reading_order() {
        let geom = g(3, 2, Origin::Tl, Fill::Row, 0);
        assert_eq!(grid(geom), vec![vec![0, 1, 2], vec![3, 4, 5]]);
    }

    #[test]
    fn cell_roundtrip_all_origins_and_fills() {
        for origin in [Origin::Tl, Origin::Tr, Origin::Bl, Origin::Br] {
            for fill in [Fill::Column, Fill::Row] {
                for base in [0, 1] {
                    let geom = g(5, 3, origin, fill, base);
                    for slot in geom.slots() {
                        let cell = geom.slot_to_cell(slot).unwrap();
                        assert_eq!(geom.cell_to_slot(cell).unwrap(), slot);
                    }
                }
            }
        }
    }

    #[test]
    fn out_of_range_slot() {
        let geom = g(2, 2, Origin::Tl, Fill::Row, 0);
        assert!(geom.slot_to_cell(4).is_err());
        assert!(geom.cell_to_slot(Cell { col: 2, row: 0 }).is_err());
    }
}
