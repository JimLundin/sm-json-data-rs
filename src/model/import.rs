//! Builds a [`World`] from the parsed JSON, resolving and validating every reference.

use super::*;
use crate::raw;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

type Res<T> = Result<T, String>;

/// One problem found while importing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelError {
    /// Where the problem is, such as `room Landing Site / strat Base`.
    pub location: String,
    pub problem: String,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.problem)
    }
}

/// Every problem found while importing, rather than only the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportErrors(pub Vec<ModelError>);

impl fmt::Display for ImportErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} import errors:", self.0.len())?;
        for error in &self.0 {
            writeln!(f, "  {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ImportErrors {}

/// A failure to load a checkout into a [`World`].
#[derive(Debug)]
pub enum WorldError {
    Load(raw::LoadError),
    Import(ImportErrors),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Load(error) => error.fmt(f),
            WorldError::Import(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WorldError {}

impl World {
    /// Loads and imports an `sm-json-data` checkout.
    pub fn load(root: impl AsRef<Path>) -> Result<World, WorldError> {
        let data = raw::GameData::load(root).map_err(WorldError::Load)?;
        World::import(&data).map_err(WorldError::Import)
    }

    /// Builds a world from parsed data, reporting every invalid reference.
    pub fn import(data: &raw::GameData) -> Result<World, ImportErrors> {
        Importer::new(data).run()
    }
}

fn dense(index: usize, what: &str, errors: &RefCell<Vec<ModelError>>) -> u16 {
    u16::try_from(index).unwrap_or_else(|_| {
        errors.borrow_mut().push(ModelError { location: what.to_owned(), problem: "too many entries".to_owned() });
        0
    })
}

fn doc(note: &Option<Note>, detail: &Option<Note>, dev: &Option<Note>) -> Doc {
    Doc { note: note.clone(), detail: detail.clone(), dev: dev.clone() }
}

fn slice<T>(list: &Option<Vec<T>>) -> &[T] {
    list.as_deref().unwrap_or_default()
}

fn implicit(value: Option<bool>) -> Implicit {
    match value {
        None => Implicit::Automatic,
        Some(true) => Implicit::On,
        Some(false) => Implicit::Off,
    }
}

fn speed_range(min: &Option<ExtraRunSpeed>, max: &Option<ExtraRunSpeed>) -> SpeedRange {
    SpeedRange { min: min.clone(), max: max.clone() }
}

fn open_ends(value: u8) -> Res<u8> {
    if value <= 2 { Ok(value) } else { Err(format!("runway has {value} open ends")) }
}

fn runway(r: &raw::Runway) -> Res<Runway> {
    let raw::Runway {
        length,
        open_end,
        gentle_up_tiles,
        gentle_down_tiles,
        steep_up_tiles,
        steep_down_tiles,
        starting_steep_down_tiles,
        starting_gentle_down_tiles,
        starting_stutterless_tiles,
    } = r;
    Ok(Runway {
        length: *length,
        open_ends: open_ends(*open_end)?,
        slopes: RunwaySlopes {
            gentle_up: gentle_up_tiles.unwrap_or(0),
            gentle_down: gentle_down_tiles.unwrap_or(0),
            steep_up: steep_up_tiles.unwrap_or(0),
            steep_down: steep_down_tiles.unwrap_or(0),
            starting_steep_down: starting_steep_down_tiles.unwrap_or(0),
            starting_gentle_down: starting_gentle_down_tiles.unwrap_or(0),
            starting_stutterless: starting_stutterless_tiles.unwrap_or(0),
        },
    })
}

fn charge_runway(r: &raw::ShineChargeRunway) -> Res<Runway> {
    let raw::ShineChargeRunway {
        used_tiles,
        open_end,
        gentle_up_tiles,
        gentle_down_tiles,
        steep_up_tiles,
        steep_down_tiles,
        starting_steep_down_tiles,
        starting_gentle_down_tiles,
        starting_stutterless_tiles,
    } = r;
    Ok(Runway {
        length: *used_tiles,
        open_ends: open_ends(*open_end)?,
        slopes: RunwaySlopes {
            gentle_up: gentle_up_tiles.unwrap_or(0),
            gentle_down: gentle_down_tiles.unwrap_or(0),
            steep_up: steep_up_tiles.unwrap_or(0),
            steep_down: steep_down_tiles.unwrap_or(0),
            starting_steep_down: starting_steep_down_tiles.unwrap_or(0),
            starting_gentle_down: starting_gentle_down_tiles.unwrap_or(0),
            starting_stutterless: starting_stutterless_tiles.unwrap_or(0),
        },
    })
}

fn run_up(adjacent: &Option<f64>, remote: &Option<Vec<[f64; 2]>>) -> Res<RunUp> {
    let remote_and_landing_min: Vec<_> = slice(remote).iter().map(|[a, b]| (*a, *b)).collect();
    if adjacent.is_none() && remote_and_landing_min.is_empty() {
        return Err("run-up has neither an adjacent nor a remote runway".to_owned());
    }
    Ok(RunUp { adjacent_min: *adjacent, remote_and_landing_min })
}

fn reject(fields: &[(bool, &str)]) -> Res<()> {
    match fields.iter().find(|(present, _)| *present) {
        Some((_, name)) => Err(format!("`{name}` does not apply to this kind of node")),
        None => Ok(()),
    }
}

/// The names that are valid inside one room.
struct RoomScope {
    nodes: HashSet<NodeId>,
    obstacles: HashMap<String, ObstacleId>,
    notables: HashMap<String, NotableId>,
}

impl RoomScope {
    fn new(room: &raw::Room) -> Res<Self> {
        let mut obstacles = HashMap::new();
        for (index, obstacle) in slice(&room.obstacles).iter().enumerate() {
            let id = u8::try_from(index).map_err(|_| "too many obstacles".to_owned())?;
            obstacles.insert(obstacle.id.as_str().to_owned(), ObstacleId(id));
        }
        Ok(Self {
            nodes: room.nodes.iter().map(|node| node.id).collect(),
            obstacles,
            notables: slice(&room.notables).iter().map(|n| (n.name.clone(), n.id)).collect(),
        })
    }
}

struct Importer<'a> {
    data: &'a raw::GameData,
    lookup: Lookup,
    attacks: Vec<HashMap<String, u8>>,
    rooms_by_source: HashMap<raw::RoomId, RoomId>,
    internals: RefCell<(Vec<String>, HashMap<String, InternalId>)>,
    errors: RefCell<Vec<ModelError>>,
}

fn name_table<'n, Id>(
    names: impl Iterator<Item = &'n str>,
    what: &str,
    make: impl Fn(u16) -> Id,
    errors: &RefCell<Vec<ModelError>>,
) -> HashMap<String, Id> {
    let mut table = HashMap::new();
    for (index, name) in names.enumerate() {
        let id = make(dense(index, what, errors));
        if table.insert(name.to_owned(), id).is_some() {
            errors.borrow_mut().push(ModelError { location: what.to_owned(), problem: format!("duplicate name `{name}`") });
        }
    }
    table
}

impl<'a> Importer<'a> {
    fn new(data: &'a raw::GameData) -> Self {
        let errors = RefCell::new(Vec::new());
        let all_enemies = || data.enemies.enemies.iter().chain(&data.bosses.enemies);
        let lookup = Lookup {
            rooms: name_table(data.rooms.iter().map(|r| r.name.as_str()), "rooms", RoomId, &errors),
            techs: name_table(data.tech.all_techs().map(|t| t.name.as_str()), "tech", TechId, &errors),
            helpers: name_table(data.helpers.all_helpers().map(|h| h.name.as_str()), "helpers", HelperId, &errors),
            numerics: name_table(data.numerics.all_numerics().map(|n| n.name.as_str()), "numerics", NumericId, &errors),
            flags: name_table(data.items.game_flags.iter().map(|f| f.as_str()), "flags", FlagId, &errors),
            weapons: name_table(data.weapons.weapons.iter().map(|w| w.name.as_str()), "weapons", WeaponId, &errors),
            enemies: name_table(all_enemies().map(|e| e.name.as_str()), "enemies", EnemyId, &errors),
        };
        let attacks = all_enemies()
            .map(|enemy| {
                let attacks = enemy.attacks.iter().enumerate();
                attacks.filter_map(|(i, a)| Some((a.name.clone(), u8::try_from(i).ok()?))).collect()
            })
            .collect();
        let rooms_by_source = data
            .rooms
            .iter()
            .enumerate()
            .map(|(index, room)| (room.id, RoomId(dense(index, "rooms", &errors))))
            .collect();
        Self { data, lookup, attacks, rooms_by_source, internals: RefCell::default(), errors }
    }

    /// Converts each element, recording failures so that all of them get reported.
    fn each<T, U>(&self, items: &[T], label: impl Fn(&T) -> String, f: impl Fn(&T) -> Res<U>) -> Vec<U> {
        items
            .iter()
            .filter_map(|item| match f(item) {
                Ok(value) => Some(value),
                Err(problem) => {
                    self.errors.borrow_mut().push(ModelError { location: label(item), problem });
                    None
                }
            })
            .collect()
    }

    fn flag(&self, name: &raw::FlagName) -> Res<FlagId> {
        self.lookup.flags.get(name.as_str()).copied().ok_or_else(|| format!("unknown flag `{name}`"))
    }

    fn flags(&self, names: &Option<Vec<raw::FlagName>>) -> Res<Vec<FlagId>> {
        slice(names).iter().map(|name| self.flag(name)).collect()
    }

    fn tech(&self, name: &raw::TechName) -> Res<TechId> {
        self.lookup.techs.get(name.as_str()).copied().ok_or_else(|| format!("unknown tech `{name}`"))
    }

    fn enemy(&self, name: &str) -> Res<EnemyId> {
        self.lookup.enemies.get(name).copied().ok_or_else(|| format!("unknown enemy `{name}`"))
    }

    fn attack(&self, enemy: &str, attack: &str) -> Res<AttackRef> {
        let id = self.enemy(enemy)?;
        let attack = self.attacks[id.index()].get(attack).copied();
        let attack = attack.ok_or_else(|| format!("enemy `{enemy}` has no attack named `{attack:?}`"))?;
        Ok(AttackRef { enemy: id, attack })
    }

    fn drops(&self, drops: &[raw::EnemyDrop]) -> Res<Vec<EnemyDrop>> {
        drops.iter().map(|d| Ok(EnemyDrop { enemy: self.enemy(&d.enemy)?, count: d.count })).collect()
    }

    fn item(&self, item: Item) -> Res<Item> {
        let items = &self.data.items;
        let known = items.implicit_items.contains(&item)
            || items.upgrade_items.iter().any(|u| u.name == item)
            || items.expansion_items.iter().any(|e| e.name == item);
        if known { Ok(item) } else { Err(format!("item `{}` is not defined by this dataset", item.as_str())) }
    }

    fn selector(&self, name: &str) -> Res<WeaponSelector> {
        if let Some(id) = self.lookup.weapons.get(name) {
            return Ok(WeaponSelector::Weapon(*id));
        }
        serde_json::from_value(serde_json::Value::String(name.to_owned()))
            .map(WeaponSelector::Category)
            .map_err(|_| format!("unknown weapon or category `{name}`"))
    }

    fn selectors(&self, names: &[String]) -> Res<Vec<WeaponSelector>> {
        names.iter().map(|name| self.selector(name)).collect()
    }

    fn weapon_filter(&self, only: &Option<Vec<String>>, except: &Option<Vec<String>>) -> Res<WeaponFilter> {
        match (only, except) {
            (None, None) => Ok(WeaponFilter::Default),
            (Some(only), None) => Ok(WeaponFilter::Only(self.selectors(only)?)),
            (None, Some(except)) => Ok(WeaponFilter::Except(self.selectors(except)?)),
            (Some(_), Some(_)) => Err("both explicit and excluded weapons are set".to_owned()),
        }
    }

    fn quantity(&self, numeric: &raw::Numeric) -> Res<Quantity> {
        let list = |items: &[raw::Numeric]| items.iter().map(|n| self.quantity(n)).collect::<Res<Vec<_>>>();
        Ok(match numeric {
            raw::Numeric::Literal(value) => Quantity::Const(*value),
            raw::Numeric::Parameter(name) => Quantity::Param(
                self.lookup.numerics.get(name.as_str()).copied().ok_or_else(|| format!("unknown numeric `{name}`"))?,
            ),
            raw::Numeric::Add(terms) => Quantity::Sum(list(terms)?),
            raw::Numeric::Mul(terms) => Quantity::Product(list(terms)?),
        })
    }

    fn node(&self, scope: Option<&RoomScope>, id: NodeId) -> Res<NodeId> {
        let scope = scope.ok_or_else(|| format!("node {id} is referenced outside a room"))?;
        if scope.nodes.contains(&id) { Ok(id) } else { Err(format!("this room has no node {id}")) }
    }

    fn nodes(&self, scope: Option<&RoomScope>, ids: &[NodeId]) -> Res<Vec<NodeId>> {
        ids.iter().map(|id| self.node(scope, *id)).collect()
    }

    fn obstacles(&self, scope: Option<&RoomScope>, ids: &[raw::ObstacleId]) -> Res<Vec<ObstacleId>> {
        let scope = scope.ok_or("obstacle referenced outside a room")?;
        ids.iter()
            .map(|id| scope.obstacles.get(id.as_str()).copied().ok_or_else(|| format!("this room has no obstacle `{id}`")))
            .collect()
    }

    fn internal(&self, name: &str) -> InternalId {
        let mut internals = self.internals.borrow_mut();
        let (names, ids) = &mut *internals;
        if let Some(id) = ids.get(name) {
            return *id;
        }
        let id = InternalId(dense(names.len(), "internal requirements", &self.errors));
        names.push(name.to_owned());
        ids.insert(name.to_owned(), id);
        id
    }

    fn named(&self, named: &raw::Named) -> Res<Requirement> {
        Ok(match named {
            raw::Named::Free => Requirement::Free,
            raw::Named::Never => Requirement::Never,
            raw::Named::Item(item) => Requirement::Item(self.item(*item)?),
            raw::Named::Flag(name) => Requirement::Flag(self.flag(name)?),
            raw::Named::Helper(name) => Requirement::Helper(
                self.lookup.helpers.get(name.as_str()).copied().ok_or_else(|| format!("unknown helper `{name}`"))?,
            ),
            raw::Named::Tech(name) => Requirement::Tech(self.tech(name)?),
            raw::Named::Internal(name) => Requirement::Internal(self.internal(name.as_str())),
        })
    }

    fn resources(&self, check: ResourceCheck, amounts: &[raw::ResourceAmount]) -> Res<Requirement> {
        let amounts = amounts.iter().map(|a| Ok(ResourceAmount { kind: a.kind, count: self.quantity(&a.count)? }));
        Ok(Requirement::Resource { check, amounts: amounts.collect::<Res<_>>()? })
    }

    fn requires(&self, list: &[raw::Requirement], scope: Option<&RoomScope>) -> Res<Vec<Requirement>> {
        list.iter().map(|r| self.requirement(r, scope)).collect()
    }

    fn opt_requires(&self, list: &Option<Vec<raw::Requirement>>, scope: Option<&RoomScope>) -> Res<Option<Vec<Requirement>>> {
        list.as_ref().map(|list| self.requires(list, scope)).transpose()
    }

    fn requirement(&self, r: &raw::Requirement, scope: Option<&RoomScope>) -> Res<Requirement> {
        use raw::Requirement as R;
        use Requirement as M;
        let hazard = |hazard, amount: &raw::Numeric| -> Res<M> { Ok(M::Hazard { hazard, amount: self.quantity(amount)? }) };
        let heat = |frames: &raw::Numeric, mode| -> Res<M> { Ok(M::Heat { frames: self.quantity(frames)?, mode }) };
        Ok(match r {
            R::Named(named) => self.named(named)?,
            R::And(list) => M::All(self.requires(list, scope)?),
            R::Or(list) => M::Any(self.requires(list, scope)?),
            R::Not(named) => match named {
                raw::Named::Flag(flag) => M::NotFlag(self.flag(flag)?),
                other => return Err(format!("`not` applied to `{}`, which is not a flag", String::from(other.clone()))),
            },
            R::Ammo(a) => M::Ammo { kind: a.kind, count: self.quantity(&a.count)?, usage: AmmoUsage::Spend },
            R::AmmoDrain(a) => M::Ammo { kind: a.kind, count: self.quantity(&a.count)?, usage: AmmoUsage::Drain },
            R::Refill(kinds) => M::Refill(kinds.clone()),
            R::PartialRefill(p) => M::PartialRefill { kind: p.kind, limit: self.quantity(&p.limit)? },
            R::EnemyKill(raw::EnemyKill { enemies, explicit_weapons, excluded_weapons, farmable_ammo }) => M::Kill(EnemyKill {
                groups: enemies
                    .iter()
                    .map(|group| group.iter().map(|name| self.enemy(name)).collect())
                    .collect::<Res<_>>()?,
                weapons: self.weapon_filter(explicit_weapons, excluded_weapons)?,
                farmable_ammo: slice(farmable_ammo).to_vec(),
            }),
            R::EnemyDamage(raw::EnemyDamage { enemy, attack, hits }) => {
                M::EnemyHits { attack: self.attack(enemy, attack)?, hits: self.quantity(hits)? }
            }
            R::AcidFrames(n) => M::Acid { frames: self.quantity(n)?, gravity: GravityRule::Standard },
            R::GravitylessAcidFrames(n) => M::Acid { frames: self.quantity(n)?, gravity: GravityRule::Gravityless },
            R::DraygonElectricityFrames(n) => hazard(Hazard::DraygonElectricityFrames, n)?,
            R::ShineChargeFrames(n) => M::ShineChargeFrames(self.quantity(n)?),
            R::CycleFrames(n) => M::CycleFrames { frames: self.quantity(n)?, simple: false },
            R::SimpleCycleFrames(n) => M::CycleFrames { frames: self.quantity(n)?, simple: true },
            R::HeatFrames(n) => heat(n, HeatMode::Standard)?,
            R::SimpleHeatFrames(n) => heat(n, HeatMode::Simple)?,
            R::SuitlessHeatFrames(n) => heat(n, HeatMode::Suitless)?,
            R::GravitylessHeatFrames(n) => heat(n, HeatMode::Gravityless)?,
            R::HeatFramesWithEnergyDrops(raw::FramesWithDrops { frames, drops }) => {
                heat(frames, HeatMode::WithDrops(self.drops(drops)?))?
            }
            R::LavaFrames(n) => M::Lava { frames: self.quantity(n)?, mode: LavaMode::Standard },
            R::GravitylessLavaFrames(n) => M::Lava { frames: self.quantity(n)?, mode: LavaMode::Gravityless },
            R::LavaFramesWithEnergyDrops(raw::FramesWithDrops { frames, drops }) => {
                M::Lava { frames: self.quantity(frames)?, mode: LavaMode::WithDrops(self.drops(drops)?) }
            }
            R::SamusEaterCycles(n) => hazard(Hazard::SamusEaterCycles, n)?,
            R::MetroidFrames(n) => hazard(Hazard::MetroidFrames, n)?,
            R::HibashiHits(n) => hazard(Hazard::HibashiHits, n)?,
            R::SpikeHits(n) => hazard(Hazard::SpikeHits, n)?,
            R::ThornHits(n) => hazard(Hazard::ThornHits, n)?,
            R::ElectricityHits(n) => hazard(Hazard::ElectricityHits, n)?,
            R::ResourceAtMost(a) => self.resources(ResourceCheck::AtMost, a)?,
            R::ResourceCapacity(a) => self.resources(ResourceCheck::Capacity, a)?,
            R::ResourceMaxCapacity(a) => self.resources(ResourceCheck::MaxCapacity, a)?,
            R::ResourceAvailable(a) => self.resources(ResourceCheck::Available, a)?,
            R::ResourceConsumed(a) => self.resources(ResourceCheck::Consumed, a)?,
            R::ResourceMissingAtMost(a) => self.resources(ResourceCheck::MissingAtMost, a)?,
            R::AutoReserveTrigger(raw::AutoReserveTrigger { min_reserve_energy, max_reserve_energy, implicit_heat_frames }) => {
                let energy = |value: &Option<raw::Numeric>, default| {
                    value.as_ref().map_or(Ok(Quantity::Const(default)), |n| self.quantity(n))
                };
                M::AutoReserveTrigger {
                    min_energy: energy(min_reserve_energy, 1)?,
                    max_energy: energy(max_reserve_energy, 400)?,
                    implicit_heat: implicit_heat_frames.unwrap_or(ImplicitHeatFrames::Yes),
                }
            }
            R::DoorUnlockedAtNode(id) => M::DoorUnlocked(self.node(scope, *id)?),
            R::ObstaclesCleared(ids) => M::ObstaclesCleared(self.obstacles(scope, ids)?),
            R::ObstaclesNotCleared(ids) => M::ObstaclesNotCleared(self.obstacles(scope, ids)?),
            R::CanShineCharge(r) => M::ShineCharge(charge_runway(r)?),
            R::GetBlueSpeed(r) => M::BlueSpeed(charge_runway(r)?),
            R::SpeedBall(r) => M::SpeedBall(runway(r)?),
            R::Shinespark(raw::Shinespark { frames, excess_frames }) => {
                M::Shinespark { frames: *frames, excess_frames: excess_frames.unwrap_or(0) }
            }
            R::ResetRoom(raw::ResetRoom { nodes }) => M::EnteredFrom(self.nodes(scope, nodes)?),
            R::ItemNotCollectedAtNode(id) => M::ItemNotCollected(self.node(scope, *id)?),
            R::ItemCollectedAtNode(id) => M::ItemCollected(self.node(scope, *id)?),
            R::GainFlashSuit {} => M::FlashSuit(FlashSuitAction::Gain),
            R::UseFlashSuit {} => M::FlashSuit(FlashSuitAction::Use),
            R::NoFlashSuit {} => M::FlashSuit(FlashSuitAction::Forbid),
            R::GainBlueSuit {} => M::BlueSuit(BlueSuitAction::Gain),
            R::HaveBlueSuit {} => M::BlueSuit(BlueSuitAction::Have),
            R::BlueSuitShinecharge {} => M::BlueSuit(BlueSuitAction::Shinecharge),
            R::NoBlueSuit {} => M::BlueSuit(BlueSuitAction::Forbid),
            R::TechOnly(name) => M::TechOnly(self.tech(name)?),
            R::Notable(name) => {
                let scope = scope.ok_or("notable referenced outside a room")?;
                M::Notable(*scope.notables.get(name).ok_or_else(|| format!("this room has no notable `{name}`"))?)
            }
            R::DisableEquipment(item) => M::DisableEquipment(self.item(*item)?),
            R::RidleyKill(raw::RidleyKill { power_bombs, g_mode, stuck }) => M::RidleyKill {
                power_bombs: power_bombs.unwrap_or(true),
                g_mode: g_mode.unwrap_or(false),
                stuck: *stuck,
            },
        })
    }

    fn side_platform(&self, p: &raw::SidePlatform, scope: Option<&RoomScope>) -> Res<SidePlatform> {
        let raw::SidePlatform {
            min_height,
            max_height,
            min_tiles,
            speed_booster,
            obstructions,
            environment,
            requires,
            note,
            detail_note,
            dev_note,
        } = p;
        Ok(SidePlatform {
            min_height: *min_height,
            max_height: *max_height,
            min_tiles: *min_tiles,
            speed_booster: *speed_booster,
            obstructions: obstructions.clone(),
            environment: environment.unwrap_or(Environment::Any),
            requires: self.requires(slice(requires), scope)?,
            doc: doc(note, detail_note, dev_note),
        })
    }

    fn swing_blocks(blocks: &[raw::GrappleSwingBlock]) -> Vec<GrappleSwingBlock> {
        blocks
            .iter()
            .map(|raw::GrappleSwingBlock { position, environment, obstructions, note, dev_note }| GrappleSwingBlock {
                position: *position,
                environment: environment.unwrap_or(Environment::Air),
                obstructions: slice(obstructions).to_vec(),
                doc: doc(note, &None, dev_note),
            })
            .collect()
    }

    fn entry(&self, c: &raw::EntranceCondition, scope: Option<&RoomScope>) -> Res<DoorEntry> {
        use raw::EntranceKind as R;
        use EntryKind as M;
        let raw::EntranceCondition { kind, comes_through_toilet, comes_in_heated, dev_note } = c;
        let tiles = |min_tiles: &f64, max_tiles: &Option<f64>| TileRange { min: *min_tiles, max: *max_tiles };
        let kind = match kind {
            R::ComeInNormally {} => M::Normal,
            R::ComeInRunning { speed_booster, min_tiles, max_tiles } => {
                M::Running { speed_booster: *speed_booster, speed: tiles(min_tiles, max_tiles) }
            }
            R::ComeInJumping { speed_booster, min_tiles, max_tiles } => {
                M::Jumping { speed_booster: *speed_booster, speed: tiles(min_tiles, max_tiles) }
            }
            R::ComeInSpaceJumping { speed_booster, min_tiles, max_tiles } => {
                M::SpaceJumping { speed_booster: *speed_booster, speed: tiles(min_tiles, max_tiles) }
            }
            R::ComeInBlueSpaceJumping { min_extra_run_speed, max_extra_run_speed } => {
                M::BlueSpaceJumping { extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed) }
            }
            R::ComeInShinecharging(r) => M::Shinecharging { runway: runway(r)? },
            R::ComeInGettingBlueSpeed {
                length,
                open_end,
                gentle_up_tiles,
                gentle_down_tiles,
                steep_up_tiles,
                steep_down_tiles,
                min_extra_run_speed,
                max_extra_run_speed,
            } => M::GettingBlueSpeed {
                runway: Runway {
                    length: *length,
                    open_ends: open_ends(*open_end)?,
                    slopes: RunwaySlopes {
                        gentle_up: gentle_up_tiles.unwrap_or(0),
                        gentle_down: gentle_down_tiles.unwrap_or(0),
                        steep_up: steep_up_tiles.unwrap_or(0),
                        steep_down: steep_down_tiles.unwrap_or(0),
                        ..RunwaySlopes::default()
                    },
                },
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
            },
            R::ComeInShinecharged {} => M::Shinecharged,
            R::ComeInShinechargedJumping {} => M::ShinechargedJumping,
            R::ComeInWithSpark { position } => M::Spark { position: *position },
            R::ComeInStutterShinecharging { min_tiles } => M::StutterShinecharging { min_tiles: *min_tiles },
            R::ComeInStutterGettingBlueSpeed { min_tiles } => M::StutterGettingBlueSpeed { min_tiles: *min_tiles },
            R::ComeInWithBombBoost {} => M::BombBoost,
            R::ComeInWithDoorStuckSetup {} => M::DoorStuckSetup,
            R::ComeInSpeedballing { runway: r, min_extra_run_speed, max_extra_run_speed } => M::Speedballing {
                runway: runway(r)?,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
            },
            R::ComeInWithTemporaryBlue { direction } => M::TemporaryBlue { direction: *direction },
            R::ComeInSpinning { speed_booster, unusable_tiles, min_extra_run_speed, max_extra_run_speed } => M::Spinning {
                speed_booster: *speed_booster,
                unusable_tiles: *unusable_tiles,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
            },
            R::ComeInBlueSpinning { unusable_tiles, min_extra_run_speed, max_extra_run_speed } => M::BlueSpinning {
                unusable_tiles: *unusable_tiles,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
            },
            R::ComeInWithMockball { speed_booster, adjacent_min_tiles, remote_and_landing_min_tiles } => {
                M::Mockball { speed_booster: *speed_booster, run_up: run_up(adjacent_min_tiles, remote_and_landing_min_tiles)? }
            }
            R::ComeInWithSpringBallBounce { speed_booster, movement_type, adjacent_min_tiles, remote_and_landing_min_tiles } => {
                M::SpringBallBounce {
                    speed_booster: *speed_booster,
                    movement: *movement_type,
                    run_up: run_up(adjacent_min_tiles, remote_and_landing_min_tiles)?,
                }
            }
            R::ComeInWithBlueSpringBallBounce { movement_type, min_extra_run_speed, max_extra_run_speed, min_landing_tiles } => {
                M::BlueSpringBallBounce {
                    movement: *movement_type,
                    extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
                    min_landing_tiles: *min_landing_tiles,
                }
            }
            R::ComeInWithStoredFallSpeed { fall_speed_in_tiles } => M::StoredFallSpeed { tiles: *fall_speed_in_tiles },
            R::ComeInWithKnockback {} => M::Knockback,
            R::ComeInWithDamageBoost { i_frames_needed, position } => M::DamageBoost {
                i_frames: *i_frames_needed,
                position: match position {
                    Some(raw::DamageBoostEntry::High) => DamageBoostEntry::High,
                    Some(raw::DamageBoostEntry::Any) | None => DamageBoostEntry::Any,
                },
            },
            R::ComeInWithIFrames { i_frames_needed } => M::IFrames { i_frames: *i_frames_needed },
            R::ComeInWithRMode {} => M::RMode,
            R::ComeInWithGMode { mode, morphed, mobility } => {
                M::GMode { mode: *mode, morphed: *morphed, mobility: mobility.unwrap_or(GModeMobility::Any) }
            }
            R::ComeInWithXMode {} => M::XMode,
            R::ComeInWithWallJumpBelow { min_height } => M::WallJumpBelow { min_height: *min_height },
            R::ComeInWithSpaceJumpBelow {} => M::SpaceJumpBelow,
            R::ComeInWithPlatformBelow { min_height, max_height, max_left_position, min_right_position } => M::PlatformBelow {
                min_height: *min_height,
                max_height: *max_height,
                max_left: *max_left_position,
                min_right: *min_right_position,
            },
            R::ComeInWithSidePlatform { platforms } => M::SidePlatform {
                platforms: platforms.iter().map(|p| self.side_platform(p, scope)).collect::<Res<_>>()?,
            },
            R::ComeInWithGrappleSwing { blocks } => M::GrappleSwing { blocks: Self::swing_blocks(blocks) },
            R::ComeInWithGrappleJump { position } => M::GrappleJump { position: *position },
            R::ComeInWithGrappleTeleport { block_positions } => M::GrappleTeleport { blocks: block_positions.clone() },
            R::ComeInWithSamusEaterTeleport { floor_positions, ceiling_positions } => {
                M::SamusEaterTeleport { floor: floor_positions.clone(), ceiling: ceiling_positions.clone() }
            }
            R::ComeInWithSuperSink {} => M::SuperSink,
        };
        Ok(DoorEntry {
            kind,
            through_toilet: comes_through_toilet.unwrap_or(TriState::No),
            heated: comes_in_heated.unwrap_or(TriState::Any),
            dev_note: dev_note.clone(),
        })
    }

    fn exit(&self, c: &raw::ExitCondition) -> Res<DoorExit> {
        use raw::ExitCondition as R;
        use DoorExit as M;
        Ok(match c {
            R::LeaveNormally {} => M::Normal,
            R::LeaveWithRunway {
                length,
                open_end,
                gentle_up_tiles,
                gentle_down_tiles,
                steep_up_tiles,
                steep_down_tiles,
                starting_steep_down_tiles,
                starting_gentle_down_tiles,
                starting_stutterless_tiles,
                min_extra_run_speed,
                heated,
            } => M::Runway {
                runway: Runway {
                    length: *length,
                    open_ends: open_ends(*open_end)?,
                    slopes: RunwaySlopes {
                        gentle_up: gentle_up_tiles.unwrap_or(0),
                        gentle_down: gentle_down_tiles.unwrap_or(0),
                        steep_up: steep_up_tiles.unwrap_or(0),
                        steep_down: steep_down_tiles.unwrap_or(0),
                        starting_steep_down: starting_steep_down_tiles.unwrap_or(0),
                        starting_gentle_down: starting_gentle_down_tiles.unwrap_or(0),
                        starting_stutterless: starting_stutterless_tiles.unwrap_or(0),
                    },
                },
                min_extra_speed: min_extra_run_speed.clone(),
                heated: *heated,
            },
            R::LeaveShinecharged {} => M::Shinecharged,
            R::LeaveWithTemporaryBlue { direction } => M::TemporaryBlue { direction: *direction },
            R::LeaveWithSpark { position, grounded } => M::Spark { position: *position, grounded: *grounded },
            R::LeaveSpinning { remote_runway, min_extra_run_speed, max_extra_run_speed, blue } => M::Spinning {
                remote: runway(remote_runway)?,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
                blue: blue.unwrap_or(TriState::Any),
            },
            R::LeaveWithMockball { remote_runway, landing_runway, min_extra_run_speed, max_extra_run_speed, blue } => M::Mockball {
                remote: runway(remote_runway)?,
                landing: runway(landing_runway)?,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
                blue: blue.unwrap_or(TriState::Any),
            },
            R::LeaveWithSpringBallBounce {
                remote_runway,
                landing_runway,
                movement_type,
                min_extra_run_speed,
                max_extra_run_speed,
                blue,
            } => M::SpringBallBounce {
                remote: runway(remote_runway)?,
                landing: runway(landing_runway)?,
                movement: *movement_type,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
                blue: blue.unwrap_or(TriState::Any),
            },
            R::LeaveSpaceJumping { remote_runway, min_extra_run_speed, max_extra_run_speed, blue } => M::SpaceJumping {
                remote: runway(remote_runway)?,
                extra_speed: speed_range(min_extra_run_speed, max_extra_run_speed),
                blue: blue.unwrap_or(TriState::Any),
            },
            R::LeaveWithStoredFallSpeed { fall_speed_in_tiles } => M::StoredFallSpeed { tiles: *fall_speed_in_tiles },
            R::LeaveWithDamage { enemy, attack, knockback } => M::Damage {
                attack: self.attack(enemy, attack.as_deref().unwrap_or("contact"))?,
                knockback: knockback.unwrap_or(true),
            },
            R::LeaveWithKnockback {} => M::Knockback,
            R::LeaveWithDamageBoost { i_frames_remaining, position } => M::DamageBoost {
                i_frames_remaining: *i_frames_remaining,
                position: match position {
                    Some(raw::DamageBoostExit::High) => DamageBoostExit::High,
                    Some(raw::DamageBoostExit::NotHigh) | None => DamageBoostExit::NotHigh,
                },
            },
            R::LeaveWithIFrames { i_frames_remaining } => M::IFrames { i_frames_remaining: *i_frames_remaining },
            R::LeaveWithXModeSetup { enemy, attack } => M::XModeSetup {
                attack: match (enemy, attack) {
                    (Some(enemy), attack) => Some(self.attack(enemy, attack.as_deref().unwrap_or("contact"))?),
                    (None, None) => None,
                    (None, Some(attack)) => return Err(format!("X-mode attack `{attack}` has no enemy")),
                },
            },
            R::LeaveWithGMode { morphed } => M::GMode { morphed: *morphed },
            R::LeaveWithDoorFrameBelow { height } => M::DoorFrameBelow { height: *height },
            R::LeaveWithPlatformBelow { height, left_position, right_position } => {
                M::PlatformBelow { height: *height, left: *left_position, right: *right_position }
            }
            R::LeaveWithSidePlatform { height, runway: r, obstruction } => {
                M::SidePlatform { height: *height, runway: runway(r)?, obstruction: *obstruction }
            }
            R::LeaveWithGrappleSwing { blocks } => M::GrappleSwing { blocks: Self::swing_blocks(blocks) },
            R::LeaveWithGrappleJump { position } => M::GrappleJump { position: *position },
            R::LeaveWithGrappleTeleport { block_positions } => M::GrappleTeleport { blocks: block_positions.clone() },
            R::LeaveWithSamusEaterTeleport { floor_positions, ceiling_positions } => {
                M::SamusEaterTeleport { floor: floor_positions.clone(), ceiling: ceiling_positions.clone() }
            }
            R::LeaveWithSuperSink {} => M::SuperSink,
        })
    }

    fn strat(&self, s: &raw::Strat, scope: &RoomScope) -> Res<Strat> {
        let scope_ref = Some(scope);
        let raw::Strat {
            id: _,
            link: _,
            name,
            entrance_condition,
            starts_with_shine_charge,
            requires,
            exit_condition,
            g_mode_regain_mobility,
            bypasses_door_shell,
            unlocks_doors,
            clears_obstacles,
            resets_obstacles,
            collects_items,
            sets_flags,
            ends_with_shine_charge,
            farm_cycle_drops,
            wall_jump_avoid,
            flash_suit_checked,
            blue_suit_checked,
            failures,
            note,
            detail_note,
            dev_note,
        } = s;
        Ok(Strat {
            name: name.clone(),
            entry: entrance_condition.as_ref().map(|c| self.entry(c, scope_ref)).transpose()?,
            requires: self.requires(requires, scope_ref)?,
            exit: exit_condition.as_ref().map(|c| self.exit(c)).transpose()?,
            starts_shinecharged: starts_with_shine_charge.unwrap_or(false),
            ends_shinecharged: ends_with_shine_charge.unwrap_or(false),
            regains_g_mode_mobility: g_mode_regain_mobility.is_some(),
            door_shell: match bypasses_door_shell {
                None | Some(raw::DoorShellBypass::No) => DoorShellBypass::No,
                Some(raw::DoorShellBypass::Yes) => DoorShellBypass::Yes,
                Some(raw::DoorShellBypass::Free) => DoorShellBypass::Free,
            },
            door_unlocks: slice(unlocks_doors)
                .iter()
                .map(|raw::UnlockDoor { node_id, types, requires, use_implicit_requires, note, dev_note }| {
                    Ok(DoorUnlock {
                        node: node_id.map(|id| self.node(scope_ref, id)).transpose()?,
                        types: types.clone(),
                        requires: self.requires(slice(requires), scope_ref)?,
                        use_implicit_requires: use_implicit_requires.unwrap_or(true),
                        doc: doc(note, &None, dev_note),
                    })
                })
                .collect::<Res<_>>()?,
            clears_obstacles: self.obstacles(scope_ref, slice(clears_obstacles))?,
            resets_obstacles: self.obstacles(scope_ref, slice(resets_obstacles))?,
            collects_items: self.nodes(scope_ref, slice(collects_items))?,
            sets_flags: self.flags(sets_flags)?,
            farm_drops: self.drops(slice(farm_cycle_drops))?,
            wall_jump_avoid: wall_jump_avoid.unwrap_or(false),
            failures: slice(failures)
                .iter()
                .map(|raw::StratFailure { name, leads_to_node, cost, softlock, note, dev_note }| {
                    let outcome = match (leads_to_node, softlock.unwrap_or(false)) {
                        (None, false) => FailureOutcome::Continues,
                        (Some(node), false) => FailureOutcome::LeadsTo(self.node(scope_ref, *node)?),
                        (None, true) => FailureOutcome::Softlock,
                        (Some(_), true) => return Err(format!("failure `{name}` both softlocks and leads to a node")),
                    };
                    Ok(StratFailure {
                        name: name.clone(),
                        outcome,
                        cost: self.requires(slice(cost), scope_ref)?,
                        doc: doc(note, &None, dev_note),
                    })
                })
                .collect::<Res<_>>()?,
            review: SuitReview { flash_suit: *flash_suit_checked, blue_suit: *blue_suit_checked },
            doc: doc(note, detail_note, dev_note),
        })
    }

    fn nested_strat(&self, s: &raw::Strat, scope: &RoomScope) -> Res<Strat> {
        if s.id.is_some() || s.link.is_some() {
            return Err(format!("nested strat `{}` has an id or link", s.name));
        }
        self.strat(s, scope)
    }

    fn node_kind(&self, n: &raw::Node) -> Res<NodeKind> {
        use raw::{NodeSubType as S, NodeType as T};
        let raw::Node {
            id: _,
            name: _,
            node_type,
            node_sub_type,
            node_item,
            node_address,
            door_orientation,
            door_environments,
            use_implicit_leave_normally,
            use_implicit_come_in_normally,
            use_implicit_come_in_with_mockball,
            use_implicit_carry_g_mode_back_through,
            use_implicit_carry_g_mode_morph_back_through,
            use_implicit_come_in_with_grapple_jump,
            use_implicit_door_unlocks,
            is_door_immediately_closed,
            locks: _,
            twin_door_addresses,
            utility,
            viewable_nodes: _,
            yields: _,
            map_tile_mask: _,
            note: _,
            dev_note: _,
        } = n;
        let invalid = || format!("node type {node_type:?} cannot have subtype {node_sub_type:?}");
        let door_only = [
            (door_environments.is_some(), "doorEnvironments"),
            (use_implicit_leave_normally.is_some(), "useImplicitLeaveNormally"),
            (use_implicit_come_in_normally.is_some(), "useImplicitComeInNormally"),
            (use_implicit_come_in_with_mockball.is_some(), "useImplicitComeInWithMockball"),
            (use_implicit_carry_g_mode_back_through.is_some(), "useImplicitCarryGModeBackThrough"),
            (use_implicit_carry_g_mode_morph_back_through.is_some(), "useImplicitCarryGModeMorphBackThrough"),
            (use_implicit_come_in_with_grapple_jump.is_some(), "useImplicitComeInWithGrappleJump"),
            (use_implicit_door_unlocks.is_some(), "useImplicitDoorUnlocks"),
            (is_door_immediately_closed.is_some(), "isDoorImmediatelyClosed"),
            (twin_door_addresses.is_some(), "twinDoorAddresses"),
        ];
        let transit = || match node_sub_type {
            S::Elevator => Ok(Transit::Elevator),
            S::Sandpit => Ok(Transit::Sandpit),
            _ => Err(invalid()),
        };
        let orientation = || door_orientation.ok_or_else(|| "this node needs a doorOrientation".to_owned());
        if !matches!(node_type, T::Door) {
            reject(&door_only)?;
        }
        if !matches!(node_type, T::Item) {
            reject(&[(node_item.is_some(), "nodeItem")])?;
        }
        if !matches!(node_type, T::Utility) {
            reject(&[(utility.is_some(), "utility")])?;
        }
        Ok(match node_type {
            T::Door => NodeKind::Door(Door {
                style: match node_sub_type {
                    S::Blue => DoorStyle::Blue,
                    S::ClosedWall => DoorStyle::ClosedWall,
                    S::Doorway => DoorStyle::Doorway,
                    S::Elevator => DoorStyle::Elevator,
                    S::Eye => DoorStyle::Eye,
                    S::Gray => DoorStyle::Gray,
                    S::Green => DoorStyle::Green,
                    S::Passage => DoorStyle::Passage,
                    S::Red => DoorStyle::Red,
                    S::Vertical => DoorStyle::Vertical,
                    S::Yellow => DoorStyle::Yellow,
                    _ => return Err(invalid()),
                },
                orientation: *door_orientation,
                address: node_address.clone(),
                environments: slice(door_environments)
                    .iter()
                    .map(|raw::DoorEnvironment { physics, entrance_nodes, note, dev_note }| DoorEnvironment {
                        physics: *physics,
                        entrances: entrance_nodes.clone(),
                        doc: doc(note, &None, dev_note),
                    })
                    .collect(),
                implicit: ImplicitStrats {
                    leave_normally: implicit(*use_implicit_leave_normally),
                    come_in_normally: implicit(*use_implicit_come_in_normally),
                    come_in_with_mockball: implicit(*use_implicit_come_in_with_mockball),
                    come_in_with_grapple_jump: implicit(*use_implicit_come_in_with_grapple_jump),
                    carry_g_mode_back_through: implicit(*use_implicit_carry_g_mode_back_through),
                    carry_g_mode_morph_back_through: implicit(*use_implicit_carry_g_mode_morph_back_through),
                    door_unlocks: implicit(*use_implicit_door_unlocks),
                },
                immediately_closed: is_door_immediately_closed.unwrap_or(false),
                twins: slice(twin_door_addresses)
                    .iter()
                    .map(|raw::TwinDoorAddress { room_address, door_address }| TwinDoor {
                        room_address: room_address.clone(),
                        door_address: door_address.clone(),
                    })
                    .collect(),
            }),
            T::Entrance => NodeKind::Entrance { transit: transit()?, orientation: orientation()?, address: node_address.clone() },
            T::Exit => NodeKind::Exit { transit: transit()?, orientation: orientation()?, address: node_address.clone() },
            T::Item => NodeKind::Item {
                item: self.item(node_item.ok_or("item node has no nodeItem")?)?,
                placement: match node_sub_type {
                    S::Chozo => ItemPlacement::Chozo,
                    S::Hidden => ItemPlacement::Hidden,
                    S::Visible => ItemPlacement::Visible,
                    _ => return Err(invalid()),
                },
                address: node_address.clone().ok_or("item node has no nodeAddress")?,
            },
            T::Junction => {
                reject(&[(node_address.is_some(), "nodeAddress"), (door_orientation.is_some(), "doorOrientation")])?;
                NodeKind::Junction(match node_sub_type {
                    S::Junction => JunctionKind::Plain,
                    S::GMode => JunctionKind::GMode,
                    S::RMode => JunctionKind::RMode,
                    _ => return Err(invalid()),
                })
            }
            T::Utility => {
                reject(&[(node_address.is_some(), "nodeAddress")])?;
                NodeKind::Station {
                    station: match node_sub_type {
                        S::Energy => StationKind::Energy,
                        S::Map => StationKind::Map,
                        S::Missile => StationKind::Missile,
                        S::Save => StationKind::Save,
                        S::Ship => StationKind::Ship,
                        _ => return Err(invalid()),
                    },
                    services: slice(utility).to_vec(),
                    orientation: *door_orientation,
                }
            }
        })
    }

    fn room_node(&self, n: &raw::Node, scope: &RoomScope) -> Res<Node> {
        Ok(Node {
            id: n.id,
            name: n.name.clone(),
            kind: self.node_kind(n)?,
            map_tiles: n.map_tile_mask.clone(),
            locks: slice(&n.locks)
                .iter()
                .map(|raw::Lock { name, lock_type, lock, unlock_strats, yields, note, dev_note }| {
                    Ok(Lock {
                        name: name.clone(),
                        kind: *lock_type,
                        active_when: self.opt_requires(lock, Some(scope))?,
                        unlock_strats: unlock_strats.iter().map(|s| self.nested_strat(s, scope)).collect::<Res<_>>()?,
                        yields: self.flags(yields)?,
                        doc: doc(note, &None, dev_note),
                    })
                })
                .collect::<Res<_>>()?,
            viewable: slice(&n.viewable_nodes)
                .iter()
                .map(|raw::ViewableNode { id, strats }| {
                    Ok(ViewableNode {
                        node: self.node(Some(scope), *id)?,
                        strats: strats.iter().map(|s| self.nested_strat(s, scope)).collect::<Res<_>>()?,
                    })
                })
                .collect::<Res<_>>()?,
            yields: self.flags(&n.yields)?,
            doc: doc(&n.note, &None, &n.dev_note),
        })
    }

    fn room(&self, r: &raw::Room) -> Res<Room> {
        let raw::Room {
            schema: _,
            id,
            name,
            area,
            subarea,
            subsubarea,
            room_address,
            room_environments,
            map_tile_mask,
            nodes,
            obstacles,
            enemies,
            strats,
            notables,
            next_strat_id: _,
            next_notable_id: _,
            note,
            dev_note,
        } = r;
        let scope = RoomScope::new(r)?;
        let at = |what: String| format!("room {name} / {what}");
        Ok(Room {
            source_id: *id,
            name: name.clone(),
            area: area.clone(),
            subarea: subarea.clone(),
            subsubarea: subsubarea.clone(),
            address: room_address.clone(),
            environments: room_environments
                .iter()
                .map(|raw::RoomEnvironment { heated, entrance_nodes, note, dev_note }| RoomEnvironment {
                    heated: *heated,
                    entrances: entrance_nodes.clone(),
                    doc: doc(note, &None, dev_note),
                })
                .collect(),
            map_tiles: map_tile_mask.clone(),
            nodes: self.each(nodes, |n| at(format!("node {}", n.name)), |n| self.room_node(n, &scope)),
            obstacles: slice(obstacles)
                .iter()
                .map(|raw::Obstacle { id, name, obstacle_type, note, dev_note }| Obstacle {
                    key: id.as_str().to_owned(),
                    name: name.clone(),
                    kind: *obstacle_type,
                    doc: doc(note, &None, dev_note),
                })
                .collect(),
            enemies: self.each(slice(enemies), |e| at(format!("enemy group {}", e.group_name)), |e| self.room_enemy(e, &scope)),
            strats: self.each(strats, |s| at(format!("strat {}", s.name)), |s| {
                let [from, to] = s.link.ok_or("room strat has no link")?;
                Ok(RoomStrat {
                    source_id: s.id,
                    from: self.node(Some(&scope), from)?,
                    to: self.node(Some(&scope), to)?,
                    strat: self.strat(s, &scope)?,
                })
            }),
            notables: slice(notables)
                .iter()
                .map(|raw::Notable { id, name, wall_jump_avoid, note, dev_note }| Notable {
                    id: *id,
                    name: name.clone(),
                    wall_jump_avoid: wall_jump_avoid.unwrap_or(false),
                    note: note.clone(),
                    dev_note: dev_note.clone(),
                })
                .collect(),
            doc: doc(note, &None, dev_note),
        })
    }

    fn room_enemy(&self, e: &raw::RoomEnemy, scope: &RoomScope) -> Res<RoomEnemy> {
        let raw::RoomEnemy { id, group_name, enemy_name, quantity, home_nodes, between_nodes, spawn, stop_spawn, note, dev_note } = e;
        Ok(RoomEnemy {
            key: id.clone(),
            group_name: group_name.clone(),
            enemy: self.enemy(enemy_name)?,
            quantity: *quantity,
            placement: match (home_nodes, between_nodes) {
                (Some(home), None) => EnemyPlacement::Home(self.nodes(Some(scope), home)?),
                (None, Some([a, b])) => EnemyPlacement::Between(self.node(Some(scope), *a)?, self.node(Some(scope), *b)?),
                _ => return Err("enemy group needs exactly one of homeNodes and betweenNodes".to_owned()),
            },
            spawn: self.opt_requires(spawn, Some(scope))?,
            stop_spawn: self.opt_requires(stop_spawn, Some(scope))?,
            doc: doc(note, &None, dev_note),
        })
    }

    fn enemy_def(&self, e: &raw::Enemy, kind: EnemyKind) -> Res<Enemy> {
        let raw::Enemy {
            id,
            name,
            attacks,
            hp,
            amount_of_drops,
            drops,
            buffed_drops,
            farmable_drops,
            dims,
            freezable,
            grapplable,
            invul,
            damage_multipliers,
            areas,
            note,
            dev_note,
        } = e;
        Ok(Enemy {
            source_id: *id,
            name: name.clone(),
            kind,
            attacks: attacks
                .iter()
                .map(|raw::EnemyAttack { name, base_damage, affected_by_varia, affected_by_gravity }| Attack {
                    name: name.clone(),
                    base_damage: *base_damage,
                    reduced_by_varia: affected_by_varia.unwrap_or(true),
                    reduced_by_gravity: affected_by_gravity.unwrap_or(true),
                })
                .collect(),
            hp: *hp,
            drop_count: *amount_of_drops,
            drops: *drops,
            buffed_drops: *buffed_drops,
            farmable_drops: *farmable_drops,
            width: dims.w,
            height: dims.h,
            size_doc: doc(&dims.note, &None, &dims.dev_note),
            freezable: *freezable,
            grapplable: *grapplable,
            invulnerable_to: self.selectors(invul)?,
            damage_multipliers: damage_multipliers
                .iter()
                .map(|raw::DamageMultiplier { weapon, value }| Ok((self.selector(weapon)?, *value)))
                .collect::<Res<_>>()?,
            areas: slice(areas).to_vec(),
            doc: doc(note, &None, dev_note),
        })
    }

    fn scenario(&self, s: &raw::BossScenario) -> Res<BossScenario> {
        let raw::BossScenario {
            id,
            name,
            boss,
            explicit_weapons,
            excluded_weapons,
            requires,
            boss_dodge_rate,
            attack_opportunity_duration,
            damage_windows,
            incoming_damage,
            particle_frequency_frames,
            note,
            dev_note,
        } = s;
        Ok(BossScenario {
            source_id: *id,
            name: name.clone(),
            boss: self.enemy(boss)?,
            weapons: self.weapon_filter(explicit_weapons, excluded_weapons)?,
            requires: self.requires(requires, None)?,
            dodge_rate: *boss_dodge_rate,
            attack_opportunity_duration: *attack_opportunity_duration,
            damage_windows: slice(damage_windows)
                .iter()
                .map(|raw::DamageWindow { name, window_percent, requires, note, dev_note }| {
                    Ok(DamageWindow {
                        name: name.clone(),
                        percent: *window_percent,
                        requires: self.requires(requires, None)?,
                        doc: doc(note, &None, dev_note),
                    })
                })
                .collect::<Res<_>>()?,
            incoming_damage: slice(incoming_damage)
                .iter()
                .map(|raw::IncomingDamage { name, attack, frequency_frames, avoiding_requires, note, dev_note }| {
                    Ok(IncomingDamage {
                        name: name.clone(),
                        attack: self.attack(boss, attack)?,
                        frequency_frames: *frequency_frames,
                        avoiding_requires: self.requires(avoiding_requires, None)?,
                        doc: doc(note, &None, dev_note),
                    })
                })
                .collect::<Res<_>>()?,
            particle_frequency_frames: *particle_frequency_frames,
            doc: doc(note, &None, dev_note),
        })
    }

    fn connection(&self, c: &raw::Connection) -> Res<Connection> {
        let raw::Connection { connection_type, direction, description, nodes, note, dev_note } = c;
        let end = |n: &raw::ConnectionNode| -> Res<ConnectionEnd> {
            let raw::ConnectionNode { area, subarea, room_id, room_name, node_id, node_name, position, note, dev_note } = n;
            let room = *self.rooms_by_source.get(room_id).ok_or_else(|| format!("unknown room {room_id}"))?;
            if !self.data.rooms[room.index()].nodes.iter().any(|node| node.id == *node_id) {
                return Err(format!("room {room_name} has no node {node_id}"));
            }
            Ok(ConnectionEnd {
                room,
                node: *node_id,
                position: *position,
                label: ConnectionLabel {
                    area: area.clone(),
                    subarea: subarea.clone(),
                    room: room_name.clone(),
                    node: node_name.clone(),
                },
                doc: doc(note, &None, dev_note),
            })
        };
        Ok(Connection {
            kind: *connection_type,
            direction: *direction,
            description: description.clone(),
            ends: [end(&nodes[0])?, end(&nodes[1])?],
            doc: doc(note, &None, dev_note),
        })
    }

    fn start(&self) -> Res<StartState> {
        let items = &self.data.items;
        let room = *self
            .lookup
            .rooms
            .get(&items.starting_room)
            .ok_or_else(|| format!("unknown starting room `{}`", items.starting_room))?;
        if !self.data.rooms[room.index()].nodes.iter().any(|node| node.id == items.starting_node) {
            return Err(format!("starting room has no node {}", items.starting_node));
        }
        Ok(StartState {
            room,
            node: items.starting_node,
            items: items.starting_items.iter().map(|i| self.item(*i)).collect::<Res<_>>()?,
            flags: items.starting_flags.iter().map(|f| self.flag(f)).collect::<Res<_>>()?,
            open_locks: items.starting_locks.clone(),
            resources: items.starting_resources.iter().map(|r| (r.resource, r.max_amount)).collect(),
        })
    }

    fn run(self) -> Result<World, ImportErrors> {
        let data = self.data;
        let all_techs: Vec<&raw::Tech> = data.tech.all_techs().collect();
        let techs = self.each(&all_techs, |t| format!("tech {}", t.name), |t| {
            let raw::Tech { id, name, tech_requires, other_requires, note, detail_note, dev_note, extension_techs } = *t;
            Ok(Tech {
                source_id: *id,
                name: name.as_str().to_owned(),
                tech_requires: self.requires(tech_requires, None)?,
                other_requires: self.requires(other_requires, None)?,
                extensions: slice(extension_techs).iter().map(|e| self.tech(&e.name)).collect::<Res<_>>()?,
                doc: doc(note, detail_note, dev_note),
            })
        });
        let tech_categories = data
            .tech
            .tech_categories
            .iter()
            .map(|c| Category {
                name: c.name.clone(),
                description: c.description.clone(),
                members: c.techs.iter().map(|t| self.lookup.techs[t.name.as_str()]).collect(),
            })
            .collect();
        let all_helpers: Vec<&raw::Helper> = data.helpers.all_helpers().collect();
        let helpers = self.each(&all_helpers, |h| format!("helper {}", h.name), |h| {
            let raw::Helper { name, requires, map_rando_override, note, dev_note } = *h;
            Ok(Helper {
                name: name.as_str().to_owned(),
                requires: self.requires(requires, None)?,
                map_rando_override: map_rando_override.unwrap_or(false),
                doc: doc(note, &None, dev_note),
            })
        });
        let helper_categories = data
            .helpers
            .helper_categories
            .iter()
            .map(|c| Category {
                name: c.name.clone(),
                description: c.description.clone(),
                members: c.helpers.iter().map(|h| self.lookup.helpers[h.name.as_str()]).collect(),
            })
            .collect();
        let all_numerics: Vec<&raw::NumericParameter> = data.numerics.all_numerics().collect();
        let numerics = self.each(&all_numerics, |n| format!("numeric {}", n.name), |n| {
            let raw::NumericParameter { name, value, minimum, maximum, note, dev_note } = *n;
            Ok(Numeric {
                name: name.as_str().to_owned(),
                default: self.quantity(value)?,
                minimum: *minimum,
                maximum: *maximum,
                doc: doc(note, &None, dev_note),
            })
        });
        let numeric_categories = data
            .numerics
            .numeric_categories
            .iter()
            .map(|c| Category {
                name: c.name.clone(),
                description: c.description.clone(),
                members: c.numerics.iter().map(|n| self.lookup.numerics[n.name.as_str()]).collect(),
            })
            .collect();
        let weapons = self.each(&data.weapons.weapons, |w| format!("weapon {}", w.name), |w| {
            let raw::Weapon { id, name, damage, cooldown_frames, use_requires, shot_requires, situational, hits_group, categories, note, dev_note } = w;
            Ok(Weapon {
                source_id: *id,
                name: name.clone(),
                damage: *damage,
                cooldown_frames: *cooldown_frames,
                use_requires: self.requires(use_requires, None)?,
                shot_requires: self.requires(slice(shot_requires), None)?,
                situational: *situational,
                hits_group: *hits_group,
                categories: categories.clone(),
                doc: doc(note, &None, dev_note),
            })
        });
        let mut enemies = self.each(&data.enemies.enemies, |e| format!("enemy {}", e.name), |e| self.enemy_def(e, EnemyKind::Regular));
        enemies.extend(self.each(&data.bosses.enemies, |e| format!("boss {}", e.name), |e| self.enemy_def(e, EnemyKind::Boss)));
        let boss_scenarios = self.each(&data.boss_scenarios.scenarios, |s| format!("boss scenario {}", s.name), |s| self.scenario(s));
        let rooms = self.each(&data.rooms, |r| format!("room {}", r.name), |r| self.room(r));
        let connection_lists: Vec<&raw::Connection> = data.connections.iter().flat_map(|file| &file.connections).collect();
        let connections = self.each(&connection_lists, |_| "connection".to_owned(), |c| self.connection(c));
        let start = self.start();

        let raw::ItemsFile { implicit_items, upgrade_items, expansion_items, game_flags, dev_note, .. } = &data.items;
        let items = ItemCatalog {
            implicit: implicit_items.clone(),
            upgrades: upgrade_items.iter().map(|u| Upgrade { item: u.name, data: u.data.clone() }).collect(),
            expansions: expansion_items
                .iter()
                .map(|e| Expansion { item: e.name, data: e.data.clone(), resource: e.resource, amount: e.resource_amount })
                .collect(),
            doc: doc(&None, &None, dev_note),
        };
        let flags = game_flags.iter().map(|f| f.as_str().to_owned()).collect();

        let mut errors = self.errors.into_inner();
        if let Err(problem) = &start {
            errors.push(ModelError { location: "starting state".to_owned(), problem: problem.clone() });
        }
        if errors.is_empty() {
            errors.extend(numeric_cycle_errors(&numerics));
        }
        if !errors.is_empty() {
            return Err(ImportErrors(errors));
        }
        Ok(World {
            items,
            start: start.expect("errors were checked above"),
            flags,
            internals: self.internals.into_inner().0,
            techs,
            tech_categories,
            helpers,
            helper_categories,
            numerics,
            numeric_categories,
            weapons,
            weapon_categories: slice(&data.weapons.categories).to_vec(),
            enemies,
            boss_scenarios,
            rooms,
            connections,
            lookup: self.lookup,
        })
    }
}

fn numeric_cycle_errors(numerics: &[Numeric]) -> Vec<ModelError> {
    fn params(quantity: &Quantity, out: &mut Vec<NumericId>) {
        match quantity {
            Quantity::Const(_) => {}
            Quantity::Param(id) => out.push(*id),
            Quantity::Sum(terms) | Quantity::Product(terms) => terms.iter().for_each(|term| params(term, out)),
        }
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Active,
        Done,
    }
    fn visit(id: usize, numerics: &[Numeric], marks: &mut [Mark]) -> bool {
        match marks[id] {
            Mark::Done => return true,
            Mark::Active => return false,
            Mark::New => {}
        }
        marks[id] = Mark::Active;
        let mut next = Vec::new();
        params(&numerics[id].default, &mut next);
        let ok = next.iter().all(|param| visit(param.index(), numerics, marks));
        marks[id] = Mark::Done;
        ok
    }
    let mut marks = vec![Mark::New; numerics.len()];
    (0..numerics.len())
        .filter(|id| !visit(*id, numerics, &mut marks))
        .map(|id| ModelError { location: format!("numeric {}", numerics[id].name), problem: "its default depends on itself".to_owned() })
        .collect()
}
