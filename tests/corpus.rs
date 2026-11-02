//! Checks the types against a real checkout of the data.
//!
//! Set `SM_JSON_DATA` to the checkout's path. These tests fail rather than skip
//! when it is missing, so a green run always means the data was actually checked.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sm_json_data::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(std::env::var("SM_JSON_DATA").expect("set SM_JSON_DATA to an sm-json-data checkout"))
}

/// Compares JSON values, treating `33` and `33.0` as equal because `f64` fields
/// cannot preserve that distinction. Records the path of each difference.
fn diff(path: &str, original: &Value, reserialized: &Value, out: &mut Vec<String>) {
    match (original, reserialized) {
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                let child = format!("{path}.{key}");
                match b.get(key) {
                    Some(other) => diff(&child, value, other, out),
                    None => out.push(format!("{child}: dropped {value}")),
                }
            }
            for key in b.keys().filter(|key| !a.contains_key(*key)) {
                out.push(format!("{path}.{key}: added"));
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{index}]"), x, y, out);
            }
        }
        (a, b) if a == b => {}
        (a, b) => out.push(format!("{path}: {a} became {b}")),
    }
}

fn check<T: DeserializeOwned + Serialize>(path: &Path, failures: &mut Vec<String>) {
    let parsed: T = match load_file(path) {
        Ok(parsed) => parsed,
        Err(error) => {
            failures.push(error.to_string());
            return;
        }
    };
    let text = std::fs::read_to_string(path).unwrap();
    let original: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
    let reserialized = serde_json::to_value(&parsed).unwrap();
    let mut differences = Vec::new();
    diff("$", &original, &reserialized, &mut differences);
    failures.extend(differences.into_iter().map(|d| format!("{}: {d}", path.display())));
}

#[test]
fn every_file_parses_strictly_and_round_trips() {
    let root = root();
    let mut failures = Vec::new();
    check::<ItemsFile>(&root.join("items.json"), &mut failures);
    check::<TechFile>(&root.join("tech.json"), &mut failures);
    check::<HelpersFile>(&root.join("helpers.json"), &mut failures);
    check::<NumericsFile>(&root.join("numerics.json"), &mut failures);
    check::<WeaponsFile>(&root.join("weapons/main.json"), &mut failures);
    check::<EnemiesFile>(&root.join("enemies/main.json"), &mut failures);
    check::<EnemiesFile>(&root.join("enemies/bosses/main.json"), &mut failures);
    check::<BossScenariosFile>(&root.join("enemies/bosses/scenarios.json"), &mut failures);

    let connections = connection_paths(&root).unwrap();
    let rooms = room_paths(&root).unwrap();
    assert!(!connections.is_empty() && !rooms.is_empty(), "no data found under {}", root.display());
    for path in &connections {
        check::<ConnectionFile>(path, &mut failures);
    }
    for path in &rooms {
        check::<Room>(path, &mut failures);
    }
    println!("checked {} rooms and {} connection files", rooms.len(), connections.len());

    let shown: Vec<&str> = failures.iter().take(60).map(String::as_str).collect();
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), shown.join("\n"));
}

#[test]
fn every_named_requirement_resolves() {
    let data = GameData::load(root()).unwrap();
    let techs: HashSet<&str> = data.tech.all_techs().map(|t| t.name.as_str()).collect();
    let helpers: HashSet<&str> = data.helpers.all_helpers().map(|h| h.name.as_str()).collect();
    let flags: HashSet<&str> = data.items.game_flags.iter().map(FlagName::as_str).collect();
    let items: HashSet<Item> = data
        .items
        .upgrade_items
        .iter()
        .map(|i| i.name)
        .chain(data.items.expansion_items.iter().map(|i| i.name))
        .chain(data.items.implicit_items.iter().copied())
        .collect();

    let mut unresolved = HashSet::new();
    let mut checked = 0usize;
    let mut check = |requirements: &[Requirement]| {
        for requirement in requirements {
            requirement.walk(&mut |r| {
                let named = match r {
                    Requirement::Named(named) | Requirement::Not(named) => named,
                    _ => return,
                };
                checked += 1;
                let ok = match named {
                    Named::Free | Named::Never => true,
                    Named::Item(item) => items.contains(item),
                    Named::Flag(flag) => flags.contains(flag.as_str()),
                    Named::Helper(helper) => helpers.contains(helper.as_str()),
                    Named::Tech(tech) => techs.contains(tech.as_str()),
                    // Defined by the randomizer's code rather than by any data file.
                    Named::Internal(_) => true,
                };
                if !ok {
                    unresolved.insert(String::from(named.clone()));
                }
            });
        }
    };

    for tech in data.tech.all_techs() {
        check(&tech.tech_requires);
        check(&tech.other_requires);
    }
    for helper in data.helpers.all_helpers() {
        check(&helper.requires);
    }
    for room in &data.rooms {
        for strat in room.all_strats() {
            check(&strat.requires);
        }
    }
    println!("resolved {checked} named requirements");
    assert!(checked > 10_000, "only checked {checked} names");
    assert!(unresolved.is_empty(), "unresolved names: {unresolved:?}");
}

/// Fails if the schema defines a requirement or door condition these types lack,
/// including ones no data file uses yet.
#[test]
fn every_schema_variant_is_known() {
    let root = root();
    let read = |name: &str| -> Value {
        let text = std::fs::read_to_string(root.join("schema").join(name)).unwrap();
        serde_json::from_str(&text).unwrap()
    };
    let requirements = read("m3-requirements.schema.json");
    let rooms = read("m3-room.schema.json");
    let keys = |schema: &Value, pointer: &str| -> Vec<String> {
        let properties = schema.pointer(pointer).unwrap().as_object().unwrap();
        assert!(!properties.is_empty(), "{pointer} is empty");
        properties.keys().cloned().collect()
    };

    let mut unknown = Vec::new();
    let mut probe = |key: &str, error: Option<serde_json::Error>| {
        if error.is_some_and(|e| e.to_string().contains("unknown variant")) {
            unknown.push(key.to_owned());
        }
    };
    let strat = "/definitions/strat/properties";
    for key in keys(&requirements, "/definitions/logicalRequirements/items/properties") {
        let value = serde_json::json!({ &key: null });
        probe(&key, serde_json::from_value::<Requirement>(value).err());
    }
    for key in keys(&rooms, &format!("{strat}/entranceCondition/properties")) {
        if !["comesThroughToilet", "comesInHeated", "devNote"].contains(&key.as_str()) {
            let value = serde_json::json!({ &key: null });
            probe(&key, serde_json::from_value::<EntranceCondition>(value).err());
        }
    }
    for key in keys(&rooms, &format!("{strat}/exitCondition/properties")) {
        let value = serde_json::json!({ &key: null });
        probe(&key, serde_json::from_value::<ExitCondition>(value).err());
    }
    assert!(unknown.is_empty(), "schema variants missing from the types: {unknown:?}");
}

#[test]
fn all_numeric_defaults_evaluate() {
    let data = GameData::load(root()).unwrap();
    let numerics: Vec<_> = data.numerics.all_numerics().collect();
    assert!(!numerics.is_empty());
    fn lookup(numerics: &[&NumericParameter], name: &NumericName) -> Option<i64> {
        let parameter = numerics.iter().find(|n| &n.name == name)?;
        parameter.value.eval(&|inner: &NumericName| lookup(numerics, inner))
    }
    for numeric in &numerics {
        assert!(lookup(&numerics, &numeric.name).is_some(), "{} did not evaluate", numeric.name);
    }
}
