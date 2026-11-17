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
        if !self.has(id, Trait::Flammable) || self.flag(id, "burning") {
            return false;
        }
        self.set(id, "burning", Value::Bool(true));
        self.set(id, "_fire", Value::Int(BURNS_FOR));
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
        self.world.ignite(t);
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

    /// Each turn: every fire spreads to one more flammable thing in its
    /// room (or inside it), then burns down; burnt out, a destructible
    /// thing is gone.
    pub(crate) fn fires(&mut self) {
        let here = self.world.here();
        let burning: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&i| self.world.on_fire(i) && self.world.location(i).is_some())
            .collect();
        for fire in burning {
            let Some(room) = self.world.room_of(fire) else { continue };
            let seen = room == here;
            // Spread: to something inside it first, then something nearby.
            let next = (0..self.world.story().entities.len() as u32).map(EntityId).find(|&i| {
                i != fire
                    && self.world.has(i, Trait::Flammable)
                    && !self.world.flag(i, "burning")
                    && self.world.def(i).kind == EntityKind::Object
                    && (self.world.location(i) == Some(fire) || (self.world.room_of(i) == Some(room) && self.world.in_plain_sight(i)))
            });
            if let Some(n) = next
                && self.world.ignite(n)
                && seen
            {
                self.msg("fire.spreads", &Ctx::new().entity("target", n));
            }
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
}
