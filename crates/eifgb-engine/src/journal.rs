//! Goals (JOURNAL) and the time of day (TIME, `at` timers).
//!
//! Goals are listed once their `when` is true and done once their `done`
//! is true (or a script says so); both are checked after every action, and
//! a notice says when a goal is added or done. Which goals have been
//! added, and which are done, is kept in [`GameState::goals`], so it's
//! saved and undone with everything else.
//!
//! The clock is [`GameState::clock`]: minutes since midnight on the first
//! day. Each action moves it on by the story's `minutes_per_turn`, and
//! scripts can move it on further (`pass_time`, `set_clock`).
//!
//! [`GameState::goals`]: crate::state::GameState::goals
//! [`GameState::clock`]: crate::state::GameState::clock

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use crate::world::World;
use eifgb_ir::{time_text, Goal, Schedule, Value, DAY};

/// The time of day as scripts and messages see it.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockReading {
    /// The day, counting from 1.
    pub day: u64,
    pub hour: u32,
    pub minute: u32,
    /// Minutes after midnight.
    pub minutes: u32,
    /// "7:30 am".
    pub time: String,
    pub night: bool,
    /// "night", "morning", "afternoon" or "evening".
    pub period: &'static str,
}

impl World {
    /// The time of day, if the game keeps one.
    pub fn clock(&self) -> Option<ClockReading> {
        let c = self.story().clock.as_ref()?;
        let now = self.state.clock;
        let at = (now % u64::from(DAY)) as u32;
        Some(ClockReading {
            day: now / u64::from(DAY) + 1,
            hour: at / 60,
            minute: at % 60,
            minutes: at,
            time: time_text(at),
            night: c.is_night(at),
            period: c.period(at),
        })
    }

    /// Moves the clock on. Returns false when the game keeps no time.
    pub fn pass_time(&mut self, minutes: u64) -> bool {
        if self.story().clock.is_none() {
            return false;
        }
        self.state.clock += minutes;
        true
    }

    /// Moves the clock on to the next time it's `at` (minutes after
    /// midnight): later today, or tomorrow. Already `at` stays put.
    pub fn set_clock(&mut self, at: u32) -> bool {
        let now = self.state.clock;
        let today = now % u64::from(DAY);
        let wait = (u64::from(at) + u64::from(DAY) - today) % u64::from(DAY);
        self.pass_time(wait)
    }

    /// Whether the goal is listed in the journal: None if not (yet), else whether it's done.
    pub fn goal(&self, key: &str) -> Option<bool> {
        self.state.goals.get(key).copied()
    }

    /// Actions until a running timer is next due, or None when it isn't running.
    pub fn timer_left(&self, key: &str) -> Option<u64> {
        let due = *self.state.timers.get(key)?;
        let t = self.story().timers.iter().find(|t| t.key == key)?;
        Some(match (t.schedule, self.story().clock.as_ref()) {
            (Schedule::At(at), Some(c)) => {
                let today = self.state.clock % u64::from(DAY);
                let wait = (u64::from(at) + u64::from(DAY) - today) % u64::from(DAY);
                let wait = if wait == 0 { u64::from(DAY) } else { wait };
                wait.div_ceil(u64::from(c.per_turn))
            }
            _ => due.saturating_sub(self.state.turn),
        })
    }
}

/// Whether the time `at` (minutes after midnight) comes after `from` and no later than `to`.
pub(crate) fn passes(from: u64, to: u64, at: u32) -> bool {
    let day = u64::from(DAY);
    let mut next = from - from % day + u64::from(at);
    if next <= from {
        next += day;
    }
    next <= to
}

impl<H: ScriptHost> Game<H> {
    /// Adds goals whose `when` has come true and finishes those whose `done` has.
    pub(crate) fn update_goals(&mut self) {
        if self.world.story().goals.is_empty() || self.world.state.ending.is_some() {
            return;
        }
        let goals = self.world.story().goals.clone();
        for g in &goals {
            let listed = self.world.goal(&g.key);
            if listed == Some(true) {
                continue;
            }
            if listed.is_none() && g.when.as_ref().is_some_and(|w| !self.run(w, &Ctx::new()).truthy()) {
                continue;
            }
            if g.done.as_ref().is_some_and(|d| self.run(d, &Ctx::new()).truthy()) {
                self.finish_goal(g);
            } else if listed.is_none() {
                self.add_goal(g);
            }
        }
    }

    // -------------------------------------------------------------- scenes

    /// Begins scenes whose `when` has come true, and ends those whose `done` has.
    pub(crate) fn update_scenes(&mut self) {
        if self.world.story().scenes.is_empty() || self.world.state.ending.is_some() {
            return;
        }
        for s in self.world.story().scenes.clone() {
            match self.world.state.scenes.get(&s.key).copied() {
                Some(true) => {}
                Some(false) => {
                    if s.done.as_ref().is_some_and(|d| self.run(d, &Ctx::new()).truthy()) {
                        self.end_scene(&s.key);
                    }
                }
                None => {
                    if s.when.as_ref().is_none_or(|w| self.run(w, &Ctx::new()).truthy()) {
                        self.begin_scene(&s.key);
                    }
                }
            }
        }
    }

