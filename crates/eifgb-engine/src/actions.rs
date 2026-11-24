//! Built-in behaviour for the standard library's verbs. Every reply
//! comes from the message catalog, so authors can override any of it
//! and other languages can translate it.
//!
//! Each action returns whether it succeeded. `after_` handlers run only
//! on success.

use crate::game::{Game, LineKind, Pending};
use crate::parser::Command;
use crate::script::{Ctx, ScriptHost};
use crate::world::{Blocked, Fit};
use eifgb_ir::{BuiltinAction, Direction, EntityId, EntityKind, Exit, PatternToken, Segment, Text, Trait, Value, VerbAction, PLAYER};

/// State fields with built-in meaning. `open`, `locked` and `worn` are
/// described by [`StateNote::Builtin`]; `hidden` is never described.
/// `blocked` is built in on doors only (see [`World::door_blocked`]).
///
/// [`World::door_blocked`]: crate::World::door_blocked
const BUILTIN_STATE: &[&str] = &[
    "open", "locked", "worn", "hidden", "health", "money", "dead", "patience", "foes", "killed_by", "killed_with", "asleep",
    "sees_in_dark", "fuel", "uses", "liquid", "dark", "disposition",
];

/// One thing the automatic state descriptions can say.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StateNote {
    /// "open", "closed", "locked", "worn", "blocked" (both sides) or
    /// "blocked_here" (from the player's side).
    Builtin(&'static str),
    /// An author's boolean field, e.g. `fueled`. `words` is the display form.
    Flag { field: String, words: String, on: bool },
    /// An author's number or text field, e.g. `channel: 16`.
    Value { field: String, words: String, value: String },
}

/// Calls `f` with the source of every Lua snippet in a template.
fn visit_text_code(text: &Text, f: &mut dyn FnMut(&str)) {
    for seg in &text.0 {
        match seg {
            Segment::Lit(_) => {}
            Segment::Expr(c) => f(&c.src),
            Segment::If { cond, then, otherwise } => {
                f(&cond.src);
                visit_text_code(then, f);
                visit_text_code(otherwise, f);
            }
            Segment::Vary { options, .. } => options.iter().for_each(|o| visit_text_code(o, f)),
        }
    }
}

impl<H: ScriptHost> Game<H> {
    pub(crate) fn builtin(&mut self, a: BuiltinAction, target: Option<EntityId>, cmd: &Command, ctx: &Ctx) -> bool {
        use BuiltinAction as A;
        // Looking at or handling something tells the player its state, so
        // from now on lists tag it: "a small box (empty)".
        if matches!(
            a,
            A::Examine
                | A::Search
                | A::Open
                | A::Close
                | A::Lock
                | A::Unlock
                | A::Wear
                | A::TakeOff
                | A::PutIn
                | A::PutOn
                | A::Empty
                | A::Attack
        ) || (a == A::Take && cmd.tool.is_some())
        {
            for id in target.into_iter().chain(cmd.tool) {
                self.world.learn(id);
            }
        }
        match (a, target) {
            (A::Look, _) => {
                self.look();
                true
            }
            (A::Inventory, _) => self.inventory(),
            (A::Wait, _) => {
                if !self.quiet_wait {
                    self.msg("wait.done", ctx);
                }
                true
            }
            (A::Use, Some(t)) => self.use_action(t, cmd.tool, ctx),
            (A::Drink, Some(t)) if self.world.def(t).holds_liquid => self.drink_from(t, ctx),
            (A::Drink, Some(t)) if self.world.def(t).liquid_source.is_some() => {
                self.msg("drink.from", ctx);
                true
            }
            (A::Drink, Some(t)) => self.drink(t, ctx),
            (A::Read, Some(t)) => {
                let ok = match self.read(t, ctx) {
                    Some(ok) => ok,
                    None => self.examine(t),
                };
                self.taught_by(t, true);
                ok
            }
            (A::Listen, Some(t)) => self.sense(t, false, ctx),
            (A::Listen, None) => self.sense_around(false, ctx),
            (A::Smell, Some(t)) => self.sense(t, true, ctx),
            (A::Smell, None) => self.sense_around(true, ctx),
            (A::Fill, Some(t)) => self.fill(t, cmd.tool, ctx),
            (A::Pour, Some(t)) => self.pour(t, cmd.tool, ctx),
            (A::EnterCode, t) => {
                let text = cmd.text.clone().unwrap_or_default();
                self.enter_code(t, &text, ctx)
            }
            (A::Climb, Some(t)) => self.climb(t, ctx),
            (A::Wake, Some(t)) => self.wake_action(t, ctx),
            (A::FeelAround, _) => self.feel_around(ctx),
            (A::FeelFor, _) => self.feel_for(cmd, ctx),
            (A::Go, _) => self.go(cmd, ctx),
            (A::Enter, Some(t)) => self.enter(t, ctx),
            (A::GetOut, t) => self.get_out(t),
            (A::Empty, Some(t)) if self.world.def(t).holds_liquid => self.pour(t, cmd.tool, ctx),
            (A::Empty, Some(t)) => self.empty(t, cmd.tool, ctx),
            (A::Examine, Some(t)) => {
                let ok = self.examine(t);
                self.taught_by(t, false);
                ok
            }
            (A::Take, Some(t)) => self.take(t, cmd.tool, ctx),
            (A::Drop, Some(t)) => self.drop_thing(t, ctx),
            (A::PutIn, Some(t)) => self.put(t, cmd.tool, false, ctx),
            (A::PutOn, Some(t)) => self.put(t, cmd.tool, true, ctx),
            (A::Open, Some(t)) => self.open(t, ctx),
            (A::Close, Some(t)) => self.close(t, ctx),
            (A::Unlock, Some(t)) => self.lock_or_unlock(t, cmd.tool, false),
            (A::Lock, Some(t)) => self.lock_or_unlock(t, cmd.tool, true),
            (A::Search, Some(t)) => self.search(t, ctx),
            (A::Talk, Some(t)) => {
                self.topic = cmd.text.clone();
                self.talk(t, ctx)
            }
            // ASK ABOUT THE GATE: whoever here there is to talk to.
            (A::Talk, None) => match self.people_to_talk_to()[..] {
                [t] => {
                    self.topic = cmd.text.clone();
                    self.talk(t, &ctx.clone().entity("target", t))
                }
                [] => {
                    self.msg("talk.nobody", ctx);
                    false
                }
                _ => {
                    self.msg("talk.who", ctx);
                    false
                }
            },
            (A::Wear, Some(t)) => self.wear(t, ctx),
            (A::TakeOff, Some(t)) => self.take_off(t, ctx),
            (A::Eat, Some(t)) => self.eat(t, ctx),
            (A::Attack, Some(t)) => self.attack_action(t, cmd.tool, ctx),
            (A::Buy, Some(t)) => self.buy(t, cmd.tool, ctx),
            (A::Sell, Some(t)) => self.sell(t, cmd.tool, cmd.text.as_deref(), ctx),
            (A::Accept, _) => self.accept(ctx),
            (A::Give, Some(t)) => self.give_action(t, cmd.tool, ctx),
            (A::Bribe, Some(t)) => self.bribe_action(t, cmd.text.as_deref(), ctx),
            (A::Trade, Some(t)) => self.trade(t, cmd.tool, ctx),
            (A::Combine, Some(t)) => self.combine_action(t, cmd.tool, ctx),
            (A::Barricade, t) => self.barricade(t, cmd.tool, ctx),
            (A::Unbarricade, t) => self.unbarricade(t, cmd.tool, ctx),
            (A::Mark, _) => self.mark(cmd.text.as_deref(), ctx),
            (A::Unmark, _) => self.unmark(ctx),
            (A::Swap, Some(t)) => self.swap(t, cmd.tool, ctx),
            (A::PlayDead, _) => self.play_dead(ctx),
            (A::Reach, Some(t)) => self.reach(t, cmd.tool, ctx),
            (A::TakeApart, Some(t)) => self.take_apart(t, ctx),
            (A::LookAround, _) => self.look_around(cmd.tool.or(cmd.targets.first().copied()), cmd.direction, ctx),
            (A::Reflect, Some(t)) => self.reflect(t, cmd.direction, ctx),
            (A::Imitate, _) => self.imitate(cmd.text.as_deref(), ctx),
            (A::Plant, Some(t)) => self.plant(t, cmd.tool, ctx),
            (A::SwitchTo, _) => self.switch_to(cmd.text.as_deref(), ctx),
            (A::Directions, t) => self.directions(t, cmd.text.as_deref(), ctx),
            (A::Promise, t) => self.promise_action(t, ctx),
            (A::Copy, Some(t)) => self.copy_action(t, cmd.tool, ctx),
            (A::Swim, _) => self.swim(cmd, ctx),
            (A::Scent, Some(t)) => self.scent_action(t, cmd.tool, ctx),
            (A::PushTo, Some(t)) => self.push_to(t, cmd, ctx),
            (A::Throw, Some(t)) => self.throw_action(t, cmd.tool, cmd.direction, ctx),
            (A::Rename, Some(t)) => self.rename_action(t, cmd.text.as_deref(), ctx),
            (A::Tracks, _) => self.tracks_action(),
            (A::Bury, Some(t)) => self.bury_action(t, ctx),
            (A::Sneak, _) => self.sneak_action(cmd, ctx),
            (A::Peek, t) => self.peek_action(t, cmd.direction, ctx),
            // PICKPOCKET GOBLIN / ROB GOBLIN: the one named is the victim.
            (A::Steal, Some(t)) if cmd.tool.is_none() && self.world.def(t).kind == EntityKind::Character => {
                self.steal_action(None, Some(t), &ctx.clone().entity("tool", t))
            }
            (A::Steal, t) => self.steal_action(t, cmd.tool, ctx),
            (A::Eavesdrop, t) => self.eavesdrop_action(t, cmd.direction, ctx),
            (A::Order, Some(t)) => self.order_action(t, cmd.text.as_deref(), ctx),
            (A::Brandish, Some(t)) => self.brandish_action(t, cmd.tool, ctx),
            (A::Burn, Some(t)) => self.burn_action(t, cmd.tool, ctx),
            (A::Douse, Some(t)) if cmd.tool.is_some_and(|w| self.world.def(w).holds_liquid) => match cmd.tool {
                Some(w) if self.liquid_in(w).is_some() => self.douse_with_liquid(t, w, ctx),
                _ => {
                    self.msg("pour.empty", &ctx.clone().entity("target", cmd.tool.unwrap_or(t)));
                    false
                }
            },
            (A::Douse, Some(t)) => self.douse(t, cmd.tool, ctx),
            (A::Sound, Some(t)) => self.sound_action(t, cmd, ctx),
            (A::SetTrap, Some(t)) => self.set_trap(t, ctx),
            (A::DisarmTrap, Some(t)) => self.disarm_trap(t, ctx),
            // IMITATE THE CAPTAIN: a voice the player has heard, not a call.
            (A::Mimic, _) if self.names_known_voice(cmd.text.as_deref()) => self.imitate(cmd.text.as_deref(), ctx),
            (A::Mimic, _) => self.mimic_action(cmd, ctx),
            (A::Light, Some(t)) => self.light(t, true, ctx),
            (A::Extinguish, Some(t)) => self.light(t, false, ctx),
            (A::FollowTracks, _) => self.follow_tracks_action(cmd.text.as_deref(), ctx),
            (A::Wares, t) => self.wares(t, ctx),
            (A::Health, _) => self.health_action(),
            (A::Apologize, Some(t)) => self.apologize_action(t, ctx),
            (A::Offer, Some(t)) => self.offer(t, cmd.text.as_deref(), ctx),
            (A::Shout, _) => {
                if !self.echo(cmd.text.as_deref()) {
                    self.msg("shout.done", ctx);
                }
                let voice = self.world.story().meta.settings.voice;
                self.world.make_noise(PLAYER, voice);
                true
            }
            _ => {
                self.msg("action.nothing", ctx);
                false
            }
        }
    }

