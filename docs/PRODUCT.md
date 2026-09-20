# diskmgr — Product Specification

This is the source of truth for *what* diskmgr is and *how* it should behave.
Implementation guidance for agentic coders lives in [`AGENTS.md`](../AGENTS.md).

Status: **specification**. The crate is a stub (`Hello, world!`). Nothing described
here is implemented yet.

---

## 1. Problem

The lab has run storage servers, disk shelves, and JBODs for 10+ years. Disks have
been replaced and physically shuffled. The main storage server is **FreeBSD**. Some
ZFS pools still refer to disks as `/dev/daNN`.

That addressing is **dangerous and incorrect**: CAM `da` unit numbers are not a
stable identity. They can change across reboots, rescan, or HBA/enclosure changes.
Using them as the mental (or pool) map of “which physical bay is which disk” is how
the wrong drive gets pulled.

Operators need a **visual map of the physical chassis** that answers:

- Which bay is this?
- Is a disk in it?
- What disk is it (serial / WWN / model — not `daNN`)?
- How is it used (GPT, mount, ZFS pool + vdev)?
- Are the locate / fault LEDs on?

The tool exists so a human can swap disks, add spares, and audit layout without
guessing from `da` numbers.

---

## 2. Product

**diskmgr** is a fullscreen terminal application (Rust + Ratatui) that:

1. Loads a TOML configuration describing each enclosure’s *physical* slot grid
   (SES identity, rows, columns, origin, fill order, 0- vs 1-based silk-screen).
2. Probes the live host (FreeBSD first) for occupancy, LEDs, disk identity, and
   usage.
3. Draws an ASCII map of a selected shelf that matches the real chassis numbering.
4. Lets the operator move around the grid and inspect everything known about a
   slot.

A web UI, extra CLI dump modes, LED toggling, and Linux support are **eventual**
goals. They are not in the first deliverable.

Primary platform: **FreeBSD**. Development on macOS/Linux must work against
**fixtures / mock probes** (see §9).

---

## 3. Non-goals (v1)

Do not implement these until the product owner asks:

| Item | Notes |
|------|--------|
| Web interface | Future |
| Linux probes | Future; keep probe traits OS-agnostic |
| Search mode | Menu item 3, deferred |
| Dump / CSV mode | Menu item 4, deferred |
| Setup / wizard mode | Menu item 9, deferred |
| Interactive LED toggle | Eventual; status *display* is in v1 |
| Rewriting ZFS pools off `daNN` | Out of scope; diskmgr *shows* usage, it does not mutate pools |
| Auto-discovering chassis geometry | Config is required; SES slot IDs do not equal silk-screen layout |

---

## 4. Users and environment

- Operator at a keyboard on the storage server (or SSH), often as root.
- Typical hardware: SuperMicro-style chassis and SAS JBODs with SES (`ses(4)`).
- Example geometry (must be configurable, not hardcoded): SuperMicro numbering
  from **bottom-left**, filling **up the column**, then the next column; some
  chassis have **front and back** planes.

Privileged commands (`sesutil`, some `diskinfo` / CAM / ZFS queries) will often
require root. The TUI should degrade: show config-defined empty grids and a clear
error in the status area rather than crashing if a probe fails.

---

## 5. Application shell

Fullscreen Ratatui app. Each **mode** is a different fullscreen view. There is
one active mode at a time.

### 5.1 Startup / main menu

On launch, show a numbered menu:

```
diskmgr
1. Disk shelf map
2. Disk grid
3. Search          (deferred — show as disabled)
4. Dump            (deferred — show as disabled)
9. Setup           (deferred — show as disabled)

Press number to enter. ESC quits.
```

- Immediate number key enters that mode (no Enter required).
- **ESC on the main menu quits the program.**
- Deferred items: visible, visually disabled, key ignored or a one-line
  “not implemented” status. Do not stub fake screens.

### 5.2 Global navigation rules

| Where | ESC |
|-------|-----|
| Main menu | Quit |
| Shelf picker | Main menu |
| Shelf map | Shelf picker |
| Disk grid | Main menu |

Never skip a level. From a shelf map, ESC must not quit the process.

---

## 6. Mode 1 — Disk shelf map

This is the core product.

### 6.1 Enclosure picker

Opens first. List every enclosure from config (not only those SES currently
sees), with enough live/config info to pick the right one:

