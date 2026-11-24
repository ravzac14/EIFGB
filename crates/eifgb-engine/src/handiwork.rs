//! Working with your hands: ropes (tied to something, or round someone),
//! knocking on doors and walls, mending what wears out, food going bad,
//! how much the player can hold at once, a doorkeeper's password, and
//! the cold.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{Direction, EntityId, EntityKind, Exit, Trait, Value, PLAYER};

/// How long the player lasts in the cold, unless the game says.
const WARMTH: i64 = 8;
/// Warmth this low, and the player is told they're shivering.
const SHIVERING: i64 = 2;

impl crate::World {
    /// What a rope is tied to: an anchor, or someone tied up with it.
    pub fn tied_to(&self, rope: EntityId) -> Option<EntityId> {
        match self.get(rope, "_tied_to") {
            Some(Value::Str(k)) => self.id(k),
            _ => None,
        }
    }

    /// Tied up: it can't do anything until someone unties it.
    pub fn is_bound(&self, id: EntityId) -> bool {
        self.def(id).kind == EntityKind::Character && self.get(id, "_bound").is_some()
    }

    /// A rope tied to an anchor in `room`.
    pub fn rope_tied_in(&self, room: EntityId) -> Option<EntityId> {
        self.ids().find(|&r| self.def(r).rope && self.tied_to(r).is_some_and(|a| !self.is_bound(a) && self.def(a).kind != EntityKind::Character && self.room_of(a) == Some(room)))
    }

    /// Whether a way needs a rope, and has none tied at either end.
    pub fn needs_rope(&self, e: &Exit, here: EntityId) -> bool {
        e.rope && self.rope_tied_in(here).is_none() && e.to.is_none_or(|to| self.rope_tied_in(to).is_none())
    }

    /// What `t` takes of the player's hands: both if it's `two_handed`, one
    /// if it's big (size 2 or more), a weapon, or a light that's burning;
    /// small things go in pockets.
    pub fn hands_for(&self, t: EntityId) -> u32 {
        let d = self.def(t);
        if d.two_handed {
            2
        } else if d.size >= 2 || d.damage.is_some() || (self.has(t, Trait::LightSource) && self.is_burning(t)) {
            1
        } else {
            0
        }
    }

    /// What's in the player's hands, in a game where hands count.
    pub fn in_hands(&self) -> Vec<EntityId> {
        if self.def(PLAYER).hands.is_none() {
            return Vec::new();
        }
        self.contents(PLAYER).into_iter().filter(|&t| !self.flag(t, "worn") && self.hands_for(t) > 0).collect()
    }

    /// Hands free, or None if hands don't count in this game.
    pub fn hands_free(&self) -> Option<u32> {
        let hands = self.def(PLAYER).hands?;
        let used: u32 = self.in_hands().into_iter().map(|t| self.hands_for(t)).sum();
        Some(hands.saturating_sub(used))
    }

    /// Whether the game has any cold in it, for the player to keep out.
    pub fn has_cold(&self) -> bool {
        self.ids().any(|r| self.def(r).cold) || self.story().weather.as_ref().is_some_and(|w| w.kinds.iter().any(|k| k.cold))
    }

    /// Bitterly cold: a cold room, or outdoors in cold weather.
    pub fn is_cold(&self, room: EntityId) -> bool {
        if self.def(room).cold {
            return true;
        }
        if !self.def(room).outdoors {
            return false;
        }
        let Some(w) = self.story().weather.as_ref() else { return false };
        let now = self.state.weather.clone().unwrap_or_else(|| w.start.clone());
        w.kinds.iter().any(|k| k.key == now && k.cold)
    }

    /// A fire to warm up by in `room`: something on fire, or a fixed flame.
    pub fn hearth_in(&self, room: EntityId) -> bool {
        self.ids().any(|t| {
            self.room_of(t) == Some(room) && (self.on_fire(t) || (self.is_burning(t) && !self.has(t, Trait::Portable) && self.def(t).kind == EntityKind::Object))
        })
    }

    /// How long the player lasts in the cold, at most.
    pub fn most_warmth(&self) -> i64 {
        self.def(PLAYER).warmth.map_or(WARMTH, i64::from)
    }

    /// How warm the player is now: turns left before the cold hurts.
    pub fn warmth(&self) -> i64 {
        match self.get(PLAYER, "warmth") {
            Some(Value::Int(n)) => *n,
            _ => self.most_warmth(),
        }
    }

