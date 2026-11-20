//! How things in the world behave: flames and fuel, liquids, wear,
//! codes, reading, sounds and smells, and the weather.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{EntityId, EntityKind, Trait, Value, PLAYER};

/// How many actions before a light burns out its warning comes.
const LOW_FUEL: i64 = 2;

impl<H: ScriptHost> Game<H> {
    // ------------------------------------------------------------ flames

    /// What lights `t`, which `needs_flame`: the tool if it's burning (a
    /// lighter or match that isn't is lit first), or a burning thing the
    /// player holds. None, with the reason said, if there's nothing.
    pub(crate) fn flame_for(&mut self, t: EntityId, tool: Option<EntityId>, ctx: &Ctx) -> Result<EntityId, ()> {
        let w = &self.world;
        if let Some(f) = tool {
            if w.is_burning(f) {
                return Ok(f);
            }
            // LIGHT THE CANDLE WITH THE LIGHTER: flick the lighter first.
            if w.has(f, Trait::LightSource) && !w.def(f).needs_flame && w.is_within(f, PLAYER) && self.fuel_left(f) != Some(0) {
                self.set_lit(f, true);
                self.msg_kind(LineKind::Notice, "light.first", &Ctx::new().entity("tool", f));
                return Ok(f);
            }
            self.msg("light.tool_not_lit", &ctx.clone().entity("tool", f));
            return Err(());
        }
        let held = (0..w.story().entities.len() as u32)
            .map(EntityId)
            .find(|&i| i != t && w.is_within(i, PLAYER) && w.is_burning(i));
        match held {
            Some(f) => Ok(f),
            None => {
                self.msg("light.no_flame", ctx);
                Err(())
            }
        }
    }

    pub(crate) fn fuel_left(&self, t: EntityId) -> Option<i64> {
        self.world.def(t).fuel?;
        match self.world.get(t, "fuel") {
            Some(Value::Int(n)) => Some(*n),
            Some(Value::Float(f)) => Some(*f as i64),
            _ => Some(0),
        }
    }

    pub(crate) fn set_lit(&mut self, t: EntityId, on: bool) {
        let field = if self.world.get(t, "on").is_some() { "on" } else { "lit" };
        self.world.set(t, field, Value::Bool(on));
    }