- Display name
- Face (front / back / n/a)
- SES device (`ses0`) and/or enclosure ID
- Grid size (`ncols × nrows`)
- Occupied / total slots (if probe succeeded)
- Enclosure / SES status if known

Controls:

- **Up / Down** (and **j / k** optional) — move selection. Do **not** use number
  keys as the only selector; there may be more than ten enclosures.
- **Enter** — open that enclosure’s map.
- **ESC** — main menu.

If config has one enclosure, still show the picker (explicit choice, consistent
ESC behavior). A later convenience “auto-open the only enclosure” is optional
and must remain easy to turn off.

### 6.2 Map view

Two-pane fullscreen:

- **Left (or main): ASCII grid** of the chassis, drawn to match physical layout.
- **Right (or bottom on narrow terminals): status panel** for the focused slot.

Recompute layout on terminal resize. If the grid cannot fit, scroll the grid
around the focused cell rather than shrinking cells into unreadability.

### 6.3 Grid drawing rules

Each cell is one physical bay.

Must show:

1. **Silk-screen slot number** as labeled on the chassis (0- or 1-based per
   config — never “SES element index” unless it happens to match).
2. **Occupancy**: distinct glyph for occupied vs empty.
3. **Locate LED**: blue indicator when on.
4. **Fault LED**: red indicator when on.
5. **Focus**: the selected cell is clearly highlighted.

Both LEDs may be on at once; show both (e.g. two colored dots). Empty bays still
show slot number and LED state (locate/fault can apply to empty elements).

Suggested glyphs (implementers may refine, keep them distinguishable on
color and mono terminals):

| State | Example |
|-------|---------|
| Occupied | `██` or `[==]` |
| Empty | `░░` or `[  ]` |
| Locate on | blue `●` |
| Fault on | red `●` |
| Focus | reverse video / bold box |

Include a compact legend on the map.

**Front and back** are separate grids (separate enclosure entries, optionally
grouped — see §8). Do not try to draw a 3D chassis in v1.

### 6.4 Slot focus

The operator moves a focus cell:

| Input | Action |
|-------|--------|
| Arrow keys | Move one bay (skip off-grid; do not wrap unless documented) |
| Tab / Shift-Tab | Next / previous slot in silk-screen order |
| Digits, then Enter | Jump to that silk-screen slot number |
| ESC | Back to enclosure picker |

Arrow movement follows **visual** grid position, not SES element order.

As focus moves, the status panel **updates immediately**. No extra key to
“open details”. A popup/dialog is acceptable later if the panel cannot fit
the fields; the default is a persistent side (or bottom) panel.

### 6.5 Status panel fields

Show everything known. Missing data is `—` (or `unknown`), never a guessed
`da` name.

**Slot**

- Enclosure name, face
- Silk-screen slot number
- Grid coordinate (row, col) as drawn
- SES device + SES element id (for debugging / later LED control)
- Occupied: yes / no
- Locate LED: on / off
- Fault LED: on / off
- SES element status string if available (`OK`, `Not Installed`, `Swapped`, …)

**Disk** (if occupied)

- Current kernel device (`da42`) — labeled **unstable**, never treated as identity
- Manufacturer
- Model
- Serial (primary human identity)
- Size (human, e.g. `16.0 TB`, plus bytes available for the grid view)
- WWN / NAA if known

**How the disk is used** (the point of the tool)

- GPT: whether a GPT is present; list partition GPT labels (`gpt/…`)
- Mounted filesystem: yes/no; mountpoint(s) and type if yes
- ZFS: if a member of any pool:
  - pool name
  - vdev / raid group (`raidz2-1`, `mirror-0`, …)
  - role: data, spare, log, cache, special, dedup, …
  - online/offline/degraded if `zpool status` provides it

If the disk is not in ZFS, say so explicitly. If ZFS tools are missing, say
ZFS data is unavailable — do not imply the disk is unused.

Warn visually when the only known name is `daNN` and no serial/WWN/GPT label
was found.

---

## 7. Mode 2 — Disk grid (spreadsheet)

Fullscreen table: **one row per slot** (empty slots included) or per disk;
default is **one row per configured slot** so empty bays are visible.

Columns (minimum):

- Enclosure
- Face
- Slot (silk-screen)
- Occupied
- Device (`daNN`, marked unstable)
- Serial
- Model
- Size
- WWN
- GPT labels
- Mounted
- ZFS pool
- ZFS vdev / group
- ZFS role
- Fault LED
- Locate LED

