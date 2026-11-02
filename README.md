# sm-json-data (Rust)

Typed Rust bindings for [sm-json-data](https://github.com/vg-json-data/sm-json-data), built on serde.
They cover rooms, strats, requirements, connections, items, tech, helpers, numerics, enemies, bosses, boss scenarios and weapons.

```rust
use sm_json_data::GameData;

let data = GameData::load("path/to/sm-json-data")?;
let room = data.rooms.iter().find(|room| room.name == "Morph Ball Room").unwrap();
for strat in room.all_strats() {
    println!("{} needs {:?}", strat.name, strat.requires);
}
```

`load_file` parses a single file, for example `load_file::<Room>(path)`.
The crate doesn't bundle the data, so you supply your own checkout.

## Design

- **Strict:** unknown fields and variants are errors, so an upstream change can't silently drop a logic condition. Updating the data may therefore require updating these types.
- **Faithful:** optional fields stay `Option` and schema defaults are documented rather than applied. Serializing reproduces the file's content, except that an explicit `null` is dropped. The schema allows `null` on a few fields, but no current data file uses it.
- **Numbers:** `f64` fields can't preserve the difference between `33` and `33.0`, so a value written as `33` reserializes as `33.0`. All other integers keep their exact form.
- **Closed and open sets:** values the schema fixes are enums. Tech, helper, flag and numeric names change often, so they are string newtypes.
- **Two tech forms:** `"canX"` parses to `Named::Tech`, which includes what the tech inherits. `{"tech": "canX"}` parses to `Requirement::TechOnly`, which doesn't.

## Upstream and Map Rando

The types accept both upstream and Map Rando's fork (`kjbranch/sm-json-data`, branch `map-rando`).
The fork's additions are documented where they appear: `Named::Internal`, `Helper::map_rando_override`, `Enemy::buffed_drops`, and three extra `Item` variants.

`region/*/roomDiagrams.json` isn't typed. It holds diagram layout data, has no schema, and contains no game logic.

## Testing

The unit tests need no data. The corpus tests need a checkout, and they fail rather than skip without one:

```sh
cargo test --lib
SM_JSON_DATA=path/to/sm-json-data cargo test --test corpus
```

The corpus tests check that:

- every file parses strictly and round-trips
- every named requirement in tech, helpers and strats resolves to a definition
- every requirement and door condition in the schema has a matching variant
- every numeric default evaluates

## License

The code is under [0BSD](LICENSE), so you can use it for anything without attribution. This covers only the crate, not the sm-json-data files, which have their own license.
