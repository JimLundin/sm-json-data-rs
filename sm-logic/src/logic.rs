//! The logic language: what Samus must have, pay, and do.

use crate::*;

/// A non-negative quantity, possibly depending on tunable numeric parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Quantity {
    Const(u32),
    Param(NumericId),
    Sum(Vec<Quantity>),
    Product(Vec<Quantity>),
}

impl Quantity {
    /// Evaluates the quantity, returning `None` if `param` has no value or on overflow.
    pub fn eval(&self, param: &impl Fn(NumericId) -> Option<i64>) -> Option<i64> {
        match self {
            Quantity::Const(value) => Some(i64::from(*value)),
            Quantity::Param(id) => param(*id),
            Quantity::Sum(terms) => terms.iter().try_fold(0i64, |sum, term| sum.checked_add(term.eval(param)?)),
            Quantity::Product(terms) => terms.iter().try_fold(1i64, |product, term| product.checked_mul(term.eval(param)?)),
        }
    }
}

/// One condition, cost or effect. Treat lists of these as ordered, because costs
/// and refills can depend on the sequence.
#[derive(Clone, Debug, PartialEq)]
pub enum Requirement {
    Free,
    Never,
    /// All of these, in order.
    All(Vec<Requirement>),
    /// Any one of these.
    Any(Vec<Requirement>),

    Item(Item),
    Flag(FlagId),
    NotFlag(FlagId),
    /// A tech and everything it inherits.
    Tech(TechId),
    /// A tech's dependencies on other tech, without its item or ammo requirements.
    TechOnly(TechId),
    Helper(HelperId),
    /// A technique specific to the room this requirement appears in.
    Notable(NotableId),
    /// Defined by the randomizer's code rather than by any data file.
    Internal(InternalId),
    DisableEquipment(Item),

    Ammo { kind: AmmoKind, count: Quantity, usage: AmmoUsage },
    Refill(Vec<ResourceKind>),
    PartialRefill { kind: ResourceKind, limit: Quantity },
    Resource { check: ResourceCheck, amounts: Vec<ResourceAmount> },
    AutoReserveTrigger { min_energy: Quantity, max_energy: Quantity, implicit_heat: ImplicitHeatFrames },

    Kill(EnemyKill),
    RidleyKill { power_bombs: bool, g_mode: bool, stuck: Option<RidleyStuck> },
    EnemyHits { attack: AttackRef, hits: Quantity },
    Hazard { hazard: Hazard, amount: Quantity },
    Heat { frames: Quantity, mode: HeatMode },
    Lava { frames: Quantity, mode: LavaMode },
    Acid { frames: Quantity, gravity: GravityRule },
    CycleFrames { frames: Quantity, simple: bool },

    ShineCharge(Runway),
    BlueSpeed(Runway),
    SpeedBall(Runway),
    ShineChargeFrames(Quantity),
    Shinespark { frames: u32, excess_frames: u32 },
    FlashSuit(FlashSuitAction),
    BlueSuit(BlueSuitAction),

    DoorUnlocked(NodeId),
    ItemCollected(NodeId),
    ItemNotCollected(NodeId),
    ObstaclesCleared(Vec<ObstacleId>),
    ObstaclesNotCleared(Vec<ObstacleId>),
    /// Must have entered the room from one of these nodes.
    EnteredFrom(Vec<NodeId>),
}

impl Requirement {
    /// Calls `visit` on this requirement and everything nested inside it.
    pub fn walk(&self, visit: &mut impl FnMut(&Requirement)) {
        visit(self);
        if let Requirement::All(children) | Requirement::Any(children) = self {
            for child in children {
                child.walk(visit);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AmmoUsage {
    /// Fails if the ammo isn't available.
    Spend,
    /// Always succeeds, draining whatever ammo there is.
    Drain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceCheck {
    /// At least this much is available now.
    Available,
    /// Spends this much.
    Consumed,
    /// Reduces the resource to at most this much.
    AtMost,
    /// Missing at most this much from full.
    MissingAtMost,
    /// Maximum capacity is at least this much.
    Capacity,
    /// Maximum capacity is at most this much.
    MaxCapacity,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceAmount {
    pub kind: ResourceKind,
    pub count: Quantity,
}

/// Damage sources measured by a simple count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hazard {
    SpikeHits,
    ThornHits,
    ElectricityHits,
    HibashiHits,
    SamusEaterCycles,
    MetroidFrames,
    DraygonElectricityFrames,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HeatMode {
    Standard,
    /// A simple action, so no extra leniency applies.
    Simple,
    /// With Varia and Gravity Suit turned off.
    Suitless,
    /// With Gravity Suit turned off.
    Gravityless,
    /// Damage is offset by collecting these drops.
    WithDrops(Vec<EnemyDrop>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LavaMode {
    Standard,
    /// With Gravity Suit turned off.
    Gravityless,
    /// Damage is offset by collecting these drops.
    WithDrops(Vec<EnemyDrop>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GravityRule {
    Standard,
    /// With Gravity Suit turned off.
    Gravityless,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlashSuitAction {
    Gain,
    Use,
    Forbid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlueSuitAction {
    Gain,
    Have,
    Shinecharge,
    Forbid,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnemyKill {
    /// Groups of enemies; each group can be hit by one area-of-effect attack.
    pub groups: Vec<Vec<EnemyId>>,
    pub weapons: WeaponFilter,
    pub farmable_ammo: Vec<AmmoKind>,
}

/// Which weapons are allowed for a fight.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum WeaponFilter {
    /// Every non-situational weapon that can damage the target.
    #[default]
    Default,
    Only(Vec<WeaponSelector>),
    Except(Vec<WeaponSelector>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponSelector {
    Weapon(WeaponId),
    Category(WeaponCategory),
}

/// A specific attack of a specific enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttackRef {
    pub enemy: EnemyId,
    /// Position in [`Enemy::attacks`].
    pub attack: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnemyDrop {
    pub enemy: EnemyId,
    pub count: u32,
}

/// The shape of a stretch of floor Samus can run on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Runway {
    /// Usable length; for speed-gaining requirements, the tiles actually used.
    pub length: Tiles,
    /// How many ends drop off rather than meeting a wall, from 0 to 2.
    pub open_ends: u8,
    pub slopes: RunwaySlopes,
}

/// Tile counts for sloped parts of a runway. All are zero on a flat runway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RunwaySlopes {
    pub gentle_up: u32,
    pub gentle_down: u32,
    pub steep_up: u32,
    pub steep_down: u32,
    /// Steep tiles at the start, which prevent a stutter.
    pub starting_steep_down: u32,
    /// Gentle tiles at the start, which make a stutter harder.
    pub starting_gentle_down: u32,
    /// Flat tiles at the start that prevent a stutter.
    pub starting_stutterless: u32,
}
