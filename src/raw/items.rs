//! `items.json`: items, game flags, and the initial game state.

use crate::raw::*;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

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