    // ------------------------------------------------------------ looking

    /// "Known exits: north and up."
    pub(crate) fn list_exits(&mut self, here: EntityId, exits: &[Exit]) {
        // The ways out the player can take right now: not through a secret
        // door they haven't found, and not one whose `when` is false.
        let mut dirs: Vec<Direction> = Vec::new();
        for x in exits {
            if x.to.is_some() && self.world.exit_visible(x) && !dirs.contains(&x.direction) && self.exit_allowed(x, here) {
                dirs.push(x.direction);
            }
        }
        let ways: Vec<String> = dirs.iter().map(|&d| self.grammar.direction_word(d)).collect();
        if ways.is_empty() {
            self.msg("look.no_exits", &Ctx::new());
        } else {
            let list = (self.grammar.lang().list_and)(&ways);
            self.msg("look.exits", &Ctx::new().text("list", list));
        }
    }

    /// EXITS: the ways out of here, even where the room description leaves them out.
    pub(crate) fn exits_action(&mut self) {
        if self.world.in_darkness() {
            return self.msg("look.dark", &Ctx::new());
        }
        let here = self.world.here();
        let exits = self.world.def(here).exits.clone();
        self.list_exits(here, &exits);
    }

    pub(crate) fn look(&mut self) {
        let here = self.world.here();
        self.seen_dark = Some(self.world.in_darkness());
        if self.world.in_darkness() {
            self.msg_kind(LineKind::Title, "look.dark_title", &Ctx::new());
            self.msg("look.dark", &Ctx::new());
            let known: Vec<EntityId> = self.world.felt_here().into_iter().filter(|t| self.world.state.located.contains_key(t)).collect();
            if !known.is_empty() {
                let list = self.list_a(&known);
                self.msg("look.dark_known", &Ctx::new().text("list", list));
            }
            self.light_next_door(here, true);
            return;
        }
        let (name, description, exits) = {
            let d = self.world.def(here);
            (d.name.clone(), d.description.clone(), d.exits.clone())
        };
        // "Hall (in the wardrobe)" when the player is in or on something.
        let title = match self.world.location(PLAYER) {
            Some(holder) if holder != here => {
                let key = if self.world.has(holder, Trait::Supporter) { "look.title_on" } else { "look.title_in" };
                self.msg_str(key, &Ctx::new().text("room", name.clone()).entity("target", holder))
            }
            _ => name,
        };
        self.push(LineKind::Title, title);
        if self.passing {
            return;
        }
        if !self.briefly {
            self.show_picture(here);
        }
        // Shut inside something: that's all there is to see.
        let ceiling = self.world.ceiling();
        if ceiling != here {
            self.msg("look.enclosed", &Ctx::new().entity("target", ceiling));
            let items: Vec<EntityId> = self.listable(ceiling);
            if !items.is_empty() {
                let list = self.list_a(&items);
                self.msg("examine.contents", &Ctx::new().entity("target", ceiling).text("list", list));
            }
            return;
        }
        let before = self.out.lines.len();
        // In a mood: the room as it seems, or the game's line for the mood.
        let mood_line = self.mood_line(here);
        let description = self.mood_text(here).or(description);
        if !self.briefly {
            if let Some(d) = &description {
                self.render_line(LineKind::Text, d, &Ctx::new().this(here));
            }
            if let Some(m) = &mood_line {
                self.render_line(LineKind::Text, m, &Ctx::new().this(here));
            }
            if let Some(w) = self.water_line(here) {
                self.push_sentence(w, true);
            }
            // Outdoors: the weather.
            self.describe_weather();
        }
        let described = self.out.lines.len() > before;
        // Things that won't be listed (scenery) still get their state
        // mentioned, unless the room description already covers it.
        let things = self.listable(here);
        for c in self.world.present(here) {
            if self.world.is_hidden(c) || c == PLAYER || things.contains(&c) {
                continue;
            }
            self.describe_state_here(c, description.as_ref(), described);
        }
        if !things.is_empty() {
            let list = self.list_a(&things);
            self.msg("look.you_see", &Ctx::new().text("list", list));
        }
        for c in self.world.present(here) {
            if !self.world.is_hidden(c) && c != PLAYER {
                self.describe_contents(c);
            }
        }
        if !self.world.player_sees_in_dark() {
            self.light_next_door(here, false);
        }
        // The player's own mark, left here.
        if let Some(mark) = self.mark_in(here) {
            self.msg("mark.here", &Ctx::new().text("mark", mark));
        }
        let list = self.world.def(here).list_exits.unwrap_or(self.world.story().meta.settings.list_exits);
        if !list {
            return;
        }
        self.list_exits(here, &exits);
    }

