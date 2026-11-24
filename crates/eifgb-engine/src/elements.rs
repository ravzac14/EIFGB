//! The physical world: water that rises and drains (and holding a breath
//! in it), weight on a pressure plate, a dog following a scent, echoes and
//! a whispering gallery, and the moon.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{EntityId, EntityKind, Value, MOON_PHASES, PLAYER};

/// How long the player can hold their breath, unless the game says.
const BREATH: i64 = 4;
/// Water this deep is over the player's head.
pub(crate) const DEEP: i64 = 3;

impl<H: ScriptHost> Game<H> {
    /// How deep the water is in a room: 0 dry … 3 over the player's head.
    pub(crate) fn water(&self, room: EntityId) -> i64 {
        match self.world.get(room, "_water") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        }
    }

    /// For LOOK: how deep the water is here.
    pub(crate) fn water_line(&mut self, room: EntityId) -> Option<String> {
        let n = self.water(room);
        (n > 0).then(|| self.msg_str(&format!("water.level_{}", n.min(DEEP)), &Ctx::new()))
    }

    /// What's done after every turn: water, weight, scent.
    pub(crate) fn elements_turn(&mut self) {
        self.floods_turn();
        self.pressure_turn();
        self.trails_turn();
    }

    // -------------------------------------------------------------- water

    fn floods_turn(&mut self) {
        let turn = self.world.state.turn;
        let here = self.world.here();
        let rooms: Vec<EntityId> = self.world.ids().filter(|&r| self.world.def(r).floods.is_some()).collect();
        for r in rooms {
            let Some(f) = self.world.def(r).floods.clone() else { continue };
            let level = self.water(r);
            let rising = match self.world.get(r, "_flooding") {
                Some(Value::Bool(b)) => *b,
                _ => f.starts,
            };
            let drained = f.drains.is_some_and(|d| self.world.is_open(d));
            let next = if drained && level > 0 {
                level - 1
            } else if !drained && rising && level < i64::from(f.depth) && turn.is_multiple_of(u64::from(f.every)) {
                level + 1
            } else {
                level
            };
            if next == level {
                continue;
            }
            self.world.set(r, "_water", Value::Int(next));
            if r == here {
                let depth = if next > 0 { self.msg_str(&format!("water.depth_{}", next.min(DEEP)), &Ctx::new()) } else { String::new() };
                let key = if next > level { "water.rises" } else if next == 0 { "water.gone" } else { "water.falls" };
                self.msg(key, &Ctx::new().text("depth", depth));
            }
        }
        self.hold_breath();
    }

    /// In water over their head, the player holds their breath (unless
    /// something keeps them afloat), and when it's gone, struggles back.
    fn hold_breath(&mut self) {
        let here = self.world.here();
        let afloat = self.world.inventory_all().into_iter().any(|t| self.world.def(t).floats);
        if self.water(here) < DEEP || afloat {
            self.world.unset(PLAYER, "_breath");
            return;
        }
        let most = self.world.def(PLAYER).breath.map_or(BREATH, i64::from);
        let left = match self.world.get(PLAYER, "_breath") {
            Some(Value::Int(n)) => *n,
            _ => most,
        } - 1;
        self.world.set(PLAYER, "_breath", Value::Int(left));
        if left == 1 {
            self.msg_kind(LineKind::Notice, "water.breath_low", &Ctx::new());
        }
        if left > 0 {
            return;
        }
        self.world.unset(PLAYER, "_breath");
        self.msg("water.gasping", &Ctx::new());
        if let Some(h) = self.world.health(PLAYER) {
            self.world.set(PLAYER, "health", Value::Int((h - 1).max(0)));
        }
        // Back the way they came, if there's air there.
        if let Some(back) = self.world.state.came_from.filter(|&b| b != here && self.water(b) < DEEP) {
            let _ = self.world.move_to(PLAYER, Some(back));
            self.world.state.came_from = Some(here);
            self.look();
        }
    }

    /// SWIM NORTH: going, in water. SWIM on its own: which way?
    pub(crate) fn swim(&mut self, cmd: &crate::parser::Command, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let into = cmd.direction.and_then(|d| self.world.def(here).exits.iter().find(|e| e.direction == d).and_then(|e| e.to));
        if self.water(here) == 0 && into.is_none_or(|r| self.water(r) == 0) {
            self.no_turn = true;
            self.msg("swim.no_water", ctx);
            return false;
        }
        if cmd.direction.is_none() {
            self.no_turn = true;
            self.msg("swim.where", ctx);
            return false;
        }
        self.go(cmd, ctx)
    }

    // ------------------------------------------------------------- weight

    fn pressure_turn(&mut self) {
        let plates: Vec<EntityId> = self.world.ids().filter(|&p| self.world.def(p).pressure.is_some()).collect();
        let here = self.world.here();
        for p in plates {
            let Some(pr) = self.world.def(p).pressure.clone() else { continue };
            let down = self.world.load(p) >= pr.at_least;
            if down == self.world.flag(p, "_pressed") {
                continue;
            }
            self.world.set(p, "_pressed", Value::Bool(down));
            let seen = self.world.room_of(p) == Some(here) && !self.world.in_darkness();
            if seen {
                match if down { &pr.down } else { &pr.up } {
                    Some(t) => self.render_line(LineKind::Text, t, &Ctx::new().this(p)),
                    None => self.msg(if down { "pressure.down" } else { "pressure.up" }, &Ctx::new().entity("target", p)),
                }
            }
            for d in pr.opens {
                self.world.set(d, "open", Value::Bool(down));
                if down {
                    self.world.set(d, "locked", Value::Bool(false));
                }
                if self.world.in_scope(d) {
                    self.msg(if down { "pressure.opens" } else { "pressure.closes" }, &Ctx::new().entity("target", d));
                }
            }
        }
    }

    // -------------------------------------------------------------- scent

    /// Whose scent a thing carries: its `scent_of`, or whoever had it at the start.
    fn scent_of(&self, t: EntityId) -> Option<EntityId> {
        if let Some(s) = self.world.def(t).scent_of {
            return Some(s);
        }
        let first = self.world.def(t).location?;
        if first != PLAYER && self.world.def(first).kind == EntityKind::Character {
            return Some(first);
        }
        match self.world.get(t, "_stolen_from") {
            Some(Value::Int(i)) => Some(EntityId(*i as u32)),
            _ => None,
        }
    }

    /// LET THE DOG SNIFF THE GLOVE: it follows the scent, leading the way.
    pub(crate) fn scent_action(&mut self, dog: EntityId, thing: Option<EntityId>, ctx: &Ctx) -> bool {
        let w = &self.world;
        if w.def(dog).kind != EntityKind::Character || dog == PLAYER || !w.is_active(dog) || w.is_asleep(dog) || !w.def(dog).sniffs || w.is_hostile(dog, PLAYER) {
            self.msg("sniff.wont", &ctx.clone().entity("npc", dog));
            return false;
        }
        let Some(thing) = thing else {
            self.no_turn = true;
            self.msg("sniff.what", &ctx.clone().entity("npc", dog));
            return false;
        };
        let ctx = ctx.clone().entity("npc", dog).entity("target", thing);
        let Some(who) = self.scent_of(thing).filter(|&s| s != PLAYER && self.world.room_of(s).is_some()) else {
            self.msg("sniff.nothing", &ctx);
            return true;
        };
        self.msg("sniff.start", &ctx);
        self.world.set(dog, "_trailing", Value::Int(i64::from(who.0)));
        true
    }

    fn trails_turn(&mut self) {
        let dogs: Vec<EntityId> = self.world.ids().filter(|&d| matches!(self.world.get(d, "_trailing"), Some(Value::Int(_)))).collect();
        for dog in dogs {
            let Some(Value::Int(i)) = self.world.get(dog, "_trailing").cloned() else { continue };
            let target = EntityId(i as u32);
            if !self.world.is_active(dog) || self.world.room_of(target).is_none() {
                self.world.unset(dog, "_trailing");
                continue;
            }
            // It leads: it waits for the player to catch up.
            if !self.world.together(dog, PLAYER) {
                continue;
            }
            let ctx = Ctx::new().entity("npc", dog).entity("target", target);
            if self.world.room_of(dog) == self.world.room_of(target) {
                self.world.unset(dog, "_trailing");
                if self.world.is_hidden(target) {
                    self.world.unset(target, "hidden");
                    self.msg("sniff.digs", &ctx);
                } else {
                    self.msg("sniff.found", &ctx);
                }
                continue;
            }
            match self.world.npc_step_toward(dog, target) {
                Some(event) => self.narrate(&event),
                None => {
                    self.world.unset(dog, "_trailing");
                    self.msg("sniff.lost", &ctx);
                }
            }
        }
    }

    // ------------------------------------------------------------- echoes

    /// Something said or shouted here: the echo, if the room has one. True
    /// if there was one.
    pub(crate) fn echo(&mut self, words: Option<&str>) -> bool {
        let here = self.world.here();
        let Some(answers) = self.world.def(here).echo.clone() else { return false };
        let said = words.unwrap_or_default().trim().to_lowercase();
        if let Some((_, t)) = answers.iter().find(|(k, _)| !said.is_empty() && said.contains(k.as_str())) {
            self.render_line(LineKind::Text, t, &Ctx::new().this(here));
            return true;
        }
        match said.split_whitespace().last() {
            Some(last) => {
                let last = last.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                self.msg("echo.repeat", &Ctx::new().text("word", last));
            }
            None => self.msg("echo.plain", &Ctx::new()),
        }
        true
    }
}

impl crate::World {
    /// The moon tonight: "full", "waxing crescent"…, if the game keeps time.
    pub fn moon(&self) -> Option<&'static str> {
        let c = self.story().clock.as_ref()?;
        let day = self.state.clock / u64::from(eifgb_ir::DAY);
        Some(MOON_PHASES[((day + u64::from(c.moon)) % 8) as usize])
    }
}