    /// Each action, lights with `fuel` burn a little: a warning when it's
    /// nearly gone, then they go out (or, `burns_away`, are gone).
    pub(crate) fn burn_fuel(&mut self) {
        let lights: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&i| self.world.def(i).fuel.is_some() && self.world.has(i, Trait::LightSource) && self.world.is_burning(i))
            .collect();
        for t in lights {
            let left = self.fuel_left(t).unwrap_or(0) - 1;
            self.world.set(t, "fuel", Value::Int(left.max(0)));
            let seen = self.world.is_within(t, PLAYER) || self.world.room_of(t) == Some(self.world.here());
            let ctx = Ctx::new().entity("target", t);
            if left > 0 {
                if left == LOW_FUEL && seen {
                    self.msg("fuel.low", &ctx);
                }
                continue;
            }
            if seen {
                match self.world.def(t).spent.clone() {
                    Some(s) => self.render_line(LineKind::Text, &s, &ctx.clone().this(t)),
                    None => self.msg(if self.world.def(t).burns_away { "fuel.gone" } else { "fuel.out" }, &ctx),
                }
            }
            self.set_lit(t, false);
            if self.world.def(t).burns_away {
                let _ = self.world.move_to(t, None);
            }
        }
    }

    // ------------------------------------------------------------- wear

    /// Something used as a tool wears a little; with no uses left, it breaks.
    pub(crate) fn wear_out(&mut self, t: EntityId) {
        if self.world.def(t).uses.is_none() {
            return;
        }
        let left = match self.world.get(t, "uses") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        } - 1;
        if left > 0 {
            self.world.set(t, "uses", Value::Int(left));
            return;
        }
        self.world.set(t, "uses", Value::Int(0));
        let ctx = Ctx::new().entity("target", t);
        match self.world.def(t).breaks.clone() {
            Some(s) => self.render_line(LineKind::Text, &s, &ctx.clone().this(t)),
            None => self.msg("wear.breaks", &ctx),
        }
        let _ = self.world.move_to(t, None);
    }

    // ----------------------------------------------------------- liquids

    pub(crate) fn liquid_in(&self, t: EntityId) -> Option<String> {
        match self.world.get(t, "liquid") {
            Some(Value::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    /// Whether a liquid puts fires out: water does, and so does any liquid
    /// whose source `douses`.
    fn douses(&self, liquid: &str) -> bool {
        liquid.contains("water")
            || self.world.story().entities.iter().any(|e| e.liquid_source.as_deref() == Some(liquid) && e.douses)
    }

    /// FILL THE BOTTLE (FROM THE STREAM).
    pub(crate) fn fill(&mut self, t: EntityId, from: Option<EntityId>, ctx: &Ctx) -> bool {
        if !self.world.def(t).holds_liquid {
            self.msg("fill.cant", ctx);
            return false;
        }
        if self.liquid_in(t).is_some() {
            self.msg("fill.already", ctx);
            return false;
        }
        let w = &self.world;
        let source = match from {
            Some(f) if w.def(f).liquid_source.is_some() || (w.def(f).holds_liquid && self.liquid_in(f).is_some()) => Some(f),
            Some(f) => {
                self.msg("fill.not_source", &ctx.clone().entity("tool", f));
                return false;
            }
            None => w.scope().into_iter().find(|&i| i != t && w.def(i).liquid_source.is_some()),
        };
        let Some(s) = source else {
            self.msg("fill.no_source", ctx);
            return false;
        };
        let liquid = match self.world.def(s).liquid_source.clone() {
            Some(l) => l,
            None => {
                // From another container: it's poured across.
                let l = self.liquid_in(s).unwrap_or_default();
                self.world.unset(s, "liquid");
                l
            }
        };
        self.world.set(t, "liquid", Value::Str(liquid.clone()));
        // Handled: lists say what's in it now.
        self.world.state.known.insert(t);
        self.msg("fill.done", &ctx.clone().entity("tool", s).text("liquid", liquid));
        true
    }

    /// POUR OUT THE WATER, POUR THE BOTTLE INTO THE BUCKET.
    pub(crate) fn pour(&mut self, t: EntityId, into: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(liquid) = self.liquid_in(t) else {
            self.msg(if self.world.def(t).holds_liquid { "pour.empty" } else { "pour.cant" }, ctx);
            return false;
        };
        let ctx = ctx.clone().text("liquid", liquid.clone());
        if let Some(i) = into {
            // Onto a fire: that's dousing it.
            if self.world.on_fire(i) || self.world.is_burning(i) {
                return self.douse_with_liquid(i, t, &ctx);
            }
            if !self.world.def(i).holds_liquid {
                self.msg("pour.not_into", &ctx);
                return false;
            }
            if self.liquid_in(i).is_some() {
                self.msg("pour.full", &ctx);
                return false;
            }
            self.world.unset(t, "liquid");
            self.world.set(i, "liquid", Value::Str(liquid));
            self.msg("pour.into", &ctx);
            return true;
        }
        self.world.unset(t, "liquid");
        self.msg("pour.out", &ctx);
        true
    }

    /// POUR THE WATER ON THE FIRE: a fire, or a burning light, is put out.
    pub(crate) fn douse_with_liquid(&mut self, fire: EntityId, with: EntityId, ctx: &Ctx) -> bool {
        let liquid = self.liquid_in(with).unwrap_or_default();
        let ctx = ctx.clone().entity("target", fire).entity("tool", with).text("liquid", liquid.clone());
        self.world.unset(with, "liquid");
        if !self.douses(&liquid) {
            self.msg("pour.no_douse", &ctx);
            return true;
        }
        if self.world.on_fire(fire) {
            self.world.set(fire, "burning", Value::Bool(false));
            self.world.unset(fire, "_fire");
        } else {
            self.set_lit(fire, false);
        }
        self.msg("pour.doused", &ctx);
        true
    }

    /// DRINK the water in a bottle.
    pub(crate) fn drink_from(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let Some(liquid) = self.liquid_in(t) else {
            self.msg("pour.empty", ctx);
            return false;
        };
        self.world.unset(t, "liquid");
        self.msg("drink.liquid", &ctx.clone().text("liquid", liquid));
        true
    }

    // ------------------------------------------------------------- codes

    /// TYPE 4512 (ON THE KEYPAD), OPEN THE SAFE WITH 4512.
    pub(crate) fn enter_code(&mut self, target: Option<EntityId>, code: &str, ctx: &Ctx) -> bool {
        let code: String = code.split_whitespace().collect::<Vec<_>>().join("");
        let w = &self.world;
        let locks: Vec<EntityId> = match target {
            Some(t) => vec![t],
            None => w.scope().into_iter().filter(|&i| w.def(i).code.is_some()).collect(),
        };
        let Some(&lock) = locks.first().filter(|&&l| self.world.def(l).code.is_some()) else {
            self.msg("code.nowhere", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("target", lock).text("code", code.clone());
        if self.world.def(lock).code.as_deref().map(|c| c.replace(' ', "")) != Some(code) {
            self.msg("code.wrong", &ctx);
            return true;
        }
        if !self.world.flag(lock, "locked") {
            self.msg("code.already", &ctx);
            return true;
        }
        self.world.set(lock, "locked", Value::Bool(false));
        self.msg("code.right", &ctx);
        true
    }

    // ----------------------------------------------------------- reading

    /// READ: what's written on it, or else a look at it.
    pub(crate) fn read(&mut self, t: EntityId, ctx: &Ctx) -> Option<bool> {
        let writing = self.world.def(t).writing.clone()?;
        if self.world.in_darkness() {
            self.msg("read.dark", ctx);
            return Some(false);
        }
        self.render_line(LineKind::Text, &writing, &ctx.clone().this(t));
        Some(true)
    }

    // ------------------------------------------------- sounds and smells

    /// LISTEN or SMELL with nothing named: the room, what's in it, and the
    /// rooms next door, by the way to them.
    pub(crate) fn sense_around(&mut self, smell: bool, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let pick = |w: &crate::world::World, id: EntityId| if smell { w.def(id).smell.clone() } else { w.def(id).sound.clone() };
        let before = self.out.lines.len();
        if let Some(t) = pick(&self.world, here) {
            self.render_line(LineKind::Text, &t, &Ctx::new().this(here));
        }
        let things: Vec<EntityId> = self
            .world
            .scope()
            .into_iter()
            .filter(|&i| i != here && i != PLAYER && self.world.def(i).kind != EntityKind::Room)
            .collect();
        for i in things {
            if let Some(t) = pick(&self.world, i) {
                self.render_line(LineKind::Text, &t, &Ctx::new().this(i));
            }
        }
        // Next door: by the ways out (not through closed doors, for smells).
        let exits = self.world.def(here).exits.clone();
        let mut done = Vec::new();
        for e in exits {
            let Some(to) = e.to else { continue };
            if done.contains(&to) || !self.world.exit_visible(&e) {
                continue;
            }
            if smell && e.through.is_some_and(|d| !self.world.is_open(d)) {
                continue;
            }
            done.push(to);
            if let Some(t) = pick(&self.world, to) {
                let what = self.render_str(&t, &Ctx::new().this(to));
                let way = self.grammar.direction_word(e.direction);
                self.msg(if smell { "smell.next_door" } else { "listen.next_door" }, &Ctx::new().text("direction", way).text("what", what));
            }
        }
        if self.out.lines.len() == before {
            self.msg(if smell { "smell.nothing" } else { "listen.nothing" }, ctx);
        }
        true
    }

    /// LISTEN TO X, SMELL X.
    pub(crate) fn sense(&mut self, t: EntityId, smell: bool, ctx: &Ctx) -> bool {
        let text = if smell { self.world.def(t).smell.clone() } else { self.world.def(t).sound.clone() };
        match text {
            Some(s) => self.render_line(LineKind::Text, &s, &ctx.clone().this(t)),
            None => self.msg(if smell { "smell.nothing" } else { "listen.nothing" }, ctx),
        }
        true
    }

    // ----------------------------------------------------------- weather

    pub(crate) fn weather_now(&self) -> Option<String> {
        self.world.story().weather.as_ref()?;
        self.world.state.weather.clone().or_else(|| self.world.story().weather.as_ref().map(|w| w.start.clone()))
    }

    /// Changes the weather; outdoors, the player is told.
    pub(crate) fn set_weather(&mut self, kind: &str) {
        let Some(w) = self.world.story().weather.clone() else { return };
        let Some(k) = w.kinds.iter().find(|k| k.key == kind) else { return };
        if self.weather_now().as_deref() == Some(kind) {
            return;
        }
        self.world.state.weather = Some(kind.to_string());
        if self.world.def(self.world.here()).outdoors
            && !self.world.in_darkness()
            && let Some(b) = &k.begins
        {
            self.render_line(LineKind::Text, b, &Ctx::new());
        }
    }

    /// Each action: the weather turns, when it's time.
    pub(crate) fn weather_turn(&mut self) {
        let Some(w) = self.world.story().weather.clone() else { return };
        let Some(every) = w.every else { return };
        let now = self.world.state.turn;
        let due = *self.world.state.weather_due.get_or_insert(u64::from(every));
        if now < due {
            return;
        }
        let current = self.weather_now();
        let others: Vec<&str> = w.kinds.iter().map(|k| k.key.as_str()).filter(|k| Some(*k) != current.as_deref()).collect();
        if !others.is_empty() {
            let i = self.world.state.rng.range(0, others.len() as i64 - 1) as usize;
            let next = others[i].to_string();
            self.set_weather(&next);
        }
        let spread = if w.spread == 0 { 0 } else { self.world.state.rng.range(0, i64::from(w.spread)) as u64 };
        self.world.state.weather_due = Some(now + u64::from(every) + spread);
    }

    /// A line about the weather, for an outdoor room's description.
    pub(crate) fn describe_weather(&mut self) {
        let here = self.world.here();
        if !self.world.def(here).outdoors {
            return;
        }
        let Some(now) = self.weather_now() else { return };
        let text = self.world.story().weather.as_ref().and_then(|w| w.kinds.iter().find(|k| k.key == now)).and_then(|k| k.describe.clone());
        if let Some(t) = text {
            let s = self.render_str(&t, &Ctx::new().this(here));
            self.push_sentence(s, true);
        }
    }
}
