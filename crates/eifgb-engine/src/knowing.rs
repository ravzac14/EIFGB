//! What the player learns and owes: facts they've been told or read (WHAT
//! DO I KNOW, and "As I told you…" when they ask again), places they've
//! heard of and the way there (ASK THE BAKER THE WAY TO THE MILL),
//! promises made and kept or broken, and copying (a forged pass, or an
//! inscription into the notes).

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use crate::state::Learned;
use eifgb_ir::{Direction, EntityId, EntityKind, Value, PLAYER};
use std::collections::{HashMap, VecDeque};

/// How many steps of directions anyone gives before "and ask again there".
const DIRECTIONS_SAID: usize = 5;

impl<H: ScriptHost> Game<H> {
    // ------------------------------------------------------------- facts

    /// Whether the player has learned `key`.
    pub(crate) fn knows(&self, key: &str) -> bool {
        self.world.state.learned.iter().any(|l| l.key == key)
    }

    /// The player learns a fact (from `from`, if someone told them). False
    /// if they knew it already, or there's no such fact.
    pub(crate) fn learn(&mut self, key: &str, from: Option<EntityId>) -> bool {
        let Some(fact) = self.world.story().facts.iter().find(|f| f.key == key).cloned() else { return false };
        if self.knows(key) {
            return false;
        }
        let first = self.world.state.learned.is_empty();
        self.world.state.learned.push(Learned { key: key.into(), from });
        if let Some(p) = fact.place {
            self.world.state.heard_of.insert(p);
        }
        self.msg_kind(LineKind::Notice, "fact.learned", &Ctx::new().value("first", Value::Bool(first)));
        true
    }

    /// Before a conversation says something again: "As I told you…"
    pub(crate) fn told_before(&mut self, teaches: &[String], who: Option<EntityId>) {
        if teaches.is_empty() || !teaches.iter().all(|k| self.knows(k)) {
            return;
        }
        let ctx = match who {
            Some(w) => Ctx::new().entity("npc", w),
            None => Ctx::new(),
        };
        self.msg("talk.told_you", &ctx);
    }

    pub(crate) fn learn_all(&mut self, teaches: &[String], who: Option<EntityId>) {
        for k in teaches {
            self.learn(k, who);
        }
    }

    /// READ or EXAMINE: what a thing teaches, read (or, with nothing written
    /// on it, looked at).
    pub(crate) fn taught_by(&mut self, t: EntityId, read: bool) {
        let def = self.world.def(t);
        if def.teaches.is_empty() || (!read && def.writing.is_some()) || self.world.in_darkness() {
            return;
        }
        let teaches = def.teaches.clone();
        self.learn_all(&teaches, None);
    }