Behavior:

- Arrow keys move row (and later column).
- Sort by the current column: keys `s` / `S` or `<` / `>` — pick one and
  document it in the footer. First implementation: **Enter on header or `s`
  cycles sort column; `r` reverses**.
- ESC returns to main menu.

This view is for “find the 16 TB Seagate with serial ZA…” and “which slots
are spare”. The map is for “I am standing in front of the chassis”.

---

## 8. Configuration

Path: `--config <path>`, else `./diskmgr.toml`, else
`$XDG_CONFIG_HOME/diskmgr/config.toml` (default `~/.config/diskmgr/config.toml`).
Fail with a clear message if none found. Do not invent enclosures.

Format: **TOML**.

The essential problem config solves: **SES element order is not the silk-screen
layout.** SuperMicro (and others) may number from bottom-left up columns; SES
`Slot00` may not be the bay labeled `0` or `1` on the chassis. Config maps
SES-addressable elements onto a rectangle the operator can trust.

### 8.1 Schema (v1)

```toml
# diskmgr.toml

[host]
# Optional display name of this server
name = "storage1"

[[enclosure]]
# Operator-facing name
name = "sm846-front"

# Optional grouping for front/back of the same box
chassis = "sm846"
face = "front"          # "front" | "back" | "unknown"

# How to find the SES device. At least one required.
ses_device = "ses0"     # /dev/ses0; also accept "ses0"
enclosure_id = "500304801820593f"  # SES enclosure ID; preferred if present

ncols = 6
nrows = 4

# Where silk-screen slot `slot_index_base` sits
# TL = top-left, TR = top-right, BL = bottom-left, BR = bottom-right
origin = "BL"

# Walk order from origin for successive silk-screen numbers:
#   column = stay in column, step toward the opposite edge, then next column
#   row    = stay in row, step toward the opposite edge, then next row
fill = "column"

# Silk-screen numbers: 0 or 1
slot_index_base = 0

# Optional: first SES array-device element that corresponds to slot_index_base.
# Default: lowest Array Device Slot element that looks like a real bay
# (skip the "ArrayDevicesInSubEnclsr0" header element). Overridable per chassis.
# ses_element_base = 1
```

`origin` + `fill` + `slot_index_base` + `ncols` + `nrows` fully determine the
map from silk-screen number ↔ `(col, row)`.

Worked example: `ncols=6`, `nrows=4`, `origin=BL`, `fill=column`,
`slot_index_base=0` (SuperMicro-style: bottom-left, up, then next column):

```
 col 0  1  2  3  4  5
r3   3  7 11 15 19 23
r2   2  6 10 14 18 22
r1   1  5  9 13 17 21
r0   0  4  8 12 16 20
```

If `slot_index_base = 1`, add 1 to every number (silk-screen 1–24).

Front and back of one chassis: **two `[[enclosure]]` tables**, same `chassis`
value, different `face` and usually different `ses_device`.

### 8.2 Validation

On load, reject:

- `ncols` or `nrows` < 1
- unknown `origin` / `fill` / `face`
- `slot_index_base` other than 0 or 1
- missing both `ses_device` and `enclosure_id`
- duplicate `(chassis, face)` or duplicate `name`

---

## 9. Live data and identity

### 9.1 Identity rule (mandatory)

**Stable identity** of a disk is, in order of preference:

1. Serial number
2. WWN / NAA
3. GPT label (for *usage*, not uniqueness of the platter)
4. Current `da` / `ada` name — **display only**, never a join key if a better
   key exists

Join SES occupancy → `diskinfo` → GPT → ZFS on serial/WWN. If a pool still
uses `/dev/da12`, resolve `da12` → serial at probe time, then attach that
usage to the slot that currently has that serial. The UI may show “ZFS sees
this disk as `da12` (unstable)”.

### 9.2 FreeBSD probe sources

Use existing utilities before writing ioctls. Parse **libxo JSON** where the
tool supports it.

