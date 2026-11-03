//! Rooms, their nodes, and the connections between them.

use crate::*;

/// An obstacle, by its position in [`Room::obstacles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObstacleId(pub u8);

impl ObstacleId {
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub source_id: source::RoomId,
    pub name: String,
    pub area: String,
    pub subarea: String,
    pub subsubarea: Option<String>,
    pub address: Option<Address>,
    pub environments: Vec<RoomEnvironment>,
    pub map_tiles: Vec<Vec<u8>>,
    pub nodes: Vec<Node>,
    pub obstacles: Vec<Obstacle>,
    pub enemies: Vec<RoomEnemy>,
    pub strats: Vec<RoomStrat>,
    pub notables: Vec<Notable>,
    pub doc: Doc,
}

impl Room {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn notable(&self, id: NotableId) -> Option<&Notable> {
        self.notables.iter().find(|notable| notable.id == id)
    }

    /// The strats leaving a node.
    pub fn strats_from(&self, node: NodeId) -> impl Iterator<Item = &RoomStrat> {
        self.strats.iter().filter(move |strat| strat.from == node)
    }

    /// Every strat in the room, including those nested in locks and viewable nodes.
    pub fn all_strats(&self) -> impl Iterator<Item = &Strat> {
        let nested = self.nodes.iter().flat_map(|node| {
            let unlocks = node.locks.iter().flat_map(|lock| &lock.unlock_strats);
            let views = node.viewable.iter().flat_map(|view| &view.strats);
            unlocks.chain(views)
        });
        self.strats.iter().map(|room_strat| &room_strat.strat).chain(nested)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEnvironment {
    pub heated: bool,
    /// The entrances this applies to; all of them when absent.
    pub entrances: Option<Vec<NodeId>>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
    pub map_tiles: Vec<Vec<u8>>,
    pub locks: Vec<Lock>,
    pub viewable: Vec<ViewableNode>,
    pub yields: Vec<FlagId>,
    pub doc: Doc,
}

/// What a node is. Each kind has only the fields that apply to it.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Door(Door),
    /// One-way arrival from outside the room, such as falling in through a sandpit.
    Entrance { transit: Transit, orientation: DoorOrientation, address: Option<Address> },
    /// One-way departure from the room, such as falling out through a sandpit.
    Exit { transit: Transit, orientation: DoorOrientation, address: Option<Address> },
    Item { item: Item, placement: ItemPlacement, address: Address },
    Junction(JunctionKind),
    Station { station: StationKind, services: Vec<Utility>, orientation: Option<DoorOrientation> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Door {
    pub style: DoorStyle,
    pub orientation: Option<DoorOrientation>,
    pub address: Option<Address>,
    /// Empty for elevators, which have no door environment.
    pub environments: Vec<DoorEnvironment>,
    pub implicit: ImplicitStrats,
    pub immediately_closed: bool,
    pub twins: Vec<TwinDoor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DoorStyle {
    Blue,
    ClosedWall,
    Doorway,
    Elevator,
    Eye,
    Gray,
    Green,
    Passage,
    Red,
    Vertical,
    Yellow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transit {
    Elevator,
    Sandpit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemPlacement {
    Chozo,
    Hidden,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JunctionKind {
    Plain,
    GMode,
    RMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StationKind {
    Energy,
    Map,
    Missile,
    Save,
    Ship,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoorEnvironment {
    pub physics: Physics,
    /// The entrances this applies to; all of them when absent.
    pub entrances: Option<Vec<NodeId>>,
    pub doc: Doc,
}

/// Whether a standard strat is generated for a door.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Implicit {
    /// The documented rule applies. For G-mode strats this can depend on the door.
    #[default]
    Automatic,
    /// Explicitly on, which overrides exceptions in the documented rule.
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ImplicitStrats {
    pub leave_normally: Implicit,
    pub come_in_normally: Implicit,
    pub come_in_with_mockball: Implicit,
    pub come_in_with_grapple_jump: Implicit,
    pub carry_g_mode_back_through: Implicit,
    pub carry_g_mode_morph_back_through: Implicit,
    pub door_unlocks: Implicit,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TwinDoor {
    pub room_address: Address,
    pub door_address: Address,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lock {
    pub name: String,
    pub kind: LockType,
    /// The conditions under which the lock is active; always active when absent.
    pub active_when: Option<Vec<Requirement>>,
    pub unlock_strats: Vec<Strat>,
    pub yields: Vec<FlagId>,
    pub doc: Doc,
}

/// An item node that can be seen from this node, and how.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewableNode {
    pub node: NodeId,
    pub strats: Vec<Strat>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle {
    pub key: String,
    pub name: Option<String>,
    pub kind: ObstacleType,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEnemy {
    pub key: String,
    pub group_name: Option<String>,
    pub enemy: EnemyId,
    pub quantity: u32,
    pub placement: EnemyPlacement,
    pub spawn: Option<Vec<Requirement>>,
    pub stop_spawn: Option<Vec<Requirement>>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EnemyPlacement {
    /// Engages Samus at these nodes.
    Home(Vec<NodeId>),
    /// Can only be reached while traveling between these two nodes.
    Between(NodeId, NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notable {
    pub id: NotableId,
    pub name: Option<String>,
    pub wall_jump_avoid: bool,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

/// A link between nodes in two rooms.
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub kind: ConnectionType,
    pub direction: ConnectionDirection,
    pub description: Option<String>,
    pub ends: [ConnectionEnd; 2],
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionEnd {
    pub room: RoomId,
    pub node: NodeId,
    pub position: ConnectionPosition,
    /// The display names upstream gives this end, which often differ from the room's.
    pub label: Option<ConnectionLabel>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionLabel {
    pub area: String,
    pub subarea: String,
    pub room: String,
    pub node: String,
}
