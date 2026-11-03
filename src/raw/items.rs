//! `items.json`: items, game flags, and the initial game state.

use crate::raw::common::*;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::str::FromStr;

macro_rules! items {
    ($($variant:ident),+ $(,)?) => {
        /// An item that appears in logical requirements or at an item location.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemsFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub starting_room: String,
    pub starting_node: NodeId,
    pub starting_items: Vec<Item>,
    pub starting_flags: Vec<FlagName>,
    pub starting_locks: Vec<String>,
    pub starting_resources: Vec<StartingResource>,
    pub implicit_items: Vec<Item>,
    pub upgrade_items: Vec<UpgradeItem>,
    pub expansion_items: Vec<ExpansionItem>,
    pub game_flags: Vec<FlagName>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsumableResource {
    RegularEnergy,
    ReserveEnergy,
    Missile,
    Super,
    PowerBomb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartingResource {
    pub resource: ConsumableResource,
    pub max_amount: u32,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeItem {
    pub name: Item,
    /// Always present upstream; Map Rando's fork omits it for items it adds.
    pub data: Option<HexValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpansionItem {
    pub name: Item,
    pub data: HexValue,
    pub resource: ConsumableResource,
    pub resource_amount: u32,
}