    /// The doorkeeper here who won't let the player go `dir`: awake, free,
    /// and not yet given the password.
    pub fn barred_by(&self, here: EntityId, dir: Direction) -> Option<EntityId> {
        self.ids().find(|&c| {
            self.def(c).npc.as_ref().and_then(|n| n.password.as_ref()).is_some_and(|p| p.bars.contains(&dir))
                && self.room_of(c) == Some(here)
                && self.is_active(c)
                && !self.is_asleep(c)
                && !self.is_bound(c)
                && !self.flag(c, "_passed")
        })
    }
}

impl<H: ScriptHost> Game<H> {
    // ---------------------------------------------------------------- ropes

    /// TIE THE ROPE TO THE TREE, TIE UP THE GUARD (WITH THE ROPE).
    pub(crate) fn tie(&mut self, a: EntityId, b: Option<EntityId>, ctx: &Ctx) -> bool {
        let w = &self.world;
        let held_rope = || w.contents(PLAYER).into_iter().find(|&r| w.def(r).rope);
        let (rope, other) = if w.def(a).rope {
            (Some(a), b)
        } else if b.is_some_and(|r| w.def(r).rope) {
            (b, Some(a))
        } else if w.def(a).kind == EntityKind::Character || w.def(a).anchor {
            (held_rope(), Some(a))
        } else {
            (None, b)
        };
        let Some(rope) = rope else {
            self.msg(if w.def(a).kind == EntityKind::Character || w.def(a).anchor { "tie.no_rope" } else { "tie.cant" }, ctx);
            return false;
        };
        let ctx = ctx.clone().entity("rope", rope);
        if let Some(t) = w.tied_to(rope) {
            self.msg("tie.already", &ctx.entity("anchor", t));
            return false;
        }
        if !w.is_within(rope, PLAYER) {
            self.msg("tie.hold", &ctx);
            return false;
        }
        let Some(other) = other else {
            self.no_turn = true;
            self.msg("tie.to_what", &ctx);
            return false;
        };
        let ctx = ctx.entity("anchor", other);
        if w.def(other).kind == EntityKind::Character && other != PLAYER {
            return self.tie_up(other, rope, &ctx);
        }
        if !w.def(other).anchor {
            self.msg("tie.no_anchor", &ctx);
            return false;
        }
        let Some(room) = w.room_of(other) else { return false };
        let _ = self.world.move_to(rope, Some(room));
        self.world.set(rope, "_tied_to", Value::Str(self.world.def(other).key.clone()));
        self.world.learn(rope);
        let drop = self.world.def(room).exits.iter().find(|e| e.rope).map(|e| e.direction);
        match drop {
            Some(d) => {
                let way = self.grammar.direction_word(d);
                self.msg("tie.drop", &ctx.text("direction", way));
            }
            None => self.msg("tie.done", &ctx),
        }
        true
    }

    /// Ties someone up: only someone who can't stop the player.
    fn tie_up(&mut self, who: EntityId, rope: EntityId, ctx: &Ctx) -> bool {
        let w = &self.world;
        let ctx = ctx.clone().entity("npc", who);
        if w.is_dead(who) {
            self.msg("tie.dead", &ctx);
            return false;
        }
        if w.is_bound(who) {
            self.msg("tie.already_bound", &ctx);
            return false;
        }
        let helpless = w.is_asleep(who) || w.held_by(who).is_some() || w.is_frightened(who) || w.health(who).is_some_and(|h| h <= 1);
        if !helpless {
            self.msg("tie.wont", &ctx);
            return false;
        }
        let _ = self.world.move_to(rope, Some(who));
        self.world.set(rope, "_tied_to", Value::Str(self.world.def(who).key.clone()));
        self.world.set(who, "_bound", Value::Str(self.world.def(rope).key.clone()));
        self.world.learn(who);
        self.msg(if self.world.is_asleep(who) { "tie.up_asleep" } else { "tie.up" }, &ctx);
        true
    }

    /// UNTIE THE ROPE, UNTIE THE GUARD, UNTIE THE ROPE FROM THE TREE.
    pub(crate) fn untie(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let w = &self.world;
        let rope = if w.def(t).rope {
            Some(t)
        } else {
            w.ids().find(|&r| w.def(r).rope && w.tied_to(r) == Some(t))
        };
        let Some(rope) = rope.filter(|&r| w.tied_to(r).is_some()) else {
            self.msg("untie.not_tied", ctx);
            return false;
        };
        let Some(to) = w.tied_to(rope) else { return false };
        let ctx = ctx.clone().entity("rope", rope).entity("anchor", to);
        if self.world.is_bound(to) {
            self.world.unset(to, "_bound");
            self.world.unset(rope, "_tied_to");
            let _ = self.world.move_to(rope, Some(PLAYER));
            self.msg("untie.person", &ctx.entity("npc", to));
            return true;
        }
        if self.world.room_of(rope) != Some(self.world.here()) {
            self.no_turn = true;
            self.msg("untie.far", &ctx);
            return false;
        }
        self.world.unset(rope, "_tied_to");
        let _ = self.world.move_to(rope, Some(PLAYER));
        self.msg("untie.done", &ctx);
        true
    }