    /// WHAT DO I KNOW (ABOUT THE MILL).
    pub(crate) fn facts_action(&mut self, about: Option<&str>) {
        let words: Vec<String> = about
            .unwrap_or_default()
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty() && !["the", "a", "an"].contains(w))
            .map(str::to_string)
            .collect();
        let learned = self.world.state.learned.clone();
        let mut lines = Vec::new();
        for l in &learned {
            let Some(f) = self.world.story().facts.iter().find(|f| f.key == l.key).cloned() else { continue };
            let text = self.render_str(&f.text, &Ctx::new());
            let lower = text.to_lowercase();
            let place = f.place.map(|p| self.world.def(p).name.to_lowercase()).unwrap_or_default();
            if !words.is_empty() && !words.iter().any(|w| lower.contains(w.as_str()) || place.contains(w.as_str())) {
                continue;
            }
            let ctx = match l.from {
                Some(w) => Ctx::new().text("text", text).entity("from", w),
                None => Ctx::new().text("text", text),
            };
            lines.push(self.msg_str("facts.fact", &ctx));
        }
        if lines.is_empty() {
            return self.msg(if words.is_empty() { "facts.none" } else { "facts.none_about" }, &Ctx::new());
        }
        self.msg_kind(LineKind::Notice, "facts.head", &Ctx::new());
        for l in lines {
            self.push(LineKind::Text, l);
        }
    }

    // ------------------------------------------------------------ places

    /// Rooms called this, anywhere in the game ("the old mill", "mill").
    fn rooms_called(&self, words: &[String]) -> Vec<EntityId> {
        let words: Vec<&String> = words.iter().filter(|w| !["the", "a", "an", "to"].contains(&w.as_str())).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.world
            .ids()
            .filter(|&r| self.world.def(r).kind == EntityKind::Room)
            .filter(|&r| {
                let name: Vec<String> = self.world.def(r).name.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '\'').map(str::to_string).collect();
                words.iter().all(|w| name.contains(w))
            })
            .collect()
    }

    /// The shortest way from `from` to `to` by any way out there is, whether
    /// the player's been or not.
    fn way_to(&self, from: EntityId, to: EntityId) -> Option<Vec<Direction>> {
        let w = &self.world;
        let mut came: HashMap<EntityId, (EntityId, Direction)> = HashMap::new();
        let mut queue = VecDeque::from([from]);
        while let Some(r) = queue.pop_front() {
            if r == to {
                let mut steps = Vec::new();
                let mut at = r;
                while at != from {
                    let (prev, d) = came[&at];
                    steps.push(d);
                    at = prev;
                }
                steps.reverse();
                return Some(steps);
            }
            for e in &w.def(r).exits {
                let Some(next) = e.to else { continue };
                if next == from || came.contains_key(&next) || !w.exit_visible(e) {
                    continue;
                }
                came.insert(next, (r, e.direction));
                queue.push_back(next);
            }
        }
        None
    }

    /// Who here would tell the player the way: awake, not hostile, and the
    /// sort to know (`knows_the_way`).
    fn guides_here(&self) -> Vec<EntityId> {
        let w = &self.world;
        w.ids()
            .filter(|&c| {
                c != PLAYER
                    && w.def(c).kind == EntityKind::Character
                    && w.together(c, PLAYER)
                    && w.is_active(c)
                    && !w.is_asleep(c)
                    && !w.is_hostile(c, PLAYER)
                    && w.def(c).npc.as_ref().is_some_and(|n| n.knows_the_way)
            })
            .collect()
    }

    /// ASK THE BAKER THE WAY TO THE MILL, WHICH WAY TO THE MILL.
    pub(crate) fn directions(&mut self, target: Option<EntityId>, text: Option<&str>, ctx: &Ctx) -> bool {
        let who = match target {
            Some(t) if self.world.def(t).kind != EntityKind::Character || t == PLAYER => {
                self.msg("talk.not_person", ctx);
                return false;
            }
            Some(t) if !self.guides_here().contains(&t) => {
                self.msg("directions.doesnt_know", &ctx.clone().entity("npc", t));
                return false;
            }
            Some(t) => t,
            None => match self.guides_here().first() {
                Some(&t) => t,
                None => {
                    self.no_turn = true;
                    self.msg("directions.nobody", ctx);
                    return false;
                }
            },
        };
        let ctx = ctx.clone().entity("npc", who);
        self.hear_voice(who);
        let words: Vec<String> = text.unwrap_or_default().to_lowercase().split_whitespace().map(str::to_string).collect();
        let here = self.world.here();
        let named = self.rooms_called(&words);
        if named.is_empty() {
            self.msg("directions.unknown", &ctx);
            return true;
        }
        if named.contains(&here) {
            self.msg("directions.here", &ctx);
            return true;
        }
        // The nearest of those called that.
        let Some((to, steps)) = named.iter().filter_map(|&r| self.way_to(here, r).map(|s| (r, s))).min_by_key(|(_, s)| s.len()) else {
            self.msg("directions.no_way", &ctx);
            return true;
        };
        let mut said: Vec<String> = steps.iter().take(DIRECTIONS_SAID).map(|&d| self.grammar.direction_word(d)).collect();
        let more = steps.len() > DIRECTIONS_SAID;
        // "north, north" is "north twice".
        let mut i = 1;
        while i < said.len() {
            if said[i] == said[i - 1] {
                let twice = self.msg_str("directions.twice", &Ctx::new().text("dir", said[i].clone()));
                said[i - 1] = twice;
                said.remove(i);
            } else {
                i += 1;
            }
        }
        let then = self.msg_str("directions.then", &Ctx::new());
        let route = said.join(&format!(", {then} "));
        let mut cs = route.chars();
        let route = cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default();
        let place = self.world.def(to).name.clone();
        self.msg("directions.give", &ctx.clone().text("route", route).value("more", Value::Bool(more)));
        self.world.state.heard_of.insert(to);
        self.world.state.way_known.insert(to);
        self.msg_kind(LineKind::Notice, "directions.noted", &Ctx::new().text("place", place.to_uppercase()));
        true
    }

    /// Rooms the player has heard of but never been to, in story order.
    pub(crate) fn heard_of_only(&self) -> Vec<EntityId> {
        self.world.heard_of_only()
    }

    // ----------------------------------------------------------- promises

    fn promise_state(&self, c: EntityId) -> Option<String> {
        match self.world.get(c, "_promise") {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// What the player promised someone: "find her cat".
    fn promise_what(&mut self, c: EntityId) -> String {
        let Some(ask) = self.world.def(c).npc.as_ref().and_then(|n| n.asks.clone()) else { return String::new() };
        match &ask.promise {
            Some(t) => self.render_str(t, &Ctx::new().this(c)),
            None => self.msg_str("promise.what", &Ctx::new().entity("target", ask.wants)),
        }
    }

    /// TALK TO someone with a favour to ask, and no conversation: they ask
    /// it (or, promised, ask how it's going). False if they've nothing to ask.
    pub(crate) fn ask_favour(&mut self, c: EntityId) -> bool {
        let Some(ask) = self.world.def(c).npc.as_ref().and_then(|n| n.asks.clone()) else { return false };
        let ctx = Ctx::new().this(c).entity("npc", c);
        match self.promise_state(c).as_deref() {
            None => match &ask.say {
                Some(s) => {
                    self.render_line(LineKind::Text, s, &ctx);
                    self.world.set(c, "_asked", Value::Bool(true));
                    true
                }
                None => false,
            },
            Some("made") => {
                self.msg("promise.waiting", &ctx);
                true
            }
            _ => false,
        }
    }

    /// PROMISE THE BAKER, I'LL DO IT: the favour they asked.
    pub(crate) fn promise_action(&mut self, target: Option<EntityId>, ctx: &Ctx) -> bool {
        let askers: Vec<EntityId> = self
            .world
            .ids()
            .filter(|&c| self.world.def(c).npc.as_ref().is_some_and(|n| n.asks.is_some()) && self.world.together(c, PLAYER) && self.world.is_active(c))
            .collect();
        let who = match target {
            Some(t) => t,
            // The one who just asked, else the only one here who has a favour to ask.
            None => match askers.iter().find(|&&c| self.world.flag(c, "_asked") && self.promise_state(c).is_none()).or(if askers.len() == 1 { askers.first() } else { None }) {
                Some(&c) => c,
                None => {
                    self.no_turn = true;
                    self.msg("promise.nobody", ctx);
                    return false;
                }
            },
        };
        if self.world.def(who).kind != EntityKind::Character || who == PLAYER {
            self.msg("talk.not_person", ctx);
            return false;
        }
        let ctx = ctx.clone().entity("npc", who);
        if self.world.def(who).npc.as_ref().and_then(|n| n.asks.as_ref()).is_none() {
            self.msg("promise.nothing", &ctx);
            return false;
        }
        match self.promise_state(who).as_deref() {
            Some("made") => {
                self.msg("promise.already", &ctx);
                return false;
            }
            Some("kept") => {
                self.msg("promise.kept_already", &ctx);
                return false;
            }
            Some(_) => {
                self.msg("promise.broken_already", &ctx);
                return false;
            }
            None => {}
        }
        self.make_promise(who);
        true
    }

    /// The player promises `who` their favour (by PROMISE, a conversation
    /// option with `promise: true`, or a script).
    pub(crate) fn make_promise(&mut self, who: EntityId) {
        let Some(ask) = self.world.def(who).npc.as_ref().and_then(|n| n.asks.clone()) else { return };
        if self.promise_state(who).is_some() {
            return;
        }
        let turn = self.world.state.turn as i64;
        self.world.set(who, "_promise", Value::Str("made".into()));
        if let Some(n) = ask.within {
            self.world.set(who, "_promise_due", Value::Int(turn + i64::from(n)));
        }
        let ctx = Ctx::new().entity("npc", who);
        // In a conversation, the conversation says the thanks.
        if self.world.state.choice.is_none() {
            self.msg("promise.made", &ctx);
        }
        self.world.shift_disposition(who, 1);
        let what = self.promise_what(who);
        let first = !self.world.ids().any(|c| c != who && self.promise_state(c).is_some());
        self.msg_kind(LineKind::Notice, "promise.noted", &ctx.text("what", what).value("first", Value::Bool(first)));
    }

    /// GIVE THE CAT TO THE BAKER, when the baker asked for it: thanks, and
    /// whatever they give for it. None if it isn't what they asked for.
    pub(crate) fn keep_favour(&mut self, item: EntityId, to: EntityId) -> Option<bool> {
        let ask = self.world.def(to).npc.as_ref().and_then(|n| n.asks.clone())?;
        if ask.wants != item || matches!(self.promise_state(to).as_deref(), Some("kept")) || !self.world.is_active(to) {
            return None;
        }
        if self.world.location(item) != Some(PLAYER) {
            self.msg("give.not_held", &Ctx::new().entity("target", item));
            return Some(false);
        }
        let promised = self.promise_state(to).as_deref() == Some("made");
        let ctx = Ctx::new().this(to).entity("npc", to).entity("target", item);
        self.world.unset(item, "worn");
        let _ = self.world.move_to(item, Some(to));
        match &ask.thanks {
            Some(t) => self.render_line(LineKind::Text, t, &ctx),
            None => self.msg("promise.thanks", &ctx),
        }
        let gives: Vec<EntityId> = ask.gives.iter().copied().filter(|&g| !self.world.is_within(g, PLAYER)).collect();
        if !gives.is_empty() {
            let list = self.list_a(&gives);
            self.msg("promise.reward", &ctx.clone().text("list", list));
            for g in gives {
                let _ = self.world.move_to(g, Some(PLAYER));
                self.world.state.known.insert(g);
            }
        }
        self.world.set(to, "_promise", Value::Str("kept".into()));
        self.world.unset(to, "_promise_due");
        self.world.remember(to, "gift");
        self.world.shift_disposition(to, if promised { 2 } else { 1 });
        if promised {
            self.msg_kind(LineKind::Notice, "promise.kept", &ctx);
        }
        Some(true)
    }

    /// After each turn: promises past their time are broken; whoever was
    /// let down says so when they next see the player.
    pub(crate) fn promises_turn(&mut self) {
        let turn = self.world.state.turn as i64;
        let askers: Vec<EntityId> = self.world.ids().filter(|&c| self.world.def(c).npc.as_ref().is_some_and(|n| n.asks.is_some())).collect();
        for c in askers {
            if self.promise_state(c).as_deref() == Some("made")
                && matches!(self.world.get(c, "_promise_due"), Some(Value::Int(d)) if turn > *d)
            {
                self.world.set(c, "_promise", Value::Str("broken".into()));
                self.world.unset(c, "_promise_due");
                self.world.set(c, "_let_down", Value::Bool(true));
                self.world.shift_disposition(c, -2);
                self.msg_kind(LineKind::Notice, "promise.broken", &Ctx::new().entity("npc", c));
            }
            if self.world.flag(c, "_let_down") && self.world.is_active(c) && !self.world.is_asleep(c) && self.world.together(c, PLAYER) && self.world.can_see(c, PLAYER) {
                self.world.unset(c, "_let_down");
                if let Some(t) = self.world.def(c).npc.as_ref().and_then(|n| n.asks.as_ref()).and_then(|a| a.broken.clone()) {
                    self.render_line(LineKind::Text, &t, &Ctx::new().this(c).entity("npc", c));
                }
            }
        }
    }

    /// JOURNAL: the promises made, and how they stand.
    pub(crate) fn journal_promises(&mut self) -> bool {
        let made: Vec<(EntityId, String)> = self.world.ids().filter_map(|c| self.promise_state(c).map(|s| (c, s))).collect();
        if made.is_empty() {
            return false;
        }
        self.msg("journal.promises", &Ctx::new());
        for (c, s) in made {
            let what = self.promise_what(c);
            self.msg("journal.promise", &Ctx::new().entity("npc", c).text("what", what).text("how", s));
        }
        true
    }

    // ------------------------------------------------------------ copying

    /// COPY THE PASS (ONTO THE PAPER): a forgery, with paper and a pen; or
    /// COPY THE INSCRIPTION: into the notes.
    pub(crate) fn copy_action(&mut self, t: EntityId, tool: Option<EntityId>, ctx: &Ctx) -> bool {
        if self.world.in_darkness() {
            self.msg("read.dark", ctx);
            return false;
        }
        let def = self.world.def(t).clone();
        let Some(c) = def.copy else {
            let Some(w) = def.writing else {
                self.msg("copy.cant", ctx);
                return false;
            };
            let text = self.render_str(&w, &ctx.clone().this(t));
            let name = self.world.display_name(t);
            let mut cs = name.chars();
            let name = cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default();
            self.msg("copy.noted", ctx);
            self.add_note(format!("{name}: {text}"));
            return true;
        };
        if self.world.location(c).is_some() {
            self.msg("copy.already", ctx);
            return false;
        }
        let held = self.world.inventory_all();
        let blank = tool.filter(|&x| self.world.def(x).blank && held.contains(&x)).or_else(|| held.iter().copied().find(|&x| self.world.def(x).blank));
        let Some(blank) = blank else {
            self.msg("copy.no_blank", ctx);
            return false;
        };
        if !held.iter().any(|&x| self.world.def(x).writes) {
            self.msg("copy.no_pen", ctx);
            return false;
        }
        // Said first, while the paper still exists to be named.
        self.msg("copy.done", &ctx.clone().entity("blank", blank).entity("copy", c));
        let _ = self.world.move_to(blank, None);
        let _ = self.world.move_to(c, Some(PLAYER));
        self.world.state.known.insert(c);
        true
    }
}

impl crate::World {
    /// Whether the player knows of a room: been there, heard of it, or it's
    /// `heard_of` from the start.
    pub fn heard_of(&self, room: EntityId) -> bool {
        self.state.visited.contains(&room) || self.state.heard_of.contains(&room) || self.def(room).heard_of
    }

    /// Rooms the player has heard of but never been to, in story order.
    pub fn heard_of_only(&self) -> Vec<EntityId> {
        self.ids().filter(|&r| self.def(r).kind == EntityKind::Room && self.heard_of(r) && !self.state.visited.contains(&r)).collect()
    }
}