| Source | Provides |
|--------|----------|
| `sesutil map --libxo json` | Enclosures, elements, device names, extra status (locate/fault, swapped) |
| `sesutil show --libxo json` | Per-slot model, ident/serial, size, device |
| `sesutil status` | Enclosure health |
| `diskinfo -v daN` | Serial (`Disk ident`), size, sectorsize |
| `camcontrol inquiry` / identify | Vendor, product, WWN when `diskinfo` lacks it |
| `gpart show -p`, `glabel status` | GPT, geom labels |
| `mount` | Mounted filesystems |
| `zpool status -P` / `zpool status -v` | Pool, vdev, role |
| `sas3ircu DISPLAY` | **Fallback only** when SES is missing fields |

LED *read* path: `sesutil map` extra status (`LED=locate`, `LED=fault`).

LED *write* path (not v1, but design toward it):

```
sesutil locate da15 on
sesutil fault -u /dev/ses2 7 on
```

Note: SES **element id** is not the silk-screen slot. The model must store both.

### 9.3 Probe architecture

Abstract behind a `Probe` trait (name flexible):

- `list_enclosures() -> Vec<SesEnclosure>`
- `slots(enc) -> Vec<SesSlot>`
- `disk_identity(dev) -> DiskIdentity`
- `usage(dev|serial) -> Usage` (GPT, mounts, ZFS)

Implementations:

- `FreebsdProbe` — real commands
- `FixtureProbe` — replay recorded `sesutil` JSON (same parser as live)
- Later: `LinuxProbe`

The TUI must run on a machine with **no SES devices** using fixtures, so the
map and grid can be built without the storage server.

Probe failures are per-source: a dead ZFS probe must not blank SES occupancy.

Refresh: probe at mode entry and on a manual key (`R`). Periodic refresh is
optional; if added, it must not block the UI thread.

### 9.4 Mock / fixture data

The Mac virtual enclosure **is** a directory of captured libxo JSON:

```
examples/sesutil/map.json
examples/sesutil/show.json
examples/sesutil/status.json
```

`--fixture examples/sesutil` feeds those files through the same SES parser
as `sesutil … --libxo json` on FreeBSD. Do not invent a second slot schema.

This capture has occupancy, swapped bits, empty bays, shuffled `da` names,
and **scrambled** serials (same length/charset, unique, not the lab values).
It does **not** have locate/fault LEDs on. Add a small overlay or a second
capture for LED rendering tests; do not fake LEDs by editing the lab dump
in place without labeling it.

ZFS/GPT/mount data is still a separate fixture layer, joined on serial.

### 9.5 Lab SES capture (parser contract)

Recorded 2026-09-20 from the lab storage host. Treat these as parser tests,
not as silk-screen layout (layout still comes from TOML).

**Enclosures**

| `enc` | `name` | `id` | Disk bays | Notes |
|-------|--------|------|-----------|--------|
| ses0 | SMC SC846P 0c1f | `50030480186d133f` | 24 (Slot00–23) | `daN` matches slot index in this dump only |
| ses1 | LSI SAS3x28 0601 | `500304801f48f13f` | 12 | Slot06 empty; Slot04=`da57`; Slot08=`da30` |
| ses2 | SMC SC846-P 100b | `500304802125927f` | 24 | `da` vs slot fully shuffled; status INFO+CRITICAL |
| ses3 | AHCI SGPIO Enclosure 2.00 | `3061686369656d30` | 4 empty | motherboard |
| ses4 | AHCI SGPIO Enclosure 2.00 | `3061686369656d31` | 6 (ada0, ada1) | boot SSDs |

Join config to live/fixture SES by **enclosure `id`**, not `sesN`.

**Map JSON (`sesutil map`)**

- Envelope: `{ "__version": "1", "sesutil": { "enclosures": [ … ] } }`
- Bay elements: `type == "Array Device Slot"` **and** `description` matches
  `SlotNN` or `Slot NN`. Skip header rows (`ArrayDevicesInSubEnclsr0`,
  `Array Devices`, `Drive Slots`) even though they share the same type.
- Skip expanders, SAS connectors, enclosure objects, temp, voltage on the
  disk map.
- `device_names` is a comma-separated string (`da0,pass0` or `pass64,ada0`).
  Prefer `daN` / `adaN`; ignore `passN`.
- `extra_status.swapped` is the **string** `"true"`, not a JSON boolean.
- Locate/fault LEDs were not present in this capture; parser must tolerate
  missing `extra_status`.
- `id` on map elements is the SES element index (slot 0 silk-screen is
  typically element 1).

**Show JSON (`sesutil show`)**

- No element `id`. Join to map on `(enclosure id, normalized description)`.
- Descriptions are padded (`"Slot00  "`, `"Slot 00 "`). Strip whitespace;
  accept both `Slot00` and `Slot 00`.