    /// CLIMB THE ROPE: down (or up) the way it hangs.
    pub(crate) fn climb_rope(&mut self, rope: EntityId) -> Option<bool> {
        let here = self.world.here();
        if !self.world.def(rope).rope || self.world.tied_to(rope).is_none_or(|a| self.world.def(a).kind == EntityKind::Character) {
            return None;
        }
        let e = self.world.def(here).exits.iter().find(|e| e.rope && e.to.is_some()).cloned()?;
        Some(self.travel(here, &e))
    }

    // ------------------------------------------------------------- knocking

    /// KNOCK ON THE DOOR, KNOCK ON THE PANEL, KNOCK ON THE WALLS.
    pub(crate) fn knock(&mut self, t: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(t) = t else { return self.knock_around(ctx) };
        let w = &self.world;
        if w.has(t, Trait::Door) && !w.is_open(t) {
            return self.knock_door(t, ctx);
        }
        // Something hollow, hidden in or on it (or, for scenery like a
        // wall, in the room itself).
        let here = w.here();
        let inside: Vec<EntityId> = w
            .ids()
            .filter(|&h| {
                w.def(h).hollow && w.is_hidden(h) && (w.location(h) == Some(t) || (w.has(t, Trait::Scenery) && w.location(h) == Some(here)))
            })
            .collect();
        if !inside.is_empty() {
            for &h in &inside {
                self.world.unset(h, "hidden");
            }
            let list = self.list_a(&inside);
            self.msg("knock.hollow_found", &ctx.clone().text("list", list));
            return true;
        }
        if w.def(t).hollow {
            self.msg("knock.hollow", ctx);
            return true;
        }
        if w.def(t).kind == EntityKind::Character {
            self.msg("knock.person", ctx);
            return false;
        }
        self.msg("knock.solid", ctx);
        true
    }

    /// Knocking along the walls of the room for a hollow spot.
    fn knock_around(&mut self, ctx: &Ctx) -> bool {
        let w = &self.world;
        let here = w.here();
        let found: Vec<EntityId> = w
            .ids()
            .filter(|&h| w.def(h).hollow && w.is_hidden(h) && (w.location(h) == Some(here) || w.location(h).is_some_and(|l| w.location(l) == Some(here) && w.has(l, Trait::Scenery))))
            .collect();
        if found.is_empty() {
            self.msg("knock.walls", ctx);
            return true;
        }
        for &h in &found {
            self.world.unset(h, "hidden");
        }
        let list = self.list_a(&found);
        self.msg("knock.walls_found", &ctx.clone().text("list", list));
        true
    }

    /// Someone on the other side may open up: if they don't mind the
    /// player, and can (it isn't locked, or they have the key).
    fn knock_door(&mut self, door: EntityId, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let beyond = self.world.def(here).exits.iter().find(|e| e.through == Some(door)).and_then(|e| e.to);
        let w = &self.world;
        let who = beyond.and_then(|room| {
            w.ids().find(|&c| c != PLAYER && w.def(c).npc.is_some() && w.room_of(c) == Some(room) && w.is_active(c) && !w.is_asleep(c) && !w.is_bound(c))
        });
        self.world.make_noise(PLAYER, 1);
        let Some(who) = who else {
            self.msg("knock.no_answer", ctx);
            return true;
        };
        let ctx = ctx.clone().entity("npc", who);
        let w = &self.world;
        if w.is_hostile(who, PLAYER) {
            self.msg("knock.hostile", &ctx);
            return true;
        }
        if w.disposition(who) < -2 {
            self.msg("knock.go_away", &ctx);
            return true;
        }
        let has_key = w.def(door).unlocks_with.is_some_and(|k| w.is_within(k, who));
        if w.flag(door, "locked") && !has_key {
            self.msg("knock.no_key", &ctx);
            return true;
        }
        self.world.set(door, "locked", Value::Bool(false));
        self.world.set(door, "open", Value::Bool(true));
        self.world.learn(door);
        self.msg("knock.opens", &ctx);
        true
    }

    // ------------------------------------------------------------ passwords

