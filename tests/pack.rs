//! Checks the pack format against a real checkout of the data.
//!
//! Set `SM_JSON_DATA` to the checkout's path. These tests fail rather than skip
//! when it is missing, so a green run always means the data was actually checked.

use sm_json_data::{export_world, load_world};
use sm_logic::pack::{decode, decode_unverified, encode, inspect, Docs, EncodeOptions, PackError, VERSION};
use sm_logic::{Requirement, World};

fn world() -> World {
    let root = std::env::var("SM_JSON_DATA").expect("set SM_JSON_DATA to an sm-json-data checkout");
    load_world(root).unwrap_or_else(|error| panic!("{error}"))
}

fn options(docs: bool) -> EncodeOptions {
    EncodeOptions { docs, source: "test-commit".to_owned() }
}

fn stripped(world: &World) -> World {
    let mut copy = world.clone();
    copy.strip_docs();
    copy
}

/// Reports the first differing room or table, because `World`'s debug form is huge.
fn assert_same(a: &World, b: &World) {
    assert_eq!(a.techs, b.techs);
    assert_eq!(a.helpers, b.helpers);
    assert_eq!(a.enemies, b.enemies);
    assert_eq!(a.connections, b.connections);
    for (x, y) in a.rooms.iter().zip(&b.rooms) {
        assert!(x == y, "room {} differs", x.name);
    }
    assert!(a == b, "worlds differ outside the rooms");
}

#[test]
fn a_pack_with_docs_decodes_to_the_same_world() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    assert_same(&decode(&bytes, Docs::Load).unwrap(), &world);
}

#[test]
fn a_pack_without_docs_decodes_to_the_stripped_world() {
    let world = world();
    let expected = stripped(&world);
    let bytes = encode(&world, &options(false)).unwrap();
    assert_same(&decode(&bytes, Docs::Load).unwrap(), &expected);
    assert_eq!(inspect(&bytes).unwrap().docs_bytes, 0);
}

#[test]
fn skipping_docs_gives_the_same_logic() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    assert_same(&decode(&bytes, Docs::Skip).unwrap(), &stripped(&world));
}

#[test]
fn a_decoded_pack_exports_like_the_original() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    assert!(export_world(&world) == export_world(&decode(&bytes, Docs::Load).unwrap()));
}

#[test]
fn encoding_is_deterministic() {
    let world = world();
    let bytes = encode(&world, &options(false)).unwrap();
    assert!(bytes == encode(&world, &options(false)).unwrap());
    assert!(bytes == encode(&decode(&bytes, Docs::Load).unwrap(), &options(false)).unwrap());
}

#[test]
fn the_header_identifies_the_pack() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    let info = inspect(&bytes).unwrap();
    assert_eq!(info.version, VERSION);
    assert_eq!(info.source, "test-commit");
    assert!(info.logic_bytes > 0 && info.docs_bytes > 0);
    assert!(info.logic_bytes + info.docs_bytes < bytes.len());
}

#[test]
fn name_lookups_work_after_decoding() {
    let world = world();
    let decoded = decode(&encode(&world, &options(false)).unwrap(), Docs::Skip).unwrap();
    let room = decoded.room_id("Landing Site").unwrap();
    assert_eq!(decoded[room].name, "Landing Site");
    assert!(decoded.tech_id(&world.techs[0].name).is_some());
    assert!(decoded[room].strats.iter().all(|strat| strat.strat.name.is_none()));
}

/// Touches every reference, which would panic if validation had missed a bad one.
fn use_everything(world: &World) {
    for room in &world.rooms {
        for strat in room.all_strats() {
            for requirement in &strat.requires {
                requirement.walk(&mut |r| match r {
                    Requirement::Tech(id) | Requirement::TechOnly(id) => assert!(world[*id].extensions.len() < usize::MAX),
                    Requirement::Helper(id) => assert!(world[*id].requires.len() < usize::MAX),
                    Requirement::Flag(id) | Requirement::NotFlag(id) => assert!(world[*id].len() < usize::MAX),
                    Requirement::EnemyHits { attack, .. } => assert!(usize::from(attack.attack) < world[attack.enemy].attacks.len()),
                    _ => {}
                });
            }
        }
    }
    for connection in &world.connections {
        for end in &connection.ends {
            assert!(world[end.room].node(end.node).is_some());
        }
    }
}

#[test]
fn truncated_packs_are_rejected() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    let step = (bytes.len() / 200).max(1);
    for length in (0..bytes.len()).step_by(step) {
        assert!(decode(&bytes[..length], Docs::Load).is_err(), "accepted {length} bytes");
    }
}

#[test]
fn corrupted_packs_never_panic() {
    let world = world();
    let bytes = encode(&world, &options(true)).unwrap();
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut rejected, mut accepted) = (0, 0);
    for _ in 0..400 {
        let mut copy = bytes.clone();
        for _ in 0..1 + next() % 8 {
            let index = (next() % copy.len() as u64) as usize;
            copy[index] = next() as u8;
        }
        match decode_unverified(&copy, Docs::Load) {
            Ok(decoded) => {
                use_everything(&decoded);
                accepted += 1;
            }
            Err(_) => rejected += 1,
        }
    }
    println!("corrupted packs: {rejected} rejected, {accepted} decoded to a valid world");
    assert!(rejected > 0);
}

#[test]
fn damage_anywhere_fails_the_checksum() {
    let bytes = encode(&world(), &options(true)).unwrap();
    let step = (bytes.len() / 200).max(1);
    for index in (14..bytes.len()).step_by(step) {
        let mut copy = bytes.clone();
        copy[index] ^= 0x10;
        assert!(decode(&copy, Docs::Skip).is_err(), "accepted damage at byte {index}");
    }
    let mut copy = bytes.clone();
    *copy.last_mut().unwrap() ^= 1;
    assert_eq!(decode(&copy, Docs::Skip).unwrap_err(), PackError::Corrupt);
}

#[test]
fn corrupted_reference_tables_are_rejected_by_validation() {
    let mut world = world();
    world.rooms[0].strats[0].from = sm_logic::NodeId(9999);
    let bytes = encode(&world, &options(false)).unwrap();
    assert!(matches!(decode(&bytes, Docs::Skip), Err(PackError::Invalid(_))));
}

#[test]
fn other_versions_are_refused() {
    let world = world();
    let mut bytes = encode(&world, &options(false)).unwrap();
    bytes[4] = VERSION + 1;
    assert_eq!(decode(&bytes, Docs::Skip).unwrap_err(), PackError::UnsupportedVersion(VERSION + 1));
}
