//! Fire, and smashing things. `flammable` things can be set alight; a
//! fire burns for a few turns, spreads to one more flammable thing nearby
//! each turn, lights the room and frightens creatures that fear fire.
//! When it burns out, a `destructible` thing is gone, spilling whatever
//! was in it; anything else is just scorched. `destructible` things can
//! also be smashed.

use crate::game::Game;
use crate::script::{Ctx, ScriptHost};
use crate::world::World;
use eifgb_ir::{EntityId, EntityKind, Trait, Value, PLAYER};

/// How many turns a fire burns before it goes out.
const BURNS_FOR: i64 = 3;

impl World {
    /// Sets a flammable thing alight. False if it can't burn, or already is.
    pub fn ignite(&mut self, id: EntityId) -> bool {
        self.ignite_from(id, None)
    }

    /// Sets `id` alight, remembering whose fire it is (the player's, when
    /// they started it), so a death in it can be laid at their door.
    pub fn ignite_from(&mut self, id: EntityId, from: Option<EntityId>) -> bool {
        if !self.has(id, Trait::Flammable) || self.flag(id, "burning") {
            return false;
        }
        self.set(id, "burning", Value::Bool(true));
        self.set(id, "_fire", Value::Int(BURNS_FOR));
        let lit_by = from.and_then(|f| match self.get(f, "_lit_by") {
            Some(v) => Some(v.clone()),
            None if f == PLAYER => Some(Value::Str(self.def(PLAYER).key.clone())),
            None => None,
        });
        match lit_by {
            Some(v) => self.set(id, "_lit_by", v),
            None => self.unset(id, "_lit_by"),
        }
        true
    }

    /// Something flammable that's on fire right now.
    pub fn on_fire(&self, id: EntityId) -> bool {
        self.has(id, Trait::Flammable) && self.flag(id, "burning")
    }

    /// Destroys `id`: what was in or on it falls where it was. Returns what fell.
    pub fn destroy(&mut self, id: EntityId) -> Vec<EntityId> {
        let place = self.location(id);
        let contents = self.contents(id);
        for &c in &contents {
            let _ = self.move_to(c, place);
        }
        self.unset(id, "burning");
        let _ = self.move_to(id, None);
        contents
    }

    /// A burning thing in `room` that creatures afraid of fire would run from.
    pub fn fire_in(&self, room: EntityId) -> Option<EntityId> {
        (0..self.story().entities.len() as u32)
            .map(EntityId)
            .find(|&i| self.on_fire(i) && self.room_of(i) == Some(room) && self.in_plain_sight(i))
    }

    /// Whether `id` (or its faction) fears fire.
    pub fn fears_fire(&self, id: EntityId) -> bool {
        let own = self.def(id).npc.as_ref().is_some_and(|n| n.fears.iter().any(|f| f == "fire"));
        own || self.faction(id).is_some_and(|f| f.fears.iter().any(|x| x == "fire"))
    }
}

impl<H: ScriptHost> Game<H> {
    /// SET X ALIGHT / BURN X (WITH Y): needs something burning in hand.
    pub(crate) fn burn_action(&mut self, t: EntityId, with: Option<EntityId>, ctx: &Ctx) -> bool {
        if !self.world.has(t, Trait::Flammable) {
            self.msg("burn.cant", ctx);
            return false;
        }
        if self.world.flag(t, "burning") {
            self.msg("burn.already", ctx);
            return false;
        }
        let flame = match with {
            Some(f) => (self.world.is_within(f, PLAYER) && self.world.is_burning(f)).then_some(f),
            None => (0..self.world.story().entities.len() as u32)
                .map(EntityId)
                .find(|&i| i != t && self.world.is_within(i, PLAYER) && self.world.is_burning(i)),
        };
        let Some(flame) = flame else {
            self.msg("burn.no_flame", ctx);
            return false;
        };
        self.world.ignite_from(t, Some(PLAYER));
        self.msg("burn.done", &ctx.clone().entity("tool", flame));
        true
    }

    /// SMASH X, for something `destructible` that has no health to lose.
    pub(crate) fn smash(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let fell = self.world.destroy(t);
        self.msg("smash.done", ctx);
        self.spill(t, &fell);
        true
    }

    fn spill(&mut self, from: EntityId, fell: &[EntityId]) {
        let visible: Vec<EntityId> = fell.iter().copied().filter(|&i| !self.world.is_hidden(i)).collect();
        if !visible.is_empty() && fell.iter().any(|&i| self.world.room_of(i) == Some(self.world.here())) {
            let list = self.list_a(&visible);
            self.msg("destroy.spills", &Ctx::new().entity("target", from).text("list", list));
        }
    }

    /// PUT OUT / POUR WATER ON / THROW SAND AT a fire: with something that
    /// `douses` (used up), or by hand if it's small enough to smother.
    pub(crate) fn douse(&mut self, t: EntityId, with: Option<EntityId>, ctx: &Ctx) -> bool {
        if let Some(w) = with
            && !self.world.def(w).douses
        {
            self.msg("douse.cant", ctx);
            return false;
        }
        if !self.world.on_fire(t) {
            self.msg("extinguish.not_lit", ctx);
            return false;
        }
        if with.is_none() && !self.world.has(t, Trait::Portable) {
            self.msg("fire.too_big", ctx);
            return false;
        }
        self.world.set(t, "burning", Value::Bool(false));
        self.world.unset(t, "_fire");
        match with {
            Some(w) => {
                let _ = self.world.move_to(w, None);
                self.msg("douse.done", ctx);
            }
            None => self.msg("extinguish.done", ctx),
        }
        true
    }