    /// Light through an open way (no door, or an open one) between rooms.
    /// From a dark room (`from_dark`): "Light spills in from the north."
    /// for each lit room next door. From a lit one: "It's dark to the
    /// north." for each dark room next door.
    fn light_next_door(&mut self, here: EntityId, from_dark: bool) {
        let mut dirs: Vec<Direction> = Vec::new();
        for e in &self.world.def(here).exits {
            let Some(to) = e.to else { continue };
            let open = e.through.is_none_or(|d| self.world.flag(d, "open"));
            if to == here || !open || !self.world.exit_visible(e) || dirs.contains(&e.direction) {
                continue;
            }
            if self.world.is_lit(to) == from_dark {
                dirs.push(e.direction);
            }
        }
        if dirs.is_empty() {
            return;
        }
        let base = if from_dark { "look.from" } else { "look.toward" };
        let phrases: Vec<String> = dirs
            .iter()
            .map(|&d| match d {
                Direction::Up | Direction::Down | Direction::In | Direction::Out => {
                    self.msg_str(&format!("{base}_{}", d.key()), &Ctx::new())
                }
                _ => {
                    let word = self.grammar.direction_word(d);
                    self.msg_str(base, &Ctx::new().text("direction", word))
                }
            })
            .collect();
        let list = (self.grammar.lang().list_and)(&phrases);
        let key = if from_dark { "look.light_from" } else { "look.dark_beyond" };
        self.msg(key, &Ctx::new().text("list", list));
    }

    /// Things worth naming in a room description or a list of contents.
    /// For a room, that includes doors from either side.
    fn listable(&self, holder: EntityId) -> Vec<EntityId> {
        let things = if self.world.def(holder).kind == EntityKind::Room {
            self.world.present(holder)
        } else {
            self.world.contents(holder)
        };
        things
            .into_iter()
            .filter(|&e| e != PLAYER && !self.world.is_hidden(e) && !self.world.has(e, Trait::Scenery))
            .collect()
    }

    /// "On the table you can see a box. In the box you can see a coin."
    /// Goes as deep as the player can see.
    fn describe_contents(&mut self, holder: EntityId) {
        self.describe_contents_depth(holder, 0);
    }

    fn describe_contents_depth(&mut self, holder: EntityId, depth: usize) {
        if depth > 8 || self.world.def(holder).kind != EntityKind::Object || !self.world.shows_contents(holder) {
            return;
        }
        let items = self.listable(holder);
        if items.is_empty() {
            return;
        }
        // In the dark, it's by feel, and the player knows where they are now.
        let dark = self.world.in_darkness();
        let key = match (self.world.has(holder, Trait::Supporter), dark) {
            (true, false) => "examine.on_top",
            (false, false) => "examine.contents",
            (true, true) => "feel.on_top",
            (false, true) => "feel.contents",
        };
        if dark {
            let here = self.world.here();
            for &t in &items {
                self.world.state.located.insert(t, here);
            }
        }
        let list = self.list_a(&items);
        self.msg(key, &Ctx::new().entity("target", holder).text("list", list));
        for item in items {
            self.describe_contents_depth(item, depth + 1);
        }
    }

    fn examine(&mut self, t: EntityId) -> bool {
        let def = self.world.def(t);
        if def.kind == EntityKind::Room {
            self.look();
            return true;
        }
        // Found by feel: there, but not to be seen.
        if self.world.in_darkness() && !self.world.is_within(t, PLAYER) && t != PLAYER {
            self.msg("examine.dark", &Ctx::new().entity("target", t));
            return true;
        }
        // Being someone else: their description is the player's.
        let as_who = (t == PLAYER).then_some(self.world.state.playing_as).flatten();
        let description = self.mood_text(t).or_else(|| as_who.and_then(|c| self.world.def(c).description.clone())).or_else(|| def.description.clone());
        let pictured = self.show_picture(t);
        let before = self.out.lines.len();
        match &description {
            Some(d) => self.render_line(LineKind::Text, d, &Ctx::new().this(t)),
            None if t == PLAYER => self.msg("examine.self", &Ctx::new()),
            None if pictured || !self.state_notes(t).is_empty() => {}
            None => self.msg("examine.default", &Ctx::new().entity("target", t)),
        }
        let described = self.out.lines.len() > before;
        self.describe_state(t, description.as_ref(), described);
        // Pushed against a door.
        if let Some(s) = self.against_note(t) {
            self.push_sentence(s, true);
        }
        // How someone seems to feel about the player, unless it's neither way.
        if self.world.def(t).kind == EntityKind::Character && self.world.is_active(t) {
            let feeling = eifgb_ir::feeling(self.world.disposition(t));
            if feeling != "neutral" {
                let s = self.msg_str(&format!("disposition.{feeling}"), &Ctx::new().entity("target", t));
                self.push_sentence(s, true);
            }
        }
        self.describe_contents(t);
        true
    }

