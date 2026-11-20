//! Commands that save the player typing: USE, which does whatever a thing
//! is for, and the ones that take several turns: GO TO a place visited
//! before, GO BACK, WAIT 3 and WAIT UNTIL DUSK.
//!
//! The several-turn commands run ordinary turns one after another (each
//! step a GO, each wait a WAIT), and stop as soon as anything happens, so
//! the player never walks past a wolf or sleeps through a visitor. UNDO
//! takes the whole thing back at once.

use crate::game::{Game, LineKind};
use crate::parser::Command;
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{parse_time, BuiltinAction, Direction, EntityId, EntityKind, Trait, VerbAction, DAY, PLAYER};
use std::collections::{HashMap, VecDeque};

/// The longest a single WAIT goes on for, in turns.
const MOST_TURNS: u64 = 1000;

impl<H: ScriptHost> Game<H> {
    /// USE X, or USE X ON Y: whatever X is for, done as that verb (so the
    /// game's own handlers for UNLOCK, LIGHT and so on still run).
    pub(crate) fn use_action(&mut self, t: EntityId, on: Option<EntityId>, ctx: &Ctx) -> bool {
        let w = &self.world;
        let lit = |id| w.flag(id, "lit") || w.flag(id, "burning") || w.flag(id, "on");
        let (a, target, tool) = if let Some(o) = on {
            if w.def(o).unlocks_with == Some(t) {
                (BuiltinAction::Unlock, o, Some(t))
            } else if w.has(o, Trait::Flammable) && lit(t) {
                (BuiltinAction::Burn, o, Some(t))
            } else if w.def(o).kind == EntityKind::Character && o != PLAYER && !w.is_dead(o) {
                // A weapon on someone is an attack; anything else, a gift.
                if w.def(t).damage.is_some() { (BuiltinAction::Attack, o, Some(t)) } else { (BuiltinAction::Give, t, Some(o)) }
            } else {
                self.msg("use.on_how", ctx);
                return false;
            }
        } else if w.has(t, Trait::LightSource) {
            (if lit(t) { BuiltinAction::Extinguish } else { BuiltinAction::Light }, t, None)
        } else if w.has(t, Trait::Wearable) && !w.flag(t, "worn") {
            (BuiltinAction::Wear, t, None)
        } else if w.has(t, Trait::Edible) {
            (BuiltinAction::Eat, t, None)
        } else if w.has(t, Trait::Door) && w.flag(t, "open") {
            (BuiltinAction::Enter, t, None)
        } else if w.has(t, Trait::Openable) && !w.flag(t, "open") {
            (BuiltinAction::Open, t, None)
        } else if w.has(t, Trait::Enterable) {
            (BuiltinAction::Enter, t, None)
        } else if w.def(t).kind == EntityKind::Character && w.def(t).talk.is_some() {
            (BuiltinAction::Talk, t, None)
        } else if w.def(t).calls.is_some() {
            (BuiltinAction::Sound, t, None)
        } else {
            self.msg("use.how", ctx);
            return false;
        };
        let Some(vi) = w.story().verbs.iter().position(|v| v.action == VerbAction::Builtin(a)) else {
            self.msg("use.how", ctx);
            return false;
        };
        let verb = w.story().verbs[vi].clone();
        let words = verb.patterns.first().and_then(|p| match p.0.first() {
            Some(eifgb_ir::PatternToken::Word(w)) => Some(vec![w.clone()]),
            _ => None,
        });
        let cmd = Command { verb: vi, targets: vec![target], tool, text: None, direction: None, words: words.unwrap_or_default() };
        self.perform_one(&verb, Some(target), &cmd);
        true
    }

    /// GO TO, GO BACK, WAIT 3, WAIT UNTIL DUSK. Returns false when the rest
    /// of a chained input should be skipped.
    pub(crate) fn errand(&mut self, a: BuiltinAction, cmd: &Command) -> bool {
        let ctx = Ctx::new();
        let words: Vec<String> = cmd
            .text
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .filter(|w| !self.world.story().lexicon.articles.iter().any(|a| a == w))
            .map(str::to_string)
            .collect();
        let ok = match a {
            BuiltinAction::GoTo => self.go_to(&words),
            BuiltinAction::GoBack => match self.world.state.came_from {
                Some(r) => self.walk_to(r),
                None => {
                    self.msg("go.no_back", &ctx);
                    false
                }
            },
            BuiltinAction::WaitFor | BuiltinAction::WaitUntil => {
                let turns = if a == BuiltinAction::WaitFor { self.how_long(&words) } else { self.turns_until(&words) };
                match turns {
                    Ok(n) => self.wait_turns(n),
                    Err(key) => {
                        let time = self.world.clock().map(|c| c.time).unwrap_or_default();
                        self.msg(key, &ctx.text("time", time));
                        false
                    }
                }
            }
            other => unreachable!("{other:?} isn't an errand"),
        };
        self.last = Some(cmd.words.clone());
        ok && self.world.state.ending.is_none() && self.world.state.choice.is_none()
    }