    /// Each turn: every fire spreads to one more flammable thing in its
    /// room (or inside it), or else through an open way into the next room;
    /// scorches everyone in a burning room; then burns down. Burnt out, a
    /// destructible thing is gone. Smoke drifts into neighbouring rooms.
    pub(crate) fn fires(&mut self) {
        let here = self.world.here();
        let burning_before: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&i| self.world.on_fire(i) && self.world.location(i).is_some())
            .collect();
        let mut burning_rooms: Vec<EntityId> = Vec::new();
        for &fire in &burning_before {
            let Some(room) = self.world.room_of(fire) else { continue };
            if !burning_rooms.contains(&room) {
                burning_rooms.push(room);
            }
            let seen = room == here;
            let catchable = |w: &World, i: EntityId| {
                i != fire && w.has(i, Trait::Flammable) && !w.flag(i, "burning") && w.def(i).kind == EntityKind::Object
            };
            // Spread: to something inside it first, then something nearby.
            let near = (0..self.world.story().entities.len() as u32).map(EntityId).find(|&i| {
                catchable(&self.world, i)
                    && (self.world.location(i) == Some(fire) || (self.world.room_of(i) == Some(room) && self.world.in_plain_sight(i)))
            });
            if let Some(n) = near {
                if self.world.ignite_from(n, Some(fire)) && seen {
                    self.msg("fire.spreads", &Ctx::new().entity("target", n));
                }
            } else if let Some((dir, n)) = self.spread_next_door(fire, room) {
                // Nothing left here to catch: it leaps through an open way.
                self.world.ignite_from(n, Some(fire));
                if seen {
                    let d = self.grammar.direction_word(dir);
                    self.msg("fire.spreads_away", &Ctx::new().text("direction", d));
                } else if self.world.room_of(n) == Some(here) {
                    self.msg("fire.spreads_in", &Ctx::new().entity("target", n));
                }
            }
        }
        for room in &burning_rooms {
            self.scorch(*room);
        }
        // Smoke drifts into the player's room from a fire next door.
        if !burning_rooms.contains(&here) {
            let exits = self.world.def(here).exits.clone();
            if let Some(e) = exits.iter().find(|e| e.to.is_some_and(|t| burning_rooms.contains(&t))) {
                let from = self.msg_str(&format!("from.{}", e.direction.key()), &Ctx::new());
                self.msg("fire.smoke", &Ctx::new().text("from", from));
            }
        }
        // Burn down: only fires that were already burning at the start of
        // the turn, not ones that caught just now.
        for fire in burning_before {
            if !self.world.on_fire(fire) {
                continue;
            }
            let seen = self.world.room_of(fire) == Some(here);
            let left = match self.world.get(fire, "_fire") {
                Some(Value::Int(i)) => *i,
                _ => BURNS_FOR,
            } - 1;
            if left > 0 {
                self.world.set(fire, "_fire", Value::Int(left));
                continue;
            }
            self.world.unset(fire, "_fire");
            if self.world.has(fire, Trait::Destructible) {
                let fell = self.world.destroy(fire);
                if seen {
                    self.msg("fire.burnt_up", &Ctx::new().entity("target", fire));
                    self.spill(fire, &fell);
                }
            } else {
                self.world.set(fire, "burning", Value::Bool(false));
                if seen {
                    self.msg("fire.burns_out", &Ctx::new().entity("target", fire));
                }
            }
        }
    }

    /// The first flammable thing, in plain sight, in a room next to `room`
    /// through a way that isn't shut.
    fn spread_next_door(&self, fire: EntityId, room: EntityId) -> Option<(eifgb_ir::Direction, EntityId)> {
        let w = &self.world;
        w.def(room).exits.iter().filter(|e| e.through.is_none_or(|d| w.is_open(d))).find_map(|e| {
            let to = e.to?;
            let n = (0..w.story().entities.len() as u32).map(EntityId).find(|&i| {
                i != fire
                    && w.has(i, Trait::Flammable)
                    && !w.flag(i, "burning")
                    && w.def(i).kind == EntityKind::Object
                    && w.room_of(i) == Some(to)
                    && w.in_plain_sight(i)
            })?;
            Some((e.direction, n))
        })
    }

    /// Everyone in a burning room takes a point of damage, and sleepers wake.
    fn scorch(&mut self, room: EntityId) {
        let here = self.world.here();
        let fire = self.world.fire_in(room);
        let lit_by = fire.and_then(|f| self.world.get(f, "_lit_by").cloned());
        self.world.wake_room(room);
        let victims: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&c| self.world.room_of(c) == Some(room) && (c == PLAYER || self.world.is_active(c)))
            .filter(|&c| self.world.health(c).is_some_and(|h| h > 0) && !self.world.is_invincible(c))
            .collect();
        for c in victims {
            let h = self.world.health(c).unwrap_or(0) - 1;
            self.world.set(c, "health", Value::Int(h.max(0)));
            if c == PLAYER {
                self.msg("fire.scorches_you", &Ctx::new());
                continue;
            }
            if h <= 0 {
                self.world.set(c, "_burnt", Value::Bool(true));
                match &lit_by {
                    Some(v) => self.world.set(c, "killed_by", v.clone()),
                    None => self.world.unset(c, "killed_by"),
                }
            }
            if room == here {
                self.msg("fire.scorches", &Ctx::new().entity("npc", c));
            }
        }
    }
}
