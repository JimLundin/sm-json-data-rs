//! Checks that every reference in a [`World`] resolves.

use crate::*;

fn check(valid: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if valid { Ok(()) } else { Err(what()) }
}

impl World {
    /// Checks every reference, so that indexing with this world's IDs can't panic.
    pub fn validate(&self) -> Result<(), String> {
        for (index, tech) in self.techs.iter().enumerate() {
            let at = || format!("tech {index}");
            self.requirements(&tech.tech_requires, None).map_err(|e| format!("{}: {e}", at()))?;
            self.requirements(&tech.other_requires, None).map_err(|e| format!("{}: {e}", at()))?;
            for extension in &tech.extensions {
                check(extension.index() < self.techs.len(), || format!("{}: bad extension", at()))?;
            }
        }
        for category in &self.tech_categories {
            check(category.members.iter().all(|id| id.index() < self.techs.len()), || "bad tech category".to_owned())?;
        }
        for helper in &self.helpers {
            self.requirements(&helper.requires, None)?;
        }
        for category in &self.helper_categories {
            check(category.members.iter().all(|id| id.index() < self.helpers.len()), || "bad helper category".to_owned())?;
        }
        for numeric in &self.numerics {
            self.quantity(&numeric.default)?;
        }
        for category in &self.numeric_categories {
            check(category.members.iter().all(|id| id.index() < self.numerics.len()), || "bad numeric category".to_owned())?;
        }
        self.check_numeric_cycles()?;
        for weapon in &self.weapons {
            self.requirements(&weapon.use_requires, None)?;
            self.requirements(&weapon.shot_requires, None)?;
        }
        for enemy in &self.enemies {
            check(enemy.attacks.len() <= usize::from(u8::MAX) + 1, || "enemy has too many attacks".to_owned())?;
            for selector in enemy.invulnerable_to.iter().chain(enemy.damage_multipliers.iter().map(|(s, _)| s)) {
                self.selector(selector)?;
            }
        }
        for scenario in &self.boss_scenarios {
            self.enemy(scenario.boss)?;
            self.weapon_filter(&scenario.weapons)?;
            self.requirements(&scenario.requires, None)?;
            for window in &scenario.damage_windows {
                self.requirements(&window.requires, None)?;
            }
            for damage in &scenario.incoming_damage {
                self.attack(&damage.attack)?;
                self.requirements(&damage.avoiding_requires, None)?;
            }
        }
        for flag in &self.start.flags {
            self.flag(*flag)?;
        }
        check(self.start.room.index() < self.rooms.len(), || "bad starting room".to_owned())?;
        let start = &self[self.start.room];
        self.node(Some(start), self.start.node)?;
        for room in &self.rooms {
            self.room(room).map_err(|e| format!("room {}: {e}", room.name))?;
        }
        for connection in &self.connections {
            for end in &connection.ends {
                check(end.room.index() < self.rooms.len(), || "connection has a bad room".to_owned())?;
                self.node(Some(&self[end.room]), end.node)?;
            }
        }
        Ok(())
    }

    fn check_numeric_cycles(&self) -> Result<(), String> {
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
        let mut marks = vec![Mark::New; self.numerics.len()];
        for id in 0..self.numerics.len() {
            check(visit(id, &self.numerics, &mut marks), || format!("numeric {} depends on itself", self.numerics[id].name))?;
        }
        Ok(())
    }

    fn enemy(&self, id: EnemyId) -> Result<(), String> {
        check(id.index() < self.enemies.len(), || format!("bad enemy {}", id.index()))
    }

    fn flag(&self, id: FlagId) -> Result<(), String> {
        check(id.index() < self.flags.len(), || format!("bad flag {}", id.index()))
    }

    fn attack(&self, attack: &AttackRef) -> Result<(), String> {
        self.enemy(attack.enemy)?;
        check(usize::from(attack.attack) < self[attack.enemy].attacks.len(), || "bad enemy attack".to_owned())
    }

