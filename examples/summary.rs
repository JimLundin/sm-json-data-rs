//! Prints an overview of a checkout, plus the most heat-intensive strats.
//!
//! Run with: `cargo run --example summary -- path/to/sm-json-data`

use sm_json_data::{GameData, Numeric, Requirement};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).ok_or("usage: summary <sm-json-data path>")?;
    let data = GameData::load(root)?;

    let strats: usize = data.rooms.iter().map(|room| room.all_strats().count()).sum();
    println!("{} rooms, {strats} strats", data.rooms.len());
    println!("{} techs, {} helpers", data.tech.all_techs().count(), data.helpers.all_helpers().count());
    println!("{} enemies, {} bosses", data.enemies.enemies.len(), data.bosses.enemies.len());

    let mut heat = Vec::new();
    for room in &data.rooms {
        for strat in room.all_strats() {
            let mut frames = 0;
            for requirement in &strat.requires {
                requirement.walk(&mut |r| {
                    if let Requirement::HeatFrames(Numeric::Literal(n)) = r {
                        frames = frames.max(*n);
                    }
                });
            }
            heat.push((frames, &room.name, &strat.name));
        }
    }
    heat.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    println!("\nLargest single heatFrames requirements:");
    for (frames, room, strat) in heat.iter().take(5) {
        println!("  {frames:>5}  {room}: {strat}");
    }
    Ok(())
}
