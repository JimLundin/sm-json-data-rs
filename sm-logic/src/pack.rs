//! The pack file format: a compact binary form of a [`World`].
//!
//! ```text
//! magic "SMLP" | version u8 | flags u8 | checksum u64 | source text | logic length | docs length
//! logic: strings, requirements, requirement lists, door entries, door exits, records
//! docs (optional): strings, then the documentation of each entry, exit and record
//! ```
//!
//! Integers are little-endian base-128 varints, so small values take one byte.
//! Strings, requirements, requirement lists and door conditions are stored once in
//! a table and referenced by index; a table entry can only refer to earlier entries.
//! Everything nothing in the logic depends on goes in the docs section, so a reader
//! can skip it and a pack can leave it out entirely. Decoding always validates the
//! result, so a decoded [`World`] upholds the same guarantees as an imported one.

use crate::*;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

const MAGIC: &[u8; 4] = b"SMLP";
/// The format version this crate reads and writes.
pub const VERSION: u8 = 1;
const FLAG_DOCS: u8 = 1;
/// Where the checksum sits; it covers everything after itself.
const CHECKSUM_AT: usize = 6;
const BODY_AT: usize = CHECKSUM_AT + 8;
/// The most requirement nodes a pack may expand to, which stops a small corrupt file
/// from expanding exponentially through shared tables.
const EXPANSION_BUDGET: u64 = 1 << 26;

/// Why a pack could not be written or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    NotAPack,
    UnsupportedVersion(u8),
    Truncated,
    /// The content doesn't match its checksum, so the file is damaged.
    Corrupt,
    Invalid(String),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::NotAPack => f.write_str("not a logic pack"),
            PackError::UnsupportedVersion(version) => write!(f, "unsupported pack version {version}"),
            PackError::Truncated => f.write_str("pack ends unexpectedly"),
            PackError::Corrupt => f.write_str("pack is damaged: its checksum doesn't match"),
            PackError::Invalid(problem) => write!(f, "invalid pack: {problem}"),
        }
    }
}

impl std::error::Error for PackError {}

/// What to put in a pack.
#[derive(Clone, Debug, Default)]
pub struct EncodeOptions {
    /// Include the docs section: notes, strat names and other descriptive text.
    pub docs: bool,
    /// Identifies the source data, such as a commit hash.
    pub source: String,
}

/// Whether to load a pack's docs section, if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Docs {
    Load,
    Skip,
}

/// A pack's header and section sizes, available without decoding it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackInfo {
    pub version: u8,
    pub source: String,
    pub logic_bytes: usize,
    pub docs_bytes: usize,
}

#[derive(Default)]
struct Pool<K> {
    ids: HashMap<K, u32>,
    items: Vec<K>,
}

impl<K: Clone + Eq + Hash> Pool<K> {
    fn intern(&mut self, key: K) -> u32 {
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = u32::try_from(self.items.len()).expect("fewer than 2^32 table entries");
        self.items.push(key.clone());
        self.ids.insert(key, id);
        id
    }
}

type Encoded = (Vec<u8>, Vec<u8>);

struct Writer {
    logic: Vec<u8>,
    docs: Vec<u8>,
    docs_enabled: bool,
    strings: Pool<String>,
    doc_strings: Pool<String>,
    requirements: Pool<Encoded>,
    lists: Pool<Encoded>,
    entries: Pool<Encoded>,
    exits: Pool<Encoded>,
    previous_strat: i64,
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

impl Writer {
    fn varint(&mut self, value: u64) {
        put_varint(&mut self.logic, value);
    }

    fn doc_varint(&mut self, value: u64) {
        put_varint(&mut self.docs, value);
    }

    /// Runs `write` against empty buffers and returns what it produced.
    fn capture(&mut self, write: impl FnOnce(&mut Writer)) -> Encoded {
        let logic = std::mem::take(&mut self.logic);
        let docs = std::mem::take(&mut self.docs);
        write(self);
        (std::mem::replace(&mut self.logic, logic), std::mem::replace(&mut self.docs, docs))
    }

    fn doc_text(&mut self, text: &Option<String>) {
        if !self.docs_enabled {
            return;
        }
        match text {
            None => self.doc_varint(0),
            Some(text) => {
                let id = self.doc_strings.intern(text.clone());
                self.doc_varint(u64::from(id) + 1);
            }
        }
    }

    fn doc_note(&mut self, note: &Option<Note>) {
        if !self.docs_enabled {
            return;
        }
        match note {
            None => self.doc_varint(0),
            Some(Note::Text(text)) => {
                self.doc_varint(1);
                self.doc_text(&Some(text.clone()));
            }
            Some(Note::Lines(lines)) => {
                self.doc_varint(2);
                self.doc_varint(lines.len() as u64);
                for line in lines {
                    self.doc_text(&Some(line.clone()));
                }
            }
        }
    }
}

struct Tables {
    strings: Vec<String>,
    doc_strings: Vec<String>,
    requirements: Vec<(Requirement, u64)>,
    lists: Vec<(Vec<Requirement>, u64)>,
    entries: Vec<(DoorEntry, u64)>,
    exits: Vec<(DoorExit, u64)>,
}

struct Reader<'a> {
    logic: &'a [u8],
    docs: Option<&'a [u8]>,
    tables: Tables,
    budget: u64,
    previous_strat: i64,
}

fn take_varint(bytes: &mut &[u8]) -> Result<u64, PackError> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = bytes.split_first().ok_or(PackError::Truncated)?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(PackError::Invalid("varint is too long".to_owned()))
}

fn invalid<T>(problem: impl Into<String>) -> Result<T, PackError> {
    Err(PackError::Invalid(problem.into()))
}

impl Reader<'_> {
    fn varint(&mut self) -> Result<u64, PackError> {
        take_varint(&mut self.logic)
    }

    fn doc_varint(&mut self) -> Result<u64, PackError> {
        match &mut self.docs {
            Some(docs) => take_varint(docs),
            None => Ok(0),
        }
    }

    fn doc_text(&mut self) -> Result<Option<String>, PackError> {
        match self.doc_varint()? {
            0 => Ok(None),
            id => match self.tables.doc_strings.get((id - 1) as usize) {
                Some(text) => Ok(Some(text.clone())),
                None => invalid("doc string index out of range"),
            },
        }
    }

    fn doc_note(&mut self) -> Result<Option<Note>, PackError> {
        match self.doc_varint()? {
            0 => Ok(None),
            1 => Ok(self.doc_text()?.map(Note::Text)),
            2 => {
                let count = self.doc_varint()? as usize;
                if count > self.docs.map_or(0, <[u8]>::len) {
                    return Err(PackError::Truncated);
                }
                let mut lines = Vec::with_capacity(count);
                for _ in 0..count {
                    lines.push(self.doc_text()?.ok_or(PackError::Invalid("empty note line".to_owned()))?);
                }
                Ok(Some(Note::Lines(lines)))
            }
            kind => invalid(format!("unknown note kind {kind}")),
        }
    }

    /// Records that a table entry of this weight was copied into the result.
    fn spend(&mut self, weight: u64) -> Result<(), PackError> {
        self.budget = self.budget.checked_sub(weight).ok_or(PackError::Invalid("pack expands too far".to_owned()))?;
        Ok(())
    }
}

trait Codec: Sized {
    fn write(&self, w: &mut Writer);
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError>;
}

