//! Strats and the entrance and exit conditions that connect them across doors.

use crate::raw::*;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_with::skip_serializing_none;
use std::fmt;

/// A series of maneuvers within a room, usually from one node to another.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Strat {
    /// Absent on strats nested in locks and viewable nodes.
    pub id: Option<StratId>,
    /// The `[from, to]` nodes. Absent on strats nested in locks and viewable nodes.
    pub link: Option<[NodeId; 2]>,
    pub name: String,
    pub entrance_condition: Option<EntranceCondition>,
    pub starts_with_shine_charge: Option<bool>,
    pub requires: Vec<Requirement>,
    pub exit_condition: Option<ExitCondition>,
    pub g_mode_regain_mobility: Option<Empty>,
    /// Defaults to [`DoorShellBypass::No`] when absent.
    pub bypasses_door_shell: Option<DoorShellBypass>,
    pub unlocks_doors: Option<Vec<UnlockDoor>>,
    pub clears_obstacles: Option<Vec<ObstacleId>>,
    pub resets_obstacles: Option<Vec<ObstacleId>>,
    pub collects_items: Option<Vec<NodeId>>,
    pub sets_flags: Option<Vec<FlagName>>,
    pub ends_with_shine_charge: Option<bool>,
    pub farm_cycle_drops: Option<Vec<EnemyDrop>>,
    /// Defaults to `false` when absent.
    pub wall_jump_avoid: Option<bool>,
    pub flash_suit_checked: bool,
    pub blue_suit_checked: bool,
    pub failures: Option<Vec<StratFailure>>,
    pub note: Option<Note>,
    pub detail_note: Option<Note>,
    pub dev_note: Option<Note>,
}

impl Strat {
    /// The node the strat starts from, if it has a link.
    pub fn from_node(&self) -> Option<NodeId> {
        self.link.map(|[from, _]| from)
    }

