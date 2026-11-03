# sm-json-data (Rust)

Rust tools for [sm-json-data](https://github.com/vg-json-data/sm-json-data), the community model of Super Metroid's layout and logic. They accept both upstream and Map Rando's fork (`kjbranch/sm-json-data`, branch `map-rando`).

| Crate | Purpose | Dependencies |
|---|---|---|
| `sm-logic` | The domain model, and the pack file format | None (Serde is optional, for `sm-json-data`) |
| `sm-json-data` | Parses the JSON and imports it into `sm-logic` | Serde |

A game only needs `sm-logic`. It loads a pack produced ahead of time:

```rust
use sm_logic::pack::{decode, Docs};
use sm_logic::{NodeKind, Requirement};

let world = decode(&std::fs::read("logic.pack")?, Docs::Skip)?;
let room = &world[world.room_id("Morph Ball Room").unwrap()];

for node in &room.nodes {
    if let NodeKind::Item { item, .. } = &node.kind {
        println!("{} holds {}", node.name, item.as_str());
    }
}
for room_strat in room.strats_from(room.nodes[0].id) {
    for requirement in &room_strat.strat.requires {
        if let Requirement::Tech(tech) = requirement {
            println!("needs {}", world[*tech].name);
        }
    }
}
```

To produce a pack from a checkout of the data:

```sh
cargo run --release --example pack -- path/to/sm-json-data logic.pack
```

Add `--docs` to include the docs section. The crates don't bundle the data, so you supply your own checkout.

## The domain model

Importing checks the whole dataset and reports every problem at once, and decoding runs the same validation, so a `World` is known to be consistent.

- **References are typed IDs.** A `TechId` from a world always resolves in it. Names that don't resolve are import errors, including nodes, obstacles and notables within a room.
- **Invalid combinations can't exist.** `NodeKind::Door` has door fields and `NodeKind::Item` has an item, instead of one struct with every field optional. The same goes for enemy placement and strat failures.
- **Defaults are applied.** `door_shell` is a `DoorShellBypass`, not an `Option`. `Implicit` keeps three states because the docs give an explicit `true` a different meaning from leaving it out.
- **Related concepts share a shape.** The separate heat fields become `Requirement::Heat` with a `HeatMode`. All runway fields become one `Runway`, and every resource check becomes `Requirement::Resource`.
- **Names follow the game's meaning.** `resetRoom` becomes `EnteredFrom`, and `lock` becomes `active_when`.

Requirement order is preserved, so treat lists as ordered: costs and refills can depend on the sequence.

Dense IDs such as `TechId` are assigned at import and aren't stable between datasets, so use `source_id` or names when persisting. Node and notable IDs are upstream's own.

## The pack format

A pack has a logic section and an optional docs section. The layout is described at the top of `sm-logic/src/pack.rs`.

- **Shared tables:** strings, requirements, requirement lists and door conditions are stored once and referenced by index.
- **Compact values:** integers are varints, numbers use the shortest exact form, map tiles take two bits each, and strats start with a bitmask of which fields they have.
- **Optional docs:** notes, strat and failure names, notable names and descriptions, enemy group names, obstacle names and connection labels all go in the docs section. Load it with `Docs::Load`, skip it with `Docs::Skip`, or leave it out with `EncodeOptions { docs: false, .. }`.
- **Safe to load:** a checksum catches damage, and every index is checked, so a bad file gives a `PackError` rather than a panic. The checksum isn't tamper-proof.
- **Versioned:** the format version is checked on load, and there is no compatibility between versions yet. The header also records which source data the pack came from.

Decoding currently expands everything into a `World` of owned trees, which takes far more memory than the pack itself.

## The JSON layer

`sm_json_data::load_world` loads a checkout, and `export_world` writes a `World` back to the JSON's shape. Export puts all connections in one file, leaves out `$schema` and the `next*Id` counters, spells addresses and speeds canonically, and may write defaults differently, but the meaning is unchanged.

`sm_json_data::raw` mirrors the JSON files:

- **Strict:** unknown fields and variants are errors, so an upstream change can't silently drop a logic condition. Updating the data may therefore require updating these types.
- **Faithful:** optional fields stay `Option` and schema defaults are documented rather than applied. Serializing reproduces the file's content, except that an explicit `null` is dropped. The schema allows `null` on a few fields, but no current data file uses it.
- **Numbers:** `f64` fields can't preserve the difference between `33` and `33.0`, so a value written as `33` reserializes as `33.0`. All other integers keep their exact form.

The fork's additions are documented where they appear: `Requirement::Internal`, `Helper::map_rando_override`, `Enemy::buffed_drops`, and three extra `Item` variants.

`region/*/roomDiagrams.json` isn't typed. It holds diagram layout data, has no schema, and contains no game logic.

## Testing

The unit tests need no data. The other tests need a checkout, and they fail rather than skip without one:

```sh
cargo test --workspace --lib
SM_JSON_DATA=path/to/sm-json-data cargo test --tests
```

These check that:

- every file parses strictly and round-trips through `raw`
- every requirement and door condition in the schema has a matching variant
- every reference resolves when importing into the model
- importing and then exporting preserves everything, apart from the normalization above
- each similar-looking requirement maps to its intended meaning
- invalid references and node shapes are rejected
- a pack decodes to the same world with docs, and to the stripped world without them
- truncated or damaged packs give an error, and never a panic

## License

The code is under [0BSD](LICENSE), so you can use it for anything without attribution. This covers only the crates, not the sm-json-data files, which have their own license.