    // ---------------------------------------------------------- going places

    fn go_to(&mut self, words: &[String]) -> bool {
        let here = self.world.here();
        let ctx = Ctx::new();
        let room_words = &self.world.story().lexicon.room_words;
        // Rooms the player has been to whose name has every word.
        let named: Vec<EntityId> = self
            .world
            .state
            .visited
            .iter()
            .copied()
            .filter(|&r| {
                let name = name_words(&self.world.def(r).name);
                !words.is_empty() && words.iter().all(|w| name.contains(w) || room_words.contains(w))
            })
            .collect();
        if named.is_empty() {
            self.msg("go.unknown_place", &ctx);
            return false;
        }
        if named.contains(&here) && named.len() == 1 {
            self.msg("go.already_there", &ctx);
            return false;
        }
        // Several called that: the nearest.
        let Some((to, _)) = self.route(here, &named) else {
            let place = self.world.def(named[0]).name.clone();
            self.msg("go.no_route", &ctx.text("place", place));
            return false;
        };
        self.walk_to(to)
    }

    /// Walks to a room by the shortest known way, a turn a step.
    fn walk_to(&mut self, to: EntityId) -> bool {
        let here = self.world.here();
        if to == here {
            self.msg("go.already_there", &Ctx::new());
            return false;
        }
        let Some((_, steps)) = self.route(here, &[to]) else {
            let place = self.world.def(to).name.clone();
            self.msg("go.no_route", &Ctx::new().text("place", place));
            return false;
        };
        let Some(go) = self.world.story().verbs.iter().position(|v| v.action == VerbAction::Builtin(BuiltinAction::Go)) else {
            return false;
        };
        let (turn, before) = (self.world.state.turn, self.world.state.clone());
        let last = steps.len() - 1;
        let mut ok = true;
        for (i, (dir, next)) in steps.into_iter().enumerate() {
            let before = self.out.lines.len();
            self.passing = i < last;
            let word = self.grammar.direction_word(dir);
            ok = self.execute_command(Command { verb: go, targets: Vec::new(), tool: None, text: None, direction: Some(dir), words: vec![word] });
            self.passing = false;
            if !ok || self.world.here() != next {
                break;
            }
            // Something happened on the way: stop, and show where.
            let eventful = self.out.lines[before..].iter().any(|l| !matches!(l.kind, LineKind::Title | LineKind::Notice));
            if i < last && eventful {
                break;
            }
        }
        self.squash_turns(turn, before);
        ok
    }

    /// The nearest of `targets` by exits the player knows (to rooms they've
    /// been to), and the steps to it: each a direction and the room it leads to.
    fn route(&self, from: EntityId, targets: &[EntityId]) -> Option<(EntityId, Vec<(Direction, EntityId)>)> {
        let w = &self.world;
        let mut came: HashMap<EntityId, (EntityId, Direction)> = HashMap::new();
        let mut queue = VecDeque::from([from]);
        while let Some(r) = queue.pop_front() {
            if r != from && targets.contains(&r) {
                let mut steps = Vec::new();
                let mut at = r;
                while at != from {
                    let (prev, d) = came[&at];
                    steps.push((d, at));
                    at = prev;
                }
                steps.reverse();
                return Some((r, steps));
            }
            for e in &w.def(r).exits {
                let Some(to) = e.to else { continue };
                if to == from || came.contains_key(&to) || !w.state.visited.contains(&to) || !w.exit_visible(e) {
                    continue;
                }
                came.insert(to, (r, e.direction));
                queue.push_back(to);
            }
        }
        None
    }

    // ---------------------------------------------------------------- waiting

