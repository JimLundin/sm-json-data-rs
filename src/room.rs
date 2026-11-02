//! Room files under `region/`.

use crate::common::*;
use crate::items::Item;
use crate::requirement::Requirement;
use crate::strat::Strat;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Room {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub id: RoomId,
    pub name: String,
    pub area: String,
    pub subarea: String,
    pub subsubarea: Option<String>,
    pub room_address: Option<HexValue>,
    pub room_environments: Vec<RoomEnvironment>,
    pub map_tile_mask: Vec<Vec<u8>>,
    pub nodes: Vec<Node>,
    pub obstacles: Option<Vec<Obstacle>>,
    pub enemies: Option<Vec<RoomEnemy>>,
    pub strats: Vec<Strat>,
    pub notables: Option<Vec<Notable>>,
    pub next_strat_id: Option<u32>,
    pub next_notable_id: Option<u32>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

impl Room {
    /// Finds a node by its ID within this room.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Iterates over the room's strats and those nested in locks and viewable nodes.
    pub fn all_strats(&self) -> impl Iterator<Item = &Strat> {
        let nested = self.nodes.iter().flat_map(|node| {
            let unlocks = node.locks.iter().flatten().flat_map(|lock| &lock.unlock_strats);
            let views = node.viewable_nodes.iter().flatten().flat_map(|view| &view.strats);
            unlocks.chain(views)
        });
        self.strats.iter().chain(nested)
    }
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoomEnvironment {
    pub heated: bool,
    /// The entrances this applies to; all of them when absent.
    pub entrance_nodes: Option<Vec<NodeId>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoorEnvironment {
    pub physics: Physics,
    /// The entrances this applies to; all of them when absent.
    pub entrance_nodes: Option<Vec<NodeId>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Physics {
    Air,
    Water,
    Lava,
    Acid,
    Normal,
}

/// A door, item location, junction, or other point of interest in a room.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub node_type: NodeType,
    pub node_sub_type: NodeSubType,
    /// The item found here in the unmodified game.
    pub node_item: Option<Item>,
    pub node_address: Option<HexValue>,
    pub door_orientation: Option<DoorOrientation>,
    pub door_environments: Option<Vec<DoorEnvironment>>,
    pub use_implicit_leave_normally: Option<bool>,
    pub use_implicit_come_in_normally: Option<bool>,
    pub use_implicit_come_in_with_mockball: Option<bool>,
    pub use_implicit_carry_g_mode_back_through: Option<bool>,
    pub use_implicit_carry_g_mode_morph_back_through: Option<bool>,
    pub use_implicit_come_in_with_grapple_jump: Option<bool>,
    /// Defaults to `true` when absent.
    pub use_implicit_door_unlocks: Option<bool>,
    /// Defaults to `false` when absent.
    pub is_door_immediately_closed: Option<bool>,
    pub locks: Option<Vec<Lock>>,
    pub twin_door_addresses: Option<Vec<TwinDoorAddress>>,
    pub utility: Option<Vec<Utility>>,
    pub viewable_nodes: Option<Vec<ViewableNode>>,
    pub yields: Option<Vec<FlagName>>,
    pub map_tile_mask: Vec<Vec<u8>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Door,
    Entrance,
    Exit,
    Item,
    Junction,
    Utility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeSubType {
    Chozo,
    Hidden,
    Visible,
    Blue,
    ClosedWall,
    Doorway,
    Elevator,
    Eye,
    Gray,
    Green,
    Passage,
    Red,
    Sandpit,
    Vertical,
    Yellow,
    Junction,
    #[serde(rename = "g-mode")]
    GMode,
    #[serde(rename = "r-mode")]
    RMode,
    Save,
    Missile,
    Energy,
    Ship,
    Map,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoorOrientation {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Utility {
    Save,
    Missile,
    Super,
    Powerbomb,
    Energy,
    Reserve,
    Map,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lock {
    pub name: String,
    pub lock_type: LockType,
    /// The conditions under which the lock is active; always active when absent.
    pub lock: Option<Vec<Requirement>>,
    pub unlock_strats: Vec<Strat>,
    pub yields: Option<Vec<FlagName>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LockType {
    BossFight,
    ColoredDoor,
    Cutscene,
    EscapeFunnel,
    GameFlag,
    KillEnemies,
    Permanent,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TwinDoorAddress {
    pub room_address: HexValue,
    pub door_address: HexValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewableNode {
    pub id: NodeId,
    pub strats: Vec<Strat>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Obstacle {
    pub id: ObstacleId,
    pub name: String,
    pub obstacle_type: ObstacleType,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObstacleType {
    Inanimate,
    Enemies,
    Abstract,
}

/// A group of enemies placed in a room.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoomEnemy {
    pub id: String,
    pub group_name: String,
    pub enemy_name: String,
    pub quantity: u32,
    /// Nodes where the enemies engage Samus. Upstream sets this or `between_nodes`.
    pub home_nodes: Option<Vec<NodeId>>,
    /// The two nodes between which the enemies can be reached.
    pub between_nodes: Option<[NodeId; 2]>,
    pub spawn: Option<Vec<Requirement>>,
    pub stop_spawn: Option<Vec<Requirement>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

/// A named technique specific to a room, referenced by strats.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Notable {
    pub id: NotableId,
    pub name: String,
    /// Defaults to `false` when absent.
    pub wall_jump_avoid: Option<bool>,
    pub note: Note,
    pub dev_note: Option<Note>,
}
