# diskmgr

Fullscreen TUI for **seeing which disk is in which bay**, and **how that disk is
used** (GPT, mounts, ZFS vdev/spare/etc.), on lab storage servers.

The main storage host is FreeBSD. Some ZFS pools still refer to disks as
`/dev/daNN`. Those names are not stable. diskmgr’s job is a **visual chassis
map** so operators can swap disks and add spares without guessing.

## Status

TUI + fixture probes on macOS; live FreeBSD probes (`sesutil`, `gpart`/`geom`,
`zpool status -P`). Spec: [`docs/PRODUCT.md`](docs/PRODUCT.md). Agent notes:
[`AGENTS.md`](AGENTS.md).

## What it will do

On startup, a numbered menu:

1. **Disk shelf map** — pick an enclosure, then an ASCII grid that matches the
   chassis silk-screen (origin, row/column fill, 0- vs 1-based numbering).
   Arrow / Tab / slot number to focus a bay; a side panel shows occupancy,
   locate (blue) / fault (red) LEDs, manufacturer, model, serial, size, WWN,
   GPT labels, mounts, and ZFS pool + raid group + role.
2. **Disk grid** — spreadsheet of the same facts, sortable by column.

Deferred: search, CSV dump, setup wizard, interactive LED toggle, web UI, Linux.

## Configuration

Enclosure geometry cannot be inferred from SES alone (element IDs ≠ the numbers
printed on the chassis). A TOML file describes each shelf: SES address, grid
size, where slot 0/1 sits (`TL`/`BL`/`TR`/`BR`), whether numbers walk by column
or by row, and 0- vs 1-based silk-screen.

Example: [`examples/diskmgr.toml`](examples/diskmgr.toml).

## Stack

Rust, Ratatui, TOML config. FreeBSD probes via `sesutil` (and `diskinfo`, geom,
ZFS; `sas3ircu` only as fallback). Linux later.

## Development

This tree is often edited on macOS. The TUI must run against **fixture**
enclosure data; live `sesutil` is a FreeBSD backend behind the same probe
trait.

```
cargo test
cargo run -- --config examples/diskmgr.toml --fixture examples/sesutil
```

On FreeBSD 13 (as root; rustc 1.85+ / Ratatui needs ~1.88):

```
diskmgr --config /usr/local/etc/diskmgr.toml
```

Omit `--fixture` to run `sesutil --libxo json`, `geom disk list`, `gpart list`
(text — JSON libxo is ignored on 13), and `zpool status -P`. A failed source
becomes a footer warning; the rest still load.

Fullscreen TUI: `1` shelf map, `2` spreadsheet. ESC backs up; ESC on the main
menu quits.