    /// The node the strat ends at, if it has a link.
    pub fn to_node(&self) -> Option<NodeId> {
        self.link.map(|[_, to]| to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoorShellBypass {
    Yes,
    No,
    Free,
}


#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnlockDoor {
    /// Defaults to the strat's destination node when absent.
    pub node_id: Option<NodeId>,
    pub types: Vec<DoorUnlockType>,
    pub requires: Option<Vec<Requirement>>,
    /// Defaults to `true` when absent.
    pub use_implicit_requires: Option<bool>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StratFailure {
    pub name: String,
    pub leads_to_node: Option<NodeId>,
    pub cost: Option<Vec<Requirement>>,
    /// Defaults to `false` when absent.
    pub softlock: Option<bool>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

/// How a strat must be entered, plus modifiers that apply to any way of entering.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntranceCondition {
    #[serde(flatten)]
    pub kind: EntranceKind,
    /// Defaults to [`TriState::No`] when absent.
    pub comes_through_toilet: Option<TriState>,
    /// Defaults to [`TriState::Any`] when absent.
    pub comes_in_heated: Option<TriState>,
    pub dev_note: Option<Note>,
}

// Written by hand because `flatten` cannot be combined with `deny_unknown_fields`.
impl<'de> Deserialize<'de> for EntranceCondition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ConditionVisitor;

        impl<'de> Visitor<'de> for ConditionVisitor {
            type Value = EntranceCondition;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an entrance condition object")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut kind = None;
                let mut comes_through_toilet = None;
                let mut comes_in_heated = None;
                let mut dev_note = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "comesThroughToilet" => comes_through_toilet = Some(map.next_value()?),
                        "comesInHeated" => comes_in_heated = Some(map.next_value()?),
                        "devNote" => dev_note = Some(map.next_value()?),
                        _ => {
                            if kind.is_some() {
                                return Err(de::Error::custom(format!(
                                    "entrance condition has more than one kind, including `{key}`"
                                )));
                            }
                            let value: serde_json::Value = map.next_value()?;
                            let tagged = serde_json::json!({ key: value });
                            kind = Some(EntranceKind::deserialize(tagged).map_err(de::Error::custom)?);
                        }
                    }
                }
                Ok(EntranceCondition {
                    kind: kind.ok_or_else(|| de::Error::custom("entrance condition has no kind"))?,
                    comes_through_toilet,
                    comes_in_heated,
                    dev_note,
                })
            }
        }

        deserializer.deserialize_map(ConditionVisitor)
    }
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum EntranceKind {
    ComeInNormally {},
    ComeInRunning {
        speed_booster: TriState,
        min_tiles: f64,
        max_tiles: Option<f64>,
    },
    ComeInJumping {
        speed_booster: TriState,
        min_tiles: f64,
        max_tiles: Option<f64>,
    },
    ComeInSpaceJumping {
        speed_booster: TriState,
        min_tiles: f64,
        max_tiles: Option<f64>,
    },
    ComeInBlueSpaceJumping {
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
    },
    ComeInShinecharging(Runway),
    ComeInGettingBlueSpeed {
        length: f64,
        open_end: u8,
        gentle_up_tiles: Option<u32>,
        gentle_down_tiles: Option<u32>,
        steep_up_tiles: Option<u32>,
        steep_down_tiles: Option<u32>,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
    },
    ComeInShinecharged {},
    ComeInShinechargedJumping {},
    ComeInWithSpark {
        position: Option<SparkPosition>,
    },
    ComeInStutterShinecharging {
        min_tiles: f64,
    },
    ComeInStutterGettingBlueSpeed {
        min_tiles: f64,
    },
    ComeInWithBombBoost {},
    ComeInWithDoorStuckSetup {},
    ComeInSpeedballing {
        runway: Runway,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
    },
    ComeInWithTemporaryBlue {
        direction: Option<SidePosition>,
    },
    ComeInSpinning {
        speed_booster: TriState,
        unusable_tiles: f64,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
    },
    ComeInBlueSpinning {
        unusable_tiles: f64,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
    },
    ComeInWithMockball {
        speed_booster: TriState,
        adjacent_min_tiles: Option<f64>,
        remote_and_landing_min_tiles: Option<Vec<[f64; 2]>>,
    },
    ComeInWithSpringBallBounce {
        speed_booster: TriState,
        movement_type: MovementType,
        adjacent_min_tiles: Option<f64>,
        remote_and_landing_min_tiles: Option<Vec<[f64; 2]>>,
    },
    ComeInWithBlueSpringBallBounce {
        movement_type: MovementType,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
        min_landing_tiles: Option<f64>,
    },
    ComeInWithStoredFallSpeed {
        fall_speed_in_tiles: u32,
    },
    ComeInWithKnockback {},
    ComeInWithDamageBoost {
        i_frames_needed: u32,
        /// Defaults to [`DamageBoostEntry::Any`] when absent.
        position: Option<DamageBoostEntry>,
    },
    ComeInWithIFrames {
        i_frames_needed: u32,
    },
    ComeInWithRMode {},
    ComeInWithGMode {
        mode: GModeMode,
        morphed: bool,
        /// Defaults to [`GModeMobility::Any`] when absent.
        mobility: Option<GModeMobility>,
    },
    ComeInWithXMode {},
    ComeInWithWallJumpBelow {
        min_height: Option<f64>,
    },
    ComeInWithSpaceJumpBelow {},
    ComeInWithPlatformBelow {
        min_height: Option<f64>,
        max_height: Option<f64>,
        max_left_position: Option<f64>,
        min_right_position: Option<f64>,
    },
    ComeInWithSidePlatform {
        platforms: Vec<SidePlatform>,
    },
    ComeInWithGrappleSwing {
        blocks: Vec<GrappleSwingBlock>,
    },
    ComeInWithGrappleJump {
        position: SidePosition,
    },
    ComeInWithGrappleTeleport {
        block_positions: Vec<[i32; 2]>,
    },
    ComeInWithSamusEaterTeleport {
        floor_positions: Vec<[i32; 2]>,
        ceiling_positions: Vec<[i32; 2]>,
    },
    ComeInWithSuperSink {},
}

