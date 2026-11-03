//! Prints an overview of a checkout, plus the most heat-intensive strats.
//!
//! Run with: `cargo run --example summary -- path/to/sm-json-data`

use sm_json_data::load_world;
use sm_logic::{NodeKind, Quantity, Requirement};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).ok_or("usage: summary <sm-json-data path>")?;
    let world = load_world(root)?;

    let strats: usize = world.rooms.iter().map(|room| room.all_strats().count()).sum();
    let nodes = || world.rooms.iter().flat_map(|room| &room.nodes);
    let doors = nodes().filter(|node| matches!(node.kind, NodeKind::Door(_))).count();
    let items = nodes().filter(|node| matches!(node.kind, NodeKind::Item { .. })).count();
    println!("{} rooms, {doors} doors, {items} item locations, {strats} strats", world.rooms.len());
    println!("{} techs, {} helpers, {} enemies", world.techs.len(), world.helpers.len(), world.enemies.len());

    let mut heat = Vec::new();
    for room in &world.rooms {
        for strat in room.all_strats() {
            let mut most = 0;
            for requirement in &strat.requires {
                requirement.walk(&mut |r| {
                    if let Requirement::Heat { frames: Quantity::Const(frames), .. } = r {
                        most = most.max(*frames);
                    }
                });
            }
            heat.push((most, &room.name, strat.name.as_deref().unwrap_or("")));
        }
    }
    heat.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    println!("\nLargest single heat requirements:");
    for (frames, room, strat) in heat.iter().take(5) {
        println!("  {frames:>5}  {room}: {strat}");
    }
    Ok(())
}
