//! Commands that help the player keep track, and recover from slips:
//! WHERE IS THE KEY, RECAP, UNDO 3, OOPS and "Did you mean…?", AGAIN WITH
//! THE IRON KEY, ASK ABOUT THE GATE going straight to the gate, WHAT DID
//! SHE SAY?, MAP in words, and INVENTORY BY PLACE.
//!
//! What they use is what the game already keeps: where things were last
//! seen (`located`), the last command, and the moves taken.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{EntityId, EntityKind, Value, PLAYER};
use std::collections::HashSet;

/// How many moves RECAP remembers.
const RECENT: usize = 5;

/// The words with the first `word` swapped for `with`.
fn replace_word(toks: &[String], word: &str, with: &[String]) -> Option<Vec<String>> {
    let i = toks.iter().position(|t| t == word)?;
    let mut out = toks[..i].to_vec();
    out.extend(with.iter().cloned());
    out.extend(toks[i + 1..].iter().cloned());
    Some(out)
}

impl<H: ScriptHost> Game<H> {
    /// Runs `f` (UNDO, RESTART) keeping what's the player's, not the
    /// story's: their notes, and BRIEF or VERBOSE.
    pub(crate) fn keep_players<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let notes = std::mem::take(&mut self.world.state.notes);
        let brief = self.world.state.brief;
        let r = f(self);
        self.forget_this_turn();
        self.world.state.notes = notes;
        self.world.state.brief = brief;
        r
    }

    /// A move that took a turn, as typed, for RECAP.
    pub(crate) fn remember_move(&mut self, typed: String) {
        let recent = &mut self.world.state.recent;
        recent.push(typed);
        if recent.len() > RECENT {
            recent.remove(0);
        }
    }

    /// The first number in some words: "3", "three", "3 moves".
    pub(crate) fn count_in(&self, text: &str) -> Option<usize> {
        let numbers = &self.world.story().lexicon.numbers;
        text.split_whitespace().find_map(|w| w.parse::<usize>().ok().or_else(|| numbers.get(w).map(|&n| n as usize)))
    }

    // ------------------------------------------------- slips of the keyboard

    /// The reply to "Did you mean TAKE?": YES, or one of the words offered,
    /// runs the command again with it. None when the reply is something else.
    pub(crate) fn guess_reply(&mut self, reply: &[String], was: &[String], word: &str, options: &[String]) -> Option<bool> {
        let lx = &self.world.story().lexicon;
        let pick = match reply {
            [w] if lx.yes_words.contains(w) && options.len() == 1 => options[0].clone(),
            // "Did you mean TAKE or TALK?" "Yes." Which, then?
            [w] if lx.yes_words.contains(w) => {
                let shown: Vec<String> = options.iter().map(|o| o.to_uppercase()).collect();
                let list = (self.grammar.lang().list_or)(&shown);
                self.msg("parser.guess_which", &Ctx::new().text("options", list));
                self.pending = Some(crate::game::Pending::Guess { toks: was.to_vec(), word: word.to_string(), options: options.to_vec() });
                return Some(false);
            }
            [w] if lx.no_words.contains(w) => {
                self.msg("parser.guess_no", &Ctx::new());
                return Some(false);
            }
            [w] if options.contains(w) => w.clone(),
            _ => return None,
        };
        self.oops = None;
        Some(self.command(replace_word(was, word, &[pick])?))
    }

    /// OOPS LANTERN: the last command again, with the word that wasn't
    /// understood swapped for this one.
    pub(crate) fn oops_action(&mut self, text: &str) -> bool {
        let Some((toks, bad)) = self.oops.take() else {
            self.msg("oops.nothing", &Ctx::new());
            return false;
        };
        let with: Vec<String> = text.split_whitespace().map(str::to_string).collect();
        if with.is_empty() {
            self.msg("oops.what", &Ctx::new().text("word", bad.clone()));
            self.oops = Some((toks, bad));
            return false;
        }
        self.pending = None;
        match replace_word(&toks, &bad, &with) {
            Some(fixed) => self.command(fixed),
            None => {
                self.msg("oops.nothing", &Ctx::new());
                false
            }
        }
    }

    /// AGAIN WITH THE IRON KEY, AGAIN, THE PEAR: the last command, with
    /// something else in it.
    pub(crate) fn again_with(&mut self, last: Vec<String>, with: &str) -> bool {
        let and = &self.world.story().lexicon.and_words;
        let new: Vec<String> = with.split_whitespace().skip_while(|w| and.iter().any(|a| a == w)).map(str::to_string).collect();
        if new.is_empty() {
            return self.command(last);
        }
        // The first reading that makes a command; failing that, the first,
        // so the player hears what went wrong with it.
        let tries = self.grammar.swap_object(&self.world, &last, &new);
        let pick = tries.iter().find(|t| matches!(self.grammar.parse(&self.world, t), crate::parser::Parsed::Command(_))).or(tries.first()).cloned();
        match pick {
            Some(words) => self.command(words),
            None => {
                self.msg("again.cannot", &Ctx::new());
                false
            }
        }
    }

    // ----------------------------------------------------------- where is

    /// A phrase's words, without articles.
    fn phrase_words(&self, text: &str) -> Vec<String> {
        let articles = &self.world.story().lexicon.articles;
        text.split_whitespace().filter(|w| !articles.iter().any(|a| a == w)).map(str::to_string).collect()
    }

    /// Things the player knows of called that: what's here and what they
    /// carry, and wherever else they've seen something.
    ///
    /// A reminder, never a hint: what's out of sight is matched only by
    /// what the player could know of it, and said to be where they last
    /// saw it, wherever it is now, whatever has become of it since.
    pub(crate) fn known_called(&self, words: &[String]) -> Vec<EntityId> {
        let w = &self.world;
        let thing = |t: EntityId| t != PLAYER && w.def(t).kind != EntityKind::Room;
        let here: Vec<EntityId> = w.scope().into_iter().filter(|&t| thing(t)).collect();
        let away: Vec<EntityId> = w.state.located.keys().copied().filter(|&t| thing(t) && !here.contains(&t)).collect();
        let mut found = self.grammar.name_matches_in(w, words, &here);
        found.extend(self.grammar.remembered_matches(w, words, &away));
        found
    }

    /// WHERE IS THE KEY.
    pub(crate) fn where_is(&mut self, what: &str) {
        let words = self.phrase_words(what);
        if words.is_empty() {
            return self.msg("where.what", &Ctx::new());
        }
        let found = self.known_called(&words);
        if found.is_empty() {
            let here = self.world.here();
            return match self.world.places_named(&words, &[]).first() {
                Some(&p) => {
                    let ctx = Ctx::new().text("place", self.world.def(p).name.clone()).text("typed", words.join(" ").to_uppercase());
                    self.msg(if p == here { "where.place_here" } else { "where.place" }, &ctx);
                }
                None => self.msg("where.unknown", &Ctx::new()),
            };
        }
        let typed = words.join(" ").to_uppercase();
        let mut said: Vec<(String, Option<EntityId>)> = Vec::new();
        for t in found {
            // Two gold coins in the same place are said once.
            let key = (self.world.display_name(t), self.world.state.located.get(&t).copied());
            if said.contains(&key) || said.len() == 3 {
                continue;
            }
            said.push(key);
            self.say_where(t, &typed, said.len() == 1);
        }
    }

    /// Where the player knows `t` is. `offer`: say GO TO will take them there.
    fn say_where(&mut self, t: EntityId, typed: &str, offer: bool) {
        let here = self.world.here();
        let ctx = Ctx::new().entity("target", t);
        if self.world.is_within(t, PLAYER) {
            // Where they got it, if they remember.
            let ctx = match self.world.state.got.get(&t) {
                Some(Some(r)) => ctx.text("found", self.world.def(*r).name.clone()),
                Some(None) => ctx.value("start", Value::Bool(true)),
                None => ctx,
            };
            return self.msg("where.carried", &ctx);
        }
        if self.world.in_scope(t) {
            return self.msg("where.here", &ctx);
        }
        match self.world.state.located.get(&t).copied() {
            Some(r) if r == here => self.msg("where.gone", &ctx),
            Some(r) => {
                let go = offer && self.route(here, &[r]).is_some();
                let place = self.world.def(r).name.clone();
                self.msg("where.seen", &ctx.text("place", place).text("typed", typed.to_string()).value("go", Value::Bool(go)));
            }
            None => self.msg("where.unknown", &Ctx::new()),
        }
    }

    /// GO TO THE KEEPER: to where the player last saw them. None when the
    /// words name nothing they've seen.
    pub(crate) fn go_to_thing(&mut self, words: &[String]) -> Option<bool> {
        let t = *self.known_called(words).first()?;
        let here = self.world.here();
        let seen = self.world.state.located.get(&t).copied();
        match seen {
            Some(r) if r != here && !self.world.is_within(t, PLAYER) && !self.world.in_scope(t) => {
                let place = self.world.def(r).name.clone();
                self.msg_kind(LineKind::Notice, "go.to_thing", &Ctx::new().entity("target", t).text("place", place));
                Some(self.walk_to(r))
            }
            _ => {
                self.say_where(t, "", false);
                Some(false)
            }
        }
    }

    // -------------------------------------------------------------- recap

    /// RECAP: where things stand, for a player coming back to a game.
    pub(crate) fn recap(&mut self) {
        let ctx = Ctx::new();
        let (turns, places) = (self.world.state.turn as i64, self.world.state.visited.len() as i64);
        self.msg("recap.so_far", &ctx.clone().value("turns", Value::Int(turns)).value("places", Value::Int(places)));
        let place = if self.world.in_darkness() {
            self.msg_str("look.dark_title", &ctx)
        } else {
            self.world.def(self.world.here()).name.clone()
        };
        self.msg("recap.where", &ctx.clone().text("place", place));
        let scenes = self.scenes_now();
        if !scenes.is_empty() {
            let list = (self.grammar.lang().list_and)(&scenes);
            self.msg("recap.scene", &ctx.clone().text("scene", list));
        }
        let held = self.world.inventory();
        if held.is_empty() {
            self.msg("recap.empty_handed", &ctx);
        } else {
            let list = self.list_a(&held);
            self.msg("recap.carrying", &ctx.clone().text("list", list));
        }
        if self.world.story().meta.max_score.is_some() || self.world.state.score != 0 {
            self.msg("recap.score", &ctx);
        }
        if self.world.story().clock.as_ref().is_some_and(|c| c.show) {
            self.tell_time();
        }
        let goals = self.world.story().goals.clone();
        let (mut done, mut open) = (Vec::new(), Vec::new());
        for g in &goals {
            if let Some(d) = self.world.goal(&g.key) {
                let text = self.render_str(&g.text, &ctx);
                if d { done.push(text) } else { open.push(text) }
            }
        }
        let and = self.grammar.lang().list_and;
        if !done.is_empty() {
            self.msg("recap.done", &ctx.clone().text("list", and(&done)));
        }
        if !open.is_empty() {
            self.msg("recap.to_do", &ctx.clone().text("list", and(&open)));
        }
        let recent = self.world.state.recent.clone();
        if !recent.is_empty() {
            self.msg("recap.recent", &ctx.clone().text("list", recent.join(", ")));
        }
        let notes = self.world.state.notes.len() as i64;
        if notes > 0 {
            self.msg("recap.notes", &ctx.clone().value("n", Value::Int(notes)));
        }
    }

    // ----------------------------------------------------- what was said

    /// Keeps what a conversation said, from line `from` on: its text, not
    /// the numbered options.
    pub(crate) fn keep_talk(&mut self, key: &str, from: usize) {
        const KEPT: usize = 40;
        let said: Vec<String> = self.out.lines[from.min(self.out.lines.len())..].iter().filter(|l| l.kind == LineKind::Text).map(|l| l.text.clone()).collect();
        let talk = self.world.state.talks.entry(key.to_string()).or_default();
        talk.extend(said);
        if talk.len() > KEPT {
            talk.drain(..talk.len() - KEPT);
        }
    }

    /// Who the open conversation is with: whoever has it as their `talk`
    /// and is here, else whoever has it.
    pub(crate) fn talker_now(&self, key: &str) -> Option<EntityId> {
        let story = self.world.story();
        let c = story.choices.iter().position(|c| c.key == key)?;
        let has = |i: EntityId| self.world.def(i).talk.is_some_and(|t| t.0 as usize == c);
        self.world.ids().find(|&i| has(i) && self.world.in_scope(i)).or_else(|| self.talker(key))
    }

    /// Who a conversation is with: whoever has it as their `talk`.
    fn talker(&self, key: &str) -> Option<EntityId> {
        let story = self.world.story();
        let c = story.choices.iter().position(|c| c.key == key)?;
        self.world.ids().find(|&i| self.world.def(i).talk.is_some_and(|t| t.0 as usize == c))
    }

    /// WHAT DID SHE SAY?: the last conversation again, or the one with
    /// someone named (WHAT DID THE KEEPER SAY?).
    pub(crate) fn last_talk(&mut self, who: &str) {
        let words = self.phrase_words(who);
        let pronoun = words.len() == 1 && self.world.story().lexicon.pronouns.iter().any(|p| p == &words[0]);
        let key = if words.is_empty() || pronoun {
            self.world.state.last_talk.clone()
        } else {
            let named = self.known_called(&words);
            let Some(&t) = named.first() else { return self.msg("where.unknown", &Ctx::new()) };
            let key = self.world.def(t).talk.map(|c| self.world.story().choices[c.0 as usize].key.clone());
            match key.filter(|k| self.world.state.talks.contains_key(k)) {
                Some(k) => Some(k),
                None => return self.msg("talk.recall_not_them", &Ctx::new().entity("target", t)),
            }
        };
        let Some(key) = key.filter(|k| self.world.state.talks.get(k).is_some_and(|l| !l.is_empty())) else {
            return self.msg("talk.recall_none", &Ctx::new());
        };
        let who = self.talker(&key).map(|t| self.the(t));
        let ctx = match who {
            Some(w) => Ctx::new().text("who", w),
            None => Ctx::new(),
        };
        self.msg_kind(LineKind::Notice, "talk.recall_head", &ctx);
        for line in self.world.state.talks[&key].clone() {
            self.push(LineKind::Text, line);
        }
    }

    // --------------------------------------------------------------- map

    /// MAP: the places the player has been and their ways out, in words:
    /// what the website's map shows, here first, then outward.
    pub(crate) fn text_map(&mut self) {
        // A map in the game, in view: MAP looks at it.
        let map_here = self.known_called(&["map".to_string()]).into_iter().find(|&t| self.world.in_scope(t));
        if map_here.is_some() {
            // As OOPS does: the command, run as if typed.
            self.command(vec!["examine".into(), "map".into()]);
            return;
        }
        let w = &self.world;
        let here = w.here();
        let visited = &w.state.visited;
        // Here first, then the places next to those already listed.
        let mut order = vec![here];
        let mut i = 0;
        while i < order.len() {
            for e in w.def(order[i]).exits.iter().filter(|e| w.exit_visible(e)) {
                if let Some(to) = e.to.filter(|t| visited.contains(t) && !order.contains(t)) {
                    order.push(to);
                }
            }
            i += 1;
        }
        let rest: Vec<EntityId> = visited.iter().copied().filter(|r| !order.contains(r)).collect();
        order.extend(rest);
        order.retain(|r| visited.contains(r) || *r == here);
        let n = order.len() as i64;
        self.msg_kind(LineKind::Notice, "map.head", &Ctx::new().value("n", Value::Int(n)));
        for r in order {
            // Ways that lead somewhere, as LOOK lists them ("The sea. Not tonight." isn't one).
            let exits: Vec<eifgb_ir::Exit> = self.world.def(r).exits.iter().filter(|e| e.to.is_some() && self.world.exit_visible(e)).cloned().collect();
            let mut ways = Vec::new();
            for e in exits {
                let dir = self.grammar.direction_word(e.direction);
                let known = e.to.filter(|t| self.world.state.visited.contains(t));
                let way = match known {
                    Some(to) => self.msg_str("map.way_to", &Ctx::new().text("dir", dir).text("room", self.world.def(to).name.clone())),
                    None => self.msg_str("map.way_new", &Ctx::new().text("dir", dir)),
                };
                let way = match e.through {
                    Some(d) => self.msg_str("map.through", &Ctx::new().text("way", way).entity("door", d).value("closed", Value::Bool(!self.world.flag(d, "open")))),
                    None => way,
                };
                ways.push(way);
            }
            let ways = if ways.is_empty() { self.msg_str("map.no_ways", &Ctx::new()) } else { ways.join("; ") };
            let mut ctx = Ctx::new().text("room", self.world.def(r).name.clone()).value("here", Value::Bool(r == here)).text("ways", ways);
            if let Some(m) = self.mark_in(r) {
                ctx = ctx.text("mark", m);
            }
            self.msg("map.room", &ctx);
        }
        // Places heard of, not yet found.
        let heard: Vec<String> = self.heard_of_only().into_iter().map(|r| self.world.def(r).name.clone()).collect();
        if !heard.is_empty() {
            let list = (self.grammar.lang().list_and)(&heard);
            self.msg("map.heard_of", &Ctx::new().text("places", list));
        }
    }

    // ---------------------------------------------------- inventory by place

    /// INVENTORY BY PLACE: what's carried, grouped by where it was found.
    pub(crate) fn inventory_by_place(&mut self) {
        let held = self.world.inventory();
        if held.is_empty() {
            return self.msg("inventory.empty", &Ctx::new());
        }
        // In the order the places first come up.
        let mut groups: Vec<(Option<Option<EntityId>>, Vec<EntityId>)> = Vec::new();
        for t in held {
            let from = self.world.state.got.get(&t).copied();
            match groups.iter_mut().find(|(f, _)| *f == from) {
                Some((_, list)) => list.push(t),
                None => groups.push((from, vec![t])),
            }
        }
        // From the start first.
        groups.sort_by_key(|(f, _)| !matches!(f, Some(None)));
        for (from, list) in groups {
            let ctx = Ctx::new().text("list", self.list_a(&list));
            match from {
                Some(None) => self.msg("inventory.from_start", &ctx),
                Some(Some(r)) => self.msg("inventory.from_place", &ctx.text("place", self.world.def(r).name.clone())),
                None => self.msg("inventory.from_somewhere", &ctx),
            }
        }
    }

    // ------------------------------------------------------------ topics

    /// ASK KEEPER ABOUT THE GATE: the open conversation's option that
    /// mentions the gate, if just one does.
    pub(crate) fn topic_option(&mut self, topic: &str) -> Option<usize> {
        self.world.state.choice?;
        let lx = &self.world.story().lexicon;
        let skip = |w: &str| lx.articles.iter().any(|a| a == w) || lx.stopwords.iter().any(|s| s == w);
        let want: Vec<String> = topic.split_whitespace().filter(|w| !skip(w)).map(str::to_string).collect();
        if want.is_empty() {
            return None;
        }
        let stems = self.grammar.lang().stems;
        let forms = |w: &str| {
            let mut f = stems(w);
            f.push(w.to_string());
            f
        };
        let hits: Vec<usize> = self
            .visible_options()
            .into_iter()
            .filter(|(_, text)| {
                let have: HashSet<String> = text
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric() && c != '\'')
                    .filter(|w| !w.is_empty())
                    .flat_map(forms)
                    .collect();
                want.iter().all(|w| forms(w).iter().any(|f| have.contains(f)))
            })
            .map(|(i, _)| i)
            .collect();
        match hits[..] {
            [i] => Some(i),
            _ => None,
        }
    }
}
