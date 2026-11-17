//! What characters do on their turn: moving between rooms, finding their
//! way to the player, and fighting.
//!
//! Everything here only changes the world and reports what happened as
//! [`NpcEvent`]s. The turn loop turns events into text ("The peddler goes
//! north."), so the built-in behaviours and Lua scripts calling
//! `self:wander()` describe things the same way.

use crate::world::World;
use eifgb_ir::{Behavior, Direction, EntityId, EntityKind, Value, PLAYER};
use std::collections::{HashMap, VecDeque};

/// Something a character did that the player may see.
#[derive(Debug, Clone, PartialEq)]
pub enum NpcEvent {
    /// Moved from one room to another, going `direction`.
    Moved { npc: EntityId, from: EntityId, to: EntityId, direction: Direction },
    /// Attacked `target`, taking `damage` health. `weapon` is what was used, if anything.
    Attacked { attacker: EntityId, target: EntityId, weapon: Option<EntityId>, damage: u32 },
}

impl World {
    /// Current health, if it can be hurt at all.
    pub fn health(&self, id: EntityId) -> Option<i64> {
        self.def(id).health?;
        Some(match self.get(id, "health") {
            Some(Value::Int(i)) => *i,
            Some(Value::Float(f)) => f.floor() as i64,
            _ => 0,
        })
    }

    /// Money in hand: a whole number, 0 if it has none.
    pub fn money(&self, id: EntityId) -> i64 {
        match self.get(id, "money") {
            Some(Value::Int(i)) => *i,
            Some(Value::Float(f)) => f.floor() as i64,
            _ => 0,
        }
    }

    /// Whether `id` keeps track of money at all. A merchant without it can always pay.
    pub fn has_money(&self, id: EntityId) -> bool {
        matches!(self.get(id, "money"), Some(Value::Int(_) | Value::Float(_)))
    }

    pub fn set_money(&mut self, id: EntityId, amount: i64) {
        self.set(id, "money", Value::Int(amount.max(0)));
    }

    /// A character that's in the world and hasn't been defeated.
    pub fn is_active(&self, id: EntityId) -> bool {
        self.def(id).kind == EntityKind::Character
            && self.room_of(id).is_some()
            && self.health(id).is_none_or(|h| h > 0)
    }

    /// Whether `a` and `b` are in the same room.
    pub fn together(&self, a: EntityId, b: EntityId) -> bool {
        matches!((self.room_of(a), self.room_of(b)), (Some(x), Some(y)) if x == y)
    }

