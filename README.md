# sm-json-data (Rust)

Rust bindings for [sm-json-data](https://github.com/vg-json-data/sm-json-data), the community model of Super Metroid's layout and logic.

- **`model`** is the domain model, and what most code should use.
- **`raw`** mirrors the JSON files, for parsing and writing them.

```rust
use sm_json_data::model::{NodeKind, Requirement, World};

let world = World::load("path/to/sm-json-data")?;
let room = &world[world.room_id("Morph Ball Room").unwrap()];

for node in &room.nodes {
    if let NodeKind::Item { item, .. } = &node.kind {
        println!("{} holds {}", node.name, item.as_str());
    }
}
for room_strat in room.strats_from(room.nodes[0].id) {
    for requirement in &room_strat.strat.requires {
        if let Requirement::Tech(tech) = requirement {
            println!("{} needs {}", room_strat.strat.name, world[*tech].name);
        }
    }
}
```

The crate doesn't bundle the data, so you supply your own checkout.

## The domain model

`World::import` checks the whole dataset and reports every problem at once, so a `World` is known to be consistent.

- **References are typed IDs.** A `TechId` always resolves, and `world[id]` can't fail. Names that don't resolve are import errors, including nodes, obstacles and notables within a room.
- **Invalid combinations can't exist.** `NodeKind::Door` has door fields and `NodeKind::Item` has an item, instead of one struct with every field optional. The same goes for enemy placement and strat failures.
- **Defaults are applied.** `door_shell` is a `DoorShellBypass`, not an `Option`. `Implicit` keeps three states because the docs give an explicit `true` a different meaning from leaving it out.
- **Related concepts share a shape.** The separate heat fields become `Requirement::Heat` with a `HeatMode`. All runway fields become one `Runway`, and every resource check becomes `Requirement::Resource`.
- **Names follow the game's meaning.** `resetRoom` becomes `EnteredFrom`, and `lock` becomes `active_when`.

Requirement order is preserved, so treat lists as ordered: costs and refills can depend on the sequence.

`World::to_raw` writes the data back to the JSON's shape. It writes all connections to one file and leaves out `$schema` and the `next*Id` counters, which only serve authoring tools. Defaults may be written out or left out, but the meaning is unchanged.

Dense IDs such as `TechId` are assigned at import and aren't stable between datasets, so use `source_id` or names when persisting. Node and notable IDs are upstream's own.

## The JSON mirror

- **Strict:** unknown fields and variants are errors, so an upstream change can't silently drop a logic condition. Updating the data may therefore require updating these types.
- **Faithful:** optional fields stay `Option` and schema defaults are documented rather than applied. Serializing reproduces the file's content, except that an explicit `null` is dropped. The schema allows `null` on a few fields, but no current data file uses it.
- **Numbers:** `f64` fields can't preserve the difference between `33` and `33.0`, so a value written as `33` reserializes as `33.0`. All other integers keep their exact form.

## Upstream and Map Rando

Both layers accept upstream and Map Rando's fork (`kjbranch/sm-json-data`, branch `map-rando`).
The fork's additions are documented where they appear: `Requirement::Internal`, `Helper::map_rando_override`, `Enemy::buffed_drops`, and three extra `Item` variants.

`region/*/roomDiagrams.json` isn't typed. It holds diagram layout data, has no schema, and contains no game logic.

## Testing

The unit tests need no data. The other tests need a checkout, and they fail rather than skip without one:

```sh
cargo test --lib
SM_JSON_DATA=path/to/sm-json-data cargo test --tests
```

These check that:

- every file parses strictly and round-trips through `raw`
- every requirement and door condition in the schema has a matching variant
- every reference resolves when importing into `model`
- importing and then exporting preserves everything, apart from the normalization above
- each similar-looking requirement maps to its intended meaning
- invalid references and node shapes are rejected

## License

The code is under [0BSD](LICENSE), so you can use it for anything without attribution. This covers only the crate, not the sm-json-data files, which have their own license.
