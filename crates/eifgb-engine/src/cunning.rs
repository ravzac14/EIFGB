//! More for a creative player: distracting a watcher, a mirror (round
//! corners, and light into the dark), reaching with a pole, taking things
//! apart, calling out in someone else's voice, word getting round,
//! characters remembering, planting evidence, moods, and being someone else.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use crate::state::Rumour;
use eifgb_ir::{Direction, EntityId, EntityKind, Exit, Text, Trait, Value, PLAYER};

/// How many turns a call draws someone, and word takes to get round.
const CALLED_FOR: i64 = 8;
const WORD_TRAVELS: u64 = 3;

impl<H: ScriptHost> Game<H> {
    fn chars_awake(&self) -> Vec<EntityId> {
        self.world.ids().filter(|&c| c != PLAYER && self.world.def(c).kind == EntityKind::Character && self.world.is_active(c) && !self.world.is_asleep(c)).collect()
    }

    // --------------------------------------------------------- distractions

    /// Something thrown off across the room or out of it: everyone here
    /// looks that way, for this turn and the next, and doesn't see the player
    /// take what they were watching.
    pub(crate) fn distract_onlookers(&mut self) {
        let turn = self.world.state.turn as i64;
        for c in self.chars_awake() {
            if !self.world.together(c, PLAYER) || self.world.is_hostile(c, PLAYER) || self.world.companion(c) {
                continue;
            }
            self.world.set(c, "_busy", Value::Int(turn + 1));
            self.msg("npc.distracted", &Ctx::new().entity("npc", c));
        }
    }

    // -------------------------------------------------------------- mirrors

    fn mirror_held(&self, tool: Option<EntityId>) -> Option<EntityId> {
        match tool {
            Some(t) => (self.world.def(t).reflects && self.world.is_within(t, PLAYER)).then_some(t),
            None => self.world.inventory_all().into_iter().find(|&t| self.world.def(t).reflects),
        }
    }