/// The encoding of a value that is stored in a shared table.
trait Body: Sized {
    fn write_body(&self, w: &mut Writer);
    fn read_body(r: &mut Reader<'_>) -> Result<Self, PackError>;
}

impl Codec for bool {
    fn write(&self, w: &mut Writer) {
        w.logic.push(u8::from(*self));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        match r.varint()? {
            0 => Ok(false),
            1 => Ok(true),
            other => invalid(format!("bad boolean {other}")),
        }
    }
}

impl Codec for u8 {
    fn write(&self, w: &mut Writer) {
        w.varint(u64::from(*self));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        u8::try_from(r.varint()?).or_else(|_| invalid("value too large"))
    }
}

impl Codec for u16 {
    fn write(&self, w: &mut Writer) {
        w.varint(u64::from(*self));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        u16::try_from(r.varint()?).or_else(|_| invalid("value too large"))
    }
}

impl Codec for u32 {
    fn write(&self, w: &mut Writer) {
        w.varint(u64::from(*self));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        u32::try_from(r.varint()?).or_else(|_| invalid("value too large"))
    }
}

impl Codec for i32 {
    fn write(&self, w: &mut Writer) {
        w.varint(zigzag(i64::from(*self)));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        i32::try_from(unzigzag(r.varint()?)).or_else(|_| invalid("value too large"))
    }
}

/// Numbers in the data are mostly whole, or have at most four decimal places. The low
/// two bits say which: whole, in ten-thousandths, or raw `f64` bits as a last resort.
impl Codec for f64 {
    fn write(&self, w: &mut Writer) {
        const SCALE: f64 = 10_000.0;
        const LIMIT: f64 = (1u64 << 40) as f64;
        let scaled = (*self * SCALE).round();
        if self.fract() == 0.0 && self.abs() < LIMIT {
            w.varint(zigzag(*self as i64) << 2);
        } else if scaled.abs() < LIMIT && scaled / SCALE == *self {
            w.varint((zigzag(scaled as i64) << 2) | 1);
        } else {
            w.varint(2);
            w.logic.extend_from_slice(&self.to_le_bytes());
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let value = r.varint()?;
        match value & 3 {
            0 => Ok(unzigzag(value >> 2) as f64),
            1 => Ok(unzigzag(value >> 2) as f64 / 10_000.0),
            2 => {
                let (bytes, rest) = r.logic.split_first_chunk::<8>().ok_or(PackError::Truncated)?;
                r.logic = rest;
                Ok(f64::from_le_bytes(*bytes))
            }
            _ => invalid("unknown number encoding"),
        }
    }
}

impl Codec for String {
    fn write(&self, w: &mut Writer) {
        let id = w.strings.intern(self.clone());
        w.varint(u64::from(id));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let id = r.varint()? as usize;
        match r.tables.strings.get(id) {
            Some(text) => Ok(text.clone()),
            None => invalid("string index out of range"),
        }
    }
}

impl<T: Codec> Codec for Vec<T> {
    fn write(&self, w: &mut Writer) {
        w.varint(self.len() as u64);
        for item in self {
            item.write(w);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let count = r.varint()? as usize;
        // Every element takes at least one byte, so a larger count must be corrupt.
        if count > r.logic.len() {
            return Err(PackError::Truncated);
        }
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(T::read(r)?);
        }
        Ok(items)
    }
}

impl<T: Codec> Codec for Option<T> {
    fn write(&self, w: &mut Writer) {
        match self {
            None => w.logic.push(0),
            Some(value) => {
                w.logic.push(1);
                value.write(w);
            }
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(if bool::read(r)? { Some(T::read(r)?) } else { None })
    }
}

impl<T: Codec> Codec for [T; 2] {
    fn write(&self, w: &mut Writer) {
        self[0].write(w);
        self[1].write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok([T::read(r)?, T::read(r)?])
    }
}

impl<A: Codec, B: Codec> Codec for (A, B) {
    fn write(&self, w: &mut Writer) {
        self.0.write(w);
        self.1.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok((A::read(r)?, B::read(r)?))
    }
}

macro_rules! newtypes {
    ($($ty:ty => $inner:ty),+ $(,)?) => {$(
        impl Codec for $ty {
            fn write(&self, w: &mut Writer) {
                self.0.write(w);
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
                Ok(Self(<$inner>::read(r)?))
            }
        }
    )+};
}

newtypes! {
    RoomId => u16, TechId => u16, HelperId => u16, NumericId => u16, FlagId => u16, InternalId => u16,
    WeaponId => u16, EnemyId => u16, ObstacleId => u8, NodeId => u32, NotableId => u32, Address => u32, RunSpeed => u32,
    source::RoomId => u32, source::TechId => u32, source::EnemyId => u32, source::WeaponId => u32,
    source::ScenarioId => u32,
}

/// Field-less enums, stored as one tag byte. The tags are part of the format.
macro_rules! tags {
    ($($ty:ident { $($variant:ident = $tag:literal),+ $(,)? })+) => {$(
        impl Codec for $ty {
            fn write(&self, w: &mut Writer) {
                w.logic.push(match self { $($ty::$variant => $tag),+ });
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
                match r.varint()? {
                    $($tag => Ok($ty::$variant),)+
                    other => invalid(format!(concat!("unknown ", stringify!($ty), " tag {}"), other)),
                }
            }
        }
    )+};
}

tags! {
    TriState { Yes = 0, No = 1, Any = 2 }
    AmmoKind { Missile = 0, Super = 1, PowerBomb = 2 }
    ResourceKind { Missile = 0, Super = 1, PowerBomb = 2, RegularEnergy = 3, ReserveEnergy = 4, Energy = 5 }
    ImplicitHeatFrames { Yes = 0, No = 1, Suitless = 2 }
    RidleyStuck { Top = 0, Bottom = 1 }
    DoorUnlockType { Missiles = 0, Super = 1, Powerbomb = 2, Gray = 3, Ammo = 4 }
    SparkPosition { Top = 0, Bottom = 1 }
    SidePosition { Left = 0, Right = 1, Any = 2 }
    MovementType { Controlled = 0, Uncontrolled = 1, Any = 2 }
    GModeMode { Direct = 0, Indirect = 1, Any = 2 }
    GModeMobility { Mobile = 0, Immobile = 1, Any = 2 }
    Environment { Water = 0, Air = 1, Any = 2 }
    Physics { Air = 0, Water = 1, Lava = 2, Acid = 3, Normal = 4 }
    DoorOrientation { Left = 0, Right = 1, Up = 2, Down = 3 }
    Utility { Save = 0, Missile = 1, Super = 2, Powerbomb = 3, Energy = 4, Reserve = 5, Map = 6 }
    LockType { BossFight = 0, ColoredDoor = 1, Cutscene = 2, EscapeFunnel = 3, GameFlag = 4, KillEnemies = 5, Permanent = 6 }
    ObstacleType { Inanimate = 0, Enemies = 1, Abstract = 2 }
    ConsumableResource { RegularEnergy = 0, ReserveEnergy = 1, Missile = 2, Super = 3, PowerBomb = 4 }
    WeaponCategory {
        All = 0, Beam = 1, ChargedBeam = 2, UnchargedBeam = 3, UnchargedSpazerBeam = 4, UnchargedIceBeam = 5,
        BeamNoPlasma = 6, UnchargedBeamNoPlasma = 7, PowerBombBlast = 8, SpecialBeamAttack = 9,
    }
    ConnectionType { HorizontalDoor = 0, VerticalDoor = 1, HorizontalMorphTunnel = 2, VerticalSandpit = 3, Elevator = 4, StoryMarker = 5 }
    ConnectionDirection { Forward = 0, Bidirectional = 1 }
    ConnectionPosition { Left = 0, Right = 1, Top = 2, Bottom = 3 }
    AmmoUsage { Spend = 0, Drain = 1 }
    ResourceCheck { Available = 0, Consumed = 1, AtMost = 2, MissingAtMost = 3, Capacity = 4, MaxCapacity = 5 }
    Hazard { SpikeHits = 0, ThornHits = 1, ElectricityHits = 2, HibashiHits = 3, SamusEaterCycles = 4, MetroidFrames = 5, DraygonElectricityFrames = 6 }
    GravityRule { Standard = 0, Gravityless = 1 }
    FlashSuitAction { Gain = 0, Use = 1, Forbid = 2 }
    BlueSuitAction { Gain = 0, Have = 1, Shinecharge = 2, Forbid = 3 }
    DamageBoostEntry { High = 0, Any = 1 }
    DamageBoostExit { NotHigh = 0, High = 1 }
    DoorShellBypass { No = 0, Yes = 1, Free = 2 }
    DoorStyle { Blue = 0, ClosedWall = 1, Doorway = 2, Elevator = 3, Eye = 4, Gray = 5, Green = 6, Passage = 7, Red = 8, Vertical = 9, Yellow = 10 }
    Transit { Elevator = 0, Sandpit = 1 }
    ItemPlacement { Chozo = 0, Hidden = 1, Visible = 2 }
    JunctionKind { Plain = 0, GMode = 1, RMode = 2 }
    StationKind { Energy = 0, Map = 1, Missile = 2, Save = 3, Ship = 4 }
    Implicit { Automatic = 0, On = 1, Off = 2 }
    EnemyKind { Regular = 0, Boss = 1 }
}

impl Codec for Item {
    fn write(&self, w: &mut Writer) {
        let index = Item::ALL.iter().position(|item| item == self).expect("Item::ALL lists every item");
        w.logic.push(index as u8);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        match Item::ALL.get(r.varint()? as usize) {
            Some(item) => Ok(*item),
            None => invalid("unknown item"),
        }
    }
}

/// Structs whose fields all live in the logic section, in declaration order.
macro_rules! structs {
    ($($ty:ident { $($field:ident),+ $(,)? })+) => {$(
        impl Codec for $ty {
            fn write(&self, w: &mut Writer) {
                $(self.$field.write(w);)+
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
                Ok($ty { $($field: Codec::read(r)?),+ })
            }
        }
    )+};
}

structs! {
    ResourceAmount { kind, count }
    EnemyKill { groups, weapons, farmable_ammo }
    AttackRef { enemy, attack }
    EnemyDrop { enemy, count }
    TileRange { min, max }
    RunUp { adjacent_min, remote_and_landing_min }
    SpeedRange { min, max }
    DropRates { no_drop, small_energy, big_energy, missile, super_missile, power_bomb }
    Upgrade { item, data }
    Expansion { item, data, resource, amount }
    StartState { room, node, items, flags, open_locks, resources }
    Attack { name, base_damage, reduced_by_varia, reduced_by_gravity }
    TwinDoor { room_address, door_address }
    ViewableNode { node, strats }
    ImplicitStrats {
        leave_normally, come_in_normally, come_in_with_mockball, come_in_with_grapple_jump,
        carry_g_mode_back_through, carry_g_mode_morph_back_through, door_unlocks,
    }
}

/// Enums with data. The tags are part of the format.
macro_rules! tagged {
    (@read $r:ident $name:ident) => { Codec::read($r)? };
    ($trait:ident $write:ident $read:ident: $ty:ident {
        $($tag:literal => $variant:ident $(($($item:ident),+))? $({$($field:ident),* $(,)?})?),+ $(,)?
    }) => {
        impl $trait for $ty {
            fn $write(&self, w: &mut Writer) {
                match self {
                    $($ty::$variant $(($($item),+))? $({$($field),*})? => {
                        w.logic.push($tag);
                        $($($item.write(w);)+)?
                        $($($field.write(w);)*)?
                    })+
                }
            }
            fn $read(r: &mut Reader<'_>) -> Result<Self, PackError> {
                Ok(match r.varint()? {
                    $($tag => $ty::$variant $(($(tagged!(@read r $item)),+))? $({$($field: Codec::read(r)?),*})?,)+
                    other => return invalid(format!(concat!("unknown ", stringify!($ty), " tag {}"), other)),
                })
            }
        }
    };
}

tagged!(Codec write read: Quantity {
    0 => Const(value),
    1 => Param(id),
    2 => Sum(terms),
    3 => Product(terms),
});

tagged!(Codec write read: WeaponFilter { 0 => Default, 1 => Only(list), 2 => Except(list) });
tagged!(Codec write read: WeaponSelector { 0 => Weapon(id), 1 => Category(category) });
tagged!(Codec write read: HeatMode { 0 => Standard, 1 => Simple, 2 => Suitless, 3 => Gravityless, 4 => WithDrops(drops) });
tagged!(Codec write read: LavaMode { 0 => Standard, 1 => Gravityless, 2 => WithDrops(drops) });
tagged!(Codec write read: FailureOutcome { 0 => Continues, 1 => LeadsTo(node), 2 => Softlock });

impl Codec for EnemyPlacement {
    fn write(&self, w: &mut Writer) {
        match self {
            EnemyPlacement::Home(nodes) => {
                w.logic.push(0);
                nodes.write(w);
            }
            EnemyPlacement::Between(a, b) => {
                w.logic.push(1);
                a.write(w);
                b.write(w);
            }
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        match r.varint()? {
            0 => Ok(EnemyPlacement::Home(Codec::read(r)?)),
            1 => Ok(EnemyPlacement::Between(Codec::read(r)?, Codec::read(r)?)),
            other => invalid(format!("unknown EnemyPlacement tag {other}")),
        }
    }
}

tagged!(Body write_body read_body: Requirement {
    0 => Free,
    1 => Never,
    2 => All(list),
    3 => Any(list),
    4 => Item(item),
    5 => Flag(id),
    6 => NotFlag(id),
    7 => Tech(id),
    8 => TechOnly(id),
    9 => Helper(id),
    10 => Notable(id),
    11 => Internal(id),
    12 => DisableEquipment(item),
    13 => Ammo { kind, count, usage },
    14 => Refill(kinds),
    15 => PartialRefill { kind, limit },
    16 => Resource { check, amounts },
    17 => AutoReserveTrigger { min_energy, max_energy, implicit_heat },
    18 => Kill(kill),
    19 => RidleyKill { power_bombs, g_mode, stuck },
    20 => EnemyHits { attack, hits },
    21 => Hazard { hazard, amount },
    22 => Heat { frames, mode },
    23 => Lava { frames, mode },
    24 => Acid { frames, gravity },
    25 => CycleFrames { frames, simple },
    26 => ShineCharge(runway),
    27 => BlueSpeed(runway),
    28 => SpeedBall(runway),
    29 => ShineChargeFrames(frames),
    30 => Shinespark { frames, excess_frames },
    31 => FlashSuit(action),
    32 => BlueSuit(action),
    33 => DoorUnlocked(node),
    34 => ItemCollected(node),
    35 => ItemNotCollected(node),
    36 => ObstaclesCleared(ids),
    37 => ObstaclesNotCleared(ids),
    38 => EnteredFrom(nodes),
});

tagged!(Body write_body read_body: DoorExit {
    0 => Normal,
    1 => Runway { runway, min_extra_speed, heated },
    2 => Shinecharged,
    3 => TemporaryBlue { direction },
    4 => Spark { position, grounded },
    5 => Spinning { remote, extra_speed, blue },
    6 => Mockball { remote, landing, extra_speed, blue },
    7 => SpringBallBounce { remote, landing, movement, extra_speed, blue },
    8 => SpaceJumping { remote, extra_speed, blue },
    9 => StoredFallSpeed { tiles },
    10 => Damage { attack, knockback },
    11 => Knockback,
    12 => DamageBoost { i_frames_remaining, position },
    13 => IFrames { i_frames_remaining },
    14 => XModeSetup { attack },
    15 => GMode { morphed },
    16 => DoorFrameBelow { height },
    17 => PlatformBelow { height, left, right },
    18 => SidePlatform { height, runway, obstruction },
    19 => GrappleSwing { blocks },
    20 => GrappleJump { position },
    21 => GrappleTeleport { blocks },
    22 => SamusEaterTeleport { floor, ceiling },
    23 => SuperSink,
});

tagged!(Codec write read: EntryKind {
    0 => Normal,
    1 => Running { speed_booster, speed },
    2 => Jumping { speed_booster, speed },
    3 => SpaceJumping { speed_booster, speed },
    4 => BlueSpaceJumping { extra_speed },
    5 => Shinecharging { runway },
    6 => GettingBlueSpeed { runway, extra_speed },
    7 => Shinecharged,
    8 => ShinechargedJumping,
    9 => Spark { position },
    10 => StutterShinecharging { min_tiles },
    11 => StutterGettingBlueSpeed { min_tiles },
    12 => BombBoost,
    13 => DoorStuckSetup,
    14 => Speedballing { runway, extra_speed },
    15 => TemporaryBlue { direction },
    16 => Spinning { speed_booster, unusable_tiles, extra_speed },
    17 => BlueSpinning { unusable_tiles, extra_speed },
    18 => Mockball { speed_booster, run_up },
    19 => SpringBallBounce { speed_booster, movement, run_up },
    20 => BlueSpringBallBounce { movement, extra_speed, min_landing_tiles },
    21 => StoredFallSpeed { tiles },
    22 => Knockback,
    23 => DamageBoost { i_frames, position },
    24 => IFrames { i_frames },
    25 => RMode,
    26 => GMode { mode, morphed, mobility },
    27 => XMode,
    28 => WallJumpBelow { min_height },
    29 => SpaceJumpBelow,
    30 => PlatformBelow { min_height, max_height, max_left, min_right },
    31 => SidePlatform { platforms },
    32 => GrappleSwing { blocks },
    33 => GrappleJump { position },
    34 => GrappleTeleport { blocks },
    35 => SamusEaterTeleport { floor, ceiling },
    36 => SuperSink,
});

impl Body for DoorEntry {
    fn write_body(&self, w: &mut Writer) {
        self.kind.write(w);
        w.logic.push(tri_pair(self.through_toilet, self.heated));
        w.doc_note(&self.dev_note);
    }
    fn read_body(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let kind = EntryKind::read(r)?;
        let (through_toilet, heated) = read_tri_pair(r)?;
        Ok(DoorEntry { kind, through_toilet, heated, dev_note: r.doc_note()? })
    }
}

fn tri_index(value: TriState) -> u8 {
    match value {
        TriState::Yes => 0,
        TriState::No => 1,
        TriState::Any => 2,
    }
}

fn tri_pair(a: TriState, b: TriState) -> u8 {
    tri_index(a) * 3 + tri_index(b)
}

fn read_tri_pair(r: &mut Reader<'_>) -> Result<(TriState, TriState), PackError> {
    const VALUES: [TriState; 3] = [TriState::Yes, TriState::No, TriState::Any];
    let packed = r.varint()? as usize;
    if packed >= 9 {
        return invalid("bad door entry modifiers");
    }
    Ok((VALUES[packed / 3], VALUES[packed % 3]))
}

macro_rules! interned {
    ($($ty:ty, $pool:ident, $table:ident;)+) => {$(
        impl Codec for $ty {
            fn write(&self, w: &mut Writer) {
                let encoded = w.capture(|w| self.write_body(w));
                let id = w.$pool.intern(encoded);
                w.varint(u64::from(id));
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
                let id = r.varint()? as usize;
                let (value, weight) = match r.tables.$table.get(id) {
                    Some((value, weight)) => (value.clone(), *weight),
                    None => return invalid(concat!(stringify!($table), " index out of range")),
                };
                r.spend(weight)?;
                Ok(value)
            }
        }
    )+};
}

interned! {
    Requirement, requirements, requirements;
    DoorEntry, entries, entries;
    DoorExit, exits, exits;
}

/// Writes a requirement list through the list table.
fn write_list(list: &Vec<Requirement>, w: &mut Writer) {
    let encoded = w.capture(|w| list.write(w));
    let id = w.lists.intern(encoded);
    w.varint(u64::from(id));
}

fn read_list(r: &mut Reader<'_>) -> Result<Vec<Requirement>, PackError> {
    let id = r.varint()? as usize;
    let (list, weight) = match r.tables.lists.get(id) {
        Some((list, weight)) => (list.clone(), *weight),
        None => return invalid("list index out of range"),
    };
    r.spend(weight)?;
    Ok(list)
}

impl Codec for Runway {
    fn write(&self, w: &mut Writer) {
        let s = &self.slopes;
        let fields = [s.gentle_up, s.gentle_down, s.steep_up, s.steep_down, s.starting_steep_down, s.starting_gentle_down, s.starting_stutterless];
        let mut head = u64::from(self.open_ends);
        for (bit, value) in fields.iter().enumerate() {
            if *value != 0 {
                head |= 1 << (2 + bit);
            }
        }
        self.length.write(w);
        w.varint(head);
        for value in fields.iter().filter(|value| **value != 0) {
            value.write(w);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let length = f64::read(r)?;
        let head = r.varint()?;
        if head >> 9 != 0 {
            return invalid("bad runway header");
        }
        let mut field = |bit: u32| -> Result<u32, PackError> {
            if head & (1 << (2 + bit)) != 0 { u32::read(r) } else { Ok(0) }
        };
        let slopes = RunwaySlopes {
            gentle_up: field(0)?,
            gentle_down: field(1)?,
            steep_up: field(2)?,
            steep_down: field(3)?,
            starting_steep_down: field(4)?,
            starting_gentle_down: field(5)?,
            starting_stutterless: field(6)?,
        };
        Ok(Runway { length, open_ends: (head & 3) as u8, slopes })
    }
}

impl Codec for Doc {
    fn write(&self, w: &mut Writer) {
        if !w.docs_enabled {
            return;
        }
        let mask = u64::from(self.note.is_some()) | (u64::from(self.detail.is_some()) << 1) | (u64::from(self.dev.is_some()) << 2);
        w.doc_varint(mask);
        if self.note.is_some() {
            w.doc_note(&self.note);
        }
        if self.detail.is_some() {
            w.doc_note(&self.detail);
        }
        if self.dev.is_some() {
            w.doc_note(&self.dev);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let mask = r.doc_varint()?;
        let mut doc = Doc::default();
        if mask & 1 != 0 {
            doc.note = r.doc_note()?;
        }
        if mask & 2 != 0 {
            doc.detail = r.doc_note()?;
        }
        if mask & 4 != 0 {
            doc.dev = r.doc_note()?;
        }
        Ok(doc)
    }
}

impl Codec for GrappleSwingBlock {
    fn write(&self, w: &mut Writer) {
        self.position.write(w);
        self.environment.write(w);
        self.obstructions.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(GrappleSwingBlock {
            position: Codec::read(r)?,
            environment: Codec::read(r)?,
            obstructions: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for SidePlatform {
    fn write(&self, w: &mut Writer) {
        self.min_height.write(w);
        self.max_height.write(w);
        self.min_tiles.write(w);
        self.speed_booster.write(w);
        self.obstructions.write(w);
        self.environment.write(w);
        self.requires.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(SidePlatform {
            min_height: Codec::read(r)?,
            max_height: Codec::read(r)?,
            min_tiles: Codec::read(r)?,
            speed_booster: Codec::read(r)?,
            obstructions: Codec::read(r)?,
            environment: Codec::read(r)?,
            requires: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for DoorUnlock {
    fn write(&self, w: &mut Writer) {
        self.node.write(w);
        self.types.write(w);
        write_list(&self.requires, w);
        self.use_implicit_requires.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(DoorUnlock {
            node: Codec::read(r)?,
            types: Codec::read(r)?,
            requires: read_list(r)?,
            use_implicit_requires: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for StratFailure {
    fn write(&self, w: &mut Writer) {
        w.doc_text(&self.name);
        self.outcome.write(w);
        self.cost.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(StratFailure {
            name: r.doc_text()?,
            outcome: Codec::read(r)?,
            cost: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

const HAS_ENTRY: u64 = 1;
const HAS_EXIT: u64 = 1 << 1;
const HAS_UNLOCKS: u64 = 1 << 2;
const HAS_CLEARS: u64 = 1 << 3;
const SHELL_SHIFT: u64 = 4;
const SHELL_MASK: u64 = 3 << SHELL_SHIFT;
const HAS_SETS_FLAGS: u64 = 1 << 6;
const HAS_RESETS: u64 = 1 << 7;
const HAS_COLLECTS: u64 = 1 << 8;
const HAS_FARM: u64 = 1 << 9;
const HAS_FAILURES: u64 = 1 << 10;
const WALL_JUMP_AVOID: u64 = 1 << 11;
const STARTS_SHINECHARGED: u64 = 1 << 12;
const ENDS_SHINECHARGED: u64 = 1 << 13;
const REGAINS_G_MODE: u64 = 1 << 14;
const FLASH_UNREVIEWED: u64 = 1 << 15;
const BLUE_UNREVIEWED: u64 = 1 << 16;
const STRAT_MASK_LIMIT: u64 = 1 << 17;

impl Codec for Strat {
    fn write(&self, w: &mut Writer) {
        let shell = match self.door_shell {
            DoorShellBypass::No => 0,
            DoorShellBypass::Yes => 1,
            DoorShellBypass::Free => 2,
        };
        let flag = |on: bool, bit: u64| if on { bit } else { 0 };
        let mask = flag(self.entry.is_some(), HAS_ENTRY)
            | flag(self.exit.is_some(), HAS_EXIT)
            | flag(!self.door_unlocks.is_empty(), HAS_UNLOCKS)
            | flag(!self.clears_obstacles.is_empty(), HAS_CLEARS)
            | (shell << SHELL_SHIFT)
            | flag(!self.sets_flags.is_empty(), HAS_SETS_FLAGS)
            | flag(!self.resets_obstacles.is_empty(), HAS_RESETS)
            | flag(!self.collects_items.is_empty(), HAS_COLLECTS)
            | flag(!self.farm_drops.is_empty(), HAS_FARM)
            | flag(!self.failures.is_empty(), HAS_FAILURES)
            | flag(self.wall_jump_avoid, WALL_JUMP_AVOID)
            | flag(self.starts_shinecharged, STARTS_SHINECHARGED)
            | flag(self.ends_shinecharged, ENDS_SHINECHARGED)
            | flag(self.regains_g_mode_mobility, REGAINS_G_MODE)
            | flag(!self.review.flash_suit, FLASH_UNREVIEWED)
            | flag(!self.review.blue_suit, BLUE_UNREVIEWED);
        w.varint(mask);
        w.doc_text(&self.name);
        write_list(&self.requires, w);
        if let Some(entry) = &self.entry {
            entry.write(w);
        }
        if let Some(exit) = &self.exit {
            exit.write(w);
        }
        macro_rules! lists {
            ($($field:ident),+) => {$(
                if !self.$field.is_empty() {
                    self.$field.write(w);
                }
            )+};
        }
        lists!(door_unlocks, clears_obstacles, sets_flags, resets_obstacles, collects_items, farm_drops, failures);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let mask = r.varint()?;
        if mask >= STRAT_MASK_LIMIT {
            return invalid("bad strat header");
        }
        let door_shell = match (mask & SHELL_MASK) >> SHELL_SHIFT {
            0 => DoorShellBypass::No,
            1 => DoorShellBypass::Yes,
            2 => DoorShellBypass::Free,
            _ => return invalid("bad door shell value"),
        };
        let name = r.doc_text()?;
        let requires = read_list(r)?;
        let entry = if mask & HAS_ENTRY != 0 { Some(DoorEntry::read(r)?) } else { None };
        let exit = if mask & HAS_EXIT != 0 { Some(DoorExit::read(r)?) } else { None };
        fn list<T: Codec>(r: &mut Reader<'_>, present: bool) -> Result<Vec<T>, PackError> {
            if present { Vec::read(r) } else { Ok(Vec::new()) }
        }
        let door_unlocks = list(r, mask & HAS_UNLOCKS != 0)?;
        let clears_obstacles = list(r, mask & HAS_CLEARS != 0)?;
        let sets_flags = list(r, mask & HAS_SETS_FLAGS != 0)?;
        let resets_obstacles = list(r, mask & HAS_RESETS != 0)?;
        let collects_items = list(r, mask & HAS_COLLECTS != 0)?;
        let farm_drops = list(r, mask & HAS_FARM != 0)?;
        let failures = list(r, mask & HAS_FAILURES != 0)?;
        Ok(Strat {
            name,
            entry,
            requires,
            exit,
            starts_shinecharged: mask & STARTS_SHINECHARGED != 0,
            ends_shinecharged: mask & ENDS_SHINECHARGED != 0,
            regains_g_mode_mobility: mask & REGAINS_G_MODE != 0,
            door_shell,
            door_unlocks,
            clears_obstacles,
            resets_obstacles,
            collects_items,
            sets_flags,
            farm_drops,
            wall_jump_avoid: mask & WALL_JUMP_AVOID != 0,
            failures,
            review: SuitReview { flash_suit: mask & FLASH_UNREVIEWED == 0, blue_suit: mask & BLUE_UNREVIEWED == 0 },
            doc: Doc::read(r)?,
        })
    }
}

/// Strat IDs mostly count up by one, so each is stored relative to the previous one.
/// Zero means the strat has no ID.
impl Codec for RoomStrat {
    fn write(&self, w: &mut Writer) {
        match self.source_id {
            None => w.varint(0),
            Some(id) => {
                let id = i64::from(id.0);
                w.varint(zigzag(id - w.previous_strat - 1) + 1);
                w.previous_strat = id;
            }
        }
        self.from.write(w);
        self.to.write(w);
        self.strat.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let source_id = match r.varint()? {
            0 => None,
            delta => {
                let id = unzigzag(delta - 1)
                    .checked_add(r.previous_strat)
                    .and_then(|id| id.checked_add(1))
                    .ok_or(PackError::Invalid("bad strat id".to_owned()))?;
                r.previous_strat = id;
                Some(source::StratId(u32::try_from(id).or_else(|_| invalid("bad strat id"))?))
            }
        };
        Ok(RoomStrat { source_id, from: Codec::read(r)?, to: Codec::read(r)?, strat: Codec::read(r)? })
    }
}

/// Map tile grids hold values from 0 to 3, so they pack to two bits per tile.
struct TileGrid;

impl TileGrid {
    fn write_rows(rows: &[Vec<u8>], w: &mut Writer) {
        let width = rows.first().map_or(0, Vec::len);
        w.varint(width as u64);
        w.varint(rows.len() as u64);
        let mut byte = 0u8;
        let mut filled = 0;
        for tile in rows.iter().flatten() {
            byte |= (tile & 3) << (filled * 2);
            filled += 1;
            if filled == 4 {
                w.logic.push(byte);
                byte = 0;
                filled = 0;
            }
        }
        if filled > 0 {
            w.logic.push(byte);
        }
    }

    fn read_rows(r: &mut Reader<'_>) -> Result<Vec<Vec<u8>>, PackError> {
        let width = r.varint()? as usize;
        let height = r.varint()? as usize;
        let tiles = width.checked_mul(height).ok_or(PackError::Truncated)?;
        let bytes = tiles.div_ceil(4);
        if bytes > r.logic.len() {
            return Err(PackError::Truncated);
        }
        let (packed, rest) = r.logic.split_at(bytes);
        r.logic = rest;
        let mut rows = Vec::with_capacity(height);
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let index = y * width + x;
                row.push((packed[index / 4] >> ((index % 4) * 2)) & 3);
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

fn check_grid(rows: &[Vec<u8>], what: &str) -> Result<(), PackError> {
    let width = rows.first().map_or(0, Vec::len);
    if rows.iter().any(|row| row.len() != width) {
        return invalid(format!("{what} has rows of different lengths"));
    }
    if rows.iter().flatten().any(|tile| *tile > 3) {
        return invalid(format!("{what} has a tile value above 3"));
    }
    Ok(())
}

impl Codec for Door {
    fn write(&self, w: &mut Writer) {
        self.style.write(w);
        self.orientation.write(w);
        self.address.write(w);
        self.environments.write(w);
        self.implicit.write(w);
        self.immediately_closed.write(w);
        self.twins.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Door {
            style: Codec::read(r)?,
            orientation: Codec::read(r)?,
            address: Codec::read(r)?,
            environments: Codec::read(r)?,
            implicit: Codec::read(r)?,
            immediately_closed: Codec::read(r)?,
            twins: Codec::read(r)?,
        })
    }
}

impl Codec for DoorEnvironment {
    fn write(&self, w: &mut Writer) {
        self.physics.write(w);
        self.entrances.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(DoorEnvironment { physics: Codec::read(r)?, entrances: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for RoomEnvironment {
    fn write(&self, w: &mut Writer) {
        self.heated.write(w);
        self.entrances.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(RoomEnvironment { heated: Codec::read(r)?, entrances: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

tagged!(Codec write read: NodeKind {
    0 => Door(door),
    1 => Entrance { transit, orientation, address },
    2 => Exit { transit, orientation, address },
    3 => Item { item, placement, address },
    4 => Junction(kind),
    5 => Station { station, services, orientation },
});

impl Codec for Lock {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        self.kind.write(w);
        self.active_when.write(w);
        self.unlock_strats.write(w);
        self.yields.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Lock {
            name: Codec::read(r)?,
            kind: Codec::read(r)?,
            active_when: Codec::read(r)?,
            unlock_strats: Codec::read(r)?,
            yields: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for Node {
    fn write(&self, w: &mut Writer) {
        self.id.write(w);
        self.name.write(w);
        self.kind.write(w);
        TileGrid::write_rows(&self.map_tiles, w);
        self.locks.write(w);
        self.viewable.write(w);
        self.yields.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Node {
            id: Codec::read(r)?,
            name: Codec::read(r)?,
            kind: Codec::read(r)?,
            map_tiles: TileGrid::read_rows(r)?,
            locks: Codec::read(r)?,
            viewable: Codec::read(r)?,
            yields: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for Obstacle {
    fn write(&self, w: &mut Writer) {
        self.key.write(w);
        w.doc_text(&self.name);
        self.kind.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Obstacle { key: Codec::read(r)?, name: r.doc_text()?, kind: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for RoomEnemy {
    fn write(&self, w: &mut Writer) {
        self.key.write(w);
        w.doc_text(&self.group_name);
        self.enemy.write(w);
        self.quantity.write(w);
        self.placement.write(w);
        self.spawn.write(w);
        self.stop_spawn.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(RoomEnemy {
            key: Codec::read(r)?,
            group_name: r.doc_text()?,
            enemy: Codec::read(r)?,
            quantity: Codec::read(r)?,
            placement: Codec::read(r)?,
            spawn: Codec::read(r)?,
            stop_spawn: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for Notable {
    fn write(&self, w: &mut Writer) {
        self.id.write(w);
        w.doc_text(&self.name);
        self.wall_jump_avoid.write(w);
        w.doc_note(&self.note);
        w.doc_note(&self.dev_note);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Notable {
            id: Codec::read(r)?,
            name: r.doc_text()?,
            wall_jump_avoid: Codec::read(r)?,
            note: r.doc_note()?,
            dev_note: r.doc_note()?,
        })
    }
}

impl Codec for Room {
    fn write(&self, w: &mut Writer) {
        w.previous_strat = 0;
        self.source_id.write(w);
        self.name.write(w);
        self.area.write(w);
        self.subarea.write(w);
        self.subsubarea.write(w);
        self.address.write(w);
        self.environments.write(w);
        TileGrid::write_rows(&self.map_tiles, w);
        self.nodes.write(w);
        self.obstacles.write(w);
        self.enemies.write(w);
        self.strats.write(w);
        self.notables.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        r.previous_strat = 0;
        Ok(Room {
            source_id: Codec::read(r)?,
            name: Codec::read(r)?,
            area: Codec::read(r)?,
            subarea: Codec::read(r)?,
            subsubarea: Codec::read(r)?,
            address: Codec::read(r)?,
            environments: Codec::read(r)?,
            map_tiles: TileGrid::read_rows(r)?,
            nodes: Codec::read(r)?,
            obstacles: Codec::read(r)?,
            enemies: Codec::read(r)?,
            strats: Codec::read(r)?,
            notables: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl<Id: Codec> Codec for Category<Id> {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        w.doc_text(&self.description);
        self.members.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Category { name: Codec::read(r)?, description: r.doc_text()?, members: Codec::read(r)? })
    }
}

impl Codec for ItemCatalog {
    fn write(&self, w: &mut Writer) {
        self.implicit.write(w);
        self.upgrades.write(w);
        self.expansions.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(ItemCatalog { implicit: Codec::read(r)?, upgrades: Codec::read(r)?, expansions: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for Tech {
    fn write(&self, w: &mut Writer) {
        self.source_id.write(w);
        self.name.write(w);
        self.tech_requires.write(w);
        self.other_requires.write(w);
        self.extensions.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Tech {
            source_id: Codec::read(r)?,
            name: Codec::read(r)?,
            tech_requires: Codec::read(r)?,
            other_requires: Codec::read(r)?,
            extensions: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for Helper {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        self.requires.write(w);
        self.map_rando_override.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Helper { name: Codec::read(r)?, requires: Codec::read(r)?, map_rando_override: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for Numeric {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        self.default.write(w);
        self.minimum.write(w);
        self.maximum.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Numeric { name: Codec::read(r)?, default: Codec::read(r)?, minimum: Codec::read(r)?, maximum: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for Weapon {
    fn write(&self, w: &mut Writer) {
        self.source_id.write(w);
        self.name.write(w);
        self.damage.write(w);
        self.cooldown_frames.write(w);
        self.use_requires.write(w);
        self.shot_requires.write(w);
        self.situational.write(w);
        self.hits_group.write(w);
        self.categories.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Weapon {
            source_id: Codec::read(r)?,
            name: Codec::read(r)?,
            damage: Codec::read(r)?,
            cooldown_frames: Codec::read(r)?,
            use_requires: Codec::read(r)?,
            shot_requires: Codec::read(r)?,
            situational: Codec::read(r)?,
            hits_group: Codec::read(r)?,
            categories: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for Enemy {
    fn write(&self, w: &mut Writer) {
        self.source_id.write(w);
        self.name.write(w);
        self.kind.write(w);
        self.attacks.write(w);
        self.hp.write(w);
        self.drop_count.write(w);
        self.drops.write(w);
        self.buffed_drops.write(w);
        self.farmable_drops.write(w);
        self.width.write(w);
        self.height.write(w);
        self.size_doc.write(w);
        self.freezable.write(w);
        self.grapplable.write(w);
        self.invulnerable_to.write(w);
        self.damage_multipliers.write(w);
        self.areas.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Enemy {
            source_id: Codec::read(r)?,
            name: Codec::read(r)?,
            kind: Codec::read(r)?,
            attacks: Codec::read(r)?,
            hp: Codec::read(r)?,
            drop_count: Codec::read(r)?,
            drops: Codec::read(r)?,
            buffed_drops: Codec::read(r)?,
            farmable_drops: Codec::read(r)?,
            width: Codec::read(r)?,
            height: Codec::read(r)?,
            size_doc: Codec::read(r)?,
            freezable: Codec::read(r)?,
            grapplable: Codec::read(r)?,
            invulnerable_to: Codec::read(r)?,
            damage_multipliers: Codec::read(r)?,
            areas: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for DamageWindow {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        self.percent.write(w);
        self.requires.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(DamageWindow { name: Codec::read(r)?, percent: Codec::read(r)?, requires: Codec::read(r)?, doc: Codec::read(r)? })
    }
}

impl Codec for IncomingDamage {
    fn write(&self, w: &mut Writer) {
        self.name.write(w);
        self.attack.write(w);
        self.frequency_frames.write(w);
        self.avoiding_requires.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(IncomingDamage {
            name: Codec::read(r)?,
            attack: Codec::read(r)?,
            frequency_frames: Codec::read(r)?,
            avoiding_requires: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for BossScenario {
    fn write(&self, w: &mut Writer) {
        self.source_id.write(w);
        self.name.write(w);
        self.boss.write(w);
        self.weapons.write(w);
        self.requires.write(w);
        self.dodge_rate.write(w);
        self.attack_opportunity_duration.write(w);
        self.damage_windows.write(w);
        self.incoming_damage.write(w);
        self.particle_frequency_frames.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(BossScenario {
            source_id: Codec::read(r)?,
            name: Codec::read(r)?,
            boss: Codec::read(r)?,
            weapons: Codec::read(r)?,
            requires: Codec::read(r)?,
            dodge_rate: Codec::read(r)?,
            attack_opportunity_duration: Codec::read(r)?,
            damage_windows: Codec::read(r)?,
            incoming_damage: Codec::read(r)?,
            particle_frequency_frames: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

impl Codec for ConnectionLabel {
    fn write(&self, w: &mut Writer) {
        w.doc_text(&Some(self.area.clone()));
        w.doc_text(&Some(self.subarea.clone()));
        w.doc_text(&Some(self.room.clone()));
        w.doc_text(&Some(self.node.clone()));
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let mut text = || -> Result<String, PackError> { Ok(r.doc_text()?.unwrap_or_default()) };
        Ok(ConnectionLabel { area: text()?, subarea: text()?, room: text()?, node: text()? })
    }
}

impl Codec for ConnectionEnd {
    fn write(&self, w: &mut Writer) {
        self.room.write(w);
        self.node.write(w);
        self.position.write(w);
        if w.docs_enabled {
            w.doc_varint(u64::from(self.label.is_some()));
            if let Some(label) = &self.label {
                label.write(w);
            }
        }
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let room = Codec::read(r)?;
        let node = Codec::read(r)?;
        let position = Codec::read(r)?;
        let label = if r.doc_varint()? == 1 { Some(ConnectionLabel::read(r)?) } else { None };
        Ok(ConnectionEnd { room, node, position, label, doc: Codec::read(r)? })
    }
}

impl Codec for Connection {
    fn write(&self, w: &mut Writer) {
        self.kind.write(w);
        self.direction.write(w);
        w.doc_text(&self.description);
        self.ends.write(w);
        self.doc.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(Connection {
            kind: Codec::read(r)?,
            direction: Codec::read(r)?,
            description: r.doc_text()?,
            ends: Codec::read(r)?,
            doc: Codec::read(r)?,
        })
    }
}

fn write_records(world: &World, w: &mut Writer) {
    world.items.write(w);
    world.start.write(w);
    world.flags.write(w);
    world.internals.write(w);
    world.techs.write(w);
    world.tech_categories.write(w);
    world.helpers.write(w);
    world.helper_categories.write(w);
    world.numerics.write(w);
    world.numeric_categories.write(w);
    world.weapons.write(w);
    world.weapon_categories.write(w);
    world.enemies.write(w);
    world.boss_scenarios.write(w);
    world.rooms.write(w);
    world.connections.write(w);
}

fn read_records(r: &mut Reader<'_>) -> Result<World, PackError> {
    Ok(World {
        items: Codec::read(r)?,
        start: Codec::read(r)?,
        flags: Codec::read(r)?,
        internals: Codec::read(r)?,
        techs: Codec::read(r)?,
        tech_categories: Codec::read(r)?,
        helpers: Codec::read(r)?,
        helper_categories: Codec::read(r)?,
        numerics: Codec::read(r)?,
        numeric_categories: Codec::read(r)?,
        weapons: Codec::read(r)?,
        weapon_categories: Codec::read(r)?,
        enemies: Codec::read(r)?,
        boss_scenarios: Codec::read(r)?,
        rooms: Codec::read(r)?,
        connections: Codec::read(r)?,
        lookup: Lookup::default(),
    })
}

fn put_strings(out: &mut Vec<u8>, strings: &[String]) {
    put_varint(out, strings.len() as u64);
    for text in strings {
        put_varint(out, text.len() as u64);
        out.extend_from_slice(text.as_bytes());
    }
}

fn take_strings(bytes: &mut &[u8]) -> Result<Vec<String>, PackError> {
    let count = take_varint(bytes)? as usize;
    if count > bytes.len() {
        return Err(PackError::Truncated);
    }
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        let length = take_varint(bytes)? as usize;
        if length > bytes.len() {
            return Err(PackError::Truncated);
        }
        let (text, rest) = bytes.split_at(length);
        *bytes = rest;
        strings.push(String::from_utf8(text.to_vec()).or_else(|_| invalid("string is not UTF-8"))?);
    }
    Ok(strings)
}

fn check_world_for_encoding(world: &World) -> Result<(), PackError> {
    for room in &world.rooms {
        check_grid(&room.map_tiles, &room.name)?;
        for node in &room.nodes {
            check_grid(&node.map_tiles, &node.name)?;
        }
    }
    Ok(())
}

/// Writes a world as a pack.
pub fn encode(world: &World, options: &EncodeOptions) -> Result<Vec<u8>, PackError> {
    check_world_for_encoding(world)?;
    let mut w = Writer {
        logic: Vec::new(),
        docs: Vec::new(),
        docs_enabled: options.docs,
        strings: Pool::default(),
        doc_strings: Pool::default(),
        requirements: Pool::default(),
        lists: Pool::default(),
        entries: Pool::default(),
        exits: Pool::default(),
        previous_strat: 0,
    };
    write_records(world, &mut w);
    let records = std::mem::take(&mut w.logic);
    let record_docs = std::mem::take(&mut w.docs);

    let mut logic = Vec::with_capacity(records.len() * 2);
    put_strings(&mut logic, &w.strings.items);
    let mut docs = Vec::new();
    if options.docs {
        put_strings(&mut docs, &w.doc_strings.items);
    }
    for pool in [&w.requirements, &w.lists, &w.entries, &w.exits] {
        put_varint(&mut logic, pool.items.len() as u64);
        for (item_logic, item_docs) in &pool.items {
            logic.extend_from_slice(item_logic);
            docs.extend_from_slice(item_docs);
        }
    }
    logic.extend_from_slice(&records);
    docs.extend_from_slice(&record_docs);

    let mut out = Vec::with_capacity(logic.len() + docs.len() + 32);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(if options.docs { FLAG_DOCS } else { 0 });
    out.extend_from_slice(&[0; 8]);
    put_varint(&mut out, options.source.len() as u64);
    out.extend_from_slice(options.source.as_bytes());
    put_varint(&mut out, logic.len() as u64);
    put_varint(&mut out, docs.len() as u64);
    out.extend_from_slice(&logic);
    out.extend_from_slice(&docs);
    let sum = checksum(&out[BODY_AT..]);
    out[CHECKSUM_AT..BODY_AT].copy_from_slice(&sum.to_le_bytes());
    Ok(out)
}

/// FNV-1a, which is simple and catches accidental damage. It isn't tamper-proof.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash: u64, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

struct Sections<'a> {
    info: PackInfo,
    logic: &'a [u8],
    docs: Option<&'a [u8]>,
}

fn split_sections(mut bytes: &[u8]) -> Result<Sections<'_>, PackError> {
    let rest = bytes.strip_prefix(MAGIC).ok_or(PackError::NotAPack)?;
    let (&version, rest) = rest.split_first().ok_or(PackError::Truncated)?;
    if version != VERSION {
        return Err(PackError::UnsupportedVersion(version));
    }
    let (&flags, rest) = rest.split_first().ok_or(PackError::Truncated)?;
    if flags & !FLAG_DOCS != 0 {
        return invalid("unknown flags");
    }
    let (_, rest) = rest.split_first_chunk::<8>().ok_or(PackError::Truncated)?;
    bytes = rest;
    let source_length = take_varint(&mut bytes)? as usize;
    if source_length > bytes.len() {
        return Err(PackError::Truncated);
    }
    let (source, rest) = bytes.split_at(source_length);
    let source = String::from_utf8(source.to_vec()).or_else(|_| invalid("source is not UTF-8"))?;
    bytes = rest;
    let logic_bytes = take_varint(&mut bytes)? as usize;
    let docs_bytes = take_varint(&mut bytes)? as usize;
    if logic_bytes.checked_add(docs_bytes) != Some(bytes.len()) {
        return Err(PackError::Truncated);
    }
    if flags & FLAG_DOCS == 0 && docs_bytes != 0 {
        return invalid("docs section without the docs flag");
    }
    let (logic, docs) = bytes.split_at(logic_bytes);
    let docs = (flags & FLAG_DOCS != 0).then_some(docs);
    Ok(Sections { info: PackInfo { version, source, logic_bytes, docs_bytes }, logic, docs })
}

/// Reads a pack's header without decoding it.
pub fn inspect(bytes: &[u8]) -> Result<PackInfo, PackError> {
    Ok(split_sections(bytes)?.info)
}

fn read_table<T>(
    r: &mut Reader<'_>,
    read: impl Fn(&mut Reader<'_>) -> Result<T, PackError>,
    push: impl Fn(&mut Tables, (T, u64)),
) -> Result<(), PackError> {
    let count = take_varint(&mut r.logic)? as usize;
    if count > r.logic.len() {
        return Err(PackError::Truncated);
    }
    for _ in 0..count {
        let before = r.budget;
        let value = read(r)?;
        // What this entry copied from earlier entries, plus the entry itself.
        let weight = before - r.budget + 1;
        push(&mut r.tables, (value, weight));
    }
    Ok(())
}

/// Reads and validates a pack.
pub fn decode(bytes: &[u8], docs: Docs) -> Result<World, PackError> {
    split_sections(bytes)?;
    let stored = bytes[CHECKSUM_AT..BODY_AT].try_into().expect("the range is eight bytes");
    if u64::from_le_bytes(stored) != checksum(&bytes[BODY_AT..]) {
        return Err(PackError::Corrupt);
    }
    decode_unverified(bytes, docs)
}

/// Decodes without the checksum, so that tests can reach the structural checks with
/// damaged input. Damage can then produce a valid but different world.
#[doc(hidden)]
pub fn decode_unverified(bytes: &[u8], docs: Docs) -> Result<World, PackError> {
    let sections = split_sections(bytes)?;
    let mut logic = sections.logic;
    let strings = take_strings(&mut logic)?;
    let mut doc_bytes = if docs == Docs::Load { sections.docs } else { None };
    let doc_strings = match &mut doc_bytes {
        Some(section) => take_strings(section)?,
        None => Vec::new(),
    };
    let mut r = Reader {
        logic,
        docs: doc_bytes,
        tables: Tables {
            strings,
            doc_strings,
            requirements: Vec::new(),
            lists: Vec::new(),
            entries: Vec::new(),
            exits: Vec::new(),
        },
        budget: EXPANSION_BUDGET,
        previous_strat: 0,
    };
    read_table(&mut r, Requirement::read_body, |t, entry| t.requirements.push(entry))?;
    read_table(&mut r, Vec::<Requirement>::read, |t, entry| t.lists.push(entry))?;
    read_table(&mut r, DoorEntry::read_body, |t, entry| t.entries.push(entry))?;
    read_table(&mut r, DoorExit::read_body, |t, entry| t.exits.push(entry))?;
    let mut world = read_records(&mut r)?;
    if !r.logic.is_empty() {
        return invalid("trailing bytes in the logic section");
    }
    if r.docs.is_some_and(|docs| !docs.is_empty()) {
        return invalid("trailing bytes in the docs section");
    }
    world.validate().map_err(PackError::Invalid)?;
    world.rebuild_index();
    Ok(world)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_writer() -> Writer {
        Writer {
            logic: Vec::new(),
            docs: Vec::new(),
            docs_enabled: true,
            strings: Pool::default(),
            doc_strings: Pool::default(),
            requirements: Pool::default(),
            lists: Pool::default(),
            entries: Pool::default(),
            exits: Pool::default(),
            previous_strat: 0,
        }
    }

    fn empty_reader(bytes: &[u8]) -> Reader<'_> {
        Reader {
            logic: bytes,
            docs: None,
            tables: Tables {
                strings: Vec::new(),
                doc_strings: Vec::new(),
                requirements: Vec::new(),
                lists: Vec::new(),
                entries: Vec::new(),
                exits: Vec::new(),
            },
            budget: EXPANSION_BUDGET,
            previous_strat: 0,
        }
    }

    #[test]
    fn varints_round_trip() {
        for value in [0, 1, 127, 128, 16_383, 16_384, u64::from(u32::MAX), u64::MAX] {
            let mut bytes = Vec::new();
            put_varint(&mut bytes, value);
            let mut slice = &bytes[..];
            assert_eq!(take_varint(&mut slice), Ok(value));
            assert!(slice.is_empty());
        }
        assert_eq!(take_varint(&mut &[0x80][..]), Err(PackError::Truncated));
    }

    #[test]
    fn numbers_use_the_smallest_form_and_round_trip() {
        for (value, size) in [(0.0, 1), (15.0, 1), (-3.0, 1), (45.0, 2), (0.5, 3), (28.2472, 4), (1.0 / 3.0, 9)] {
            let mut w = empty_writer();
            value.write(&mut w);
            assert_eq!(w.logic.len(), size, "{value}");
            assert_eq!(f64::read(&mut empty_reader(&w.logic)), Ok(value));
        }
    }

    #[test]
    fn zigzag_round_trips() {
        for value in [0, 1, -1, 12_345, -12_345, i64::MAX, i64::MIN] {
            assert_eq!(unzigzag(zigzag(value)), value);
        }
    }

    #[test]
    fn tile_grids_pack_to_two_bits() {
        let rows = vec![vec![0, 1, 2], vec![2, 1, 0]];
        let mut w = empty_writer();
        TileGrid::write_rows(&rows, &mut w);
        assert_eq!(w.logic.len(), 2 + 2);
        assert_eq!(TileGrid::read_rows(&mut empty_reader(&w.logic)).unwrap(), rows);
    }

    #[test]
    fn headers_are_checked() {
        assert_eq!(decode(b"nope", Docs::Skip), Err(PackError::NotAPack));
        assert_eq!(decode(b"SMLP", Docs::Skip), Err(PackError::Truncated));
        assert_eq!(decode(b"SMLP\xff\0", Docs::Skip), Err(PackError::UnsupportedVersion(255)));
        assert_eq!(checksum(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
