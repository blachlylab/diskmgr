# diskmgr

Fullscreen TUI for **seeing which disk is in which bay**, and **how that disk is
used** (GPT, mounts, ZFS vdev/spare/etc.), on lab storage servers.

The main storage host is FreeBSD. Some ZFS pools still refer to disks as
`/dev/daNN`. Those names are not stable. diskmgr’s job is a **visual chassis
map** so operators can swap disks and add spares without guessing.

## Status

Specification and crate stub. Behavior is defined in
[`docs/PRODUCT.md`](docs/PRODUCT.md). Implementation notes for agents:
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
cargo run -- --config examples/diskmgr.toml --fixture examples/fixture.json
```

(`--fixture` is specified; not implemented yet.)