    fn selector(&self, selector: &WeaponSelector) -> Result<(), String> {
        match selector {
            WeaponSelector::Weapon(id) => check(id.index() < self.weapons.len(), || "bad weapon".to_owned()),
            WeaponSelector::Category(_) => Ok(()),
        }
    }

    fn weapon_filter(&self, filter: &WeaponFilter) -> Result<(), String> {
        match filter {
            WeaponFilter::Default => Ok(()),
            WeaponFilter::Only(list) | WeaponFilter::Except(list) => list.iter().try_for_each(|s| self.selector(s)),
        }
    }

    fn quantity(&self, quantity: &Quantity) -> Result<(), String> {
        match quantity {
            Quantity::Const(_) => Ok(()),
            Quantity::Param(id) => check(id.index() < self.numerics.len(), || "bad numeric".to_owned()),
            Quantity::Sum(terms) | Quantity::Product(terms) => terms.iter().try_for_each(|t| self.quantity(t)),
        }
    }

    fn node(&self, room: Option<&Room>, id: NodeId) -> Result<(), String> {
        let room = room.ok_or_else(|| format!("node {id} referenced outside a room"))?;
        check(room.node(id).is_some(), || format!("no node {id}"))
    }

    fn obstacles(&self, room: Option<&Room>, ids: &[ObstacleId]) -> Result<(), String> {
        let room = room.ok_or("obstacle referenced outside a room")?;
        check(ids.iter().all(|id| id.index() < room.obstacles.len()), || "bad obstacle".to_owned())
    }

    fn requirements(&self, list: &[Requirement], room: Option<&Room>) -> Result<(), String> {
        list.iter().try_for_each(|r| self.requirement(r, room))
    }

    fn drops(&self, drops: &[EnemyDrop]) -> Result<(), String> {
        drops.iter().try_for_each(|drop| self.enemy(drop.enemy))
    }

    fn requirement(&self, r: &Requirement, room: Option<&Room>) -> Result<(), String> {
        use Requirement as R;
        match r {
            R::Free | R::Never | R::Item(_) | R::DisableEquipment(_) | R::Refill(_) | R::RidleyKill { .. } => Ok(()),
            R::ShineCharge(_) | R::BlueSpeed(_) | R::SpeedBall(_) | R::Shinespark { .. } | R::FlashSuit(_) | R::BlueSuit(_) => Ok(()),
            R::All(list) | R::Any(list) => self.requirements(list, room),
            R::Flag(id) | R::NotFlag(id) => self.flag(*id),
            R::Tech(id) | R::TechOnly(id) => check(id.index() < self.techs.len(), || "bad tech".to_owned()),
            R::Helper(id) => check(id.index() < self.helpers.len(), || "bad helper".to_owned()),
            R::Internal(id) => check(id.index() < self.internals.len(), || "bad internal requirement".to_owned()),
            R::Notable(id) => {
                let room = room.ok_or("notable referenced outside a room")?;
                check(room.notable(*id).is_some(), || format!("no notable {id}"))
            }
            R::Ammo { count, .. } => self.quantity(count),
            R::PartialRefill { limit, .. } => self.quantity(limit),
            R::Resource { amounts, .. } => amounts.iter().try_for_each(|a| self.quantity(&a.count)),
            R::AutoReserveTrigger { min_energy, max_energy, .. } => {
                self.quantity(min_energy)?;
                self.quantity(max_energy)
            }
            R::Kill(kill) => {
                kill.groups.iter().flatten().try_for_each(|id| self.enemy(*id))?;
                self.weapon_filter(&kill.weapons)
            }
            R::EnemyHits { attack, hits } => {
                self.attack(attack)?;
                self.quantity(hits)
            }
            R::Hazard { amount, .. } => self.quantity(amount),
            R::Heat { frames, mode } => {
                self.quantity(frames)?;
                match mode {
                    HeatMode::WithDrops(drops) => self.drops(drops),
                    _ => Ok(()),
                }
            }
            R::Lava { frames, mode } => {
                self.quantity(frames)?;
                match mode {
                    LavaMode::WithDrops(drops) => self.drops(drops),
                    _ => Ok(()),
                }
            }
            R::Acid { frames, .. } | R::CycleFrames { frames, .. } | R::ShineChargeFrames(frames) => self.quantity(frames),
            R::DoorUnlocked(id) | R::ItemCollected(id) | R::ItemNotCollected(id) => self.node(room, *id),
            R::ObstaclesCleared(ids) | R::ObstaclesNotCleared(ids) => self.obstacles(room, ids),
            R::EnteredFrom(nodes) => nodes.iter().try_for_each(|id| self.node(room, *id)),
        }
    }