    /// A scene begins (if it hasn't yet): its `begins` text.
    pub(crate) fn begin_scene(&mut self, key: &str) {
        let Some(s) = self.world.story().scenes.iter().find(|s| s.key == key).cloned() else { return };
        if self.world.state.scenes.contains_key(key) {
            return;
        }
        self.world.state.scenes.insert(key.to_string(), false);
        if let Some(t) = &s.begins {
            self.render_line(LineKind::Text, t, &Ctx::new());
        }
    }

    /// A scene ends (begun or not): its `ends` text.
    pub(crate) fn end_scene(&mut self, key: &str) {
        let Some(s) = self.world.story().scenes.iter().find(|s| s.key == key).cloned() else { return };
        if self.world.state.scenes.get(key) == Some(&true) {
            return;
        }
        self.world.state.scenes.insert(key.to_string(), true);
        if let Some(t) = &s.ends {
            self.render_line(LineKind::Text, t, &Ctx::new());
        }
    }

    /// The scenes going on now, by title, for RECAP.
    pub(crate) fn scenes_now(&mut self) -> Vec<String> {
        let now: Vec<eifgb_ir::Scene> = self.world.story().scenes.iter().filter(|s| self.world.state.scenes.get(&s.key) == Some(&false)).cloned().collect();
        now.iter().map(|s| self.render_str(&s.title, &Ctx::new())).collect()
    }

    fn goal_def(&self, key: &str) -> Option<Goal> {
        self.world.story().goals.iter().find(|g| g.key == key).cloned()
    }

    /// `start_goal` from a script: lists the goal now, whatever its `when`.
    pub(crate) fn start_goal(&mut self, key: &str) {
        if let Some(g) = self.goal_def(key)
            && self.world.goal(key).is_none()
        {
            self.add_goal(&g);
        }
    }

    /// `complete_goal` from a script.
    pub(crate) fn complete_goal(&mut self, key: &str) {
        if let Some(g) = self.goal_def(key)
            && self.world.goal(key) != Some(true)
        {
            self.finish_goal(&g);
        }
    }

    fn add_goal(&mut self, g: &Goal) {
        let first = self.world.state.goals.is_empty();
        self.world.state.goals.insert(g.key.clone(), false);
        let text = self.render_str(&g.text, &Ctx::new());
        let ctx = Ctx::new().text("goal", text).value("first", Value::Bool(first));
        self.msg_kind(LineKind::Notice, "goal.new", &ctx);
    }

    fn finish_goal(&mut self, g: &Goal) {
        self.world.state.goals.insert(g.key.clone(), true);
        let text = self.render_str(&g.text, &Ctx::new());
        self.msg_kind(LineKind::Notice, "goal.done", &Ctx::new().text("goal", text));
        if g.points != 0 {
            self.world.state.score += g.points;
            self.msg_kind(LineKind::Notice, "score.gained", &Ctx::new().value("points", Value::Int(g.points)));
        }
    }

    /// JOURNAL: the goals still to do, then those done.
    pub(crate) fn journal(&mut self) {
        if self.world.story().goals.is_empty() {
            let promised = self.journal_promises();
            let worked = self.journal_jobs();
            if !promised && !worked && self.world.state.notes.is_empty() {
                return self.msg("journal.none_in_game", &Ctx::new());
            }
            return self.show_notes(false);
        }
        self.journal_goals();
        self.journal_promises();
        self.journal_jobs();
        self.show_notes(false);
    }

    /// What JOURNAL would say now (goals, promises, notes), for a panel
    /// beside the story. Nothing in the game changes: it's said on a copy
    /// of the state, which is put back after. Empty when there's nothing
    /// to list yet.
    pub fn journal_lines(&mut self) -> Vec<crate::game::Line> {
        let state = self.world.state.clone();
        let out = std::mem::take(&mut self.out);
        let something = !self.world.state.notes.is_empty() || !self.world.state.goals.is_empty() || self.world.ids().any(|c| self.promise_state(c).is_some() || self.world.get(c, "_job").is_some() || self.world.int(c, "_jobs_done") > 0);
        if something {
            self.journal();
        }
        let lines = std::mem::replace(&mut self.out, out).lines;
        self.world.state = state;
        lines
    }

    fn journal_goals(&mut self) {
        let goals = self.world.story().goals.clone();
        let (mut open, mut done) = (Vec::new(), Vec::new());
        for g in &goals {
            match self.world.goal(&g.key) {
                Some(false) => open.push(g),
                Some(true) => done.push(g),
                None => {}
            }
        }
        if open.is_empty() && done.is_empty() {
            return self.msg("journal.empty", &Ctx::new());
        }
        if open.is_empty() {
            self.msg("journal.all_done", &Ctx::new());
        } else {
            self.msg("journal.goals", &Ctx::new());
            for g in open {
                let text = self.render_str(&g.text, &Ctx::new());
                self.msg("journal.goal", &Ctx::new().text("goal", text));
            }
        }
        if !done.is_empty() {
            self.msg("journal.done", &Ctx::new());
            for g in done {
                let text = self.render_str(&g.text, &Ctx::new());
                self.msg("journal.done_goal", &Ctx::new().text("goal", text));
            }
        }
    }

