//! Strats and the conditions that carry Samus's state through a door.

use crate::*;

/// A strat that moves Samus between two nodes in a room.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomStrat {
    /// Upstream's ID, which is assigned some time after a strat is added.
    pub source_id: Option<source::StratId>,
    pub from: NodeId,
    pub to: NodeId,
    pub strat: Strat,
}

/// A series of maneuvers. Locks and viewable nodes use this without a route.
#[derive(Clone, Debug, PartialEq)]
pub struct Strat {
    /// Descriptive only, and absent when a pack is loaded without docs.
    pub name: Option<String>,
    pub entry: Option<DoorEntry>,
    pub requires: Vec<Requirement>,
    pub exit: Option<DoorExit>,
    pub starts_shinecharged: bool,
    pub ends_shinecharged: bool,
    /// After entering in immobile G-mode, this strat regains mobility.
    pub regains_g_mode_mobility: bool,
    pub door_shell: DoorShellBypass,
    pub door_unlocks: Vec<DoorUnlock>,
    pub clears_obstacles: Vec<ObstacleId>,
    pub resets_obstacles: Vec<ObstacleId>,
    pub collects_items: Vec<NodeId>,
    pub sets_flags: Vec<FlagId>,
    pub farm_drops: Vec<EnemyDrop>,
    /// Only useful if wall jumping is somehow unavailable.
    pub wall_jump_avoid: bool,
    pub failures: Vec<StratFailure>,
    pub review: SuitReview,
    pub doc: Doc,
}

