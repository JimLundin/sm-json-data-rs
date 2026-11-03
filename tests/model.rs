//! Checks the domain model against a real checkout of the data.
//!
//! Set `SM_JSON_DATA` to the checkout's path. These tests fail rather than skip
//! when it is missing, so a green run always means the data was actually checked.

use serde_json::Value;
use sm_json_data::model::*;
use sm_json_data::raw;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(std::env::var("SM_JSON_DATA").expect("set SM_JSON_DATA to an sm-json-data checkout"))
}

/// Keys the model deliberately doesn't keep, because they only serve authoring tools.
const DROPPED: &[&str] = &["$schema", "nextStratId", "nextNotableId", "nextTechId"];

/// `(enclosing key, key, value)` for schema defaults, which the model applies at import.
fn defaults() -> Vec<(&'static str, &'static str, Value)> {
    use serde_json::json;
    vec![
        ("*", "comesThroughToilet", json!("no")),
        ("*", "comesInHeated", json!("any")),
        ("*", "bypassesDoorShell", json!("no")),
        ("*", "wallJumpAvoid", json!(false)),
        ("*", "softlock", json!(false)),
        ("*", "isDoorImmediatelyClosed", json!(false)),
        ("*", "useImplicitRequires", json!(true)),
        ("*", "startsWithShineCharge", json!(false)),
        ("*", "endsWithShineCharge", json!(false)),
        ("*", "affectedByVaria", json!(true)),
        ("*", "affectedByGravity", json!(true)),
        ("*", "mapRandoOverride", json!(false)),
        ("*", "blue", json!("any")),
        ("*", "excessFrames", json!(0)),
        ("*", "gentleUpTiles", json!(0)),
        ("*", "gentleDownTiles", json!(0)),
        ("*", "steepUpTiles", json!(0)),
        ("*", "steepDownTiles", json!(0)),
        ("*", "startingSteepDownTiles", json!(0)),
        ("*", "startingGentleDownTiles", json!(0)),
        ("*", "startingStutterlessTiles", json!(0)),
        ("comeInWithGMode", "mobility", json!("any")),
        ("comeInWithDamageBoost", "position", json!("any")),
        ("leaveWithDamageBoost", "position", json!("not-high")),
        ("leaveWithDamage", "attack", json!("contact")),
        ("leaveWithDamage", "knockback", json!(true)),
        ("leaveWithXModeSetup", "attack", json!("contact")),
        ("platforms", "environment", json!("any")),
        ("blocks", "environment", json!("air")),
        ("autoReserveTrigger", "minReserveEnergy", json!(1)),
        ("autoReserveTrigger", "maxReserveEnergy", json!(400)),
        ("autoReserveTrigger", "implicitHeatFrames", json!("yes")),
        ("ridleyKill", "powerBombs", json!(true)),
        ("ridleyKill", "gMode", json!(false)),
    ]
}