    /// SAY SWORDFISH to a doorkeeper. True if someone here was listening for one.
    pub(crate) fn say_password(&mut self, words: Option<&str>) -> bool {
        let here = self.world.here();
        let w = &self.world;
        let keepers: Vec<EntityId> = w
            .ids()
            .filter(|&c| w.def(c).npc.as_ref().is_some_and(|n| n.password.is_some()) && w.room_of(c) == Some(here) && w.is_active(c) && !w.is_asleep(c) && !w.is_bound(c) && !w.flag(c, "_passed"))
            .collect();
        let Some(&first) = keepers.first() else { return false };
        let said: String = words.unwrap_or_default().to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' }).collect();
        let said = said.split_whitespace().collect::<Vec<_>>().join(" ");
        for c in keepers {
            let Some(p) = self.world.def(c).npc.as_ref().and_then(|n| n.password.clone()) else { continue };
            if said.contains(&p.word) {
                self.world.set(c, "_passed", Value::Bool(true));
                let ctx = Ctx::new().entity("npc", c);
                match &p.right {
                    Some(t) => self.render_line(LineKind::Text, t, &ctx.this(c)),
                    None => self.msg("password.right", &ctx),
                }
                return true;
            }
        }
        let ctx = Ctx::new().entity("npc", first);
        match self.world.def(first).npc.as_ref().and_then(|n| n.password.as_ref()).and_then(|p| p.wrong.clone()) {
            Some(t) => self.render_line(LineKind::Text, &t, &ctx.this(first)),
            None => self.msg("password.wrong", &ctx),
        }
        true
    }

    /// Stopped by a doorkeeper, going `dir`: says so, and true.
    pub(crate) fn stopped_by_keeper(&mut self, here: EntityId, dir: Direction) -> bool {
        let Some(c) = self.world.barred_by(here, dir) else { return false };
        let ctx = Ctx::new().entity("npc", c);
        match self.world.def(c).npc.as_ref().and_then(|n| n.password.as_ref()).and_then(|p| p.ask.clone()) {
            Some(t) => self.render_line(LineKind::Text, &t, &ctx.this(c)),
            None => self.msg("password.ask", &ctx),
        }
        true
    }

    // -------------------------------------------------------------- mending