    /// The way out in `dir` that can be seen along: no closed door.
    fn open_way(&self, dir: Option<Direction>) -> Result<Exit, &'static str> {
        let here = self.world.here();
        let ways: Vec<Exit> = self.world.def(here).exits.iter().filter(|e| e.to.is_some() && self.world.exit_visible(e)).cloned().collect();
        let way = match dir {
            Some(d) => ways.into_iter().find(|e| e.direction == d).ok_or("look_around.no_way")?,
            None => {
                let open: Vec<Exit> = ways.into_iter().filter(|e| e.through.is_none_or(|d| self.world.is_open(d))).collect();
                match open.len() {
                    0 => return Err("look_around.no_way"),
                    1 => open.into_iter().next().ok_or("look_around.no_way")?,
                    _ => return Err("look_around.which"),
                }
            }
        };
        if way.through.is_some_and(|d| !self.world.is_open(d)) {
            return Err("look_around.closed");
        }
        Ok(way)
    }

    /// LOOK AROUND THE CORNER WITH THE MIRROR, LOOK NORTH WITH IT: the next
    /// room as if the player were in its doorway, unseen.
    pub(crate) fn look_around(&mut self, tool: Option<EntityId>, dir: Option<Direction>, ctx: &Ctx) -> bool {
        let Some(mirror) = self.mirror_held(tool) else {
            self.msg("look_around.no_mirror", ctx);
            return false;
        };
        let way = match self.open_way(dir) {
            Ok(w) => w,
            Err(k) => {
                self.no_turn = true;
                self.msg(k, ctx);
                return false;
            }
        };
        let room = way.to.unwrap_or(PLAYER);
        let ctx = ctx.clone().entity("tool", mirror).text("place", self.world.def(room).name.clone());
        if !self.world.is_lit(room) && !self.world.player_sees_in_dark() {
            self.msg("look_around.dark", &ctx);
            return true;
        }
        self.msg("look_around.see", &ctx);
        if let Some(d) = self.mood_text(room).or_else(|| self.world.def(room).description.clone()) {
            self.render_line(LineKind::Text, &d, &Ctx::new().this(room));
        }
        let seen: Vec<EntityId> = self
            .world
            .contents(room)
            .into_iter()
            .filter(|&c| c != PLAYER && !self.world.is_hidden(c) && !self.world.has(c, Trait::Scenery) && self.world.def(c).kind != EntityKind::Room)
            .collect();
        if !seen.is_empty() {
            for &c in &seen {
                self.world.learn(c);
            }
            let list = self.list_a(&seen);
            self.msg("look_around.things", &Ctx::new().text("list", list));
        }
        true
    }

    /// ANGLE THE MIRROR NORTH: from a lit room, light into the next one, for
    /// as long as the mirror stays here.
    pub(crate) fn reflect(&mut self, mirror: EntityId, dir: Option<Direction>, ctx: &Ctx) -> bool {
        if !self.world.def(mirror).reflects {
            self.msg("reflect.cant", ctx);
            return false;
        }
        let here = self.world.here();
        if !self.world.lit_plainly(here) {
            self.msg("reflect.no_light", ctx);
            return false;
        }
        let way = match self.open_way(dir) {
            Ok(w) => w,
            Err(k) => {
                self.no_turn = true;
                self.msg(if k == "look_around.which" { "reflect.which" } else { k }, ctx);
                return false;
            }
        };
        let room = way.to.unwrap_or(here);
        // Set down at the angle: it shines for as long as it's left there.
        if self.world.is_within(mirror, PLAYER) {
            self.world.unset(mirror, "worn");
            let _ = self.world.move_to(mirror, Some(here));
        }
        let (from, to) = (self.world.def(here).key.clone(), self.world.def(room).key.clone());
        self.world.set(mirror, "_lights", Value::Str(to));
        self.world.set(mirror, "_lights_from", Value::Str(from));
        let words = self.grammar.direction_word(way.direction);
        self.msg("reflect.done", &ctx.clone().text("direction", words).text("place", self.world.def(room).name.clone()));
        true
    }

    // ------------------------------------------------------------- reaching

    /// HOOK THE KEY WITH THE POLE.
    pub(crate) fn reach(&mut self, t: EntityId, tool: Option<EntityId>, ctx: &Ctx) -> bool {
        let pole = tool.filter(|&p| self.world.def(p).reaches);
        match pole {
            Some(p) if self.world.is_within(p, PLAYER) => {
                let ok = self.take(t, None, ctx);
                if ok {
                    self.out.lines.pop();
                    self.msg("reach.done", &ctx.clone().entity("target", t).entity("tool", p));
                }
                ok
            }
            Some(p) => {
                self.msg("put.not_held", &Ctx::new().entity("target", p));
                false
            }
            None => {
                self.msg("reach.wont", ctx);
                false
            }
        }
    }

    // ---------------------------------------------------------- taking apart

    /// TAKE THE LANTERN APART: its parts, where it was.
    pub(crate) fn take_apart(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let Some(split) = self.world.def(t).splits_into.clone() else {
            self.msg("take_apart.cant", ctx);
            return false;
        };
        let place = if self.world.is_within(t, PLAYER) { Some(PLAYER) } else { self.world.location(t) };
        let _ = self.world.move_to(t, None);
        for &p in &split.parts {
            let _ = self.world.move_to(p, place);
            self.world.learn(p);
        }
        let list = self.list_a(&split.parts);
        let ctx = ctx.clone().text("list", list);
        match &split.say {
            Some(s) => self.render_line(LineKind::Text, s, &ctx),
            None => self.msg("take_apart.done", &ctx),
        }
        true
    }

    // --------------------------------------------------------------- voices

    /// The player has heard `who` speak: talking with them, or overhearing.
    pub(crate) fn hear_voice(&mut self, who: EntityId) {
        self.world.set(who, "_voice_known", Value::Bool(true));
    }

    /// Whether `text` names someone whose voice the player has heard.
    pub(crate) fn names_known_voice(&self, text: Option<&str>) -> bool {
        let words: Vec<String> = text.unwrap_or("").split_whitespace().map(str::to_lowercase).collect();
        let heard: Vec<EntityId> = self.world.ids().filter(|&c| self.world.flag(c, "_voice_known")).collect();
        !heard.is_empty() && !self.grammar.name_matches_in(&self.world, &words, &heard).is_empty()
    }

    /// CALL OUT AS THE CAPTAIN: whoever answers to the captain, near enough
    /// to hear and unable to see who's really calling, comes running.
    pub(crate) fn imitate(&mut self, text: Option<&str>, ctx: &Ctx) -> bool {
        let words: Vec<String> = text.unwrap_or("").split_whitespace().map(str::to_lowercase).collect();
        let people: Vec<EntityId> = self.world.ids().filter(|&c| c != PLAYER && self.world.def(c).kind == EntityKind::Character).collect();
        let Some(who) = self.grammar.name_matches_in(&self.world, &words, &people).into_iter().next() else {
            self.no_turn = true;
            self.msg("imitate.who", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("target", who);
        if !self.world.flag(who, "_voice_known") {
            self.msg("imitate.unknown", &ctx);
            return false;
        }
        if self.world.together(who, PLAYER) && self.world.is_active(who) {
            self.msg("imitate.here", &ctx);
            return false;
        }
        self.msg("imitate.done", &ctx);
        let here = self.world.here();
        let key = Value::Str(self.world.def(here).key.clone());
        let turn = self.world.state.turn as i64;
        let hidden = self.world.in_darkness() || self.world.ceiling() != here;
        for c in self.chars_awake() {
            if !self.world.def(c).npc.as_ref().is_some_and(|n| n.answers_to.contains(&who)) {
                continue;
            }
            if self.world.together(c, PLAYER) && !hidden && self.world.can_see(c, PLAYER) {
                self.msg("imitate.seen", &Ctx::new().entity("npc", c));
                continue;
            }
            if self.world.distance(c, here).is_some_and(|d| d <= 2) {
                self.world.set(c, "_called_to", key.clone());
                self.world.set(c, "_called_until", Value::Int(turn + CALLED_FOR));
                self.world.set(c, "_was_called", Value::Bool(true));
                self.msg("imitate.answered", &Ctx::new().entity("npc", c).entity("target", who));
            }
        }
        true
    }

    /// Someone drawn by a false call who finds the player instead.
    fn found_out(&mut self) {
        for c in self.chars_awake() {
            if !self.world.flag(c, "_was_called") {
                continue;
            }
            if self.world.together(c, PLAYER) && self.world.can_see(c, PLAYER) && !self.world.in_darkness() {
                self.world.unset(c, "_was_called");
                self.world.unset(c, "_called_to");
                self.shift_feeling(c, -1);
                self.remember(c, "fooled");
                self.msg("npc.voice_found_out", &Ctx::new().entity("npc", c));
            } else if self.world.get(c, "_called_to").is_none() {
                self.world.unset(c, "_was_called");
            }
        }
    }

    // ------------------------------------------------------- word gets round

    /// A change in how `npc` feels about the player, which its friends and
    /// faction hear about in a few turns.
    pub(crate) fn shift_feeling(&mut self, npc: EntityId, by: i64) {
        self.world.shift_disposition(npc, by);
    }

    /// Who `npc` tells: its friends, and those it's a friend of. (A faction
    /// has word of its own: its attitude to the player.)
    fn told_by(&self, npc: EntityId) -> Vec<EntityId> {
        let w = &self.world;
        let friends = w.def(npc).npc.as_ref().map(|n| n.friends.clone()).unwrap_or_default();
        w.ids()
            .filter(|&c| c != npc && c != PLAYER && w.def(c).kind == EntityKind::Character && w.is_active(c))
            .filter(|&c| friends.contains(&c) || w.def(c).npc.as_ref().is_some_and(|n| n.friends.contains(&npc)))
            .collect()
    }

    fn spread_word(&mut self) {
        let turn = self.world.state.turn;
        let (due, later): (Vec<Rumour>, Vec<Rumour>) = std::mem::take(&mut self.world.state.rumours).into_iter().partition(|r| r.due <= turn);
        self.world.state.rumours = later;
        for r in due {
            for c in self.told_by(r.from) {
                if let Some((_, now)) = self.world.shift_disposition_quietly(c, r.by)
                    && self.world.together(c, PLAYER)
                {
                    self.msg("npc.heard_word", &Ctx::new().entity("npc", c).entity("from", r.from).text("feeling", now));
                }
            }
        }
    }

    // ----------------------------------------------------------- memories

    /// `npc` remembers what the player did (the latest thing): `gift`,
    /// `attacked`, `robbed`, `spared` or `fooled`. In its `remembers` state.
    pub(crate) fn remember(&mut self, npc: EntityId, deed: &str) {
        self.world.remember(npc, deed);
    }

    fn recall_deeds(&mut self) {
        for c in self.chars_awake() {
            let Some(Value::Str(deed)) = self.world.get(c, "_remembers").cloned() else { continue };
            if !self.world.together(c, PLAYER) {
                self.world.unset(c, "_recalled");
                continue;
            }
            if matches!(self.world.get(c, "_recalled"), Some(Value::Str(d)) if *d == deed) || !self.world.can_see(c, PLAYER) {
                continue;
            }
            self.world.set(c, "_recalled", Value::Str(deed.clone()));
            let text = self.world.def(c).npc.as_ref().and_then(|n| n.remembers.get(&deed).cloned());
            if let Some(t) = text {
                self.render_line(LineKind::Text, &t, &Ctx::new().this(c).entity("npc", c));
            }
        }
    }

    // ---------------------------------------------------------- planting

    /// PLANT THE RUBY ON THE COOK: in their pockets, unseen, for whoever's
    /// missing it to find.
    pub(crate) fn plant(&mut self, item: EntityId, on: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(on) = on.filter(|&c| self.world.def(c).kind == EntityKind::Character && c != PLAYER && self.world.is_active(c)) else {
            self.msg("plant.not_person", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("target", item).entity("npc", on);
        if !self.world.is_within(item, PLAYER) {
            self.msg("put.not_held", &Ctx::new().entity("target", item));
            return false;
        }
        if self.world.notices_theft(on) {
            self.shift_feeling(on, -1);
            self.msg("plant.caught", &ctx);
            return false;
        }
        self.world.unset(item, "worn");
        let _ = self.world.move_to(item, Some(on));
        self.world.set(item, "_planted", Value::Bool(true));
        self.msg("plant.done", &ctx);
        true
    }

    /// Who's missing `item`: those watching it, and whoever it was stolen from.
    fn owners_of(&self, item: EntityId) -> Vec<EntityId> {
        let w = &self.world;
        let robbed = match w.get(item, "_stolen_from") {
            Some(Value::Str(k)) => w.id(k),
            _ => None,
        };
        w.ids().filter(|&c| Some(c) == robbed || w.def(c).npc.as_ref().is_some_and(|n| n.watches.contains(&item))).collect()
    }

    /// A planted thing seen on someone by its owner: they take it back, and
    /// blame them, not the player.
    fn find_planted(&mut self) {
        let planted: Vec<EntityId> = self.world.ids().filter(|&i| self.world.flag(i, "_planted")).collect();
        for item in planted {
            let Some(holder) = self.world.location(item).filter(|&h| self.world.def(h).kind == EntityKind::Character && h != PLAYER) else {
                self.world.unset(item, "_planted");
                continue;
            };
            let owner = self.owners_of(item).into_iter().find(|&o| o != holder && self.world.is_active(o) && !self.world.is_asleep(o) && self.world.together(o, holder) && self.world.can_see(o, holder));
            let Some(owner) = owner else { continue };
            self.world.unset(item, "_planted");
            let _ = self.world.move_to(item, Some(owner));
            self.world.add_foe(owner, holder);
            self.world.forgive_player(owner);
            self.world.shift_disposition(holder, 0);
            if self.world.room_of(owner) == Some(self.world.here()) {
                self.msg("npc.blames", &Ctx::new().entity("npc", owner).entity("target", item).entity("holder", holder));
            }
        }
    }

    // --------------------------------------------------------------- moods

    /// The player's mood, by the first of `id`'s `moods` that's on.
    pub(crate) fn mood_text(&self, id: EntityId) -> Option<Text> {
        self.world.def(id).moods.iter().find(|(m, _)| self.world.flag(PLAYER, m)).map(|(_, t)| t.clone())
    }

    /// The game's line for LOOK in a mood, for a room without its own.
    pub(crate) fn mood_line(&self, room: EntityId) -> Option<Text> {
        if self.mood_text(room).is_some() {
            return None;
        }
        self.world.story().moods.iter().find(|(m, _)| self.world.flag(PLAYER, m)).map(|(_, t)| t.clone())
    }

    /// Eaten or drunk: the mood it brings on.
    pub(crate) fn take_mood(&mut self, t: EntityId) {
        let Some((mood, turns)) = self.world.def(t).mood.clone() else { return };
        self.world.set(PLAYER, &mood, Value::Bool(true));
        let key = format!("_mood_{mood}");
        if turns > 0 {
            self.world.set(PLAYER, &key, Value::Int(self.world.state.turn as i64 + i64::from(turns)));
        } else {
            self.world.unset(PLAYER, &key);
        }
    }

    fn moods_pass(&mut self) {
        let turn = self.world.state.turn as i64;
        let mut over = Vec::new();
        if let Some(state) = self.world.state.entities.get(PLAYER.index()) {
            for (k, v) in &state.state {
                if let (Some(m), Value::Int(until)) = (k.strip_prefix("_mood_"), v)
                    && *until <= turn
                {
                    over.push(m.to_string());
                }
            }
        }
        for m in over {
            self.world.unset(PLAYER, &format!("_mood_{m}"));
            self.world.unset(PLAYER, &m);
            self.msg_kind(LineKind::Notice, "mood.ends", &Ctx::new().text("mood", m));
        }
    }

    // ---------------------------------------------------- other characters

    /// The characters the player can be, and the one they are.
    pub(crate) fn playable(&self) -> Vec<EntityId> {
        let w = &self.world;
        w.ids().filter(|&c| w.def(c).npc.as_ref().is_some_and(|n| n.playable) || Some(c) == w.story().plays).collect()
    }

    /// SWITCH TO MARA: the player is her now, where she is, with what she
    /// carries; the one they were stays behind, as they were.
    pub(crate) fn switch_to(&mut self, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some(current) = self.world.state.playing_as else {
            self.msg("switch.none", ctx);
            return false;
        };
        let words: Vec<String> = text.unwrap_or("").split_whitespace().map(str::to_lowercase).collect();
        let options: Vec<EntityId> = self.playable().into_iter().filter(|&c| c == current || !self.world.is_dead(c)).collect();
        let Some(next) = self.grammar.name_matches_in(&self.world, &words, &options).into_iter().next() else {
            self.no_turn = true;
            let names: Vec<EntityId> = options.into_iter().filter(|&c| c != current).collect();
            let list = self.list_a(&names);
            self.msg("switch.who", &ctx.clone().text("list", list));
            return false;
        };
        if next == current {
            self.no_turn = true;
            self.msg("switch.already", &ctx.clone().entity("target", next));
            return false;
        }
        self.world.become_character(current, next);
        self.world.state.playing_as = Some(next);
        self.msg("switch.done", &ctx.clone().entity("target", next));
        self.look();
        true
    }

    /// After every turn.
    pub(crate) fn cunning_turn(&mut self) {
        self.found_out();
        self.find_planted();
        self.spread_word();
        self.recall_deeds();
        self.moods_pass();
    }
}

impl crate::World {
    /// `npc` remembers what the player did (the latest thing): `gift`,
    /// `attacked`, `robbed`, `spared` or `fooled`. Scripts read it as `remembers`.
    pub fn remember(&mut self, npc: EntityId, deed: &str) {
        if npc == PLAYER || self.def(npc).kind != EntityKind::Character {
            return;
        }
        self.set(npc, "_remembers", Value::Str(deed.into()));
        // Said the next time they meet, not now.
        self.set(npc, "_recalled", Value::Str(deed.into()));
    }

    /// Word of how `npc` now feels about the player, for its friends and
    /// faction to hear in a few turns.
    pub(crate) fn pass_word(&mut self, npc: EntityId, by: i64) {
        let due = self.state.turn + WORD_TRAVELS;
        self.state.rumours.push(Rumour { from: npc, by: by.signum(), due });
    }

    /// `npc` no longer holds anything against the player.
    pub fn forgive_player(&mut self, npc: EntityId) {
        if let Some(Value::List(l)) = self.get(npc, "foes").cloned() {
            let l: Vec<Value> = l.into_iter().filter(|v| !matches!(v, Value::Str(s) if s == "player")).collect();
            self.set(npc, "foes", Value::List(l));
        }
        if self.disposition(npc) < 0 {
            self.set(npc, "disposition", Value::Int(0));
        }
    }

    /// The player stops being `from` (left where they stood, with what they
    /// carried and how they were) and becomes `to` (wherever they are).
    pub fn become_character(&mut self, from: EntityId, to: EntityId) {
        let n = self.state.entities.len();
        let here = self.state.entities[PLAYER.index()].location;
        let mine = std::mem::take(&mut self.state.entities[PLAYER.index()].state);
        let theirs = std::mem::take(&mut self.state.entities[to.index()].state);
        let there = self.state.entities[to.index()].location;
        for i in 0..n {
            let loc = self.state.entities[i].location;
            if loc == Some(PLAYER) {
                self.state.entities[i].location = Some(from);
            } else if loc == Some(to) {
                self.state.entities[i].location = Some(PLAYER);
                self.state.known.insert(EntityId(i as u32));
            }
        }
        self.state.entities[from.index()].location = here;
        self.state.entities[from.index()].state = mine;
        self.state.entities[PLAYER.index()].state = theirs;
        self.state.entities[PLAYER.index()].location = there;
        self.state.entities[to.index()].location = None;
        if let Some(r) = self.room_of(PLAYER) {
            self.state.visited.insert(r);
        }
    }

    /// Lit without help from a mirror.
    pub fn lit_plainly(&self, room: EntityId) -> bool {
        !self.is_dark_room(room)
            || (0..self.story().entities.len() as u32)
                .map(EntityId)
                .any(|i| self.room_of(i) == Some(room) && self.is_burning(i) && self.in_plain_sight(i))
    }

    /// A mirror next door, still where it was angled from, in a lit room,
    /// with the way between still open.
    pub fn lit_by_mirror(&self, room: EntityId) -> bool {
        let key = &self.def(room).key;
        self.ids().any(|m| {
            let (Some(Value::Str(to)), Some(Value::Str(from))) = (self.get(m, "_lights"), self.get(m, "_lights_from")) else { return false };
            if to != key {
                return false;
            }
            let Some(src) = self.room_of(m).filter(|&r| self.def(r).key == *from) else { return false };
            self.lit_plainly(src)
                && self.def(src).exits.iter().any(|e| e.to == Some(room) && e.through.is_none_or(|d| self.is_open(d)))
        })
    }
}
