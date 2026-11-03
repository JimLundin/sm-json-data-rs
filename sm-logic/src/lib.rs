//! A validated model of Super Metroid's logic, and a compact file format for it.
//!
//! The model is built from the community's [sm-json-data]: the `sm-json-data` crate
//! parses the JSON and imports it, and [`pack`] stores the result compactly. This
//! crate has no dependencies, so a game can decode a pack without pulling anything in.
//!
//! Every name in a [`World`] is a typed ID known to resolve, schema defaults are
//! applied, and fields that only make sense together live on the same enum variant.
//!
//! Most IDs here are dense indices into [`World`], assigned at import and not
//! stable between datasets. Each entity keeps its upstream ID as `source_id`, and
//! node and notable IDs stay upstream's because they are already stable in a room.
//!
//! [sm-json-data]: https://github.com/vg-json-data/sm-json-data

mod catalog;
mod logic;
mod lzma;
pub mod pack;
mod room;
mod shared;
mod strat;
mod validate;

pub use catalog::*;
pub use logic::*;
pub use room::*;
pub use shared::*;
pub use strat::*;

use std::collections::HashMap;
use std::fmt;
use std::ops::Index;

/// A distance or speed measured in tiles, which can be fractional.
pub type Tiles = f64;

/// A note, written upstream as either a string or a list of lines.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Note {
    Text(String),
    Lines(Vec<String>),
}

impl Note {
    /// Iterates over the note's lines, whichever form it was written in.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        let slice: &[String] = match self {
            Note::Text(text) => std::slice::from_ref(text),
            Note::Lines(lines) => lines,
        };
        slice.iter().map(String::as_str)
    }
}

/// An address or similar value from the game's ROM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(pub u32);

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:X}", self.0)
    }
}

/// A run speed as the game stores it, in 1/65536ths of a pixel per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunSpeed(pub u32);

impl RunSpeed {
    /// The speed in pixels per frame.
    pub fn pixels_per_frame(self) -> f64 {
        f64::from(self.0) / 65_536.0
    }
}

/// Free-text documentation. Nothing in the logic depends on it, and it is absent
/// when a pack is loaded without its docs section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Doc {
    pub note: Option<Note>,
    pub detail: Option<Note>,
    pub dev: Option<Note>,
}

macro_rules! dense_ids {
    ($($(#[$meta:meta])* $name:ident => $field:ident: $entity:ty),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u16);

        impl $name {
            /// This ID's position in its table in [`World`].
            pub fn index(self) -> usize {
                usize::from(self.0)
            }
        }

        impl Index<$name> for World {
            type Output = $entity;

            fn index(&self, id: $name) -> &$entity {
                &self.$field[id.index()]
            }
        }
    )+};
}

dense_ids! {
    /// A room in [`World::rooms`].
    RoomId => rooms: Room,
    /// A tech in [`World::techs`].
    TechId => techs: Tech,
    /// A helper in [`World::helpers`].
    HelperId => helpers: Helper,
    /// A numeric parameter in [`World::numerics`].
    NumericId => numerics: Numeric,
    /// A game flag in [`World::flags`].
    FlagId => flags: String,
    /// A randomizer-defined requirement in [`World::internals`].
    InternalId => internals: String,
    /// A weapon in [`World::weapons`].
    WeaponId => weapons: Weapon,
    /// An enemy or boss in [`World::enemies`].
    EnemyId => enemies: Enemy,
}

/// Super Metroid's complete logic model.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub items: ItemCatalog,
    pub start: StartState,
    pub flags: Vec<String>,
    /// Requirements such as `i_ammoRefill`, which only Map Rando's fork uses.
    pub internals: Vec<String>,
    pub techs: Vec<Tech>,
    pub tech_categories: Vec<Category<TechId>>,
    pub helpers: Vec<Helper>,
    pub helper_categories: Vec<Category<HelperId>>,
    pub numerics: Vec<Numeric>,
    pub numeric_categories: Vec<Category<NumericId>>,
    pub weapons: Vec<Weapon>,
    pub weapon_categories: Vec<WeaponCategory>,
    pub enemies: Vec<Enemy>,
    pub boss_scenarios: Vec<BossScenario>,
    pub rooms: Vec<Room>,
    pub connections: Vec<Connection>,
    /// Finds entities by name. See [`World::rebuild_index`].
    pub lookup: Lookup,
}

/// Maps names to IDs. Derived from the rest of a [`World`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lookup {
    pub rooms: HashMap<String, RoomId>,
    pub techs: HashMap<String, TechId>,
    pub helpers: HashMap<String, HelperId>,
    pub numerics: HashMap<String, NumericId>,
    pub flags: HashMap<String, FlagId>,
    pub weapons: HashMap<String, WeaponId>,
    pub enemies: HashMap<String, EnemyId>,
}

impl World {
    /// Rebuilds the name lookups after constructing or changing a world.
    pub fn rebuild_index(&mut self) {
        fn table<T, Id>(items: &[T], name: impl Fn(&T) -> &str, id: impl Fn(u16) -> Id) -> HashMap<String, Id> {
            items.iter().enumerate().map(|(index, item)| (name(item).to_owned(), id(index as u16))).collect()
        }
        self.lookup = Lookup {
            rooms: table(&self.rooms, |r| r.name.as_str(), RoomId),
            techs: table(&self.techs, |t| t.name.as_str(), TechId),
            helpers: table(&self.helpers, |h| h.name.as_str(), HelperId),
            numerics: table(&self.numerics, |n| n.name.as_str(), NumericId),
            flags: table(&self.flags, String::as_str, FlagId),
            weapons: table(&self.weapons, |w| w.name.as_str(), WeaponId),
            enemies: table(&self.enemies, |e| e.name.as_str(), EnemyId),
        };
    }

