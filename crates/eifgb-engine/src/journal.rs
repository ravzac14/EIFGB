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
            return self.msg("journal.none_in_game", &Ctx::new());
        }
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
