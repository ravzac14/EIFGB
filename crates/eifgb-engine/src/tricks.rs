//! Things a creative player can do with what's around them: put two things
//! together to make a third, barricade a door, leave a mark to find their
//! way, swap a thing for a fake while someone's watching, and play dead.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{Combination, EntityId, EntityKind, Trait, Value, PLAYER};

/// The verbs that put two things together when a combination doesn't say.
const COMBINING: &[&str] = &["combine", "tie", "put_in", "put_on", "use", "pour", "fill"];

/// The marks a player makes: one each room, in its private `_mark` state.
const MARK: &str = "_mark";

impl<H: ScriptHost> Game<H> {
    // ------------------------------------------------------------ combining

    /// A combination that `verb` on `a` with `b` makes (either way round).
    pub(crate) fn combination(&self, verb: &str, a: Option<EntityId>, b: Option<EntityId>) -> Option<(EntityId, EntityId, Combination)> {
        let (a, b) = (a?, b?);
        let w = &self.world;
        let pick = |x: EntityId, y: EntityId| {
            w.def(x).combines.iter().find(|c| c.with == y && (if c.by.is_empty() { COMBINING.contains(&verb) } else { c.by.iter().any(|v| v == verb) })).cloned().map(|c| (x, y, c))
        };
        // Only while what it makes is still to be made: nowhere yet.
        pick(a, b).or_else(|| pick(b, a)).filter(|(_, _, c)| w.location(c.makes).is_none() && !w.state.got.contains_key(&c.makes))
    }

    /// Makes it: the two used up (but any it `keep`s), the new thing in the
    /// player's hands if they held either, else where the first was.
    pub(crate) fn combine_now(&mut self, a: EntityId, b: EntityId, c: &Combination) -> bool {
        let held = self.world.is_within(a, PLAYER) || self.world.is_within(b, PLAYER);
        let place = if held { Some(PLAYER) } else { self.world.location(a) };
        for t in [a, b] {
            if !c.keep.contains(&t) {
                let _ = self.world.move_to(t, None);
            }
        }
        let _ = self.world.move_to(c.makes, place);
        let ctx = Ctx::new().entity("target", c.makes).entity("first", a).entity("second", b);
        match &c.say {
            Some(t) => self.render_line(LineKind::Text, t, &ctx),
            None => self.msg("combine.done", &ctx),
        }
        true
    }

    /// COMBINE X WITH Y, when nothing combines them.
    pub(crate) fn combine_action(&mut self, a: EntityId, b: Option<EntityId>, ctx: &Ctx) -> bool {
        match self.combination("combine", Some(a), b) {
            Some((x, y, c)) => self.combine_now(x, y, &c),
            None => {
                self.msg("combine.cant", ctx);
                false
            }
        }
    }

    // ----------------------------------------------------------- barricades

