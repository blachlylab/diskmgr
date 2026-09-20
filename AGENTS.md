# Guidance for agentic coders

Read this before changing the crate. The product spec is
[`docs/PRODUCT.md`](docs/PRODUCT.md). If this file and the spec disagree, **the
spec wins** unless the user just contradicted it in chat.

## What this repo is

**diskmgr** is a FreeBSD-first Ratatui TUI that draws a **physical disk-shelf
map** (and a spreadsheet of the same data) so operators can swap disks and
manage ZFS without trusting `/dev/daNN`.

The crate is currently a stub. Implement against the spec, not against
`println!("Hello, world!")`.

## Hard rules

1. **`daNN` / `adaNN` are not identity.** Display them as the current kernel
   name, labeled unstable. Join occupancy, GPT, and ZFS on **serial** (then
   WWN). Never recommend configuring ZFS on `da` names.
2. **Do not implement deferred features** (search, dump/CSV, setup wizard, web
   UI, Linux probes, interactive LED toggle) unless the user asks.
3. **Config defines geometry.** Do not assume SuperMicro numbering in code
   except as an example TOML. All of `origin`, `fill`, `ncols`, `nrows`,
   `slot_index_base` are required to draw a map.
4. **Silk-screen slot ≠ SES element id.** Store both. The grid shows silk-screen
   numbers from config; LED/sesutil calls will use SES ids.
5. **Probes are swappable.** UI must run on macOS (this workspace) via
   `FixtureProbe` replaying [`examples/sesutil/`](examples/sesutil/). Parse
   that JSON with the same code as live `sesutil --libxo json`. Do not call
   `sesutil` unless the OS is FreeBSD (or the user asked to implement that
   backend). Parser contract: [`docs/PRODUCT.md`](docs/PRODUCT.md) §9.5.
6. **ESC is a back stack**, not a universal quit. Quit only from the main menu.
7. **No extra product surface:** no daemon, no DB, no second TUI library, no
   web framework.

## Build order

Follow §13 of the spec. First mergeable slices:

1. TOML config + layout math + unit tests (no TUI required for this PR)
2. Ratatui shell + main menu
3. Fixture-backed enclosure picker + ASCII map + status panel
4. Spreadsheet view
5. FreeBSD probes, one source at a time (SES → diskinfo → geom → ZFS)

Keep layout math in a dependency-free module and test every
`origin` × `fill` × `slot_index_base` combination used in the spec’s worked
example.

## UI contract (do not “improve” without asking)

- Fullscreen modes, numbered main menu, immediate number key.
- Shelf map: picker first (arrow keys + Enter), then grid.
- Focus moves with arrows, Tab/Shift-Tab, and slot-number + Enter.
- Status panel updates as focus moves; do not require a second “inspect” key.
- Fault LED = red, locate LED = blue; empty bays still drawn.
- Footer shows keys for the current mode.

## Testing

- Unit test `silk_screen ↔ (row, col)` with no hardware.
- Parse `examples/sesutil/{map,show,status}.json` in unit tests (empty bay,
  shuffled `da` vs slot, swapped bit, `ada` vs `da`, mixed status types).
- LED on/off needs a separate tiny overlay; the lab dump has no locate/fault.
- ZFS/GPT fixtures join on serial from `show.json`, not on `daN`.
- Do not require root or FreeBSD to `cargo test`.

## Style

- Match existing Rust style once it exists; until then, small modules, explicit
  types for Slot/Disk/ZfsUsage, `Result` for probes, no `unwrap` in library
  paths.
- Comments only for non-obvious constraints (e.g. why SES element 0 is skipped).
- Do not add markdown files unless the user asked, except updating this spec if
  behavior is intentionally changed.
