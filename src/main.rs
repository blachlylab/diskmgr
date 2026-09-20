use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Parser;
use diskmgr::{Config, Face, FixtureProbe, Inventory};

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
    let (config_path, config) =
        Config::find_and_load(args.config.as_deref()).with_context(|| "loading config")?;
    let Some(fixture_dir) = args.fixture else {
        bail!("live FreeBSD probe is not implemented yet; pass --fixture DIR");
    };
    let probe = FixtureProbe::load(&fixture_dir)
        .with_context(|| format!("loading fixture {}", fixture_dir.display()))?;
    let inventory = Inventory::from_fixture(&config, &probe)?;
    print_inventory(&config_path, &fixture_dir, &inventory);
    Ok(())
}

fn print_inventory(config_path: &std::path::Path, fixture: &std::path::Path, inv: &Inventory) {
    println!(
        "diskmgr  {}  config={}  fixture={}",
        inv.host_name.as_deref().unwrap_or("(unnamed host)"),
        config_path.display(),
        fixture.display()
    );
    for enc in &inv.enclosures {
        let face = match enc.config.face {
            Face::Front => "front",
            Face::Back => "back",
            Face::Unknown => "—",
        };
        let ses = enc.ses.as_ref();
        let status = ses
            .map(|s| s.status.join(","))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "—".into());
        let unmatched = if enc.unmatched { "  UNMATCHED" } else { "" };
        println!(
            "\n{}  [{face}]  {}×{}  {}/{} occupied  {}  id={}  status={status}{unmatched}",
            enc.config.name,
            enc.config.ncols,
            enc.config.nrows,
            enc.occupied(),
            enc.slots.len(),
            ses.map(|s| s.enc.as_str()).unwrap_or("—"),
            enc.config.enclosure_id.as_deref().unwrap_or("—"),
        );
        for slot in &enc.slots {
            match &slot.bay {
                None => println!(
                    "  slot {:>2}  (c{} r{})  —",
                    slot.silk, slot.cell.col, slot.cell.row
                ),
                Some(bay) if bay.kernel_disk.is_none() => {
                    let swapped = if bay.swapped { " swapped" } else { "" };
                    println!(
                        "  slot {:>2}  (c{} r{})  empty  {}{swapped}",
                        slot.silk, slot.cell.col, slot.cell.row, bay.status
                    );
                }
                Some(bay) => {
                    let disk = bay.kernel_disk.as_deref().unwrap_or("—");
                    let model = bay.model.as_deref().unwrap_or("—");
                    let serial = bay.serial.as_deref().unwrap_or("—");
                    let size = bay.size_bytes.map(fmt_size).unwrap_or_else(|| "—".into());
                    let mut flags = Vec::new();
                    if bay.swapped {
                        flags.push("swapped");
                    }
                    if bay.locate {
                        flags.push("locate");
                    }
                    if bay.fault {
                        flags.push("fault");
                    }
                    let flags = if flags.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", flags.join(","))
                    };
                    println!(
                        "  slot {:>2}  (c{} r{})  {disk:<6}  {size:>7}  {model}  {serial}{flags}",
                        slot.silk, slot.cell.col, slot.cell.row
                    );
                }
            }
        }
    }
}

fn fmt_size(bytes: u64) -> String {
    const TB: f64 = 1e12;
    const GB: f64 = 1e9;
    if bytes as f64 >= 0.95 * TB {
        format!("{:.1} TB", bytes as f64 / TB)
    } else if bytes as f64 >= 0.95 * GB {
        format!("{:.1} GB", bytes as f64 / GB)
    } else {
        format!("{bytes} B")
    }
}
