//! The domain model: Super Metroid's logic as a linked, validated world.
//!
//! [`World::import`] turns the parsed JSON into this form. Every name becomes a
//! typed ID that is known to resolve, schema defaults are applied, and fields that
//! only make sense together live on the same enum variant.
//!
//! Most IDs here are dense indices into [`World`], assigned at import and not
//! stable between datasets. Each entity keeps its upstream ID as `source_id`, and
//! node and notable IDs stay upstream's because they are already stable in a room.

mod catalog;
mod export;
mod import;
mod logic;
mod room;
mod strat;

pub use crate::raw::{
    AmmoKind, ConnectionDirection, ConnectionPosition, ConnectionType, ConsumableResource, DoorOrientation,
    DoorUnlockType, DropRates, Environment, ExtraRunSpeed, GModeMobility, GModeMode, HexValue, ImplicitHeatFrames,
    Item, LockType, MovementType, NodeId, NotableId, Note, ObstacleType, Physics, ResourceKind, RidleyStuck,
    SidePosition, SparkPosition, TriState, Utility, WeaponCategory,
};
pub use catalog::*;
pub use import::{ImportErrors, ModelError, WorldError};
pub use logic::*;
pub use room::*;
pub use strat::*;

use std::collections::HashMap;
use std::ops::Index;

/// A distance or speed measured in tiles, which can be fractional.
pub type Tiles = f64;

/// Free-text documentation. Nothing in the logic depends on it.
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
        pub struct $name(pub(crate) u16);

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
    lookup: Lookup,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Lookup {
    rooms: HashMap<String, RoomId>,
    techs: HashMap<String, TechId>,
    helpers: HashMap<String, HelperId>,
    numerics: HashMap<String, NumericId>,
    flags: HashMap<String, FlagId>,
    weapons: HashMap<String, WeaponId>,
    enemies: HashMap<String, EnemyId>,
}

impl World {
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