    /// WAIT 3, WAIT TEN MINUTES, WAIT AN HOUR: how many turns that is.
    fn how_long(&mut self, words: &[String]) -> Result<u64, &'static str> {
        // WAIT UNTIL NOON reaches here too, as WAIT {text}.
        if let Some((first, rest)) = words.split_first()
            && matches!(first.as_str(), "until" | "till" | "til")
        {
            return self.turns_until(rest);
        }
        let lx = &self.world.story().lexicon;
        let mut n: Option<u64> = None;
        let mut unit: Option<&str> = None;
        for w in words {
            if let Ok(d) = w.parse::<u64>() {
                n = Some(d);
            } else if let Some(&d) = lx.numbers.get(w) {
                n = Some(u64::from(d));
            } else if let Some(u) = lx.durations.get(w) {
                unit = Some(u.as_str());
            } else if lx.times.contains_key(w) {
                // WAIT FOR DUSK.
                return self.turns_until(words);
            } else if !matches!(w.as_str(), "for" | "of" | "about" | "around") {
                return Err("wait.how_long");
            }
        }
        if n.is_none() && unit.is_none() {
            return Err("wait.how_long");
        }
        let n = n.unwrap_or(1).max(1);
        let minutes = match unit.unwrap_or("turn") {
            "minute" => n,
            "hour" => n * 60,
            _ => return Ok(n.min(MOST_TURNS)),
        };
        let per_turn = self.world.story().clock.as_ref().map(|c| u64::from(c.per_turn)).ok_or("wait.no_clock")?;
        Ok(minutes.div_ceil(per_turn).clamp(1, MOST_TURNS))
    }

    /// WAIT UNTIL DUSK, WAIT UNTIL 9:30 PM: how many turns until then.
    fn turns_until(&mut self, words: &[String]) -> Result<u64, &'static str> {
        let Some(clock) = self.world.story().clock.clone() else { return Err("wait.no_clock") };
        let lx = &self.world.story().lexicon;
        let words: Vec<&String> = words.iter().filter(|w| !matches!(w.as_str(), "it's" | "it" | "is" | "o'clock" | "for")).collect();
        let at = match words.as_slice() {
            [w] if lx.times.contains_key(*w) => match lx.times[*w].as_str() {
                "dawn" => Some(clock.dawn),
                "dusk" => Some(clock.dusk),
                t => parse_time(t).ok(),
            },
            // "9 30 pm": the colon was lost with the punctuation.
            [h, m, rest @ ..] if h.parse::<u32>().is_ok() && m.len() == 2 && m.parse::<u32>().is_ok() => {
                let rest: String = rest.iter().map(|s| s.as_str()).collect();
                parse_time(&format!("{h}:{m}{rest}")).ok()
            }
            [] => None,
            ws => parse_time(&ws.iter().map(|s| s.as_str()).collect::<String>()).ok(),
        };
        let at = at.ok_or("wait.when")?;
        let now = self.world.state.clock % u64::from(DAY);
        let minutes = (u64::from(at) + u64::from(DAY) - now) % u64::from(DAY);
        if minutes == 0 {
            return Err("wait.already");
        }
        Ok(minutes.div_ceil(u64::from(clock.per_turn)).clamp(1, MOST_TURNS))
    }

    /// Waits up to `n` turns, stopping as soon as anything happens.
    fn wait_turns(&mut self, n: u64) -> bool {
        let Some(wait) = self.world.story().verbs.iter().position(|v| v.action == VerbAction::Builtin(BuiltinAction::Wait)) else {
            return false;
        };
        let (turn, before) = (self.world.state.turn, self.world.state.clone());
        let start = self.out.lines.len();
        let mut ok = true;
        for _ in 0..n {
            let before = self.out.lines.len();
            self.quiet_wait = true;
            ok = self.execute_command(Command { verb: wait, targets: Vec::new(), tool: None, text: None, direction: None, words: vec!["wait".into()] });
            self.quiet_wait = false;
            if !ok || self.out.lines.len() > before {
                break;
            }
        }
        if self.out.lines.len() == start {
            self.msg("wait.done", &Ctx::new());
        }
        self.squash_turns(turn, before);
        ok
    }

    /// Makes the turns since `turn` one step for UNDO, back to `before`.
    fn squash_turns(&mut self, turn: u64, before: crate::state::GameState) {
        let taken = self.world.state.turn.saturating_sub(turn);
        if taken > 0 {
            self.world.replace_undo(taken as usize, before);
        }
    }
}

/// A room name's words, lowercased.
fn name_words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(str::to_string).collect()
}