- Occupied slots: `model`, `serial`, `size` (bytes as JSON number — use
  `u64`; 16 TB is `16000900661248`).
- Empty slots: `status: "Not Installed"` with empty strings for names.
- No separate manufacturer field; vendor is a prefix of `model` (`HGST`,
  `SEAGATE`, `WDC`, `Micron`).
- `show` types are `device_slot`, `temperature_sensor`, `voltage_sensor`
  (underscore), unlike map’s title-case type strings.

**Status JSON (`sesutil status`)**

- `status` is **either** a string (`"OK"`) **or** an array of strings
  (`["INFO"]`, `["INFO","CRITICAL"]`). Serde must accept both.

**Do not trust SES type names for non-disk sensors.** On ses2, fans appear
as `voltage_sensor` named `FAN01` with `voltage: 343.11` (RPM). Disk-map
code should ignore them; if we later show enclosure health, parse by
description as well as type.

**`da` is not location.** Counterexamples in this dump: ses1 Slot04=`da57`,
ses2 Slot00=`da50`, ses2 Slot07=`da28`. ses0 looking “clean” is a trap.

---

## 10. CLI

v1:

```
diskmgr [--config PATH]
diskmgr --fixture DIR           # replay DIR/{map,show,status}.json
```

Deferred:

```
diskmgr dump --csv
```

No daemon. No background service.

---

## 11. Tech stack

| Piece | Choice |
|-------|--------|
| Language | Rust (see `Cargo.toml` edition) |
| TUI | [Ratatui](https://ratatui.rs) + crossterm, fullscreen (`ratatui::run` / `init`) |
| Config | TOML via `serde` + `toml` |
| CLI flags | `clap` |
| JSON (libxo, fixtures) | `serde_json` |
| Host OS | FreeBSD now, Linux later |
| Enclosure tools | `sesutil` first, `sas3ircu` fallback |
| ZFS | Optional integration; app is useful without it |

Do not add a web framework, database, or extra TUI toolkit.

---

## 12. Suggested crate layout

Not mandatory line-for-line, but keep UI, config, layout math, and probes
separable:

```
src/
  main.rs           # clap, terminal setup, run loop
  app.rs            # Mode enum, app state
  event.rs          # key handling per mode
  config.rs         # TOML load + validate
  layout.rs         # silk-screen ↔ (row, col); unit-test this
  model.rs          # Enclosure, Slot, Disk, ZfsUsage
  probe/
    mod.rs
    freebsd.rs
    fixture.rs
  ui/
    menu.rs
    shelf_select.rs
    shelf_map.rs
    grid.rs
    status.rs
examples/
  diskmgr.toml
  sesutil/          # recorded map.json, show.json, status.json
```

`layout.rs` must be unit-tested with no hardware: origin/fill/base permutations.

---

## 13. Implementation order

Build in this order so each step is demoable:

1. Config load + `layout.rs` math + unit tests
2. App shell: main menu, ESC quit, disabled deferred items
3. Fixture probe + enclosure picker
4. ASCII map + arrow/tab focus + status panel (fixture data)
5. Disk grid view (same model, sortable)
6. FreeBSD `sesutil` probe (occupancy, LEDs, `da` names)
7. `diskinfo` / CAM identity (serial, model, size, WWN)
8. GPT + mounts
9. ZFS pool / vdev / role
10. (Later) LED toggle, dump CSV, Linux, web

---

## 14. Eventual goals (do not implement now)

- Toggle locate / fault LEDs from the map (`L` / `F` are reserved)
- Web interface with the same map and grid
- `diskmgr dump --csv` (and friends)
- Linux (`sysfs` / `sg_ses` / `lsblk` / `zpool`)
- Setup mode that helps write the TOML by blinking LEDs one bay at a time

---

## 15. Open questions (defaults if unspecified)

| Question | Default until told otherwise |
|----------|------------------------------|
| Auto-open the only enclosure? | No; always show picker |
| Wrap arrow movement at grid edges? | No |
| Include non-disk SES elements (PSU, fans) on the map? | No |
| Color in pipelines / dumb terminals? | Still run; LEDs fall back to `L`/`F` letters |
| Vim keys? | Optional extra; arrows are required |
| Config live-reload? | No; restart or `R` after file change is fine |
