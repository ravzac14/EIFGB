//! Getting about: boats and carts that carry the player along the ways
//! that are `by` them, horses to ride (faster, until they tire), and
//! crowds to get lost in.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{EntityId, Exit, Value, PLAYER};

impl crate::World {
    /// The horse the player is riding.
    pub fn riding(&self) -> Option<EntityId> {
        match self.get(PLAYER, "_riding") {
            Some(Value::Str(k)) => self.id(k),
            _ => None,
        }
    }

    /// The boat or cart the player is in (directly, in this room).
    pub fn vehicle_here(&self) -> Option<EntityId> {
        let here = self.here();
        self.location(PLAYER).filter(|&v| v != here && self.def(v).vehicle && self.location(v) == Some(here))
    }

    /// Whether the player can go this way as they are: a way `by` a boat
    /// or a horse only in it or on it.
    pub fn conveyed(&self, e: &Exit) -> bool {
        e.by.is_empty() || self.vehicle_here().is_some_and(|v| e.by.contains(&v)) || self.riding().is_some_and(|m| e.by.contains(&m))
    }

    /// A ridden horse's moves left before it needs feeding.
    pub fn stamina(&self, mount: EntityId) -> Option<i64> {
        let full = i64::from(self.def(mount).stamina?);
        Some(match self.get(mount, "_stamina") {
            Some(Value::Int(n)) => *n,
            _ => full,
        })
    }

    /// Crowded now: always, or within its hours.
    pub fn is_crowded(&self, room: EntityId) -> bool {
        let Some(c) = self.def(room).crowd else { return false };
        match (c.from, c.to) {
            (Some(from), Some(to)) => {
                let now = (self.state.clock % u64::from(eifgb_ir::DAY)) as u32;
                if from <= to { now >= from && now < to } else { now >= from || now < to }
            }
            _ => true,
        }
    }

    /// The player can't be picked out of the crowd here, by someone who'd
    /// attack them: unless they've made a scene in it.
    pub fn lost_in_crowd(&self) -> bool {
        let Some(room) = self.room_of(PLAYER) else { return false };
        self.is_crowded(room) && !matches!(self.get(PLAYER, "_scene"), Some(Value::Str(k)) if *k == self.def(room).key)
    }

    /// The player draws every eye in this crowd.
    pub fn make_scene(&mut self) {
        if let Some(room) = self.room_of(PLAYER).filter(|&r| self.is_crowded(r)) {
            let key = self.def(room).key.clone();
            self.set(PLAYER, "_scene", Value::Str(key));
        }
    }
}

impl<H: ScriptHost> Game<H> {
    /// RIDE THE HORSE: only one that's willing.
    pub(crate) fn ride(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let w = &self.world;
        if !w.def(t).mount || t == PLAYER {
            self.msg("ride.cant", ctx);
            return false;
        }
        if w.riding() == Some(t) {
            self.msg("ride.already", ctx);
            return false;
        }
        let willing = w.is_active(t)
            && !w.is_asleep(t)
            && !w.is_bound(t)
            && !w.is_hostile(t, PLAYER)
            && (w.is_tamed(t) || w.disposition(t) >= 2 || w.def(t).npc.as_ref().is_some_and(|n| n.obeys || n.behavior == eifgb_ir::Behavior::Follower));
        if !willing {
            self.msg("ride.wont", ctx);
            return false;
        }
        if self.world.location(PLAYER) != Some(self.world.here()) {
            self.msg("ride.get_out", ctx);
            return false;
        }
        let key = self.world.def(t).key.clone();
        self.world.set(PLAYER, "_riding", Value::Str(key));
        self.msg("ride.done", ctx);
        true
    }

    /// DISMOUNT, GET OFF THE HORSE.
    pub(crate) fn dismount(&mut self, ctx: &Ctx) -> bool {
        let Some(m) = self.world.riding() else {
            self.msg("dismount.not_riding", ctx);
            return false;
        };
        self.world.unset(PLAYER, "_riding");
        self.msg("dismount.done", &ctx.clone().entity("target", m));
        true
    }

    /// Whether the horse will go on: not when it's worn out. Says so.
    pub(crate) fn mount_will_go(&mut self) -> bool {
        let Some(m) = self.world.riding() else { return true };
        if self.world.room_of(m) != self.world.room_of(PLAYER) || !self.world.is_active(m) || self.world.is_bound(m) {
            self.world.unset(PLAYER, "_riding");
            return true;
        }
        if self.world.stamina(m).is_some_and(|n| n <= 0) {
            self.msg("ride.tired", &Ctx::new().entity("target", m));
            return false;
        }
        true
    }

    /// Ridden a step: the horse comes too, and tires a little.
    pub(crate) fn mount_follows(&mut self, to: EntityId) {
        let Some(m) = self.world.riding() else { return };
        let _ = self.world.move_to(m, Some(to));
        if let Some(n) = self.world.stamina(m) {
            self.world.set(m, "_stamina", Value::Int(n - 1));
            if n - 1 == 0 {
                self.msg_kind(LineKind::Notice, "ride.tiring", &Ctx::new().entity("target", m));
            }
        }
    }

    /// Fed something it likes, a ridden horse is ready to go again.
    pub(crate) fn feed_mount(&mut self, item: EntityId, to: EntityId) {
        let w = &self.world;
        let Some(n) = w.def(to).npc.as_ref() else { return };
        if !w.def(to).mount || !(n.likes.contains(&item) || n.tame_with.contains(&item)) {
            return;
        }
        let tired = w.stamina(to).is_some_and(|s| s < i64::from(w.def(to).stamina.unwrap_or(0)));
        self.world.unset(to, "_stamina");
        if tired {
            self.msg_kind(LineKind::Notice, "ride.fed", &Ctx::new().entity("target", to));
        }
    }

    /// Caught stealing in a crowd, perhaps: someone shouts.
    pub(crate) fn thief_spotted_in_crowd(&mut self) -> bool {
        let spotted = self.world.state.rng.range(0, 2) == 0;
        if spotted {
            self.msg("crowd.thief", &Ctx::new());
            self.world.make_scene();
        }
        spotted
    }
}
