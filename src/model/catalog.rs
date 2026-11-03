//! Game-wide definitions: items, tech, helpers, numerics, enemies and weapons.

use super::*;

/// A named group, used for presenting tech, helpers and numerics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category<Id> {
    pub name: String,
    pub description: String,
    pub members: Vec<Id>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemCatalog {
    /// Items Samus always has, such as `PowerBeam`.
    pub implicit: Vec<Item>,
    pub upgrades: Vec<Upgrade>,
    pub expansions: Vec<Expansion>,
    pub doc: Doc,
}

impl ItemCatalog {
    /// Whether this dataset defines an item.
    pub fn contains(&self, item: Item) -> bool {
        self.implicit.contains(&item)
            || self.upgrades.iter().any(|upgrade| upgrade.item == item)
            || self.expansions.iter().any(|expansion| expansion.item == item)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Upgrade {
    pub item: Item,
    /// Absent for items that only Map Rando's fork adds.
    pub data: Option<HexValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Expansion {
    pub item: Item,
    pub data: HexValue,
    pub resource: ConsumableResource,
    pub amount: u32,
}

/// The game's initial state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartState {
    pub room: RoomId,
    pub node: NodeId,
    pub items: Vec<Item>,
    pub flags: Vec<FlagId>,
    pub open_locks: Vec<String>,
    pub resources: Vec<(ConsumableResource, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tech {
    pub source_id: Option<crate::raw::TechId>,
    pub name: String,
    pub tech_requires: Vec<Requirement>,
    pub other_requires: Vec<Requirement>,
    /// More complex versions of this tech, grouped under it for presentation.
    pub extensions: Vec<TechId>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Helper {
    pub name: String,
    pub requires: Vec<Requirement>,
    /// Whether Map Rando replaces this helper's definition. Only its fork sets this.
    pub map_rando_override: bool,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Numeric {
    pub name: String,
    pub default: Quantity,
    pub minimum: Option<u32>,
    pub maximum: Option<u32>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub source_id: crate::raw::WeaponId,
    pub name: String,
    pub damage: u32,
    pub cooldown_frames: u32,
    pub use_requires: Vec<Requirement>,
    /// Required on top of `use_requires` for every shot.
    pub shot_requires: Vec<Requirement>,
    pub situational: bool,
    pub hits_group: bool,
    pub categories: Vec<WeaponCategory>,
    pub doc: Doc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Regular,
    Boss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enemy {
    pub source_id: crate::raw::EnemyId,
    pub name: String,
    pub kind: EnemyKind,
    pub attacks: Vec<Attack>,
    pub hp: u32,
    pub drop_count: u32,
    pub drops: DropRates,
    /// Replaces `drops` when buffed drops are enabled. Only Map Rando's fork sets this.
    pub buffed_drops: Option<DropRates>,
    pub farmable_drops: Option<DropRates>,
    pub width: u32,
    pub height: u32,
    pub size_doc: Doc,
    pub freezable: bool,
    pub grapplable: bool,
    pub invulnerable_to: Vec<WeaponSelector>,
    pub damage_multipliers: Vec<(WeaponSelector, f64)>,
    pub areas: Vec<String>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Attack {
    pub name: String,
    pub base_damage: u32,
    pub reduced_by_varia: bool,
    pub reduced_by_gravity: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BossScenario {
    pub source_id: crate::raw::ScenarioId,
    pub name: String,
    pub boss: EnemyId,
    pub weapons: WeaponFilter,
    pub requires: Vec<Requirement>,
    pub dodge_rate: Option<f64>,
    pub attack_opportunity_duration: Option<u32>,
    pub damage_windows: Vec<DamageWindow>,
    pub incoming_damage: Vec<IncomingDamage>,
    pub particle_frequency_frames: Option<u32>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageWindow {
    pub name: String,
    pub percent: f64,
    pub requires: Vec<Requirement>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IncomingDamage {
    pub name: String,
    pub attack: AttackRef,
    pub frequency_frames: u32,
    pub avoiding_requires: Vec<Requirement>,
    pub doc: Doc,
}