    fn search(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let items = if self.world.shows_contents(t) { self.listable(t) } else { Vec::new() };
        if items.is_empty() {
            self.msg("search.nothing", ctx);
        } else {
            if self.world.in_darkness() {
                let here = self.world.here();
                for &i in &items {
                    self.world.state.located.insert(i, here);
                }
            }
            let list = self.list_a(&items);
            self.msg("search.contents", &ctx.clone().text("list", list));
        }
        true
    }

    fn inventory(&mut self) -> bool {
        let items = self.world.inventory();
        if items.is_empty() {
            self.msg("inventory.empty", &Ctx::new());
            self.report_money();
            return true;
        }
        let list = self.list_a(&items);
        self.msg("inventory.list", &Ctx::new().text("list", list));
        for item in items {
            self.inventory_contents(item, 0);
        }
        self.report_money();
        true
    }

    /// "In the backpack are a box of matches and a bag of marshmallows."
    /// for each carried thing the player can see into, then for what's in that.
    fn inventory_contents(&mut self, holder: EntityId, depth: usize) {
        if depth > 8 || !self.world.shows_contents(holder) {
            return;
        }
        let items = self.listable(holder);
        if items.is_empty() {
            return;
        }
        let key = if self.world.has(holder, Trait::Supporter) { "inventory.on" } else { "inventory.in" };
        let list = self.list_a(&items);
        let ctx = Ctx::new().entity("target", holder).text("list", list).value("count", Value::Int(items.len() as i64));
        self.msg(key, &ctx);
        for item in items {
            self.inventory_contents(item, depth + 1);
        }
    }

    // ----------------------------------------------------------- movement

    pub(crate) fn go(&mut self, cmd: &Command, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let Some(dir) = cmd.direction else {
            self.msg("go.no_exit", ctx);
            return false;
        };
        // In or on something: OUT means getting out of it; any other
        // direction gets out first, then goes.
        if self.world.location(PLAYER) != Some(here) {
            if dir == Direction::Out && self.world.exit(here, Direction::Out).is_none() {
                return self.get_out(None);
            }
            while let Some(holder) = self.world.location(PLAYER)
                && holder != here
            {
                if !self.leave_holder(holder, true) {
                    return false;
                }
            }
        }
        let group: Vec<Exit> = self
            .world
            .def(here)
            .exits
            .iter()
            .filter(|e| e.direction == dir && self.world.exit_visible(e))
            .cloned()
            .collect();
        if group.is_empty() {
            self.msg("go.no_exit", ctx);
            return false;
        }
        let mut open_routes = Vec::new();
        for e in group.iter().filter(|e| e.to.is_some()) {
            if self.exit_allowed(e, here) {
                open_routes.push(e.clone());
            }
        }
        match open_routes.len() {
            0 => {
                match group.iter().find_map(|e| e.blocked.clone()) {
                    Some(t) => self.render_line(LineKind::Text, &t, &Ctx::new().this(here)),
                    None if group.iter().any(|e| e.to.is_some()) => self.msg("go.blocked", ctx),
                    None => self.msg("go.no_exit", ctx),
                }
                false
            }
            1 => self.travel(here, &open_routes[0]),
            _ => {
                let mut doors: Vec<EntityId> = open_routes.iter().filter_map(|e| e.through).collect();
                doors.dedup();
                if doors.len() == open_routes.len() && self.ask_which_door(&doors) {
                    false
                } else {
                    self.travel(here, &open_routes[0])
                }
            }
        }
    }

