//! Logical requirements, the expression language used throughout the data.

use crate::raw::*;
use serde::de::{self, value::MapAccessDeserializer, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::skip_serializing_none;
use std::fmt;
use std::str::FromStr;

/// A requirement written as a bare string, classified by its naming convention.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Named {
    Free,
    Never,
    Item(Item),
    Flag(FlagName),
    Helper(HelperName),
    /// A tech, including the requirements it inherits. Compare [`Requirement::TechOnly`].
    Tech(TechName),
    /// Appears only in Map Rando's fork of the data.
    Internal(InternalName),
}

impl FromStr for Named {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        Ok(match name {
            "free" => Named::Free,
            "never" => Named::Never,
            _ if name.starts_with("f_") => Named::Flag(name.into()),
            _ if name.starts_with("h_") => Named::Helper(name.into()),
            _ if name.starts_with("i_") => Named::Internal(name.into()),
            _ if name.starts_with("can") => Named::Tech(name.into()),
            _ => Named::Item(
                name.parse()
                    .map_err(|_| format!("unrecognized requirement name `{name}`"))?,
            ),
        })
    }
}

impl TryFrom<String> for Named {
    type Error = String;

    fn try_from(name: String) -> Result<Self, String> {
        name.parse()
    }
}

impl From<Named> for String {
    fn from(named: Named) -> String {
        match named {
            Named::Free => "free".to_owned(),
            Named::Never => "never".to_owned(),
            Named::Item(item) => item.as_str().to_owned(),
            Named::Flag(FlagName(name))
            | Named::Helper(HelperName(name))
            | Named::Tech(TechName(name))
            | Named::Internal(InternalName(name)) => name,
        }
    }
}

/// One element of a `requires` list. Elements of a list are implicitly ANDed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Requirement {
    /// A bare string such as `"Morph"` or `"canWalljump"`.
    #[serde(skip)]
    Named(Named),
    And(Vec<Requirement>),
    Or(Vec<Requirement>),
    Not(Named),
    Ammo(AmmoAmount),
    AmmoDrain(AmmoAmount),
    Refill(Vec<ResourceKind>),
    PartialRefill(PartialRefill),
    EnemyKill(EnemyKill),
    EnemyDamage(EnemyDamage),
    AcidFrames(Numeric),
    GravitylessAcidFrames(Numeric),
    DraygonElectricityFrames(Numeric),
    ShineChargeFrames(Numeric),
    CycleFrames(Numeric),
    SimpleCycleFrames(Numeric),
    HeatFrames(Numeric),
    SimpleHeatFrames(Numeric),
    SuitlessHeatFrames(Numeric),
    GravitylessHeatFrames(Numeric),
    HeatFramesWithEnergyDrops(FramesWithDrops),
    LavaFrames(Numeric),
    GravitylessLavaFrames(Numeric),
    LavaFramesWithEnergyDrops(FramesWithDrops),
    SamusEaterCycles(Numeric),
    MetroidFrames(Numeric),
    HibashiHits(Numeric),
    SpikeHits(Numeric),
    ThornHits(Numeric),
    ElectricityHits(Numeric),
    ResourceAtMost(Vec<ResourceAmount>),
    ResourceCapacity(Vec<ResourceAmount>),
    ResourceMaxCapacity(Vec<ResourceAmount>),
    ResourceAvailable(Vec<ResourceAmount>),
    ResourceConsumed(Vec<ResourceAmount>),
    ResourceMissingAtMost(Vec<ResourceAmount>),
    AutoReserveTrigger(AutoReserveTrigger),
    DoorUnlockedAtNode(NodeId),
    ObstaclesCleared(Vec<ObstacleId>),
    ObstaclesNotCleared(Vec<ObstacleId>),
    CanShineCharge(ShineChargeRunway),
    GetBlueSpeed(ShineChargeRunway),
    SpeedBall(Runway),
    Shinespark(Shinespark),
    ResetRoom(ResetRoom),
    ItemNotCollectedAtNode(NodeId),
    ItemCollectedAtNode(NodeId),
    GainFlashSuit {},
    UseFlashSuit {},
    NoFlashSuit {},
    GainBlueSuit {},
    HaveBlueSuit {},
    BlueSuitShinecharge {},
    NoBlueSuit {},
    /// A tech alone, without the item or ammo requirements it would normally inherit.
    #[serde(rename = "tech")]
    TechOnly(TechName),
    /// A named technique specific to the room, looked up in its `notables`.
    Notable(String),
    DisableEquipment(Item),
    RidleyKill(RidleyKill),
}

impl Requirement {
    /// Calls `visit` on this requirement and everything nested in `and`/`or`.
    pub fn walk(&self, visit: &mut impl FnMut(&Requirement)) {
        visit(self);
        if let Requirement::And(children) | Requirement::Or(children) = self {
            for child in children {
                child.walk(visit);
            }
        }
    }
}

