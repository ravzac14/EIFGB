//! Commands that help the player keep track, and recover from slips:
//! WHERE IS THE KEY, RECAP, UNDO 3, OOPS and "Did you mean…?", AGAIN WITH
//! THE IRON KEY, and ASK ABOUT THE GATE going straight to the gate.
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