    fn strat(&self, strat: &Strat, room: &Room) -> Result<(), String> {
        let scope = Some(room);
        self.requirements(&strat.requires, scope)?;
        if let Some(DoorEntry { kind: EntryKind::SidePlatform { platforms }, .. }) = &strat.entry {
            for platform in platforms {
                self.requirements(&platform.requires, scope)?;
            }
        }
        if let Some(DoorExit::Damage { attack, .. } | DoorExit::XModeSetup { attack: Some(attack) }) = &strat.exit {
            self.attack(attack)?;
        }
        for unlock in &strat.door_unlocks {
            if let Some(node) = unlock.node {
                self.node(scope, node)?;
            }
            self.requirements(&unlock.requires, scope)?;
        }
        self.obstacles(scope, &strat.clears_obstacles)?;
        self.obstacles(scope, &strat.resets_obstacles)?;
        strat.collects_items.iter().try_for_each(|id| self.node(scope, *id))?;
        strat.sets_flags.iter().try_for_each(|id| self.flag(*id))?;
        self.drops(&strat.farm_drops)?;
        for failure in &strat.failures {
            if let FailureOutcome::LeadsTo(node) = failure.outcome {
                self.node(scope, node)?;
            }
            self.requirements(&failure.cost, scope)?;
        }
        Ok(())
    }

    fn room(&self, room: &Room) -> Result<(), String> {
        let scope = Some(room);
        for environment in &room.environments {
            environment.entrances.iter().flatten().try_for_each(|id| self.node(scope, *id))?;
        }
        for node in &room.nodes {
            if let NodeKind::Door(door) = &node.kind {
                for environment in &door.environments {
                    environment.entrances.iter().flatten().try_for_each(|id| self.node(scope, *id))?;
                }
            }
            node.yields.iter().try_for_each(|id| self.flag(*id))?;
            for lock in &node.locks {
                lock.yields.iter().try_for_each(|id| self.flag(*id))?;
                if let Some(active_when) = &lock.active_when {
                    self.requirements(active_when, scope)?;
                }
                lock.unlock_strats.iter().try_for_each(|strat| self.strat(strat, room))?;
            }
            for view in &node.viewable {
                self.node(scope, view.node)?;
                view.strats.iter().try_for_each(|strat| self.strat(strat, room))?;
            }
        }
        for enemy in &room.enemies {
            self.enemy(enemy.enemy)?;
            match &enemy.placement {
                EnemyPlacement::Home(nodes) => nodes.iter().try_for_each(|id| self.node(scope, *id))?,
                EnemyPlacement::Between(a, b) => {
                    self.node(scope, *a)?;
                    self.node(scope, *b)?;
                }
            }
            for list in [&enemy.spawn, &enemy.stop_spawn].into_iter().flatten() {
                self.requirements(list, scope)?;
            }
        }
        for strat in &room.strats {
            self.node(scope, strat.from)?;
            self.node(scope, strat.to)?;
            self.strat(&strat.strat, room)?;
        }
        Ok(())
    }
}