    /// PUSH THE WARDROBE AGAINST THE DOOR: blocked, both ways, until it's moved.
    pub(crate) fn barricade(&mut self, with: Option<EntityId>, door: Option<EntityId>, ctx: &Ctx) -> bool {
        // BARRICADE THE DOOR: the door is the one named.
        let (with, door) = match with {
            Some(t) if self.world.has(t, Trait::Door) && door.is_none() => (None, Some(t)),
            _ => (with, door),
        };
        let here = self.world.here();
        let Some(door) = door.filter(|&d| self.world.has(d, Trait::Door) && self.world.doors_of(here).contains(&d)) else {
            self.msg("barricade.not_door", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("tool", door);
        if self.world.blocked_anywhere(door) {
            self.msg("barricade.already", &ctx);
            return false;
        }
        // Without saying with what: the first heavy thing here.
        let heavy = |g: &Self, t: EntityId| g.world.has(t, Trait::Pushable) && g.world.location(t) == Some(here);
        let with = with.or_else(|| self.world.contents(here).into_iter().find(|&t| heavy(self, t)));
        let Some(with) = with else {
            self.msg("barricade.nothing", &ctx);
            return false;
        };
        let ctx = ctx.entity("target", with);
        if !heavy(self, with) {
            self.msg("barricade.not_pushable", &ctx);
            return false;
        }
        let door_key = self.world.def(door).key.clone();
        self.world.set(with, "_against", Value::Str(door_key));
        self.world.set(door, "blocked", Value::Bool(true));
        // A barricaded door is shut.
        self.world.enforce_door_rules();
        self.msg("barricade.done", &ctx);
        true
    }

    /// MOVE THE WARDROBE AWAY (FROM THE DOOR), UNBLOCK THE DOOR.
    pub(crate) fn unbarricade(&mut self, t: Option<EntityId>, door: Option<EntityId>, ctx: &Ctx) -> bool {
        // What's in the way: the thing named, or whatever's against the door named.
        let against = |g: &Self, x: EntityId| match g.world.get(x, "_against") {
            Some(Value::Str(k)) => g.world.story().entity_by_key(k),
            _ => None,
        };
        let here = self.world.here();
        let thing = t.filter(|&x| against(self, x).is_some()).or_else(|| {
            let door = door.or(t)?;
            self.world.contents(here).into_iter().find(|&x| against(self, x) == Some(door))
        });
        let Some(thing) = thing else {
            self.msg("unbarricade.nothing", ctx);
            return false;
        };
        let door = against(self, thing).expect("against a door");
        self.clear_barricade(thing, door);
        self.msg("unbarricade.done", &ctx.clone().entity("target", thing).entity("tool", door));
        true
    }

    fn clear_barricade(&mut self, thing: EntityId, door: EntityId) {
        self.world.unset(thing, "_against");
        self.world.set(door, "blocked", Value::Bool(false));
    }

    /// After every turn: a barricade that's been moved off (pushed away,
    /// carried off) frees its door.
    pub(crate) fn keep_barricades(&mut self) {
        let mut moved = Vec::new();
        for t in self.world.ids() {
            let Some(Value::Str(k)) = self.world.get(t, "_against") else { continue };
            let Some(door) = self.world.story().entity_by_key(k) else { continue };
            let room = self.world.location(t);
            if room.is_none_or(|r| !self.world.doors_of(r).contains(&door)) {
                moved.push((t, door));
            }
        }
        for (t, door) in moved {
            self.clear_barricade(t, door);
        }
    }

    /// What a barricade is against, for EXAMINE: "It's pushed against the door."
    pub(crate) fn against_note(&mut self, t: EntityId) -> Option<String> {
        let Some(Value::Str(k)) = self.world.get(t, "_against").cloned() else { return None };
        let door = self.world.story().entity_by_key(&k)?;
        Some(self.msg_str("barricade.against", &Ctx::new().entity("target", t).entity("tool", door)))
    }

    // ---------------------------------------------------------------- marks

    /// MARK, CHALK AN X, SCRATCH "THIS WAY": a mark of the player's here.
    pub(crate) fn mark(&mut self, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some(tool) = self.world.inventory_all().into_iter().find(|&t| self.world.def(t).marks) else {
            self.msg("mark.no_tool", ctx);
            return false;
        };
        // "the wall", "here", "a mark": a plain X.
        let said = text.unwrap_or("").trim().trim_end_matches(" here").trim_end_matches(" on the wall").trim_end_matches(" on the floor").trim();
        let plain = ["", "here", "the wall", "wall", "the floor", "floor", "the room", "room", "a mark", "mark", "it"];
        let mark = if plain.contains(&said) { self.msg_str("mark.plain", &Ctx::new()) } else { said.trim_matches('"').to_string() };
        let here = self.world.here();
        self.world.set(here, MARK, Value::Str(mark.clone()));
        self.msg("mark.done", &ctx.clone().text("mark", mark).entity("tool", tool));
        true
    }

    /// RUB OUT THE MARK.
    pub(crate) fn unmark(&mut self, ctx: &Ctx) -> bool {
        let here = self.world.here();
        if self.world.get(here, MARK).is_none() {
            self.msg("unmark.none", ctx);
            return false;
        }
        self.world.unset(here, MARK);
        self.msg("unmark.done", ctx);
        true
    }

    /// The player's mark in a room, if there is one.
    pub(crate) fn mark_in(&self, room: EntityId) -> Option<String> {
        match self.world.get(room, MARK) {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }

    // ---------------------------------------------------------------- swaps

    /// Who's watching `t` (it's in their `watches`) and can see the player
    /// take it now.
    pub(crate) fn watcher_of(&self, t: EntityId) -> Option<EntityId> {
        let w = &self.world;
        w.ids().find(|&c| {
            w.def(c).kind == EntityKind::Character
                && w.def(c).npc.as_ref().is_some_and(|n| n.watches.contains(&t))
                && w.is_active(c)
                && w.together(c, PLAYER)
                && w.notices_theft(c)
                && !w.is_tamed(c)
                && !w.is_bribed(c)
        })
    }

    /// Taking something that's watched: stopped, and they think less of you.
    pub(crate) fn stopped_taking(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let Some(npc) = self.watcher_of(t) else { return false };
        self.world.shift_disposition(npc, -1);
        self.msg("watch.stopped", &ctx.clone().entity("npc", npc).entity("target", t));
        true
    }

    /// REPLACE THE RUBY WITH THE GLASS ONE: take one, leave the other where
    /// it was. Someone watching notices, unless it's a good fake.
    pub(crate) fn swap(&mut self, take: EntityId, leave: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(leave) = leave else {
            self.msg("swap.with_what", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("target", take).entity("tool", leave);
        if !self.world.is_within(leave, PLAYER) {
            self.msg("put.not_held", &Ctx::new().entity("target", leave));
            return false;
        }
        if self.world.is_within(take, PLAYER) || !self.world.has(take, Trait::Portable) || take == leave {
            self.msg("swap.cant", &ctx);
            return false;
        }
        let watcher = self.watcher_of(take);
        let fake = self.world.def(leave).decoy_for == Some(take);
        if watcher.is_some() && !fake {
            return self.stopped_taking(take, &ctx);
        }
        let place = self.world.location(take);
        let _ = self.world.move_to(leave, place);
        let _ = self.world.move_to(take, Some(PLAYER));
        match watcher {
            Some(npc) => self.msg("swap.fooled", &ctx.entity("npc", npc)),
            None => self.msg("swap.done", &ctx),
        }
        true
    }

    // ----------------------------------------------------------- play dead

    /// PLAY DEAD: lie still. Anything that doesn't eat the dead leaves the
    /// player alone, until they do something else.
    pub(crate) fn play_dead(&mut self, ctx: &Ctx) -> bool {
        if self.world.flag(PLAYER, "_playing_dead") {
            self.msg("play_dead.again", ctx);
            return true;
        }
        self.world.set(PLAYER, "_playing_dead", Value::Bool(true));
        // Fooled again, by a new act.
        for c in self.world.ids().collect::<Vec<_>>() {
            self.world.unset(c, "_sniffed");
        }
        self.msg("play_dead.start", ctx);
        true
    }

    /// Doing anything but lying still (or waiting): up again.
    pub(crate) fn stop_playing_dead(&mut self) {
        if self.world.flag(PLAYER, "_playing_dead") {
            self.world.unset(PLAYER, "_playing_dead");
            self.msg_kind(LineKind::Notice, "play_dead.end", &Ctx::new());
        }
    }

    /// After characters act: those who'd have come for the player, fooled
    /// (once each), or not (they eat the dead).
    pub(crate) fn dead_act_seen(&mut self) {
        if !self.world.flag(PLAYER, "_playing_dead") {
            return;
        }
        let here: Vec<EntityId> = self.world.ids().filter(|&c| self.world.def(c).kind == EntityKind::Character && self.world.is_active(c) && self.world.together(c, PLAYER)).collect();
        for c in here {
            if self.world.flag(c, "_sniffed") || !self.world.would_be_hostile(c, PLAYER) {
                continue;
            }
            self.world.set(c, "_sniffed", Value::Bool(true));
            let key = if self.world.fooled_by_dead(c) { "npc.fooled" } else { "npc.not_fooled" };
            self.msg(key, &Ctx::new().entity("npc", c));
        }
    }
}