/// How Samus leaves through a door, matched against the next room's entrance condition.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ExitCondition {
    LeaveNormally {},
    LeaveWithRunway {
        length: f64,
        open_end: u8,
        gentle_up_tiles: Option<u32>,
        gentle_down_tiles: Option<u32>,
        steep_up_tiles: Option<u32>,
        steep_down_tiles: Option<u32>,
        starting_steep_down_tiles: Option<u32>,
        starting_gentle_down_tiles: Option<u32>,
        starting_stutterless_tiles: Option<u32>,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        heated: Option<bool>,
    },
    LeaveShinecharged {},
    LeaveWithTemporaryBlue {
        direction: Option<SidePosition>,
    },
    LeaveWithSpark {
        position: Option<SparkPosition>,
        grounded: Option<bool>,
    },
    LeaveSpinning {
        remote_runway: Runway,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
        /// Defaults to [`TriState::Any`] when absent.
        blue: Option<TriState>,
    },
    LeaveWithMockball {
        remote_runway: Runway,
        landing_runway: Runway,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
        /// Defaults to [`TriState::Any`] when absent.
        blue: Option<TriState>,
    },
    LeaveWithSpringBallBounce {
        remote_runway: Runway,
        landing_runway: Runway,
        movement_type: MovementType,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
        /// Defaults to [`TriState::Any`] when absent.
        blue: Option<TriState>,
    },
    LeaveSpaceJumping {
        remote_runway: Runway,
        min_extra_run_speed: Option<ExtraRunSpeed>,
        max_extra_run_speed: Option<ExtraRunSpeed>,
        /// Defaults to [`TriState::Any`] when absent.
        blue: Option<TriState>,
    },
    LeaveWithStoredFallSpeed {
        fall_speed_in_tiles: u32,
    },
    LeaveWithDamage {
        enemy: String,
        /// Defaults to `contact` when absent.
        attack: Option<String>,
        /// Defaults to `true` when absent.
        knockback: Option<bool>,
    },
    LeaveWithKnockback {},
    LeaveWithDamageBoost {
        i_frames_remaining: u32,
        /// Defaults to [`DamageBoostExit::NotHigh`] when absent.
        position: Option<DamageBoostExit>,
    },
    LeaveWithIFrames {
        i_frames_remaining: u32,
    },
    LeaveWithXModeSetup {
        enemy: Option<String>,
        /// Defaults to `contact` when absent.
        attack: Option<String>,
    },
    LeaveWithGMode {
        morphed: bool,
    },
    LeaveWithDoorFrameBelow {
        height: f64,
    },
    LeaveWithPlatformBelow {
        height: f64,
        left_position: f64,
        right_position: f64,
    },
    LeaveWithSidePlatform {
        height: f64,
        runway: Runway,
        obstruction: [i32; 2],
    },
    LeaveWithGrappleSwing {
        blocks: Vec<GrappleSwingBlock>,
    },
    LeaveWithGrappleJump {
        position: SidePosition,
    },
    LeaveWithGrappleTeleport {
        block_positions: Vec<[i32; 2]>,
    },
    LeaveWithSamusEaterTeleport {
        floor_positions: Vec<[i32; 2]>,
        ceiling_positions: Vec<[i32; 2]>,
    },
    LeaveWithSuperSink {},
}



#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DamageBoostEntry {
    High,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DamageBoostExit {
    NotHigh,
    High,
}





#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidePlatform {
    pub min_height: f64,
    pub max_height: f64,
    pub min_tiles: f64,
    pub speed_booster: TriState,
    pub obstructions: Vec<[i32; 2]>,
    /// Defaults to [`Environment::Any`] when absent.
    pub environment: Option<Environment>,
    pub requires: Option<Vec<Requirement>>,
    pub note: Option<Note>,
    pub detail_note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrappleSwingBlock {
    pub position: [f64; 2],
    /// Defaults to [`Environment::Air`] when absent.
    pub environment: Option<Environment>,
    pub obstructions: Option<Vec<[i32; 2]>>,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrance(json: &str) -> Result<EntranceCondition, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn entrance_kind_and_modifiers_round_trip() {
        let json = r#"{"comeInRunning":{"speedBooster":"any","minTiles":2.5},"comesThroughToilet":"yes"}"#;
        let condition = entrance(json).unwrap();
        assert_eq!(condition.comes_through_toilet, Some(TriState::Yes));
        assert!(matches!(condition.kind, EntranceKind::ComeInRunning { .. }));
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_value(&condition).unwrap(), original);
    }

    #[test]
    fn entrance_requires_exactly_one_kind() {
        assert!(entrance(r#"{"comesThroughToilet":"yes"}"#).is_err());
        assert!(entrance(r#"{"comeInNormally":{},"comeInShinecharged":{}}"#).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        assert!(entrance(r#"{"comeInMadeUp":{}}"#).is_err());
        assert!(entrance(r#"{"comeInNormally":{"extra":1}}"#).is_err());
        let runway = r#"{"comeInShinecharging":{"length":10,"openEnd":1,"extra":1}}"#;
        assert!(entrance(runway).is_err());
    }

    #[test]
    fn requirements_nested_in_an_entrance_stay_strict() {
        let platform = |requires: &str| {
            format!(
                r#"{{"comeInWithSidePlatform":{{"platforms":[{{"minHeight":1,"maxHeight":2,"minTiles":3,
                "speedBooster":"any","obstructions":[],"requires":[{requires}]}}]}}}}"#
            )
        };
        assert!(entrance(&platform(r#"{"heatFrames":1}"#)).is_ok());
        assert!(entrance(&platform(r#"{"heatFrames":1,"lavaFrames":2}"#)).is_err());
    }

    #[test]
    fn exit_condition_has_exactly_one_key() {
        let exit = |json| serde_json::from_str::<ExitCondition>(json);
        assert!(exit(r#"{"leaveNormally":{}}"#).is_ok());
        assert!(exit(r#"{"leaveNormally":{},"leaveShinecharged":{}}"#).is_err());
    }
}