impl<'de> Deserialize<'de> for Requirement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RequirementVisitor;

        impl<'de> Visitor<'de> for RequirementVisitor {
            type Value = Requirement;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a requirement name or single-key requirement object")
            }

            fn visit_str<E: de::Error>(self, name: &str) -> Result<Requirement, E> {
                name.parse().map(Requirement::Named).map_err(E::custom)
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Requirement, M::Error> {
                Requirement::deserialize(MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(RequirementVisitor)
    }
}

impl Serialize for Requirement {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Requirement::Named(named) => named.serialize(serializer),
            other => Requirement::serialize(other, serializer),
        }
    }
}



#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmmoAmount {
    #[serde(rename = "type")]
    pub kind: AmmoKind,
    pub count: Numeric,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAmount {
    #[serde(rename = "type")]
    pub kind: ResourceKind,
    pub count: Numeric,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialRefill {
    #[serde(rename = "type")]
    pub kind: ResourceKind,
    pub limit: Numeric,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnemyKill {
    /// Groups of enemies; each group can be hit by one area-of-effect attack.
    pub enemies: Vec<Vec<String>>,
    pub explicit_weapons: Option<Vec<String>>,
    pub excluded_weapons: Option<Vec<String>>,
    pub farmable_ammo: Option<Vec<AmmoKind>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyDamage {
    pub enemy: String,
    /// The attack's name in the enemy's list of attacks.
    #[serde(rename = "type")]
    pub attack: String,
    pub hits: Numeric,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyDrop {
    pub enemy: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramesWithDrops {
    pub frames: Numeric,
    pub drops: Vec<EnemyDrop>,
}


#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoReserveTrigger {
    /// Defaults to 1 when absent.
    pub min_reserve_energy: Option<Numeric>,
    /// Defaults to 400 when absent.
    pub max_reserve_energy: Option<Numeric>,
    /// Defaults to [`ImplicitHeatFrames::Yes`] when absent.
    pub implicit_heat_frames: Option<ImplicitHeatFrames>,
}

/// Runway geometry measured by the tiles actually used.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShineChargeRunway {
    pub used_tiles: f64,
    pub open_end: u8,
    pub gentle_up_tiles: Option<u32>,
    pub gentle_down_tiles: Option<u32>,
    pub steep_up_tiles: Option<u32>,
    pub steep_down_tiles: Option<u32>,
    pub starting_steep_down_tiles: Option<u32>,
    pub starting_gentle_down_tiles: Option<u32>,
    pub starting_stutterless_tiles: Option<u32>,
}

/// Runway geometry measured by total length.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Runway {
    pub length: f64,
    pub open_end: u8,
    pub gentle_up_tiles: Option<u32>,
    pub gentle_down_tiles: Option<u32>,
    pub steep_up_tiles: Option<u32>,
    pub steep_down_tiles: Option<u32>,
    pub starting_steep_down_tiles: Option<u32>,
    pub starting_gentle_down_tiles: Option<u32>,
    pub starting_stutterless_tiles: Option<u32>,
}

#[skip_serializing_none]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shinespark {
    pub frames: u32,
    /// Frames beyond those needed for the objective; lowers the minimum energy cost.
    pub excess_frames: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRoom {
    pub nodes: Vec<NodeId>,
}


#[skip_serializing_none]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RidleyKill {
    /// Defaults to `true` when absent.
    pub power_bombs: Option<bool>,
    /// Defaults to `false` when absent.
    pub g_mode: Option<bool>,
    pub stuck: Option<RidleyStuck>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<Requirement, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn strings_are_classified_by_convention() {
        let cases = [
            ("free", Named::Free),
            ("Morph", Named::Item(Item::Morph)),
            ("f_DefeatedKraid", Named::Flag("f_DefeatedKraid".into())),
            ("h_canFly", Named::Helper("h_canFly".into())),
            ("canWalljump", Named::Tech("canWalljump".into())),
        ];
        for (name, expected) in cases {
            let requirement = parse(&format!("\"{name}\"")).unwrap();
            assert_eq!(requirement, Requirement::Named(expected));
            assert_eq!(serde_json::to_string(&requirement).unwrap(), format!("\"{name}\""));
        }
    }

    #[test]
    fn nested_objects_round_trip() {
        let json = r#"{"or":["Morph",{"and":[{"heatFrames":{"mul":[60,"n_x"]}},{"gainFlashSuit":{}}]}]}"#;
        let requirement = parse(json).unwrap();
        assert_eq!(serde_json::to_string(&requirement).unwrap(), json);
    }

    #[test]
    fn invalid_input_is_rejected() {
        assert!(parse(r#""NotAnItem""#).is_err());
        assert!(parse(r#"{"madeUp": 1}"#).is_err());
        assert!(parse(r#"{"ammo": {"type": "Missile", "count": 1, "extra": 1}}"#).is_err());
        assert!(parse(r#"{"heatFrames": 1, "lavaFrames": 2}"#).is_err());
    }

    #[test]
    fn unknown_requirement_error_names_the_key() {
        let error = parse(r#"{"madeUp": 1}"#).unwrap_err().to_string();
        assert!(error.contains("madeUp"), "{error}");
    }

    #[test]
    fn walk_reaches_nested_requirements() {
        let requirement = parse(r#"{"or":["Morph",{"and":["Bombs"]}]}"#).unwrap();
        let mut count = 0;
        requirement.walk(&mut |_| count += 1);
        assert_eq!(count, 4);
    }
}