    /// Merchants in the player's room.
    pub fn merchants_here(&self) -> Vec<EntityId> {
        let here = self.here();
        (0..self.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| {
                self.def(id).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Merchant)
                    && self.is_active(id)
                    && self.room_of(id) == Some(here)
            })
            .collect()
    }

    /// Where a character can go from `room`: exits to another room that
    /// aren't conditional, and whose door (if any) isn't locked, blocked
    /// or hidden from this side. Characters slip through closed doors
    /// without changing them. With an `area`, only rooms in it.
    pub fn npc_exits(&self, npc: EntityId, room: EntityId) -> Vec<(Direction, EntityId)> {
        let area = self.def(npc).npc.as_ref().map(|n| n.area.as_slice()).unwrap_or(&[]);
        self.def(room)
            .exits
            .iter()
            .filter(|e| e.when.is_none())
            .filter_map(|e| Some((e, e.to?)))
            .filter(|(e, _)| {
                e.through.is_none_or(|d| {
                    !self.flag(d, "locked") && self.door_blocked(d, room).is_none() && !self.hidden_from(d, room)
                })
            })
            .filter(|(_, to)| area.is_empty() || area.contains(to))
            .map(|(e, to)| (e.direction, to))
            .collect()
    }

    /// The first step of a shortest route from `npc`'s room to `goal`, and
    /// how many rooms away `goal` is. `(None, 0)` when already there.
    fn route(&self, npc: EntityId, goal: EntityId) -> Option<(Option<(Direction, EntityId)>, u32)> {
        let start = self.room_of(npc)?;
        if start == goal {
            return Some((None, 0));
        }
        let mut first: HashMap<EntityId, ((Direction, EntityId), u32)> = HashMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(room) = queue.pop_front() {
            let (step, dist) = match first.get(&room) {
                Some(&(s, d)) => (Some(s), d),
                None => (None, 0),
            };
            for (dir, to) in self.npc_exits(npc, room) {
                if to == start || first.contains_key(&to) {
                    continue;
                }
                let step = step.unwrap_or((dir, to));
                if to == goal {
                    return Some((Some(step), dist + 1));
                }
                first.insert(to, (step, dist + 1));
                queue.push_back(to);
            }
        }
        None
    }

    /// How many moves `npc` needs to reach `target`'s room, or None if it can't.
    pub fn distance(&self, npc: EntityId, target: EntityId) -> Option<u32> {
        let goal = self.room_of(target)?;
        self.route(npc, goal).map(|(_, d)| d)
    }

    fn npc_move(&mut self, npc: EntityId, direction: Direction, to: EntityId) -> Option<NpcEvent> {
        let from = self.room_of(npc)?;
        self.move_to(npc, Some(to)).ok()?;
        Some(NpcEvent::Moved { npc, from, to, direction })
    }

    /// Goes through the exit in `direction`, if it can.
    pub fn npc_go(&mut self, npc: EntityId, direction: Direction) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        let (_, to) = self.npc_exits(npc, room).into_iter().find(|(d, _)| *d == direction)?;
        self.npc_move(npc, direction, to)
    }

    /// Moves to a random neighbouring room, if there's one it can reach.
    pub fn npc_wander(&mut self, npc: EntityId) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        let exits = self.npc_exits(npc, room);
        if exits.is_empty() {
            return None;
        }
        let pick = self.state.rng.range(0, exits.len() as i64 - 1) as usize;
        let (dir, to) = exits[pick];
        self.npc_move(npc, dir, to)
    }

    /// Takes one step along the shortest route to `target`'s room.
    pub fn npc_step_toward(&mut self, npc: EntityId, target: EntityId) -> Option<NpcEvent> {
        let goal = self.room_of(target)?;
        let (step, _) = self.route(npc, goal)?;
        let (dir, to) = step?;
        self.npc_move(npc, dir, to)
    }

    /// `attacker` hits `target`, which must be able to be hurt. The damage
    /// is the weapon's, or else the attacker's own, or else 1.
    pub fn attack(&mut self, attacker: EntityId, target: EntityId, weapon: Option<EntityId>) -> Option<NpcEvent> {
        let health = self.health(target)?;
        let damage = weapon
            .and_then(|w| self.def(w).damage)
            .or(self.def(attacker).damage)
            .unwrap_or(1);
        self.set(target, "health", Value::Int((health - i64::from(damage)).max(0)));
        Some(NpcEvent::Attacked { attacker, target, weapon, damage })
    }

    /// The best weapon `id` is carrying: the one that does the most damage.
    pub fn best_weapon(&self, id: EntityId) -> Option<EntityId> {
        self.contents(id).into_iter().filter(|&w| self.def(w).damage.is_some()).max_by_key(|&w| self.def(w).damage)
    }

    /// Rolls the character's `wander` chance, and moves if it comes up.
    fn maybe_wander(&mut self, npc: EntityId, chance: u32) -> Option<NpcEvent> {
        if chance == 0 || self.state.rng.range(1, 100) > i64::from(chance) {
            return None;
        }
        self.npc_wander(npc)
    }

    /// One turn of a character's built-in behaviour.
    pub fn npc_behave(&mut self, npc: EntityId) -> Vec<NpcEvent> {
        let Some(n) = self.def(npc).npc.clone() else { return Vec::new() };
        if !self.is_active(npc) {
            return Vec::new();
        }
        let with_player = self.together(npc, PLAYER);
        let in_range = |w: &World| w.distance(npc, PLAYER).is_some_and(|d| n.range.is_none_or(|r| d <= r));
        let event = match n.behavior {
            Behavior::Stationary | Behavior::Conversationalist | Behavior::Wanderer => self.maybe_wander(npc, n.wander),
            // Merchants don't walk off in the middle of a sale.
            Behavior::Merchant if with_player => None,
            Behavior::Merchant => self.maybe_wander(npc, n.wander),
            Behavior::Hunter | Behavior::Guard if with_player => {
                let weapon = self.best_weapon(npc);
                self.attack(npc, PLAYER, weapon).or(Some(NpcEvent::Attacked {
                    attacker: npc,
                    target: PLAYER,
                    weapon,
                    damage: 0,
                }))
            }
            Behavior::Hunter if in_range(self) => self.npc_step_toward(npc, PLAYER),
            Behavior::Hunter | Behavior::Guard => self.maybe_wander(npc, n.wander),
            Behavior::Follower if with_player => None,
            Behavior::Follower if in_range(self) => self.npc_step_toward(npc, PLAYER),
            Behavior::Follower => self.maybe_wander(npc, n.wander),
        };
        event.into_iter().collect()
    }
}
