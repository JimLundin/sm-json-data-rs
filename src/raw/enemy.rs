//! `enemies/main.json`, `enemies/bosses/main.json`, `enemies/bosses/scenarios.json`,
//! and `weapons/main.json`.

use crate::raw::common::*;
use crate::raw::requirement::Requirement;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemiesFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub enemies: Vec<Enemy>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Enemy {
    pub id: EnemyId,
    pub name: String,
    pub attacks: Vec<EnemyAttack>,
    pub hp: u32,
    pub amount_of_drops: u32,
    pub drops: DropRates,
    /// Appears only in Map Rando's fork of the data.
    pub buffed_drops: Option<DropRates>,
    pub farmable_drops: Option<DropRates>,
    pub dims: Dimensions,
    pub freezable: bool,
    pub grapplable: bool,
    /// Weapons and weapon categories that cannot damage this enemy.
    pub invul: Vec<String>,
    pub damage_multipliers: Vec<DamageMultiplier>,
    pub areas: Option<Vec<String>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnemyAttack {
    pub name: String,
    pub base_damage: u32,
    /// Defaults to `true` when absent.
    pub affected_by_varia: Option<bool>,
    /// Defaults to `true` when absent.
    pub affected_by_gravity: Option<bool>,
}

/// Drop odds, each out of 102.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DropRates {
    pub no_drop: f64,
    pub small_energy: f64,
    pub big_energy: f64,
    pub missile: f64,
    #[serde(rename = "super")]
    pub super_missile: f64,
    pub power_bomb: f64,
}

/// An enemy's size in pixels.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dimensions {
    pub w: u32,
    pub h: u32,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageMultiplier {
    /// A weapon or weapon category.
    pub weapon: String,
    pub value: f64,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossScenariosFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub scenarios: Vec<BossScenario>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BossScenario {
    pub id: ScenarioId,
    pub name: String,
    pub boss: String,
    pub explicit_weapons: Option<Vec<String>>,
    pub excluded_weapons: Option<Vec<String>>,
    pub requires: Vec<Requirement>,
    pub boss_dodge_rate: Option<f64>,
    pub attack_opportunity_duration: Option<u32>,
    pub damage_windows: Option<Vec<DamageWindow>>,
    pub incoming_damage: Option<Vec<IncomingDamage>>,
    pub particle_frequency_frames: Option<u32>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DamageWindow {
    pub name: String,
    pub window_percent: f64,
    pub requires: Vec<Requirement>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IncomingDamage {
    pub name: String,
    pub attack: String,
    pub frequency_frames: u32,
    pub avoiding_requires: Vec<Requirement>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponsFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub weapons: Vec<Weapon>,
    pub categories: Option<Vec<WeaponCategory>>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Weapon {
    pub id: WeaponId,
    pub name: String,
    pub damage: u32,
    pub cooldown_frames: u32,
    pub use_requires: Vec<Requirement>,
    pub shot_requires: Option<Vec<Requirement>>,
    pub situational: bool,
    pub hits_group: bool,
    pub categories: Vec<WeaponCategory>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponCategory {
    All,
    Beam,
    ChargedBeam,
    UnchargedBeam,
    UnchargedSpazerBeam,
    UnchargedIceBeam,
    BeamNoPlasma,
    UnchargedBeamNoPlasma,
    PowerBombBlast,
    SpecialBeamAttack,
}