    /// Removes everything the logic doesn't depend on, which is what loading a pack
    /// without its docs section gives you.
    pub fn strip_docs(&mut self) {
        let strip_doc = |doc: &mut Doc| *doc = Doc::default();
        fn strip_blocks(blocks: &mut [GrappleSwingBlock]) {
            blocks.iter_mut().for_each(|block| block.doc = Doc::default());
        }
        fn strip_strat(strat: &mut Strat) {
            strat.name = None;
            strat.doc = Doc::default();
            strat.door_unlocks.iter_mut().for_each(|unlock| unlock.doc = Doc::default());
            for failure in &mut strat.failures {
                failure.name = None;
                failure.doc = Doc::default();
            }
            if let Some(entry) = &mut strat.entry {
                entry.dev_note = None;
                match &mut entry.kind {
                    EntryKind::SidePlatform { platforms } => platforms.iter_mut().for_each(|p| p.doc = Doc::default()),
                    EntryKind::GrappleSwing { blocks } => strip_blocks(blocks),
                    _ => {}
                }
            }
            if let Some(DoorExit::GrappleSwing { blocks }) = &mut strat.exit {
                strip_blocks(blocks);
            }
        }
        strip_doc(&mut self.items.doc);
        self.techs.iter_mut().for_each(|t| strip_doc(&mut t.doc));
        self.helpers.iter_mut().for_each(|h| strip_doc(&mut h.doc));
        self.numerics.iter_mut().for_each(|n| strip_doc(&mut n.doc));
        self.weapons.iter_mut().for_each(|w| strip_doc(&mut w.doc));
        self.tech_categories.iter_mut().for_each(|c| c.description = None);
        self.helper_categories.iter_mut().for_each(|c| c.description = None);
        self.numeric_categories.iter_mut().for_each(|c| c.description = None);
        for enemy in &mut self.enemies {
            strip_doc(&mut enemy.doc);
            strip_doc(&mut enemy.size_doc);
        }
        for scenario in &mut self.boss_scenarios {
            strip_doc(&mut scenario.doc);
            scenario.damage_windows.iter_mut().for_each(|w| strip_doc(&mut w.doc));
            scenario.incoming_damage.iter_mut().for_each(|d| strip_doc(&mut d.doc));
        }
        for connection in &mut self.connections {
            connection.description = None;
            strip_doc(&mut connection.doc);
            for end in &mut connection.ends {
                end.label = None;
                strip_doc(&mut end.doc);
            }
        }
        for room in &mut self.rooms {
            strip_doc(&mut room.doc);
            room.environments.iter_mut().for_each(|e| strip_doc(&mut e.doc));
            for obstacle in &mut room.obstacles {
                obstacle.name = None;
                strip_doc(&mut obstacle.doc);
            }
            for enemy in &mut room.enemies {
                enemy.group_name = None;
                strip_doc(&mut enemy.doc);
            }
            for notable in &mut room.notables {
                notable.name = None;
                notable.note = None;
                notable.dev_note = None;
            }
            room.strats.iter_mut().for_each(|s| strip_strat(&mut s.strat));
            for node in &mut room.nodes {
                strip_doc(&mut node.doc);
                if let NodeKind::Door(door) = &mut node.kind {
                    door.environments.iter_mut().for_each(|e| strip_doc(&mut e.doc));
                }
                for lock in &mut node.locks {
                    strip_doc(&mut lock.doc);
                    lock.unlock_strats.iter_mut().for_each(strip_strat);
                }
                for view in &mut node.viewable {
                    view.strats.iter_mut().for_each(strip_strat);
                }
            }
        }
    }

    pub fn room_id(&self, name: &str) -> Option<RoomId> {
        self.lookup.rooms.get(name).copied()
    }

    pub fn tech_id(&self, name: &str) -> Option<TechId> {
        self.lookup.techs.get(name).copied()
    }

    pub fn helper_id(&self, name: &str) -> Option<HelperId> {
        self.lookup.helpers.get(name).copied()
    }

    pub fn numeric_id(&self, name: &str) -> Option<NumericId> {
        self.lookup.numerics.get(name).copied()
    }

    pub fn flag_id(&self, name: &str) -> Option<FlagId> {
        self.lookup.flags.get(name).copied()
    }

    pub fn weapon_id(&self, name: &str) -> Option<WeaponId> {
        self.lookup.weapons.get(name).copied()
    }

    pub fn enemy_id(&self, name: &str) -> Option<EnemyId> {
        self.lookup.enemies.get(name).copied()
    }

    /// Evaluates a quantity with every numeric parameter at its default value.
    ///
    /// Import rejects cyclic parameters, so this returns `None` only on overflow.
    pub fn default_quantity(&self, quantity: &Quantity) -> Option<i64> {
        quantity.eval(&|id| self.default_quantity(&self[id].default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_evaluates_with_parameters() {
        let q = Quantity::Product(vec![Quantity::Sum(vec![Quantity::Param(NumericId(0)), Quantity::Const(2)]), Quantity::Const(3)]);
        assert_eq!(q.eval(&|_| Some(4)), Some(18));
        assert_eq!(q.eval(&|_| None), None);
    }
}