/// Whether authors have checked a strat's logic for carrying each suit effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuitReview {
    pub flash_suit: bool,
    pub blue_suit: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DoorShellBypass {
    #[default]
    No,
    Yes,
    Free,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoorUnlock {
    /// The door to unlock; the strat's destination when absent.
    pub node: Option<NodeId>,
    pub types: Vec<DoorUnlockType>,
    pub requires: Vec<Requirement>,
    pub use_implicit_requires: bool,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StratFailure {
    pub name: Option<String>,
    pub outcome: FailureOutcome,
    pub cost: Vec<Requirement>,
    pub doc: Doc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureOutcome {
    /// Samus stays on the strat's route.
    Continues,
    LeadsTo(NodeId),
    Softlock,
}

/// A minimum and optional maximum run speed, in effective runway tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRange {
    pub min: Tiles,
    pub max: Option<Tiles>,
}

/// Optional bounds on extra run speed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpeedRange {
    pub min: Option<RunSpeed>,
    pub max: Option<RunSpeed>,
}

/// Runways in the previous room that make a run-up possible. At least one is set.
#[derive(Clone, Debug, PartialEq)]
pub struct RunUp {
    /// Minimum runway directly in front of the door.
    pub adjacent_min: Option<Tiles>,
    /// Possible minimum `(remote, landing)` runway pairs.
    pub remote_and_landing_min: Vec<(Tiles, Tiles)>,
}

/// A tile position: `[x, y]`.
pub type TilePosition = [i32; 2];

#[derive(Clone, Debug, PartialEq)]
pub struct SidePlatform {
    pub min_height: Tiles,
    pub max_height: Tiles,
    pub min_tiles: Tiles,
    pub speed_booster: TriState,
    pub obstructions: Vec<TilePosition>,
    pub environment: Environment,
    pub requires: Vec<Requirement>,
    pub doc: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrappleSwingBlock {
    pub position: [Tiles; 2],
    pub environment: Environment,
    pub obstructions: Vec<TilePosition>,
    pub doc: Doc,
}

/// How a strat must be entered from the previous room.
#[derive(Clone, Debug, PartialEq)]
pub struct DoorEntry {
    pub kind: EntryKind,
    /// Whether this applies when the Toilet is between the two rooms.
    pub through_toilet: TriState,
    /// Whether this applies when entering from a heated environment without protection.
    pub heated: TriState,
    pub dev_note: Option<Note>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Normal,
    Running { speed_booster: TriState, speed: TileRange },
    Jumping { speed_booster: TriState, speed: TileRange },
    SpaceJumping { speed_booster: TriState, speed: TileRange },
    BlueSpaceJumping { extra_speed: SpeedRange },
    Shinecharging { runway: Runway },
    GettingBlueSpeed { runway: Runway, extra_speed: SpeedRange },
    Shinecharged,
    ShinechargedJumping,
    Spark { position: Option<SparkPosition> },
    StutterShinecharging { min_tiles: Tiles },
    StutterGettingBlueSpeed { min_tiles: Tiles },
    BombBoost,
    DoorStuckSetup,
    Speedballing { runway: Runway, extra_speed: SpeedRange },
    TemporaryBlue { direction: Option<SidePosition> },
    Spinning { speed_booster: TriState, unusable_tiles: Tiles, extra_speed: SpeedRange },
    BlueSpinning { unusable_tiles: Tiles, extra_speed: SpeedRange },
    Mockball { speed_booster: TriState, run_up: RunUp },
    SpringBallBounce { speed_booster: TriState, movement: MovementType, run_up: RunUp },
    BlueSpringBallBounce { movement: MovementType, extra_speed: SpeedRange, min_landing_tiles: Option<Tiles> },
    StoredFallSpeed { tiles: u32 },
    Knockback,
    DamageBoost { i_frames: u32, position: DamageBoostEntry },
    IFrames { i_frames: u32 },
    RMode,
    GMode { mode: GModeMode, morphed: bool, mobility: GModeMobility },
    XMode,
    WallJumpBelow { min_height: Option<Tiles> },
    SpaceJumpBelow,
    PlatformBelow { min_height: Option<Tiles>, max_height: Option<Tiles>, max_left: Option<Tiles>, min_right: Option<Tiles> },
    SidePlatform { platforms: Vec<SidePlatform> },
    GrappleSwing { blocks: Vec<GrappleSwingBlock> },
    GrappleJump { position: SidePosition },
    GrappleTeleport { blocks: Vec<TilePosition> },
    SamusEaterTeleport { floor: Vec<TilePosition>, ceiling: Vec<TilePosition> },
    SuperSink,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DamageBoostEntry {
    High,
    #[default]
    Any,
}

/// How Samus leaves through a door, matched against the next room's [`DoorEntry`].
#[derive(Clone, Debug, PartialEq)]
pub enum DoorExit {
    Normal,
    Runway { runway: Runway, min_extra_speed: Option<RunSpeed>, heated: Option<bool> },
    Shinecharged,
    TemporaryBlue { direction: Option<SidePosition> },
    Spark { position: Option<SparkPosition>, grounded: Option<bool> },
    Spinning { remote: Runway, extra_speed: SpeedRange, blue: TriState },
    Mockball { remote: Runway, landing: Runway, extra_speed: SpeedRange, blue: TriState },
    SpringBallBounce { remote: Runway, landing: Runway, movement: MovementType, extra_speed: SpeedRange, blue: TriState },
    SpaceJumping { remote: Runway, extra_speed: SpeedRange, blue: TriState },
    StoredFallSpeed { tiles: u32 },
    Damage { attack: AttackRef, knockback: bool },
    Knockback,
    DamageBoost { i_frames_remaining: u32, position: DamageBoostExit },
    IFrames { i_frames_remaining: u32 },
    XModeSetup { attack: Option<AttackRef> },
    GMode { morphed: bool },
    DoorFrameBelow { height: Tiles },
    PlatformBelow { height: Tiles, left: Tiles, right: Tiles },
    SidePlatform { height: Tiles, runway: Runway, obstruction: TilePosition },
    GrappleSwing { blocks: Vec<GrappleSwingBlock> },
    GrappleJump { position: SidePosition },
    GrappleTeleport { blocks: Vec<TilePosition> },
    SamusEaterTeleport { floor: Vec<TilePosition>, ceiling: Vec<TilePosition> },
    SuperSink,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DamageBoostExit {
    #[default]
    NotHigh,
    High,
}
