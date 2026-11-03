//! Writes a [`World`] back to the JSON's shape, which also lets tests prove that
//! importing preserved everything.

use super::*;
use crate::raw;

fn opt_vec<T>(list: Vec<T>) -> Option<Vec<T>> {
    if list.is_empty() { None } else { Some(list) }
}

fn nonzero(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

fn implicit(value: Implicit) -> Option<bool> {
    match value {
        Implicit::Automatic => None,
        Implicit::On => Some(true),
        Implicit::Off => Some(false),
    }
}

fn raw_runway(r: &Runway) -> raw::Runway {
    raw::Runway {
        length: r.length,
        open_end: r.open_ends,
        gentle_up_tiles: nonzero(r.slopes.gentle_up),
        gentle_down_tiles: nonzero(r.slopes.gentle_down),
        steep_up_tiles: nonzero(r.slopes.steep_up),
        steep_down_tiles: nonzero(r.slopes.steep_down),
        starting_steep_down_tiles: nonzero(r.slopes.starting_steep_down),
        starting_gentle_down_tiles: nonzero(r.slopes.starting_gentle_down),
        starting_stutterless_tiles: nonzero(r.slopes.starting_stutterless),
    }
}

fn raw_charge_runway(r: &Runway) -> raw::ShineChargeRunway {
    raw::ShineChargeRunway {
        used_tiles: r.length,
        open_end: r.open_ends,
        gentle_up_tiles: nonzero(r.slopes.gentle_up),
        gentle_down_tiles: nonzero(r.slopes.gentle_down),
        steep_up_tiles: nonzero(r.slopes.steep_up),
        steep_down_tiles: nonzero(r.slopes.steep_down),
        starting_steep_down_tiles: nonzero(r.slopes.starting_steep_down),
        starting_gentle_down_tiles: nonzero(r.slopes.starting_gentle_down),
        starting_stutterless_tiles: nonzero(r.slopes.starting_stutterless),
    }
}

fn swing_blocks(blocks: &[GrappleSwingBlock]) -> Vec<raw::GrappleSwingBlock> {
    blocks
        .iter()
        .map(|b| raw::GrappleSwingBlock {
            position: b.position,
            environment: Some(b.environment),
            obstructions: opt_vec(b.obstructions.clone()),
            note: b.doc.note.clone(),
            dev_note: b.doc.dev.clone(),
        })
        .collect()
}

fn remote_pairs(run_up: &RunUp) -> Option<Vec<[f64; 2]>> {
    opt_vec(run_up.remote_and_landing_min.iter().map(|(a, b)| [*a, *b]).collect())
}

impl World {
    /// Writes this world back to the JSON's shape.
    ///
    /// All connections go in one file, defaults are written out, and `$schema` and
    /// the `next*Id` counters are absent. Everything else matches the input.
    pub fn to_raw(&self) -> raw::GameData {
        let (regular, bosses): (Vec<&Enemy>, Vec<&Enemy>) = self.enemies.iter().partition(|e| e.kind == EnemyKind::Regular);
        raw::GameData {
            items: raw::ItemsFile {
                schema: None,
                starting_room: self[self.start.room].name.clone(),
                starting_node: self.start.node,
                starting_items: self.start.items.clone(),
                starting_flags: self.start.flags.iter().map(|f| self.raw_flag(*f)).collect(),
                starting_locks: self.start.open_locks.clone(),
                starting_resources: self
                    .start
                    .resources
                    .iter()
                    .map(|(resource, max_amount)| raw::StartingResource { resource: *resource, max_amount: *max_amount })
                    .collect(),
                implicit_items: self.items.implicit.clone(),
                upgrade_items: self.items.upgrades.iter().map(|u| raw::UpgradeItem { name: u.item, data: u.data.clone() }).collect(),
                expansion_items: self
                    .items
                    .expansions
                    .iter()
                    .map(|e| raw::ExpansionItem { name: e.item, data: e.data.clone(), resource: e.resource, resource_amount: e.amount })
                    .collect(),
                game_flags: self.flags.iter().map(|f| raw::FlagName(f.clone())).collect(),
                dev_note: self.items.doc.dev.clone(),
            },
            tech: raw::TechFile {
                schema: None,
                tech_categories: self
                    .tech_categories
                    .iter()
                    .map(|c| raw::TechCategory {
                        name: c.name.clone(),
                        description: c.description.clone(),
                        techs: c.members.iter().map(|t| self.raw_tech(*t)).collect(),
                    })
                    .collect(),
                next_tech_id: None,
            },
            helpers: raw::HelpersFile {
                schema: None,
                helper_categories: self
                    .helper_categories
                    .iter()
                    .map(|c| raw::HelperCategory {
                        name: c.name.clone(),
                        description: c.description.clone(),
                        helpers: c
                            .members
                            .iter()
                            .map(|id| {
                                let h = &self[*id];
                                raw::Helper {
                                    name: raw::HelperName(h.name.clone()),
                                    requires: self.raw_requires(&h.requires, None),
                                    map_rando_override: h.map_rando_override.then_some(true),
                                    note: h.doc.note.clone(),
                                    dev_note: h.doc.dev.clone(),
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            },
            numerics: raw::NumericsFile {
                schema: None,
                numeric_categories: self
                    .numeric_categories
                    .iter()
                    .map(|c| raw::NumericCategory {
                        name: c.name.clone(),
                        description: c.description.clone(),
                        numerics: c
                            .members
                            .iter()
                            .map(|id| {
                                let n = &self[*id];
                                raw::NumericParameter {
                                    name: raw::NumericName(n.name.clone()),
                                    value: self.raw_quantity(&n.default),
                                    minimum: n.minimum,
                                    maximum: n.maximum,
                                    note: n.doc.note.clone(),
                                    dev_note: n.doc.dev.clone(),
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            },
            weapons: raw::WeaponsFile {
                schema: None,
                weapons: self
                    .weapons
                    .iter()
                    .map(|w| raw::Weapon {
                        id: w.source_id,
                        name: w.name.clone(),
                        damage: w.damage,
                        cooldown_frames: w.cooldown_frames,
                        use_requires: self.raw_requires(&w.use_requires, None),
                        shot_requires: opt_vec(self.raw_requires(&w.shot_requires, None)),
                        situational: w.situational,
                        hits_group: w.hits_group,
                        categories: w.categories.clone(),
                        note: w.doc.note.clone(),
                        dev_note: w.doc.dev.clone(),
                    })
                    .collect(),
                categories: opt_vec(self.weapon_categories.clone()),
            },
            enemies: raw::EnemiesFile { schema: None, enemies: regular.iter().map(|e| self.raw_enemy(e)).collect() },
            bosses: raw::EnemiesFile { schema: None, enemies: bosses.iter().map(|e| self.raw_enemy(e)).collect() },
            boss_scenarios: raw::BossScenariosFile {
                schema: None,
                scenarios: self.boss_scenarios.iter().map(|s| self.raw_scenario(s)).collect(),
            },
            connections: vec![raw::ConnectionFile {
                schema: None,
                connections: self.connections.iter().map(|c| self.raw_connection(c)).collect(),
            }],
            rooms: self.rooms.iter().map(|r| self.raw_room(r)).collect(),
        }
    }

    fn raw_flag(&self, id: FlagId) -> raw::FlagName {
        raw::FlagName(self[id].clone())
    }

    fn raw_flags(&self, ids: &[FlagId]) -> Option<Vec<raw::FlagName>> {
        opt_vec(ids.iter().map(|id| self.raw_flag(*id)).collect())
    }

    fn raw_tech(&self, id: TechId) -> raw::Tech {
        let t = &self[id];
        raw::Tech {
            id: t.source_id,
            name: raw::TechName(t.name.clone()),
            tech_requires: self.raw_requires(&t.tech_requires, None),
            other_requires: self.raw_requires(&t.other_requires, None),
            note: t.doc.note.clone(),
            detail_note: t.doc.detail.clone(),
            dev_note: t.doc.dev.clone(),
            extension_techs: opt_vec(t.extensions.iter().map(|e| self.raw_tech(*e)).collect()),
        }
    }

    fn raw_quantity(&self, q: &Quantity) -> raw::Numeric {
        let list = |terms: &[Quantity]| terms.iter().map(|t| self.raw_quantity(t)).collect();
        match q {
            Quantity::Const(value) => raw::Numeric::Literal(*value),
            Quantity::Param(id) => raw::Numeric::Parameter(raw::NumericName(self[*id].name.clone())),
            Quantity::Sum(terms) => raw::Numeric::Add(list(terms)),
            Quantity::Product(terms) => raw::Numeric::Mul(list(terms)),
        }
    }

    fn selector_name(&self, selector: &WeaponSelector) -> String {
        match selector {
            WeaponSelector::Weapon(id) => self[*id].name.clone(),
            WeaponSelector::Category(category) => match serde_json::to_value(category) {
                Ok(serde_json::Value::String(name)) => name,
                _ => unreachable!("weapon categories serialize as strings"),
            },
        }
    }

    fn selector_names(&self, selectors: &[WeaponSelector]) -> Vec<String> {
        selectors.iter().map(|s| self.selector_name(s)).collect()
    }

    fn filter_names(&self, filter: &WeaponFilter) -> (Option<Vec<String>>, Option<Vec<String>>) {
        match filter {
            WeaponFilter::Default => (None, None),
            WeaponFilter::Only(list) => (Some(self.selector_names(list)), None),
            WeaponFilter::Except(list) => (None, Some(self.selector_names(list))),
        }
    }

    fn attack_names(&self, attack: &AttackRef) -> (String, String) {
        let enemy = &self[attack.enemy];
        (enemy.name.clone(), enemy.attacks[usize::from(attack.attack)].name.clone())
    }

    fn raw_drops(&self, drops: &[EnemyDrop]) -> Vec<raw::EnemyDrop> {
        drops.iter().map(|d| raw::EnemyDrop { enemy: self[d.enemy].name.clone(), count: d.count }).collect()
    }

    fn raw_requires(&self, list: &[Requirement], room: Option<&Room>) -> Vec<raw::Requirement> {
        list.iter().map(|r| self.raw_requirement(r, room)).collect()
    }

    fn raw_obstacles(ids: &[ObstacleId], room: Option<&Room>) -> Vec<raw::ObstacleId> {
        let room = room.expect("obstacles are only referenced inside rooms");
        ids.iter().map(|id| raw::ObstacleId(room.obstacles[id.index()].key.clone())).collect()
    }

    fn raw_requirement(&self, r: &Requirement, room: Option<&Room>) -> raw::Requirement {
        use raw::Requirement as R;
        use Requirement as M;
        let count = |q: &Quantity| self.raw_quantity(q);
        let amounts = |list: &[ResourceAmount]| -> Vec<raw::ResourceAmount> {
            list.iter().map(|a| raw::ResourceAmount { kind: a.kind, count: count(&a.count) }).collect()
        };
        match r {
            M::Free => R::Named(raw::Named::Free),
            M::Never => R::Named(raw::Named::Never),
            M::All(list) => R::And(self.raw_requires(list, room)),
            M::Any(list) => R::Or(self.raw_requires(list, room)),
            M::Item(item) => R::Named(raw::Named::Item(*item)),
            M::Flag(id) => R::Named(raw::Named::Flag(self.raw_flag(*id))),
            M::NotFlag(id) => R::Not(raw::Named::Flag(self.raw_flag(*id))),
            M::Tech(id) => R::Named(raw::Named::Tech(raw::TechName(self[*id].name.clone()))),
            M::TechOnly(id) => R::TechOnly(raw::TechName(self[*id].name.clone())),
            M::Helper(id) => R::Named(raw::Named::Helper(raw::HelperName(self[*id].name.clone()))),
            M::Notable(id) => {
                let notable = room.and_then(|room| room.notable(*id)).expect("notables resolve within their room");
                R::Notable(notable.name.clone())
            }
            M::Internal(id) => R::Named(raw::Named::Internal(raw::InternalName(self[*id].clone()))),
            M::DisableEquipment(item) => R::DisableEquipment(*item),
            M::Ammo { kind, count: c, usage } => {
                let amount = raw::AmmoAmount { kind: *kind, count: count(c) };
                match usage {
                    AmmoUsage::Spend => R::Ammo(amount),
                    AmmoUsage::Drain => R::AmmoDrain(amount),
                }
            }
            M::Refill(kinds) => R::Refill(kinds.clone()),
            M::PartialRefill { kind, limit } => R::PartialRefill(raw::PartialRefill { kind: *kind, limit: count(limit) }),
            M::Resource { check, amounts: list } => {
                let list = amounts(list);
                match check {
                    ResourceCheck::Available => R::ResourceAvailable(list),
                    ResourceCheck::Consumed => R::ResourceConsumed(list),
                    ResourceCheck::AtMost => R::ResourceAtMost(list),
                    ResourceCheck::MissingAtMost => R::ResourceMissingAtMost(list),
                    ResourceCheck::Capacity => R::ResourceCapacity(list),
                    ResourceCheck::MaxCapacity => R::ResourceMaxCapacity(list),
                }
            }
            M::AutoReserveTrigger { min_energy, max_energy, implicit_heat } => R::AutoReserveTrigger(raw::AutoReserveTrigger {
                min_reserve_energy: Some(count(min_energy)),
                max_reserve_energy: Some(count(max_energy)),
                implicit_heat_frames: Some(*implicit_heat),
            }),
            M::Kill(kill) => {
                let (explicit_weapons, excluded_weapons) = self.filter_names(&kill.weapons);
                R::EnemyKill(raw::EnemyKill {
                    enemies: kill
                        .groups
                        .iter()
                        .map(|group| group.iter().map(|id| self[*id].name.clone()).collect())
                        .collect(),
                    explicit_weapons,
                    excluded_weapons,
                    farmable_ammo: opt_vec(kill.farmable_ammo.clone()),
                })
            }
            M::RidleyKill { power_bombs, g_mode, stuck } => {
                R::RidleyKill(raw::RidleyKill { power_bombs: Some(*power_bombs), g_mode: Some(*g_mode), stuck: *stuck })
            }
            M::EnemyHits { attack, hits } => {
                let (enemy, attack) = self.attack_names(attack);
                R::EnemyDamage(raw::EnemyDamage { enemy, attack, hits: count(hits) })
            }
            M::Hazard { hazard, amount } => {
                let amount = count(amount);
                match hazard {
                    Hazard::SpikeHits => R::SpikeHits(amount),
                    Hazard::ThornHits => R::ThornHits(amount),
                    Hazard::ElectricityHits => R::ElectricityHits(amount),
                    Hazard::HibashiHits => R::HibashiHits(amount),
                    Hazard::SamusEaterCycles => R::SamusEaterCycles(amount),
                    Hazard::MetroidFrames => R::MetroidFrames(amount),
                    Hazard::DraygonElectricityFrames => R::DraygonElectricityFrames(amount),
                }
            }
            M::Heat { frames, mode } => {
                let frames = count(frames);
                match mode {
                    HeatMode::Standard => R::HeatFrames(frames),
                    HeatMode::Simple => R::SimpleHeatFrames(frames),
                    HeatMode::Suitless => R::SuitlessHeatFrames(frames),
                    HeatMode::Gravityless => R::GravitylessHeatFrames(frames),
                    HeatMode::WithDrops(drops) => {
                        R::HeatFramesWithEnergyDrops(raw::FramesWithDrops { frames, drops: self.raw_drops(drops) })
                    }
                }
            }
            M::Lava { frames, mode } => {
                let frames = count(frames);
                match mode {
                    LavaMode::Standard => R::LavaFrames(frames),
                    LavaMode::Gravityless => R::GravitylessLavaFrames(frames),
                    LavaMode::WithDrops(drops) => {
                        R::LavaFramesWithEnergyDrops(raw::FramesWithDrops { frames, drops: self.raw_drops(drops) })
                    }
                }
            }
            M::Acid { frames, gravity } => match gravity {
                GravityRule::Standard => R::AcidFrames(count(frames)),
                GravityRule::Gravityless => R::GravitylessAcidFrames(count(frames)),
            },
            M::CycleFrames { frames, simple } => {
                if *simple { R::SimpleCycleFrames(count(frames)) } else { R::CycleFrames(count(frames)) }
            }
            M::ShineCharge(r) => R::CanShineCharge(raw_charge_runway(r)),
            M::BlueSpeed(r) => R::GetBlueSpeed(raw_charge_runway(r)),
            M::SpeedBall(r) => R::SpeedBall(raw_runway(r)),
            M::ShineChargeFrames(q) => R::ShineChargeFrames(count(q)),
            M::Shinespark { frames, excess_frames } => {
                R::Shinespark(raw::Shinespark { frames: *frames, excess_frames: nonzero(*excess_frames) })
            }
            M::FlashSuit(action) => match action {
                FlashSuitAction::Gain => R::GainFlashSuit {},
                FlashSuitAction::Use => R::UseFlashSuit {},
                FlashSuitAction::Forbid => R::NoFlashSuit {},
            },
            M::BlueSuit(action) => match action {
                BlueSuitAction::Gain => R::GainBlueSuit {},
                BlueSuitAction::Have => R::HaveBlueSuit {},
                BlueSuitAction::Shinecharge => R::BlueSuitShinecharge {},
                BlueSuitAction::Forbid => R::NoBlueSuit {},
            },
            M::DoorUnlocked(id) => R::DoorUnlockedAtNode(*id),
            M::ItemCollected(id) => R::ItemCollectedAtNode(*id),
            M::ItemNotCollected(id) => R::ItemNotCollectedAtNode(*id),
            M::ObstaclesCleared(ids) => R::ObstaclesCleared(Self::raw_obstacles(ids, room)),
            M::ObstaclesNotCleared(ids) => R::ObstaclesNotCleared(Self::raw_obstacles(ids, room)),
            M::EnteredFrom(nodes) => R::ResetRoom(raw::ResetRoom { nodes: nodes.clone() }),
        }
    }

    fn raw_entry(&self, e: &DoorEntry, room: &Room) -> raw::EntranceCondition {
        use raw::EntranceKind as R;
        use EntryKind as M;
        let kind = match &e.kind {
            M::Normal => R::ComeInNormally {},
            M::Running { speed_booster, speed } => {
                R::ComeInRunning { speed_booster: *speed_booster, min_tiles: speed.min, max_tiles: speed.max }
            }
            M::Jumping { speed_booster, speed } => {
                R::ComeInJumping { speed_booster: *speed_booster, min_tiles: speed.min, max_tiles: speed.max }
            }
            M::SpaceJumping { speed_booster, speed } => {
                R::ComeInSpaceJumping { speed_booster: *speed_booster, min_tiles: speed.min, max_tiles: speed.max }
            }
            M::BlueSpaceJumping { extra_speed } => R::ComeInBlueSpaceJumping {
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
            },
            M::Shinecharging { runway } => R::ComeInShinecharging(raw_runway(runway)),
            M::GettingBlueSpeed { runway, extra_speed } => R::ComeInGettingBlueSpeed {
                length: runway.length,
                open_end: runway.open_ends,
                gentle_up_tiles: nonzero(runway.slopes.gentle_up),
                gentle_down_tiles: nonzero(runway.slopes.gentle_down),
                steep_up_tiles: nonzero(runway.slopes.steep_up),
                steep_down_tiles: nonzero(runway.slopes.steep_down),
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
            },
            M::Shinecharged => R::ComeInShinecharged {},
            M::ShinechargedJumping => R::ComeInShinechargedJumping {},
            M::Spark { position } => R::ComeInWithSpark { position: *position },
            M::StutterShinecharging { min_tiles } => R::ComeInStutterShinecharging { min_tiles: *min_tiles },
            M::StutterGettingBlueSpeed { min_tiles } => R::ComeInStutterGettingBlueSpeed { min_tiles: *min_tiles },
            M::BombBoost => R::ComeInWithBombBoost {},
            M::DoorStuckSetup => R::ComeInWithDoorStuckSetup {},
            M::Speedballing { runway, extra_speed } => R::ComeInSpeedballing {
                runway: raw_runway(runway),
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
            },
            M::TemporaryBlue { direction } => R::ComeInWithTemporaryBlue { direction: *direction },
            M::Spinning { speed_booster, unusable_tiles, extra_speed } => R::ComeInSpinning {
                speed_booster: *speed_booster,
                unusable_tiles: *unusable_tiles,
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
            },
            M::BlueSpinning { unusable_tiles, extra_speed } => R::ComeInBlueSpinning {
                unusable_tiles: *unusable_tiles,
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
            },
            M::Mockball { speed_booster, run_up } => R::ComeInWithMockball {
                speed_booster: *speed_booster,
                adjacent_min_tiles: run_up.adjacent_min,
                remote_and_landing_min_tiles: remote_pairs(run_up),
            },
            M::SpringBallBounce { speed_booster, movement, run_up } => R::ComeInWithSpringBallBounce {
                speed_booster: *speed_booster,
                movement_type: *movement,
                adjacent_min_tiles: run_up.adjacent_min,
                remote_and_landing_min_tiles: remote_pairs(run_up),
            },
            M::BlueSpringBallBounce { movement, extra_speed, min_landing_tiles } => R::ComeInWithBlueSpringBallBounce {
                movement_type: *movement,
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
                min_landing_tiles: *min_landing_tiles,
            },
            M::StoredFallSpeed { tiles } => R::ComeInWithStoredFallSpeed { fall_speed_in_tiles: *tiles },
            M::Knockback => R::ComeInWithKnockback {},
            M::DamageBoost { i_frames, position } => R::ComeInWithDamageBoost {
                i_frames_needed: *i_frames,
                position: Some(match position {
                    DamageBoostEntry::High => raw::DamageBoostEntry::High,
                    DamageBoostEntry::Any => raw::DamageBoostEntry::Any,
                }),
            },
            M::IFrames { i_frames } => R::ComeInWithIFrames { i_frames_needed: *i_frames },
            M::RMode => R::ComeInWithRMode {},
            M::GMode { mode, morphed, mobility } => R::ComeInWithGMode { mode: *mode, morphed: *morphed, mobility: Some(*mobility) },
            M::XMode => R::ComeInWithXMode {},
            M::WallJumpBelow { min_height } => R::ComeInWithWallJumpBelow { min_height: *min_height },
            M::SpaceJumpBelow => R::ComeInWithSpaceJumpBelow {},
            M::PlatformBelow { min_height, max_height, max_left, min_right } => R::ComeInWithPlatformBelow {
                min_height: *min_height,
                max_height: *max_height,
                max_left_position: *max_left,
                min_right_position: *min_right,
            },
            M::SidePlatform { platforms } => R::ComeInWithSidePlatform {
                platforms: platforms
                    .iter()
                    .map(|p| raw::SidePlatform {
                        min_height: p.min_height,
                        max_height: p.max_height,
                        min_tiles: p.min_tiles,
                        speed_booster: p.speed_booster,
                        obstructions: p.obstructions.clone(),
                        environment: Some(p.environment),
                        requires: opt_vec(self.raw_requires(&p.requires, Some(room))),
                        note: p.doc.note.clone(),
                        detail_note: p.doc.detail.clone(),
                        dev_note: p.doc.dev.clone(),
                    })
                    .collect(),
            },
            M::GrappleSwing { blocks } => R::ComeInWithGrappleSwing { blocks: swing_blocks(blocks) },
            M::GrappleJump { position } => R::ComeInWithGrappleJump { position: *position },
            M::GrappleTeleport { blocks } => R::ComeInWithGrappleTeleport { block_positions: blocks.clone() },
            M::SamusEaterTeleport { floor, ceiling } => {
                R::ComeInWithSamusEaterTeleport { floor_positions: floor.clone(), ceiling_positions: ceiling.clone() }
            }
            M::SuperSink => R::ComeInWithSuperSink {},
        };
        raw::EntranceCondition {
            kind,
            comes_through_toilet: Some(e.through_toilet),
            comes_in_heated: Some(e.heated),
            dev_note: e.dev_note.clone(),
        }
    }

    fn raw_exit(&self, e: &DoorExit) -> raw::ExitCondition {
        use raw::ExitCondition as R;
        use DoorExit as M;
        match e {
            M::Normal => R::LeaveNormally {},
            M::Runway { runway, min_extra_speed, heated } => R::LeaveWithRunway {
                length: runway.length,
                open_end: runway.open_ends,
                gentle_up_tiles: nonzero(runway.slopes.gentle_up),
                gentle_down_tiles: nonzero(runway.slopes.gentle_down),
                steep_up_tiles: nonzero(runway.slopes.steep_up),
                steep_down_tiles: nonzero(runway.slopes.steep_down),
                starting_steep_down_tiles: nonzero(runway.slopes.starting_steep_down),
                starting_gentle_down_tiles: nonzero(runway.slopes.starting_gentle_down),
                starting_stutterless_tiles: nonzero(runway.slopes.starting_stutterless),
                min_extra_run_speed: min_extra_speed.clone(),
                heated: *heated,
            },
            M::Shinecharged => R::LeaveShinecharged {},
            M::TemporaryBlue { direction } => R::LeaveWithTemporaryBlue { direction: *direction },
            M::Spark { position, grounded } => R::LeaveWithSpark { position: *position, grounded: *grounded },
            M::Spinning { remote, extra_speed, blue } => R::LeaveSpinning {
                remote_runway: raw_runway(remote),
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
                blue: Some(*blue),
            },
            M::Mockball { remote, landing, extra_speed, blue } => R::LeaveWithMockball {
                remote_runway: raw_runway(remote),
                landing_runway: raw_runway(landing),
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
                blue: Some(*blue),
            },
            M::SpringBallBounce { remote, landing, movement, extra_speed, blue } => R::LeaveWithSpringBallBounce {
                remote_runway: raw_runway(remote),
                landing_runway: raw_runway(landing),
                movement_type: *movement,
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
                blue: Some(*blue),
            },
            M::SpaceJumping { remote, extra_speed, blue } => R::LeaveSpaceJumping {
                remote_runway: raw_runway(remote),
                min_extra_run_speed: extra_speed.min.clone(),
                max_extra_run_speed: extra_speed.max.clone(),
                blue: Some(*blue),
            },
            M::StoredFallSpeed { tiles } => R::LeaveWithStoredFallSpeed { fall_speed_in_tiles: *tiles },
            M::Damage { attack, knockback } => {
                let (enemy, attack) = self.attack_names(attack);
                R::LeaveWithDamage { enemy, attack: Some(attack), knockback: Some(*knockback) }
            }
            M::Knockback => R::LeaveWithKnockback {},
            M::DamageBoost { i_frames_remaining, position } => R::LeaveWithDamageBoost {
                i_frames_remaining: *i_frames_remaining,
                position: Some(match position {
                    DamageBoostExit::High => raw::DamageBoostExit::High,
                    DamageBoostExit::NotHigh => raw::DamageBoostExit::NotHigh,
                }),
            },
            M::IFrames { i_frames_remaining } => R::LeaveWithIFrames { i_frames_remaining: *i_frames_remaining },
            M::XModeSetup { attack } => {
                let names = attack.as_ref().map(|a| self.attack_names(a));
                R::LeaveWithXModeSetup {
                    enemy: names.as_ref().map(|(enemy, _)| enemy.clone()),
                    attack: names.map(|(_, attack)| attack),
                }
            }
            M::GMode { morphed } => R::LeaveWithGMode { morphed: *morphed },
            M::DoorFrameBelow { height } => R::LeaveWithDoorFrameBelow { height: *height },
            M::PlatformBelow { height, left, right } => {
                R::LeaveWithPlatformBelow { height: *height, left_position: *left, right_position: *right }
            }
            M::SidePlatform { height, runway, obstruction } => {
                R::LeaveWithSidePlatform { height: *height, runway: raw_runway(runway), obstruction: *obstruction }
            }
            M::GrappleSwing { blocks } => R::LeaveWithGrappleSwing { blocks: swing_blocks(blocks) },
            M::GrappleJump { position } => R::LeaveWithGrappleJump { position: *position },
            M::GrappleTeleport { blocks } => R::LeaveWithGrappleTeleport { block_positions: blocks.clone() },
            M::SamusEaterTeleport { floor, ceiling } => {
                R::LeaveWithSamusEaterTeleport { floor_positions: floor.clone(), ceiling_positions: ceiling.clone() }
            }
            M::SuperSink => R::LeaveWithSuperSink {},
        }
    }

    fn raw_strat(&self, s: &Strat, id: Option<raw::StratId>, link: Option<[NodeId; 2]>, room: &Room) -> raw::Strat {
        let r = Some(room);
        raw::Strat {
            id,
            link,
            name: s.name.clone(),
            entrance_condition: s.entry.as_ref().map(|e| self.raw_entry(e, room)),
            starts_with_shine_charge: s.starts_shinecharged.then_some(true),
            requires: self.raw_requires(&s.requires, r),
            exit_condition: s.exit.as_ref().map(|e| self.raw_exit(e)),
            g_mode_regain_mobility: s.regains_g_mode_mobility.then_some(raw::Empty {}),
            bypasses_door_shell: match s.door_shell {
                DoorShellBypass::No => None,
                DoorShellBypass::Yes => Some(raw::DoorShellBypass::Yes),
                DoorShellBypass::Free => Some(raw::DoorShellBypass::Free),
            },
            unlocks_doors: opt_vec(
                s.door_unlocks
                    .iter()
                    .map(|u| raw::UnlockDoor {
                        node_id: u.node,
                        types: u.types.clone(),
                        requires: opt_vec(self.raw_requires(&u.requires, r)),
                        use_implicit_requires: Some(u.use_implicit_requires),
                        note: u.doc.note.clone(),
                        dev_note: u.doc.dev.clone(),
                    })
                    .collect(),
            ),
            clears_obstacles: opt_vec(Self::raw_obstacles(&s.clears_obstacles, r)),
            resets_obstacles: opt_vec(Self::raw_obstacles(&s.resets_obstacles, r)),
            collects_items: opt_vec(s.collects_items.clone()),
            sets_flags: self.raw_flags(&s.sets_flags),
            ends_with_shine_charge: s.ends_shinecharged.then_some(true),
            farm_cycle_drops: opt_vec(self.raw_drops(&s.farm_drops)),
            wall_jump_avoid: s.wall_jump_avoid.then_some(true),
            flash_suit_checked: s.review.flash_suit,
            blue_suit_checked: s.review.blue_suit,
            failures: opt_vec(
                s.failures
                    .iter()
                    .map(|f| raw::StratFailure {
                        name: f.name.clone(),
                        leads_to_node: match f.outcome {
                            FailureOutcome::LeadsTo(node) => Some(node),
                            _ => None,
                        },
                        cost: opt_vec(self.raw_requires(&f.cost, r)),
                        softlock: (f.outcome == FailureOutcome::Softlock).then_some(true),
                        note: f.doc.note.clone(),
                        dev_note: f.doc.dev.clone(),
                    })
                    .collect(),
            ),
            note: s.doc.note.clone(),
            detail_note: s.doc.detail.clone(),
            dev_note: s.doc.dev.clone(),
        }
    }

    fn raw_node(&self, n: &Node, room: &Room) -> raw::Node {
        use raw::{NodeSubType as S, NodeType as T};
        let transit = |t: &Transit| match t {
            Transit::Elevator => S::Elevator,
            Transit::Sandpit => S::Sandpit,
        };
        let mut out = raw::Node {
            id: n.id,
            name: n.name.clone(),
            node_type: T::Junction,
            node_sub_type: S::Junction,
            node_item: None,
            node_address: None,
            door_orientation: None,
            door_environments: None,
            use_implicit_leave_normally: None,
            use_implicit_come_in_normally: None,
            use_implicit_come_in_with_mockball: None,
            use_implicit_carry_g_mode_back_through: None,
            use_implicit_carry_g_mode_morph_back_through: None,
            use_implicit_come_in_with_grapple_jump: None,
            use_implicit_door_unlocks: None,
            is_door_immediately_closed: None,
            locks: opt_vec(
                n.locks
                    .iter()
                    .map(|l| raw::Lock {
                        name: l.name.clone(),
                        lock_type: l.kind,
                        lock: l.active_when.as_ref().map(|list| self.raw_requires(list, Some(room))),
                        unlock_strats: l.unlock_strats.iter().map(|s| self.raw_strat(s, None, None, room)).collect(),
                        yields: self.raw_flags(&l.yields),
                        note: l.doc.note.clone(),
                        dev_note: l.doc.dev.clone(),
                    })
                    .collect(),
            ),
            twin_door_addresses: None,
            utility: None,
            viewable_nodes: opt_vec(
                n.viewable
                    .iter()
                    .map(|v| raw::ViewableNode {
                        id: v.node,
                        strats: v.strats.iter().map(|s| self.raw_strat(s, None, None, room)).collect(),
                    })
                    .collect(),
            ),
            yields: self.raw_flags(&n.yields),
            map_tile_mask: n.map_tiles.clone(),
            note: n.doc.note.clone(),
            dev_note: n.doc.dev.clone(),
        };
        match &n.kind {
            NodeKind::Door(d) => {
                out.node_type = T::Door;
                out.node_sub_type = match d.style {
                    DoorStyle::Blue => S::Blue,
                    DoorStyle::ClosedWall => S::ClosedWall,
                    DoorStyle::Doorway => S::Doorway,
                    DoorStyle::Elevator => S::Elevator,
                    DoorStyle::Eye => S::Eye,
                    DoorStyle::Gray => S::Gray,
                    DoorStyle::Green => S::Green,
                    DoorStyle::Passage => S::Passage,
                    DoorStyle::Red => S::Red,
                    DoorStyle::Vertical => S::Vertical,
                    DoorStyle::Yellow => S::Yellow,
                };
                out.door_orientation = d.orientation;
                out.node_address = d.address.clone();
                out.door_environments = opt_vec(
                    d.environments
                        .iter()
                        .map(|e| raw::DoorEnvironment {
                            physics: e.physics,
                            entrance_nodes: e.entrances.clone(),
                            note: e.doc.note.clone(),
                            dev_note: e.doc.dev.clone(),
                        })
                        .collect(),
                );
                out.use_implicit_leave_normally = implicit(d.implicit.leave_normally);
                out.use_implicit_come_in_normally = implicit(d.implicit.come_in_normally);
                out.use_implicit_come_in_with_mockball = implicit(d.implicit.come_in_with_mockball);
                out.use_implicit_come_in_with_grapple_jump = implicit(d.implicit.come_in_with_grapple_jump);
                out.use_implicit_carry_g_mode_back_through = implicit(d.implicit.carry_g_mode_back_through);
                out.use_implicit_carry_g_mode_morph_back_through = implicit(d.implicit.carry_g_mode_morph_back_through);
                out.use_implicit_door_unlocks = implicit(d.implicit.door_unlocks);
                out.is_door_immediately_closed = d.immediately_closed.then_some(true);
                out.twin_door_addresses = opt_vec(
                    d.twins
                        .iter()
                        .map(|t| raw::TwinDoorAddress { room_address: t.room_address.clone(), door_address: t.door_address.clone() })
                        .collect(),
                );
            }
            NodeKind::Entrance { transit: t, orientation, address } => {
                out.node_type = T::Entrance;
                out.node_sub_type = transit(t);
                out.door_orientation = Some(*orientation);
                out.node_address = address.clone();
            }
            NodeKind::Exit { transit: t, orientation, address } => {
                out.node_type = T::Exit;
                out.node_sub_type = transit(t);
                out.door_orientation = Some(*orientation);
                out.node_address = address.clone();
            }
            NodeKind::Item { item, placement, address } => {
                out.node_type = T::Item;
                out.node_sub_type = match placement {
                    ItemPlacement::Chozo => S::Chozo,
                    ItemPlacement::Hidden => S::Hidden,
                    ItemPlacement::Visible => S::Visible,
                };
                out.node_item = Some(*item);
                out.node_address = Some(address.clone());
            }
            NodeKind::Junction(kind) => {
                out.node_type = T::Junction;
                out.node_sub_type = match kind {
                    JunctionKind::Plain => S::Junction,
                    JunctionKind::GMode => S::GMode,
                    JunctionKind::RMode => S::RMode,
                };
            }
            NodeKind::Station { station, services, orientation } => {
                out.node_type = T::Utility;
                out.node_sub_type = match station {
                    StationKind::Energy => S::Energy,
                    StationKind::Map => S::Map,
                    StationKind::Missile => S::Missile,
                    StationKind::Save => S::Save,
                    StationKind::Ship => S::Ship,
                };
                out.utility = Some(services.clone());
                out.door_orientation = *orientation;
            }
        }
        out
    }

    fn raw_room(&self, r: &Room) -> raw::Room {
        raw::Room {
            schema: None,
            id: r.source_id,
            name: r.name.clone(),
            area: r.area.clone(),
            subarea: r.subarea.clone(),
            subsubarea: r.subsubarea.clone(),
            room_address: r.address.clone(),
            room_environments: r
                .environments
                .iter()
                .map(|e| raw::RoomEnvironment {
                    heated: e.heated,
                    entrance_nodes: e.entrances.clone(),
                    note: e.doc.note.clone(),
                    dev_note: e.doc.dev.clone(),
                })
                .collect(),
            map_tile_mask: r.map_tiles.clone(),
            nodes: r.nodes.iter().map(|n| self.raw_node(n, r)).collect(),
            obstacles: opt_vec(
                r.obstacles
                    .iter()
                    .map(|o| raw::Obstacle {
                        id: raw::ObstacleId(o.key.clone()),
                        name: o.name.clone(),
                        obstacle_type: o.kind,
                        note: o.doc.note.clone(),
                        dev_note: o.doc.dev.clone(),
                    })
                    .collect(),
            ),
            enemies: opt_vec(
                r.enemies
                    .iter()
                    .map(|e| {
                        let (home_nodes, between_nodes) = match &e.placement {
                            EnemyPlacement::Home(nodes) => (Some(nodes.clone()), None),
                            EnemyPlacement::Between(a, b) => (None, Some([*a, *b])),
                        };
                        raw::RoomEnemy {
                            id: e.key.clone(),
                            group_name: e.group_name.clone(),
                            enemy_name: self[e.enemy].name.clone(),
                            quantity: e.quantity,
                            home_nodes,
                            between_nodes,
                            spawn: e.spawn.as_ref().map(|list| self.raw_requires(list, Some(r))),
                            stop_spawn: e.stop_spawn.as_ref().map(|list| self.raw_requires(list, Some(r))),
                            note: e.doc.note.clone(),
                            dev_note: e.doc.dev.clone(),
                        }
                    })
                    .collect(),
            ),
            strats: r
                .strats
                .iter()
                .map(|s| self.raw_strat(&s.strat, s.source_id, Some([s.from, s.to]), r))
                .collect(),
            notables: opt_vec(
                r.notables
                    .iter()
                    .map(|n| raw::Notable {
                        id: n.id,
                        name: n.name.clone(),
                        wall_jump_avoid: n.wall_jump_avoid.then_some(true),
                        note: n.note.clone(),
                        dev_note: n.dev_note.clone(),
                    })
                    .collect(),
            ),
            next_strat_id: None,
            next_notable_id: None,
            note: r.doc.note.clone(),
            dev_note: r.doc.dev.clone(),
        }
    }

    fn raw_enemy(&self, e: &Enemy) -> raw::Enemy {
        raw::Enemy {
            id: e.source_id,
            name: e.name.clone(),
            attacks: e
                .attacks
                .iter()
                .map(|a| raw::EnemyAttack {
                    name: a.name.clone(),
                    base_damage: a.base_damage,
                    affected_by_varia: Some(a.reduced_by_varia),
                    affected_by_gravity: Some(a.reduced_by_gravity),
                })
                .collect(),
            hp: e.hp,
            amount_of_drops: e.drop_count,
            drops: e.drops,
            buffed_drops: e.buffed_drops,
            farmable_drops: e.farmable_drops,
            dims: raw::Dimensions { w: e.width, h: e.height, note: e.size_doc.note.clone(), dev_note: e.size_doc.dev.clone() },
            freezable: e.freezable,
            grapplable: e.grapplable,
            invul: self.selector_names(&e.invulnerable_to),
            damage_multipliers: e
                .damage_multipliers
                .iter()
                .map(|(selector, value)| raw::DamageMultiplier { weapon: self.selector_name(selector), value: *value })
                .collect(),
            areas: opt_vec(e.areas.clone()),
            note: e.doc.note.clone(),
            dev_note: e.doc.dev.clone(),
        }
    }

    fn raw_scenario(&self, s: &BossScenario) -> raw::BossScenario {
        let (explicit_weapons, excluded_weapons) = self.filter_names(&s.weapons);
        raw::BossScenario {
            id: s.source_id,
            name: s.name.clone(),
            boss: self[s.boss].name.clone(),
            explicit_weapons,
            excluded_weapons,
            requires: self.raw_requires(&s.requires, None),
            boss_dodge_rate: s.dodge_rate,
            attack_opportunity_duration: s.attack_opportunity_duration,
            damage_windows: opt_vec(
                s.damage_windows
                    .iter()
                    .map(|w| raw::DamageWindow {
                        name: w.name.clone(),
                        window_percent: w.percent,
                        requires: self.raw_requires(&w.requires, None),
                        note: w.doc.note.clone(),
                        dev_note: w.doc.dev.clone(),
                    })
                    .collect(),
            ),
            incoming_damage: opt_vec(
                s.incoming_damage
                    .iter()
                    .map(|d| raw::IncomingDamage {
                        name: d.name.clone(),
                        attack: self.attack_names(&d.attack).1,
                        frequency_frames: d.frequency_frames,
                        avoiding_requires: self.raw_requires(&d.avoiding_requires, None),
                        note: d.doc.note.clone(),
                        dev_note: d.doc.dev.clone(),
                    })
                    .collect(),
            ),
            particle_frequency_frames: s.particle_frequency_frames,
            note: s.doc.note.clone(),
            dev_note: s.doc.dev.clone(),
        }
    }

    fn raw_connection(&self, c: &Connection) -> raw::Connection {
        let end = |e: &ConnectionEnd| raw::ConnectionNode {
            area: e.label.area.clone(),
            subarea: e.label.subarea.clone(),
            room_id: self[e.room].source_id,
            room_name: e.label.room.clone(),
            node_id: e.node,
            node_name: e.label.node.clone(),
            position: e.position,
            note: e.doc.note.clone(),
            dev_note: e.doc.dev.clone(),
        };
        raw::Connection {
            connection_type: c.kind,
            direction: c.direction,
            description: c.description.clone(),
            nodes: [end(&c.ends[0]), end(&c.ends[1])],
            note: c.doc.note.clone(),
            dev_note: c.doc.dev.clone(),
        }
    }
}