    /// PUSH CRATE NORTH: something `pushable`, standing in the room, goes
    /// into the next room, and the player with it. Going works just as GO
    /// does (doors, blocked ways); if the player doesn't get there, neither
    /// does the thing.
    fn push_to(&mut self, t: EntityId, cmd: &Command, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let key = if !self.world.has(t, Trait::Pushable) {
            Some("push.cant")
        } else if self.world.is_within(t, PLAYER) {
            Some("push.carried")
        } else if self.world.is_within(PLAYER, t) {
            Some("push.on_it")
        } else if self.world.location(t) != Some(here) {
            Some("push.stuck")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        self.pushing = Some(t);
        let moved = self.go(cmd, ctx);
        self.pushing = None;
        moved
    }

    /// GO THROUGH, ENTER and the like: leave by the exit that uses this door.
    fn enter(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.has(t, Trait::Enterable) {
            return self.get_into(t, ctx);
        }
        let here = self.world.here();
        let routes: Vec<Exit> = self
            .world
            .def(here)
            .exits
            .iter()
            .filter(|e| (e.through == Some(t) || e.via.contains(&t)) && e.to.is_some())
            .cloned()
            .collect();
        if routes.is_empty() {
            self.msg("enter.cannot", ctx);
            return false;
        }
        for e in &routes {
            if self.exit_allowed(e, here) {
                return self.travel(here, e);
            }
        }
        match &routes[0].blocked {
            Some(b) => self.render_line(LineKind::Text, b, &Ctx::new().this(here)),
            None => self.msg("go.blocked", ctx),
        }
        false
    }

    fn exit_allowed(&mut self, e: &Exit, here: EntityId) -> bool {
        match &e.when {
            Some(w) => self.run(w, &Ctx::new().this(here)).truthy(),
            None => true,
        }
    }

    /// Asks "which do you mean?" between doors leading the same way. The
    /// reply runs as GO THROUGH <door>. Returns false if the story has no
    /// such verb.
    fn ask_which_door(&mut self, doors: &[EntityId]) -> bool {
        let story = self.world.story();
        let Some(lead) = story
            .verbs
            .iter()
            .find(|v| v.action == VerbAction::Builtin(BuiltinAction::Enter))
            .and_then(|v| v.patterns.first())
            .map(|p| {
                p.0.iter()
                    .map_while(|t| match t {
                        PatternToken::Word(w) => Some(w.clone()),
                        _ => None,
                    })
                    .collect::<Vec<String>>()
            })
        else {
            return false;
        };
        let names = self.which_names(doors);
        let list = (self.grammar.lang().list_or)(&names);
        self.msg("parser.ambiguous", &Ctx::new().text("options", list));
        self.pending = Some(Pending::Which { before: lead, after: Vec::new(), options: doors.to_vec() });
        self.no_turn = true;
        true
    }

    pub(crate) fn travel(&mut self, here: EntityId, exit: &Exit) -> bool {
        let Some(to) = exit.to else { return false };
        if let Some(trap) = self.world.held_by(PLAYER) {
            self.msg("trap.stuck", &Ctx::new().entity("trap", trap));
            return false;
        }
        if let Some(door) = exit.through
            && let Some(b) = self.world.door_blocked(door, here)
        {
            let key = if b == Blocked::Both { "open.blocked" } else { "open.blocked_here" };
            self.msg(key, &Ctx::new().entity("target", door));
            return false;
        }
        // Trying a door tells the player whether it's locked or open.
        if let Some(door) = exit.through {
            self.world.learn(door);
        }
        if let Some(door) = exit.through
            && !self.world.is_open(door)
        {
            if self.world.flag(door, "locked") {
                self.msg("open.locked", &Ctx::new().entity("target", door));
                return false;
            }
            // An unlocked door opens itself on the way through, which saves a
            // step. As with OPEN, that lifts a block on the far side.
            self.msg_kind(LineKind::Notice, "go.opening_first", &Ctx::new().entity("door", door));
            self.world.set(door, "blocked", Value::Bool(false));
            self.world.set(door, "open", Value::Bool(true));
        }
        self.room_event(here, "leave");
        // BRIEF: somewhere the player has been is only named, not described again.
        let been = self.world.state.visited.contains(&to);
        if self.world.move_to(PLAYER, Some(to)).is_err() {
            return false;
        }
        self.world.state.came_from = Some(here);
        // Pushing something: it goes first, and the player follows it in.
        if let Some(t) = self.pushing.take()
            && self.world.move_to(t, Some(to)).is_ok()
        {
            let d = self.grammar.direction_word(exit.direction);
            self.msg("push.done", &Ctx::new().entity("target", t).text("direction", d));
        }
        self.world.leave_tracks(PLAYER, here, exit.direction);
        // Having come through a door, the player knows it's there, even if
        // it was hidden from this side.
        if let Some(door) = exit.through {
            self.world.unset(door, "hidden");
        }
        self.briefly = been && self.world.state.brief;
        self.look();
        self.briefly = false;
        self.world.spring_traps(PLAYER, to);
        self.report_traps();
        // Walking in normally wakes anyone asleep there (after they're seen
        // asleep, so the player knows what they did).
        if !self.sneaking {
            self.world.wake_room(to);
        }
        self.room_event(to, "enter");
        true
    }

    // ------------------------------------------------------ state descriptions

    /// Everything worth saying about an entity's state, in a fixed order:
    /// built-in states first, then the author's own fields by name.
    /// Empty when `settings.describe_state` is off.
    pub(crate) fn state_notes(&self, id: EntityId) -> Vec<StateNote> {
        if !self.world.story().meta.settings.describe_state {
            return Vec::new();
        }
        let w = &self.world;
        let mut notes = Vec::new();
        if w.has(id, Trait::Openable) {
            notes.push(StateNote::Builtin(if w.flag(id, "open") {
                "open"
            } else if w.has(id, Trait::Lockable) && w.flag(id, "locked") {
                "locked"
            } else {
                "closed"
            }));
        } else if w.has(id, Trait::Lockable) && w.flag(id, "locked") {
            notes.push(StateNote::Builtin("locked"));
        }
        if w.flag(id, "worn") {
            notes.push(StateNote::Builtin("worn"));
        }
        if w.def(id).holds_liquid && matches!(w.get(id, "liquid"), Some(Value::Str(s)) if !s.is_empty()) {
            notes.push(StateNote::Builtin("full"));
        }
        if w.is_asleep(id) && !w.is_dead(id) {
            notes.push(StateNote::Builtin("asleep"));
        }
        if w.is_frightened(id) && !w.is_dead(id) {
            notes.push(StateNote::Builtin("frightened"));
        }
        if id != PLAYER && w.held_by(id).is_some() && !w.is_dead(id) {
            notes.push(StateNote::Builtin("caught"));
        }
        if w.is_dead(id) {
            notes.push(StateNote::Builtin("dead"));
            if w.get(id, "killed_by").is_some() || w.flag(id, "_burnt") {
                notes.push(StateNote::Builtin("killed"));
            }
        }
        // Wounds show on characters; the player has HEALTH for that.
        if id != PLAYER
            && !w.is_dead(id)
            && let (Some(h), Some(max)) = (w.health(id), w.def(id).health)
            && h < i64::from(max)
        {
            notes.push(StateNote::Builtin(if h * 2 <= i64::from(max) { "badly_wounded" } else { "wounded" }));
        }
        // What the player has found out by looking: only for things they know.
        if w.known(id) && w.def(id).kind == EntityKind::Object {
            if w.has(id, Trait::Container)
                && w.shows_contents(id)
                && w.contents(id).iter().all(|&c| w.is_hidden(c))
                && !self.hints_at_hidden(id)
            {
                notes.push(StateNote::Builtin("empty"));
            }
            if let Some(cap) = w.capacity(PLAYER)
                && w.has(id, Trait::Portable)
                && !w.is_within(id, PLAYER)
                && w.size(id) <= cap
                && w.weight(id) > cap
            {
                notes.push(StateNote::Builtin("heavy"));
            }
        }
        let is_door = w.has(id, Trait::Door);
        if is_door {
            match w.door_blocked(id, w.here()) {
                Some(Blocked::Both) => notes.push(StateNote::Builtin("blocked")),
                Some(Blocked::ThisSide) => notes.push(StateNote::Builtin("blocked_here")),
                None => {}
            }
        }
        for (field, value) in &w.state.entities[id.index()].state {
            if BUILTIN_STATE.contains(&field.as_str()) || field.starts_with('_') || (is_door && field == "blocked") {
                // A game's own `fuel`, `uses` or `liquid` field is described like any other.
                let builtin = match field.as_str() {
                    "fuel" => w.def(id).fuel.is_some(),
                    "uses" => w.def(id).uses.is_some(),
                    "liquid" => w.def(id).holds_liquid,
                    "dark" => w.def(id).kind == EntityKind::Room,
                    _ => true,
                };
                if builtin {
                    continue;
                }
            }
            match value {
                Value::Bool(on) => {
                    let words = field.strip_prefix("is_").unwrap_or(field).replace('_', " ");
                    notes.push(StateNote::Flag { field: field.clone(), words, on: *on });
                }
                Value::Int(_) | Value::Float(_) | Value::Str(_) => {
                    let shown = match value {
                        Value::Int(i) => i.to_string(),
                        Value::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
                        Value::Float(f) => f.to_string(),
                        Value::Str(s) => s.clone(),
                        _ => unreachable!(),
                    };
                    if !shown.is_empty() {
                        notes.push(StateNote::Value { field: field.clone(), words: field.replace('_', " "), value: shown });
                    }
                }
                // Lists and tables don't read well as a sentence.
                Value::List(_) | Value::Map(_) => {}
            }
        }
        notes
    }

    /// Short tags for lists: "(locked)", "(worn)", "(empty)". Only states
    /// that are "on" get a tag; the rest would just be noise in a list.
    /// Tags only appear once the player has examined or handled the thing.
    pub(crate) fn state_tag_texts(&self, id: EntityId) -> Vec<String> {
        // Death, sleep, terror and traps are plain to see.
        let obvious = self.world.is_dead(id)
            || self.world.is_asleep(id)
            || self.world.is_frightened(id)
            || self.world.held_by(id).is_some()
            || self.world.on_fire(id);
        if !self.world.known(id) && !obvious {
            return Vec::new();
        }
        self.state_notes(id)
            .into_iter()
            .filter_map(|n| match n {
                // "full of water": the liquid's name follows.
                StateNote::Builtin("full") => {
                    let full = self.world.story().messages.get("state.tag.full").and_then(Text::as_literal)?;
                    match self.world.get(id, "liquid") {
                        Some(Value::Str(l)) => Some(format!("{full} {l}")),
                        _ => None,
                    }
                }
                StateNote::Builtin(t) => {
                    self.world.story().messages.get(&format!("state.tag.{t}")).and_then(Text::as_literal)
                }
                StateNote::Flag { words, on: true, .. } => Some(words),
                _ => None,
            })
            .collect()
    }

    /// Whether `holder`'s description refers to something hidden inside it,
    /// as in "${if world.brass_key.hidden} Something glints at the bottom.${end}".
    /// Then calling it empty would contradict the author's hint.
    fn hints_at_hidden(&self, holder: EntityId) -> bool {
        let Some(text) = &self.world.def(holder).description else { return false };
        self.world
            .contents(holder)
            .into_iter()
            .filter(|&c| self.world.is_hidden(c))
            .any(|c| self.mentions(text, c, &["hidden"], false))
    }

    /// Whether a template already reads one of these fields of `id`, in
    /// which case an automatic sentence about it would repeat the author.
    fn mentions(&self, text: &Text, id: EntityId, fields: &[&str], is_own: bool) -> bool {
        let key = &self.world.def(id).key;
        let needles: Vec<String> = fields
            .iter()
            .flat_map(|f| {
                let mut v = vec![format!("world.{key}.{f}")];
                if is_own {
                    v.push(format!("self.{f}"));
                }
                v
            })
            .collect();
        let mut found = false;
        visit_text_code(text, &mut |src| {
            found |= needles.iter().any(|n| {
                // Whole field names only: `self.open` shouldn't match `self.opened`.
                src.match_indices(n.as_str()).any(|(i, _)| {
                    !src[i + n.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            })
        });
        found
    }

    fn note_mentioned(&self, note: &StateNote, text: Option<&Text>, id: EntityId, is_own: bool) -> bool {
        let Some(text) = text else { return false };
        match note {
            StateNote::Builtin("worn") => self.mentions(text, id, &["worn"], is_own),
            StateNote::Builtin("blocked" | "blocked_here") => self.mentions(text, id, &["blocked"], is_own),
            StateNote::Builtin("empty" | "heavy") => false,
            StateNote::Builtin("wounded" | "badly_wounded") => self.mentions(text, id, &["health"], is_own),
            StateNote::Builtin("dead") => self.mentions(text, id, &["dead"], is_own),
            StateNote::Builtin("asleep") => self.mentions(text, id, &["asleep"], is_own),
            StateNote::Builtin("frightened" | "caught") => false,
            StateNote::Builtin("killed") => self.mentions(text, id, &["killed_by", "killed_with"], is_own),
            StateNote::Builtin(_) => self.mentions(text, id, &["open", "locked"], is_own),
            StateNote::Flag { field, .. } | StateNote::Value { field, .. } => self.mentions(text, id, &[field], is_own),
        }
    }

    /// The sentences EXAMINE adds after a description. `attach` adds them
    /// to the description's paragraph.
    fn describe_state(&mut self, id: EntityId, description: Option<&Text>, mut attach: bool) {
        for note in self.state_notes(id) {
            if self.note_mentioned(&note, description, id, true) {
                continue;
            }
            let ctx = Ctx::new().entity("target", id);
            let s = match note {
                StateNote::Builtin("killed") => match self.kill_story(id) {
                    Some(s) => s,
                    None => continue,
                },
                StateNote::Builtin(t) => self.msg_str(&format!("state.{t}"), &ctx),
                StateNote::Flag { words, on, .. } => {
                    self.msg_str(if on { "state.custom.on" } else { "state.custom.off" }, &ctx.text("state", words))
                }
                StateNote::Value { words, value, .. } => {
                    self.msg_str("state.custom.value", &ctx.text("state", words).text("value", value))
                }
            };
            self.push_sentence(s, attach);
            // Later sentences join the first, even when there was no description.
            attach = true;
        }
    }

    /// The sentences a room description adds for things that aren't listed
    /// (scenery): built-in states other than "worn", and author flags that
    /// are on.
    /// Like list tags, these only appear once the player knows the thing.
    fn describe_state_here(&mut self, id: EntityId, room_description: Option<&Text>, attach: bool) {
        // Fire is plain to see, known or not.
        if !self.world.known(id) && !self.world.on_fire(id) {
            return;
        }
        let notes: Vec<StateNote> = self
            .state_notes(id)
            .into_iter()
            .filter(|n| match n {
                StateNote::Builtin("worn" | "killed") => false,
                StateNote::Builtin(_) | StateNote::Flag { on: true, .. } => true,
                _ => false,
            })
            .filter(|n| !self.note_mentioned(n, room_description, id, false))
            .collect();
        let ctx = Ctx::new().entity("target", id);
        let s = match notes.as_slice() {
            [] => return,
            [StateNote::Builtin(t)] => self.msg_str(&format!("state.here.{t}"), &ctx),
            [StateNote::Flag { words, .. }] => self.msg_str("state.here.custom.on", &ctx.text("state", words.clone())),
            // Several at once read better as one sentence: "The cupboard is open and empty."
            _ => {
                let words: Vec<String> = notes
                    .iter()
                    .filter_map(|n| match n {
                        StateNote::Builtin(t) => {
                            self.world.story().messages.get(&format!("state.tag.{t}")).and_then(Text::as_literal)
                        }
                        StateNote::Flag { words, .. } => Some(words.clone()),
                        StateNote::Value { .. } => None,
                    })
                    .collect();
                let list = (self.grammar.lang().list_and)(&words);
                self.msg_str("state.here.several", &ctx.text("list", list))
            }
        };
        self.push_sentence(s, attach);
    }

    // ------------------------------------------------------------ light

    /// LIGHT / EXTINGUISH a `light_source`. Lighting one in the dark shows
    /// the room; putting the last one out plunges it into darkness.
    fn light(&mut self, t: EntityId, on: bool, ctx: &Ctx) -> bool {
        // Something flammable: LIGHT sets it alight, PUT OUT puts it out.
        if !self.world.has(t, Trait::LightSource) && self.world.has(t, Trait::Flammable) {
            if on {
                // LIGHT X has no "with": any burning thing in hand will do.
                return self.burn_action(t, None, ctx);
            }
            return self.douse(t, None, ctx);
        }
        if !self.world.has(t, Trait::LightSource) {
            self.msg(if on { "light.cant" } else { "extinguish.cant" }, ctx);
            return false;
        }
        if self.world.is_burning(t) == on {
            self.msg(if on { "light.already" } else { "extinguish.not_lit" }, ctx);
            return false;
        }
        if on && self.fuel_left(t) == Some(0) {
            self.msg("fuel.empty", ctx);
            return false;
        }
        // A candle needs a flame: the tool, or something burning in hand.
        let flame = if on && self.world.def(t).needs_flame {
            match self.flame_for(t, ctx.arg_entity("tool"), ctx) {
                Ok(f) => Some(f),
                Err(()) => return false,
            }
        } else {
            None
        };
        let was_dark = self.world.in_darkness();
        self.set_lit(t, on);
        match flame {
            Some(f) => self.msg("light.with", &ctx.clone().entity("tool", f)),
            None => self.msg(if on { "light.done" } else { "extinguish.done" }, ctx),
        }
        if was_dark != self.world.in_darkness() {
            self.look();
        }
        true
    }

    // ---------------------------------------------------- taking and dropping

    pub(crate) fn take(&mut self, t: EntityId, from: Option<EntityId>, ctx: &Ctx) -> bool {
        // Taking from someone alive is stealing.
        if let Some(f) = from
            && self.world.def(f).kind == EntityKind::Character
            && !self.world.is_dead(f)
            && f != PLAYER
        {
            return self.steal_action(Some(t), Some(f), ctx);
        }
        // Someone's watching it: not while they can see.
        if from.is_none() && self.stopped_taking(t, ctx) {
            return false;
        }
        // TAKE THE PATH: a way out that goes by it.
        if from.is_none() && !self.world.has(t, Trait::Portable) && self.way_via(t).is_some() {
            return self.enter(t, ctx);
        }
        let w = &self.world;
        let fit = w.fits(t, PLAYER);
        let key = if let Some(f) = from
            && !w.is_within(t, f)
        {
            Some(if w.has(f, Trait::Supporter) { "take.not_on" } else { "take.not_in" })
        } else if t == PLAYER {
            Some("take.self")
        } else if w.location(t) == Some(PLAYER) {
            Some("take.already")
        } else if w.def(t).kind == EntityKind::Character && !w.is_dead(t) {
            Some("take.person")
        } else if w.def(t).kind == EntityKind::Room || w.has(t, Trait::Scenery) {
            Some("take.scenery")
        // A body can be carried off, if it isn't too heavy.
        } else if !w.has(t, Trait::Portable) && !w.is_dead(t) && w.has(t, Trait::Pushable) {
            Some("take.pushable")
        } else if !w.has(t, Trait::Portable) && !w.is_dead(t) {
            Some("take.fixed")
        } else if w.is_within(PLAYER, t) {
            Some("enter.held")
        } else if w.capacity(PLAYER) == Some(0) {
            Some("take.cant_carry")
        } else if fit == Fit::TooBig {
            Some("take.too_big")
        } else if fit == Fit::TooHeavy {
            Some("take.too_heavy")
        } else if fit == Fit::Full {
            Some("take.full")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        let armed = self.world.def(t).trap.is_some() && self.world.flag(t, "armed");
        let _ = self.world.move_to(t, Some(PLAYER));
        self.world.unset(t, "hidden");
        if armed {
            self.world.set(t, "armed", Value::Bool(false));
            self.world.unset(t, "_players");
            self.msg("trap.take_armed", ctx);
        } else {
            self.msg("take.done", ctx);
        }
        true
    }

    fn drop_thing(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.location(t) != Some(PLAYER) {
            self.msg("drop.not_held", ctx);
            return false;
        }
        let here = self.world.here();
        let _ = self.world.move_to(t, Some(here));
        self.world.unset(t, "worn");
        self.msg("drop.done", ctx);
        true
    }

    fn put(&mut self, t: EntityId, tool: Option<EntityId>, on: bool, ctx: &Ctx) -> bool {
        let Some(dest) = tool else {
            self.msg("action.nothing", ctx);
            return false;
        };
        // PUT IT ON THE FLOOR: drop it.
        if dest == self.world.here() {
            return self.drop_thing(t, ctx);
        }
        let w = &self.world;
        let key = if dest == t || w.is_within(dest, t) {
            Some("put.into_self")
        // A body lying here can be stuffed somewhere without picking it up.
        } else if w.location(t) != Some(PLAYER) && !(w.is_dead(t) && w.room_of(t) == Some(w.here())) {
            Some("put.not_held")
        } else if on && !w.has(dest, Trait::Supporter) {
            Some("put.not_supporter")
        } else if !on && !w.has(dest, Trait::Container) {
            Some("put.not_container")
        } else if !on && !w.is_open(dest) {
            Some("put.closed")
        } else if w.fits(t, dest) != Fit::Yes && w.def(dest).sinks {
            Some("put.sinks")
        } else if w.fits(t, dest) == Fit::TooBig {
            Some("put.too_big")
        } else if w.fits(t, dest) == Fit::TooHeavy {
            Some("put.too_heavy")
        } else if w.fits(t, dest) == Fit::Full {
            Some("put.full")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        let _ = self.world.move_to(t, Some(dest));
        self.world.unset(t, "worn");
        self.msg(if on { "put_on.done" } else { "put_in.done" }, ctx);
        true
    }

    // ----------------------------------------------------------- open, lock

    fn open(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let blocked = if self.world.has(t, Trait::Door) { self.world.door_blocked(t, here) } else { None };
        let key = if !self.world.has(t, Trait::Openable) {
            Some("open.cannot")
        } else if self.world.flag(t, "open") {
            Some("open.already")
        } else if blocked == Some(Blocked::Both) {
            Some("open.blocked")
        } else if blocked == Some(Blocked::ThisSide) {
            Some("open.blocked_here")
        } else if self.world.flag(t, "locked") {
            Some("open.locked")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        // Blocked only from the far side: opening from this side lifts
        // the bar, so the way is clear in both directions.
        if self.world.has(t, Trait::Door) && self.world.blocked_anywhere(t) {
            self.world.set(t, "blocked", Value::Bool(false));
        }
        self.world.set(t, "open", Value::Bool(true));
        self.msg("open.done", ctx);
        self.describe_contents(t);
        true
    }

    fn close(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let key = if !self.world.has(t, Trait::Openable) {
            Some("close.cannot")
        } else if !self.world.flag(t, "open") {
            Some("close.already")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        self.world.set(t, "open", Value::Bool(false));
        self.msg("close.done", ctx);
        true
    }

    /// LOCK and UNLOCK. With no key named, the right key is used
    /// automatically if the player is carrying it.
    fn lock_or_unlock(&mut self, t: EntityId, tool: Option<EntityId>, lock: bool) -> bool {
        let verb = if lock { "lock" } else { "unlock" };
        let ctx = Ctx::new().entity("target", t);
        if !self.world.has(t, Trait::Lockable) {
            self.msg(&format!("{verb}.cannot"), &ctx);
            return false;
        }
        let locked = self.world.flag(t, "locked");
        if lock && locked {
            self.msg("lock.already", &ctx);
            return false;
        }
        if !lock && !locked {
            self.msg("unlock.not_locked", &ctx);
            return false;
        }
        if lock && self.world.flag(t, "open") {
            self.msg("lock.close_first", &ctx);
            return false;
        }
        let right_key = self.world.def(t).unlocks_with;
        let key = match tool {
            Some(k) => k,
            None => match right_key {
                Some(k) if self.world.is_within(k, PLAYER) => k,
                _ => {
                    self.msg(&format!("{verb}.no_key"), &ctx);
                    return false;
                }
            },
        };
        let ctx = ctx.entity("tool", key);
        if Some(key) != right_key {
            self.msg("unlock.wrong_key", &ctx);
            return false;
        }
        self.world.set(t, "locked", Value::Bool(lock));
        self.msg(&format!("{verb}.done"), &ctx);
        true
    }

    // ------------------------------------------------- getting in and out

    fn get_into(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let on = self.world.has(t, Trait::Supporter) && !self.world.has(t, Trait::Container);
        if self.world.location(PLAYER) == Some(t) {
            self.msg(if on { "enter.already_on" } else { "enter.already_in" }, ctx);
            return false;
        }
        if self.world.is_within(t, PLAYER) {
            self.msg("enter.held", ctx);
            return false;
        }
        // Climbing from one thing into another: get out of the first.
        while let Some(holder) = self.world.location(PLAYER)
            && holder != self.world.here()
            && !self.world.is_within(t, holder)
        {
            if !self.leave_holder(holder, true) {
                return false;
            }
        }
        if self.world.has(t, Trait::Container) && !self.world.is_open(t) {
            if self.world.flag(t, "locked") || !self.world.has(t, Trait::Openable) {
                self.msg("put.closed", &ctx.clone().entity("tool", t));
                return false;
            }
            self.msg_kind(LineKind::Notice, "go.opening_first", &Ctx::new().entity("door", t));
            self.world.set(t, "open", Value::Bool(true));
        }
        if self.world.fits(PLAYER, t) != Fit::Yes {
            self.msg(if self.world.def(t).sinks { "enter.sinks" } else { "enter.full" }, ctx);
            return false;
        }
        if self.world.move_to(PLAYER, Some(t)).is_err() {
            self.msg("enter.held", ctx);
            return false;
        }
        self.msg(if on { "enter.done_on" } else { "enter.done_in" }, ctx);
        true
    }

    /// GET OUT, STAND UP and the like. With a target, it must be what the
    /// player is in or on.
    fn get_out(&mut self, target: Option<EntityId>) -> bool {
        let here = self.world.here();
        let Some(holder) = self.world.location(PLAYER).filter(|h| *h != here) else {
            self.msg("get_out.nothing", &Ctx::new());
            return false;
        };
        if let Some(t) = target
            && t != holder
        {
            self.msg("get_out.not_there", &Ctx::new().entity("target", t));
            return false;
        }
        self.leave_holder(holder, false)
    }

    /// Moves the player out of `holder` to wherever it is, opening it first
    /// if needed. `implicit` gives the "(first getting out of ...)" form.
    fn leave_holder(&mut self, holder: EntityId, implicit: bool) -> bool {
        let ctx = Ctx::new().entity("target", holder);
        if self.world.has(holder, Trait::Container) && !self.world.is_open(holder) {
            if self.world.flag(holder, "locked") || !self.world.has(holder, Trait::Openable) {
                self.msg("put.closed", &Ctx::new().entity("tool", holder));
                return false;
            }
            self.msg_kind(LineKind::Notice, "go.opening_first", &Ctx::new().entity("door", holder));
            self.world.set(holder, "open", Value::Bool(true));
        }
        let on = self.world.has(holder, Trait::Supporter) && !self.world.has(holder, Trait::Container);
        let outer = self.world.location(holder);
        if self.world.move_to(PLAYER, outer).is_err() {
            return false;
        }
        if implicit {
            self.msg_kind(LineKind::Notice, if on { "go.getting_off_first" } else { "go.getting_out_first" }, &ctx);
        } else {
            self.msg(if on { "get_out.done_on" } else { "get_out.done_in" }, &ctx);
        }
        true
    }

    // ------------------------------------------------------------ emptying

    /// EMPTY X tips what's in or on X onto the floor (wherever X is, or
    /// where the player is if they're holding it). EMPTY X INTO/ONTO Y moves
    /// it to Y instead, as far as it fits.
    fn empty(&mut self, t: EntityId, into: Option<EntityId>, ctx: &Ctx) -> bool {
        let w = &self.world;
        if !w.has(t, Trait::Container) && !w.has(t, Trait::Supporter) {
            self.msg("empty.cannot", ctx);
            return false;
        }
        if w.has(t, Trait::Container) && !w.is_open(t) {
            self.msg("empty.closed", ctx);
            return false;
        }
        let items: Vec<EntityId> = self.listable(t).into_iter().filter(|i| self.world.has(*i, Trait::Portable)).collect();
        if items.is_empty() {
            self.msg("empty.nothing", ctx);
            return false;
        }
        let dest = match into {
            Some(d) => {
                let w = &self.world;
                let tctx = ctx.clone().entity("target", t);
                let key = if d == t || w.is_within(d, t) {
                    Some("put.into_self")
                } else if !w.has(d, Trait::Container) && !w.has(d, Trait::Supporter) {
                    Some("put.not_container")
                } else if w.has(d, Trait::Container) && !w.is_open(d) {
                    Some("put.closed")
                } else {
                    None
                };
                if let Some(k) = key {
                    self.msg(k, &tctx);
                    return false;
                }
                d
            }
            None => match self.world.location(t) {
                Some(l) if l == PLAYER => self.world.location(PLAYER).unwrap_or(self.world.here()),
                Some(l) => l,
                None => self.world.here(),
            },
        };
        let mut moved = Vec::new();
        let mut full = false;
        for item in items {
            if into.is_some() && self.world.fits(item, dest) != Fit::Yes {
                full = true;
                continue;
            }
            if self.world.move_to(item, Some(dest)).is_ok() {
                moved.push(item);
            }
        }
        if !moved.is_empty() {
            let list = self.list_a(&moved);
            let key = match into {
                None => "empty.done",
                Some(d) if self.world.has(d, Trait::Supporter) && !self.world.has(d, Trait::Container) => "empty.onto",
                Some(_) => "empty.into",
            };
            self.msg(key, &ctx.clone().text("list", list));
        }
        if full {
            self.msg("put.full", ctx);
        }
        !moved.is_empty()
    }

    // ------------------------------------------------------------- people

    /// The characters here who'd talk, if asked.
    fn people_to_talk_to(&self) -> Vec<EntityId> {
        let w = &self.world;
        w.scope()
            .into_iter()
            .filter(|&c| c != PLAYER && w.def(c).kind == EntityKind::Character && w.def(c).talk.is_some() && !w.is_dead(c) && !w.is_asleep(c))
            .collect()
    }

    fn talk(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.def(t).kind != EntityKind::Character {
            self.topic = None;
            self.msg("talk.not_person", ctx);
            return false;
        }
        if self.world.is_active(t) && !self.world.is_asleep(t) {
            self.hear_voice(t);
        }
        if self.world.is_dead(t) || self.world.is_asleep(t) {
            self.topic = None;
            self.msg(if self.world.is_dead(t) { "talk.dead" } else { "talk.asleep" }, ctx);
            return false;
        }
        // Hostile: no talking.
        if self.world.disposition(t) <= crate::npc::HOSTILE {
            self.topic = None;
            self.msg("talk.hostile", &ctx.clone().entity("target", t));
            return false;
        }
        match self.world.def(t).talk {
            Some(c) => {
                let key = self.world.story().choice(c).key.clone();
                self.choice_requested = Some(key);
                true
            }
            None => {
                self.topic = None;
                if self.ask_favour(t) {
                    return true;
                }
                self.msg("talk.no_response", ctx);
                false
            }
        }
    }

    // ------------------------------------------------------- clothes, food

    fn wear(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let key = if !self.world.has(t, Trait::Wearable) {
            Some("wear.cannot")
        } else if self.world.flag(t, "worn") {
            Some("wear.already")
        } else if self.world.location(t) != Some(PLAYER) {
            Some("put.not_held")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        self.world.set(t, "worn", Value::Bool(true));
        self.msg("wear.done", ctx);
        true
    }

    fn take_off(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if !self.world.flag(t, "worn") {
            self.msg("take_off.not_worn", ctx);
            return false;
        }
        self.world.set(t, "worn", Value::Bool(false));
        self.msg("take_off.done", ctx);
        true
    }

    fn eat(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if !self.world.has(t, Trait::Edible) {
            self.msg("eat.cannot", ctx);
            return false;
        }
        // The message is rendered first, while the thing still exists to be named.
        self.msg("eat.done", ctx);
        let _ = self.world.move_to(t, None);
        true
    }
}