    // ------------------------------------------------------------- notes

    /// A note, if the line is one: NOTE THE CODE IS 4512 (or the lexicon's
    /// other `note_words`). The rest of the line, as typed.
    pub(crate) fn note_in(&self, line: &str) -> Option<String> {
        let t = line.trim();
        let lower = t.to_lowercase();
        // NOTE on its own: how to write one, unless something here is called a note.
        if self.world.story().lexicon.note_words.contains(&lower) && self.grammar.name_matches(&self.world, std::slice::from_ref(&lower)).is_empty() {
            return Some(String::new());
        }
        let lead = self.world.story().lexicon.note_words.iter().filter(|w| {
            lower.starts_with(w.as_str()) && lower[w.len()..].chars().next().is_some_and(|c| c.is_whitespace() || c == ':')
        });
        let w = lead.max_by_key(|w| w.len())?;
        let rest = t[w.len()..].trim_start_matches([' ', ':', '\t']).trim();
        // "note that the code…": the "that" isn't part of it.
        let rest = rest.strip_prefix("that ").or_else(|| rest.strip_prefix("That ")).unwrap_or(rest).trim();
        Some(rest.to_string())
    }

    pub(crate) fn add_note(&mut self, note: String) {
        if note.is_empty() {
            return self.msg("note.what", &Ctx::new());
        }
        let first = self.world.state.notes.is_empty();
        self.world.state.notes.push(note);
        let n = self.world.state.notes.len() as i64;
        self.msg_kind(LineKind::Notice, "note.added", &Ctx::new().value("n", Value::Int(n)).value("first", Value::Bool(first)));
    }

    /// NOTES, or the end of JOURNAL. `asked`: said even when there are none.
    pub(crate) fn show_notes(&mut self, asked: bool) {
        let notes = self.world.state.notes.clone();
        if notes.is_empty() {
            if asked {
                self.msg("notes.none", &Ctx::new());
            }
            return;
        }
        self.msg("journal.notes", &Ctx::new());
        for (i, n) in notes.iter().enumerate() {
            self.msg("journal.note", &Ctx::new().value("n", Value::Int(i as i64 + 1)).text("note", n.clone()));
        }
    }

    /// ERASE NOTE 2, ERASE NOTES.
    pub(crate) fn erase_note(&mut self, which: &str) {
        let which = which.trim().trim_start_matches('#');
        if which.is_empty() || matches!(which, "all" | "everything" | "notes" | "them" | "all notes") {
            if self.world.state.notes.is_empty() {
                return self.msg("notes.none", &Ctx::new());
            }
            self.world.state.notes.clear();
            return self.msg("notes.cleared", &Ctx::new());
        }
        let n = which.parse::<usize>().ok().or_else(|| self.world.story().lexicon.numbers.get(which).map(|&n| n as usize));
        match n.filter(|&n| n >= 1 && n <= self.world.state.notes.len()) {
            Some(n) => {
                self.world.state.notes.remove(n - 1);
                self.msg("note.erased", &Ctx::new().value("n", Value::Int(n as i64)));
            }
            None => self.msg("note.no_such", &Ctx::new().text("which", which.to_string())),
        }
    }

    /// TIME.
    pub(crate) fn tell_time(&mut self) {
        let Some(c) = self.world.clock() else {
            return self.msg("clock.none", &Ctx::new());
        };
        let ctx = Ctx::new()
            .text("time", c.time)
            .text("period", c.period)
            .value("day", Value::Int(c.day as i64))
            .value("hour", Value::Int(i64::from(c.hour)))
            .value("minute", Value::Int(i64::from(c.minute)));
        self.msg("clock.now", &ctx);
        if c.night
            && let Some(m) = self.world.moon()
        {
            self.msg("clock.moon", &Ctx::new().text("moon", m));
        }
    }

    /// Each action moves the clock on.
    pub(crate) fn tick_clock(&mut self) {
        if let Some(n) = self.world.story().clock.as_ref().map(|c| c.per_turn) {
            self.world.pass_time(u64::from(n));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::passes;

    #[test]
    fn a_time_passes_once_between_two_moments() {
        // 18:50 → 19:00 passes 19:00, but not 18:50 (already there) or 19:01.
        assert!(passes(18 * 60 + 50, 19 * 60, 19 * 60));
        assert!(!passes(18 * 60 + 50, 19 * 60, 18 * 60 + 50));
        assert!(!passes(18 * 60 + 50, 19 * 60, 19 * 60 + 1));
        // Over midnight, into the next day.
        assert!(passes(23 * 60, 24 * 60 + 30, 0));
        assert!(passes(23 * 60, 24 * 60 + 30, 15));
        assert!(!passes(23 * 60, 24 * 60 + 30, 12 * 60));
    }
}