    /// MEND THE NET (WITH THE THREAD): worn things as good as new, and
    /// broken ones made whole.
    pub(crate) fn mend(&mut self, t: EntityId, with: Option<EntityId>, ctx: &Ctx) -> bool {
        let w = &self.world;
        let tool = with.or_else(|| w.inventory_all().into_iter().find(|&m| m != t && w.def(m).mends));
        let Some(tool) = tool else {
            self.msg("mend.nothing", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("tool", tool);
        if !w.def(tool).mends {
            self.msg("mend.cant_with", &ctx);
            return false;
        }
        // What it broke from: made whole again, where the pieces were.
        let whole = w.ids().find(|&o| w.def(o).breaks_into == Some(t) && w.location(o).is_none());
        if let Some(o) = whole {
            let place = self.world.location(t);
            let _ = self.world.move_to(t, None);
            let _ = self.world.move_to(o, place);
            if let Some(u) = self.world.def(o).uses {
                self.world.set(o, "uses", Value::Int(i64::from(u)));
            }
            self.msg("mend.whole", &ctx.entity("made", o));
            self.wear_out(tool);
            return true;
        }
        let Some(full) = w.def(t).uses else {
            self.msg("mend.cant", &ctx);
            return false;
        };
        if matches!(w.get(t, "uses"), Some(Value::Int(n)) if *n >= i64::from(full)) {
            self.msg("mend.not_worn", &ctx);
            return false;
        }
        self.world.set(t, "uses", Value::Int(i64::from(full)));
        self.msg("mend.done", &ctx);
        self.wear_out(tool);
        true
    }

    /// Food and drink going bad, a turn at a time while it's in the world.
    pub(crate) fn spoil_turn(&mut self) {
        let food: Vec<EntityId> = self.world.ids().filter(|&t| self.world.def(t).spoils.is_some() && !self.world.flag(t, "spoiled")).collect();
        let here = self.world.here();
        for t in food {
            if self.world.location(t).is_none() {
                continue;
            }
            let age = match self.world.get(t, "_age") {
                Some(Value::Int(n)) => *n,
                _ => 0,
            } + 1;
            self.world.set(t, "_age", Value::Int(age));
            if age < i64::from(self.world.def(t).spoils.unwrap_or(0)) {
                continue;
            }
            self.world.set(t, "spoiled", Value::Bool(true));
            if (self.world.is_within(t, PLAYER) || self.world.room_of(t) == Some(here)) && !self.world.in_darkness() {
                self.msg_kind(LineKind::Notice, "spoil.now", &Ctx::new().entity("target", t));
            }
        }
    }

    // ---------------------------------------------------------------- hands

    /// "the torch and the shield": what fills the player's hands.
    fn hands_list(&self) -> String {
        let held: Vec<String> = self.world.in_hands().into_iter().map(|t| self.the(t)).collect();
        (self.grammar.lang().list_and)(&held)
    }

    /// Whether the player can take `t`, with their hands as full as they
    /// are. Says why not.
    pub(crate) fn hands_allow(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let Some(free) = self.world.hands_free() else { return true };
        let need = self.world.hands_for(t);
        if need <= free {
            return true;
        }
        let key = if self.world.in_hands().is_empty() { "hands.too_big" } else { "hands.full" };
        let list = self.hands_list();
        self.msg(key, &ctx.clone().text("list", list));
        false
    }

    /// Climbing takes a free hand. Says so if there isn't one.
    pub(crate) fn hand_to_climb(&mut self) -> bool {
        if self.world.hands_free().is_none_or(|f| f > 0) {
            return true;
        }
        let list = self.hands_list();
        self.msg("hands.climb", &Ctx::new().text("list", list));
        false
    }

    /// INVENTORY's last line, where hands count: what's in them.
    pub(crate) fn report_hands(&mut self) {
        if self.world.def(PLAYER).hands.is_none() || self.world.in_hands().is_empty() {
            return;
        }
        let list = self.hands_list();
        self.msg("hands.holding", &Ctx::new().text("list", list));
    }

    // ----------------------------------------------------------------- cold

    /// Each turn: the cold creeps in, or the player warms up again.
    pub(crate) fn cold_turn(&mut self) {
        if !self.world.has_cold() || self.end_requested.is_some() {
            return;
        }
        let here = self.world.here();
        let turn = self.world.state.turn as i64;
        // Soaked through, the cold bites twice as hard.
        if self.water(here) >= 2 {
            self.world.set(PLAYER, "_wet", Value::Int(turn + 10));
        }
        let wet = matches!(self.world.get(PLAYER, "_wet"), Some(Value::Int(t)) if *t > turn);
        let most = self.world.most_warmth();
        let now = self.world.warmth();
        let coat = self.world.inventory_all().into_iter().any(|t| self.world.def(t).warm && self.world.flag(t, "worn"));
        let fire = self.world.hearth_in(here);
        if self.world.is_cold(here) && !coat && !fire {
            let next = (now - if wet { 2 } else { 1 }).max(0);
            self.world.set(PLAYER, "warmth", Value::Int(next));
            if !self.world.flag(PLAYER, "_chilled") {
                self.world.set(PLAYER, "_chilled", Value::Bool(true));
                self.msg_kind(LineKind::Notice, "cold.start", &Ctx::new());
            } else if next <= SHIVERING && now > SHIVERING && next > 0 {
                self.msg_kind(LineKind::Notice, "cold.shivering", &Ctx::new());
            }
            if next > 0 {
                return;
            }
            match self.world.health(PLAYER) {
                Some(h) => {
                    self.msg("cold.hurts", &Ctx::new());
                    self.world.set(PLAYER, "health", Value::Int((h - 1).max(0)));
                    self.check_defeats();
                }
                None => {
                    self.msg("cold.froze", &Ctx::new());
                    self.end_requested = Some("froze".into());
                }
            }
            return;
        }
        if now >= most {
            self.world.unset(PLAYER, "_chilled");
            return;
        }
        let next = (now + if fire { 3 } else { 1 }).min(most);
        self.world.set(PLAYER, "warmth", Value::Int(next));
        if next == most {
            self.world.unset(PLAYER, "_chilled");
        }
        if fire && now <= SHIVERING {
            self.msg_kind(LineKind::Notice, "cold.by_fire", &Ctx::new());
        } else if next == most && now <= SHIVERING {
            self.msg_kind(LineKind::Notice, "cold.warm_again", &Ctx::new());
        }
    }

    /// Something warm eaten or drunk: warm right through.
    pub(crate) fn warm_up(&mut self, t: EntityId) {
        if !self.world.def(t).warm {
            return;
        }
        let most = self.world.most_warmth();
        if self.world.has_cold() && self.world.warmth() < most {
            self.world.set(PLAYER, "warmth", Value::Int(most));
            self.msg_kind(LineKind::Notice, "cold.warmed", &Ctx::new());
        }
    }
}