/// Removes what import legitimately normalizes: dropped keys, values equal to their
/// default, and empty lists that mean the same as an absent key.
fn normalize(value: &mut Value, parent: &str, table: &[(&'static str, &'static str, Value)]) {
    match value {
        Value::Object(map) => {
            map.retain(|key, v| {
                let is_default = table
                    .iter()
                    .any(|(scope, k, default)| (*scope == "*" || *scope == parent) && k == key && default == v);
                let empty = matches!(v, Value::Array(a) if a.is_empty());
                !DROPPED.contains(&key.as_str()) && !is_default && !empty
            });
            for (key, child) in map.iter_mut() {
                normalize(child, key, table);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| normalize(item, parent, table)),
        _ => {}
    }
}

fn diff(path: &str, original: &Value, exported: &Value, out: &mut Vec<String>) {
    match (original, exported) {
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                let child = format!("{path}.{key}");
                match b.get(key) {
                    Some(other) => diff(&child, value, other, out),
                    None => out.push(format!("{child}: lost {value}")),
                }
            }
            for (key, value) in b.iter().filter(|(key, _)| !a.contains_key(*key)) {
                out.push(format!("{path}.{key}: invented {value}"));
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

fn compare<T: serde::Serialize>(name: &str, original: &T, exported: &T, failures: &mut Vec<String>) {
    let table = defaults();
    let mut a = serde_json::to_value(original).unwrap();
    let mut b = serde_json::to_value(exported).unwrap();
    normalize(&mut a, "", &table);
    normalize(&mut b, "", &table);
    diff(name, &a, &b, failures);
}

fn load() -> raw::GameData {
    raw::GameData::load(root()).unwrap()
}

fn import(data: &raw::GameData) -> World {
    World::import(data).unwrap_or_else(|errors| panic!("{errors}"))
}

#[test]
fn importing_then_exporting_preserves_everything() {
    let data = load();
    let out = import(&data).to_raw();
    let mut failures = Vec::new();
    compare("items", &data.items, &out.items, &mut failures);
    compare("tech", &data.tech, &out.tech, &mut failures);
    compare("helpers", &data.helpers, &out.helpers, &mut failures);
    compare("numerics", &data.numerics, &out.numerics, &mut failures);
    compare("weapons", &data.weapons, &out.weapons, &mut failures);
    compare("enemies", &data.enemies, &out.enemies, &mut failures);
    compare("bosses", &data.bosses, &out.bosses, &mut failures);
    compare("scenarios", &data.boss_scenarios, &out.boss_scenarios, &mut failures);
    let all: Vec<_> = data.connections.iter().flat_map(|f| f.connections.clone()).collect();
    compare("connections", &all, &out.connections[0].connections, &mut failures);
    assert_eq!(data.rooms.len(), out.rooms.len());
    for (a, b) in data.rooms.iter().zip(&out.rooms) {
        compare(&format!("room {}", a.name), a, b, &mut failures);
    }
    println!("compared {} rooms and {} connections", out.rooms.len(), all.len());
    let shown: Vec<&str> = failures.iter().take(40).map(String::as_str).collect();
    assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), shown.join("\n"));
}

#[test]
fn counts_match_the_raw_data() {
    let data = load();
    let world = import(&data);
    let strats = |rooms: usize, nested: usize| (rooms, nested);
    let raw_counts = strats(
        data.rooms.iter().map(|r| r.strats.len()).sum(),
        data.rooms.iter().map(|r| r.all_strats().count()).sum(),
    );
    let world_counts = strats(
        world.rooms.iter().map(|r| r.strats.len()).sum(),
        world.rooms.iter().map(|r| r.all_strats().count()).sum(),
    );
    assert_eq!(raw_counts, world_counts);
    assert_eq!(data.tech.all_techs().count(), world.techs.len());
    assert_eq!(data.enemies.enemies.len() + data.bosses.enemies.len(), world.enemies.len());
    println!("{} strats including nested ones", world_counts.1);
}

#[test]
fn ids_lead_to_the_named_entities() {
    let world = import(&load());
    for tech in &world.techs {
        assert_eq!(world[world.tech_id(&tech.name).unwrap()].name, tech.name);
    }
    for room in &world.rooms {
        assert_eq!(world[world.room_id(&room.name).unwrap()].name, room.name);
        for strat in &room.strats {
            assert!(room.node(strat.from).is_some() && room.node(strat.to).is_some());
        }
    }
    let landing = &world[world.room_id("Landing Site").unwrap()];
    assert!(landing.nodes.iter().any(|n| matches!(n.kind, NodeKind::Door(_))));
    assert!(world.numerics.iter().all(|n| world.default_quantity(&n.default).is_some()));
}

/// Applies `change` to the first strat of the first room, then expects import to fail.
fn expect_rejected(change: impl FnOnce(&mut raw::GameData), expected: &str) {
    let mut data = load();
    change(&mut data);
    let errors = World::import(&data).expect_err("import should fail");
    let text = errors.to_string();
    assert!(text.contains(expected), "expected `{expected}` in:\n{text}");
}

fn add_requirement(data: &mut raw::GameData, json: &str) {
    let requirement = serde_json::from_str(json).unwrap();
    data.rooms[0].strats[0].requires.push(requirement);
}

#[test]
fn invalid_references_are_rejected() {
    let cases = [
        (r#""canNotARealTech""#, "unknown tech"),
        (r#""h_notARealHelper""#, "unknown helper"),
        (r#""f_NotARealFlag""#, "unknown flag"),
        (r#"{"doorUnlockedAtNode": 9999}"#, "no node 9999"),
        (r#"{"obstaclesCleared": ["ZZ"]}"#, "no obstacle"),
        (r#"{"notable": "Not A Real Notable"}"#, "no notable"),
        (r#"{"enemyDamage": {"enemy": "Not An Enemy", "type": "contact", "hits": 1}}"#, "unknown enemy"),
        (r#"{"heatFrames": "n_notARealNumeric"}"#, "unknown numeric"),
        (r#"{"not": "canWalljump"}"#, "not a flag"),
    ];
    for (json, expected) in cases {
        expect_rejected(|data| add_requirement(data, json), expected);
    }
}

#[test]
fn invalid_shapes_are_rejected() {
    expect_rejected(|data| data.rooms[0].strats[0].link = Some([NodeId(9999), NodeId(9999)]), "no node 9999");
    expect_rejected(|data| data.rooms[0].strats[0].link = None, "no link");
    expect_rejected(
        |data| {
            let node = data.rooms[0].nodes.iter_mut().find(|n| n.node_type == raw::NodeType::Door).unwrap();
            node.node_sub_type = raw::NodeSubType::Chozo;
        },
        "cannot have subtype",
    );
    expect_rejected(
        |data| {
            let node = data.rooms[0].nodes.iter_mut().find(|n| n.node_type != raw::NodeType::Door).unwrap();
            node.is_door_immediately_closed = Some(true);
        },
        "does not apply",
    );
    expect_rejected(|data| data.items.starting_node = NodeId(9999), "starting room has no node");
}

/// The export test can't catch a mapping that is wrong the same way in both
/// directions, so this pins each similar-looking requirement to its meaning.
#[test]
fn similar_requirements_keep_their_meaning() {
    let n = Quantity::Const(2);
    let (one, frames) = (|| Quantity::Const(2), n.clone());
    let cases = vec![
        (r#"{"spikeHits": 2}"#, Requirement::Hazard { hazard: Hazard::SpikeHits, amount: one() }),
        (r#"{"thornHits": 2}"#, Requirement::Hazard { hazard: Hazard::ThornHits, amount: one() }),
        (r#"{"electricityHits": 2}"#, Requirement::Hazard { hazard: Hazard::ElectricityHits, amount: one() }),
        (r#"{"hibashiHits": 2}"#, Requirement::Hazard { hazard: Hazard::HibashiHits, amount: one() }),
        (r#"{"samusEaterCycles": 2}"#, Requirement::Hazard { hazard: Hazard::SamusEaterCycles, amount: one() }),
        (r#"{"metroidFrames": 2}"#, Requirement::Hazard { hazard: Hazard::MetroidFrames, amount: one() }),
        (r#"{"draygonElectricityFrames": 2}"#, Requirement::Hazard { hazard: Hazard::DraygonElectricityFrames, amount: one() }),
        (r#"{"heatFrames": 2}"#, Requirement::Heat { frames: frames.clone(), mode: HeatMode::Standard }),
        (r#"{"simpleHeatFrames": 2}"#, Requirement::Heat { frames: frames.clone(), mode: HeatMode::Simple }),
        (r#"{"suitlessHeatFrames": 2}"#, Requirement::Heat { frames: frames.clone(), mode: HeatMode::Suitless }),
        (r#"{"gravitylessHeatFrames": 2}"#, Requirement::Heat { frames: frames.clone(), mode: HeatMode::Gravityless }),
        (r#"{"lavaFrames": 2}"#, Requirement::Lava { frames: frames.clone(), mode: LavaMode::Standard }),
        (r#"{"gravitylessLavaFrames": 2}"#, Requirement::Lava { frames: frames.clone(), mode: LavaMode::Gravityless }),
        (r#"{"acidFrames": 2}"#, Requirement::Acid { frames: frames.clone(), gravity: GravityRule::Standard }),
        (r#"{"gravitylessAcidFrames": 2}"#, Requirement::Acid { frames: frames.clone(), gravity: GravityRule::Gravityless }),
        (r#"{"cycleFrames": 2}"#, Requirement::CycleFrames { frames: frames.clone(), simple: false }),
        (r#"{"simpleCycleFrames": 2}"#, Requirement::CycleFrames { frames: frames.clone(), simple: true }),
        (r#"{"ammo": {"type": "Super", "count": 2}}"#, Requirement::Ammo { kind: AmmoKind::Super, count: one(), usage: AmmoUsage::Spend }),
        (r#"{"ammoDrain": {"type": "Super", "count": 2}}"#, Requirement::Ammo { kind: AmmoKind::Super, count: one(), usage: AmmoUsage::Drain }),
        (r#"{"gainFlashSuit": {}}"#, Requirement::FlashSuit(FlashSuitAction::Gain)),
        (r#"{"useFlashSuit": {}}"#, Requirement::FlashSuit(FlashSuitAction::Use)),
        (r#"{"noFlashSuit": {}}"#, Requirement::FlashSuit(FlashSuitAction::Forbid)),
        (r#"{"gainBlueSuit": {}}"#, Requirement::BlueSuit(BlueSuitAction::Gain)),
        (r#"{"haveBlueSuit": {}}"#, Requirement::BlueSuit(BlueSuitAction::Have)),
        (r#"{"blueSuitShinecharge": {}}"#, Requirement::BlueSuit(BlueSuitAction::Shinecharge)),
        (r#"{"noBlueSuit": {}}"#, Requirement::BlueSuit(BlueSuitAction::Forbid)),
    ];
    let checks = [
        ("resourceAvailable", ResourceCheck::Available),
        ("resourceConsumed", ResourceCheck::Consumed),
        ("resourceAtMost", ResourceCheck::AtMost),
        ("resourceMissingAtMost", ResourceCheck::MissingAtMost),
        ("resourceCapacity", ResourceCheck::Capacity),
        ("resourceMaxCapacity", ResourceCheck::MaxCapacity),
    ];
    let mut data = load();
    let mut expected = Vec::new();
    for (json, requirement) in cases {
        add_requirement(&mut data, json);
        expected.push(requirement);
    }
    for (key, check) in checks {
        add_requirement(&mut data, &format!(r#"{{"{key}": [{{"type": "RegularEnergy", "count": 2}}]}}"#));
        let amounts = vec![ResourceAmount { kind: ResourceKind::RegularEnergy, count: Quantity::Const(2) }];
        expected.push(Requirement::Resource { check, amounts });
    }
    let world = import(&data);
    let requires = &world.rooms[0].strats[0].strat.requires;
    assert_eq!(&requires[requires.len() - expected.len()..], &expected[..]);
}

#[test]
fn nodes_and_flags_have_their_documented_meaning() {
    let world = import(&load());
    let room = &world[world.room_id("Morph Ball Room").unwrap()];
    let items: Vec<_> = room.nodes.iter().filter_map(|n| match &n.kind {
        NodeKind::Item { item, .. } => Some(*item),
        _ => None,
    }).collect();
    assert!(items.contains(&Item::Morph), "{items:?}");
    let start = &world[world.start.room];
    assert_eq!(start.name, "Ceres Elevator Room");
    assert!(world.start.items.contains(&Item::PowerBeam));
    let forward = world.connections.iter().filter(|c| c.direction == ConnectionDirection::Forward).count();
    assert!(forward > 0 && forward < world.connections.len());
    let phantoon = world.enemy_id("Phantoon").unwrap();
    assert_eq!(world[phantoon].kind, EnemyKind::Boss);
    assert!(world.flag_id("f_DefeatedPhantoon").is_some());
}

#[test]
fn every_problem_is_reported_at_once() {
    let mut data = load();
    add_requirement(&mut data, r#""canNotARealTech""#);
    data.rooms[1].strats[0].requires.push(serde_json::from_str(r#""f_NotARealFlag""#).unwrap());
    let errors = World::import(&data).unwrap_err();
    assert_eq!(errors.0.len(), 2, "{errors}");
    assert!(errors.0[0].location.contains(&data.rooms[0].name));
}
