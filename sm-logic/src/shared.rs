//! Plain values used by both the model and the JSON layer.
//!
//! The `serde` feature exists only so the JSON layer can read these types.

use std::fmt;
use std::str::FromStr;

macro_rules! source_ids {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(transparent))]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    )+};
}

source_ids! {
    /// Node identifier from the data, unique within a room.
    NodeId,
    /// Notable identifier from the data, unique within a room.
    NotableId,
}

/// Identifiers from the data, kept to identify entities across datasets.
pub mod source {
    use super::*;

    source_ids! {
        /// Room identifier, unique across the game.
        RoomId,
        /// Strat identifier, unique within a room.
        StratId,
        /// Tech identifier, unique across all tech.
        TechId,
        /// Enemy identifier, unique across the game.
        EnemyId,
        /// Weapon identifier, unique across the game.
        WeaponId,
        /// Boss scenario identifier, unique across the game.
        ScenarioId,
    }
}

macro_rules! items {
    ($($variant:ident),+ $(,)?) => {
        /// An item that appears in logical requirements or at an item location.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum Item {
            $($variant),+
        }

        impl Item {
            /// All items, in declaration order.
            pub const ALL: &'static [Item] = &[$(Item::$variant),+];

            /// The item's name as written in the data.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Item::$variant => stringify!($variant)),+
                }
            }
        }

        impl FromStr for Item {
            type Err = String;

            fn from_str(name: &str) -> Result<Self, String> {
                match name {
                    $(stringify!($variant) => Ok(Item::$variant),)+
                    _ => Err(format!("unknown item `{name}`")),
                }
            }
        }
    };
}

items! {
    PowerBeam, Morph, Bombs, Charge, HiJump, Spazer, Varia, SpeedBooster, Ice, Grapple, Wave,
    XRayScope, Gravity, SpaceJump, SpringBall, Plasma, ScrewAttack, ETank, Super, Missile,
    ReserveTank, PowerBomb,
    // Defined only by Map Rando's fork of the data.
    WallJump, BlueBooster, SparkBooster,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AmmoKind {
    Missile,
    Super,
    PowerBomb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResourceKind {
    Missile,
    Super,
    PowerBomb,
    RegularEnergy,
    ReserveEnergy,
    Energy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ImplicitHeatFrames {
    Yes,
    No,
    Suitless,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum RidleyStuck {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConnectionType {
    HorizontalDoor,
    VerticalDoor,
    HorizontalMorphTunnel,
    VerticalSandpit,
    Elevator,
    StoryMarker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConnectionDirection {
    /// One-way, from the first node to the second.
    Forward,
    Bidirectional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ConnectionPosition {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ConsumableResource {
    RegularEnergy,
    ReserveEnergy,
    Missile,
    Super,
    PowerBomb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum DoorUnlockType {
    Missiles,
    Super,
    Powerbomb,
    Gray,
    Ammo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum SparkPosition {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum SidePosition {
    Left,
    Right,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum MovementType {
    Controlled,
    Uncontrolled,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum GModeMode {
    Direct,
    Indirect,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum GModeMobility {
    Mobile,
    Immobile,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Environment {
    Water,
    Air,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Drop odds, each out of 102.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", deny_unknown_fields))]
pub struct DropRates {
    pub no_drop: f64,
    pub small_energy: f64,
    pub big_energy: f64,
    pub missile: f64,
    #[cfg_attr(feature = "serde", serde(rename = "super"))]
    pub super_missile: f64,
    pub power_bomb: f64,
}

/// A condition that can be required, forbidden, or left open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum TriState {
    Yes,
    No,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Physics {
    Air,
    Water,
    Lava,
    Acid,
    Normal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum DoorOrientation {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Utility {
    Save,
    Missile,
    Super,
    Powerbomb,
    Energy,
    Reserve,
    Map,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum LockType {
    BossFight,
    ColoredDoor,
    Cutscene,
    EscapeFunnel,
    GameFlag,
    KillEnemies,
    Permanent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ObstacleType {
    Inanimate,
    Enemies,
    Abstract,
}
