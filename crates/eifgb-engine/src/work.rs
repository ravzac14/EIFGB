//! Earning a living: busking, and paid errands.
//!
//! **Busking:** SING, BUSK or PLAY THE FIDDLE FOR COINS where people pay a
//! busker: a room's `busking:` says what, and a crowded room pays what a
//! crowd does (1 or 2 coins, three times). An instrument earns a coin
//! more. After `times` performances the people here have heard enough,
//! until [`BUSK_REST`] turns have passed. Kept in the room's state.
//!
//! **Jobs:** a character's `jobs:`, offered in order when the player ASKs
//! FOR WORK (or talks to them, with nothing else to say). One at a time:
//! a job is done when they're given what it's `for`, or once its `done`
//! is true while they see the player. They pay (`pays`, money, and
//! `gives`, things), think better of the player, and JOURNAL lists the
//! jobs taken and done. Kept in the character's state: `_job` (the one
//! taken) and `_jobs_done` (how many are done).

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{Busking, EntityId, EntityKind, Job, Value, PLAYER};

/// What a crowd pays, where the room doesn't say.
const CROWD_PAYS: Busking = Busking { pays: 1, spread: 1, times: 3 };
/// Turns before people who've heard enough will pay again.
const BUSK_REST: i64 = 40;

impl<H: ScriptHost> Game<H> {
    // ------------------------------------------------------------ busking

    /// SING, BUSK, PLAY THE LUTE FOR MONEY. `with`: the instrument named,
    /// if any; otherwise one the player's carrying, if they have one.
    pub(crate) fn busk(&mut self, with: Option<EntityId>, singing: bool, ctx: &Ctx) -> bool {
        if let Some(i) = with {
            if !self.world.def(i).instrument {
                self.msg("busk.not_instrument", &ctx.clone().entity("target", i));
                return false;
            }
            if !self.world.is_within(i, PLAYER) {
                self.msg("busk.not_held", &ctx.clone().entity("target", i));
                return false;
            }
        }
        let instrument = with.or_else(|| if singing { None } else { self.world.inventory_all().into_iter().find(|&t| self.world.def(t).instrument) });
        let ctx = match instrument {
            Some(i) => ctx.clone().entity("instrument", i),
            None => ctx.clone(),
        }
        .value("singing", Value::Bool(instrument.is_none()))
        .value("sang", Value::Bool(singing));
        let Some(here) = self.world.room_of(PLAYER) else { return false };
        let pays = self.world.def(here).busking.or_else(|| self.world.is_crowded(here).then_some(CROWD_PAYS));
        let Some(pays) = pays.filter(|p| p.times > 0) else {
            self.msg(if pays.is_some() { "busk.unpaid" } else { "busk.alone" }, &ctx);
            return true;
        };
        let turn = self.world.state.turn as i64;
        let (mut count, since) = (self.world.int(here, "_busked"), self.world.int(here, "_busked_at"));
        if count > 0 && turn - since >= BUSK_REST {
            count = 0;
        }
        if count >= i64::from(pays.times) {
            self.msg("busk.heard_enough", &ctx);
            return true;
        }
        if count == 0 {
            self.world.set(here, "_busked_at", Value::Int(turn));
        }
        self.world.set(here, "_busked", Value::Int(count + 1));
        let coins = i64::from(pays.pays) + self.world.state.rng.range(0, i64::from(pays.spread)) + i64::from(instrument.is_some());
        let money = self.world.money(PLAYER);
        self.world.set_money(PLAYER, money + coins);
        let amount = self.money_text(coins);
        self.msg("busk.paid", &ctx.text("amount", amount).value("coins", Value::Int(coins)).value("last", Value::Bool(count + 1 >= i64::from(pays.times))));
        true
    }

    // --------------------------------------------------------------- jobs

    /// What the job is, for the journal: its `task`, or "bring them a sack of flour".
    fn job_what(&mut self, c: EntityId, j: &Job) -> String {
        match (&j.task, j.wants) {
            (Some(t), _) => self.render_str(t, &Ctx::new().this(c)),
            (None, Some(w)) => self.msg_str("promise.what", &Ctx::new().entity("target", w)),
            (None, None) => self.msg_str("job.what", &Ctx::new().entity("npc", c)),
        }
    }

    /// ASK THE BAKER FOR WORK (or, with nobody named, whoever here has some).
    pub(crate) fn work_action(&mut self, target: Option<EntityId>, ctx: &Ctx) -> bool {
        let who = match target {
            Some(t) => t,
            None => {
                let here: Vec<EntityId> = self
                    .world
                    .ids()
                    .filter(|&c| c != PLAYER && self.world.def(c).kind == EntityKind::Character && self.world.is_active(c) && self.world.together(c, PLAYER))
                    .collect();
                let hiring: Vec<EntityId> = here.iter().copied().filter(|&c| !self.world.jobs_of(c).is_empty()).collect();
                match hiring.as_slice() {
                    [one] => *one,
                    [] => {
                        self.no_turn = true;
                        self.msg(if here.is_empty() { "job.nobody_here" } else { "job.nobody_hiring" }, ctx);
                        return false;
                    }
                    _ => {
                        self.no_turn = true;
                        self.msg("job.whom", ctx);
                        return false;
                    }
                }
            }
        };
        if self.world.def(who).kind != EntityKind::Character || who == PLAYER {
            self.msg("talk.not_person", ctx);
            return false;
        }
        let ctx = ctx.clone().entity("npc", who).entity("target", who);
        if !self.world.is_active(who) || self.world.is_asleep(who) {
            self.msg(if self.world.is_dead(who) { "talk.dead" } else { "talk.asleep" }, &ctx);
            return false;
        }
        if self.world.disposition(who) <= crate::npc::HOSTILE {
            self.msg("talk.hostile", &ctx);
            return false;
        }
        self.offer_job(who, &ctx);
        true
    }

    /// `who` offers the next job (and the player takes it on), or asks
    /// how the one they're doing is going. False when they've none to give.
    pub(crate) fn offer_job(&mut self, who: EntityId, ctx: &Ctx) -> bool {
        let jobs = self.world.jobs_of(who);
        if jobs.is_empty() {
            self.msg("job.none", ctx);
            return false;
        }
        if self.world.job_taken(who).is_some() {
            self.msg("job.waiting", ctx);
            return true;
        }
        let Some(job) = jobs.get(self.world.jobs_done(who)).cloned() else {
            self.msg("job.none_left", ctx);
            return false;
        };
        let ctx = ctx.clone().this(who);
        match &job.say {
            Some(s) => self.render_line(LineKind::Text, s, &ctx),
            None => self.msg("job.offer", &ctx),
        }
        self.world.set(who, "_job", Value::Str(job.key.clone()));
        let what = self.job_what(who, &job);
        let first = !self.world.ids().any(|c| c != who && (self.world.get(c, "_job").is_some() || self.world.int(c, "_jobs_done") > 0));
        self.msg_kind(LineKind::Notice, "job.noted", &Ctx::new().entity("npc", who).text("what", what).value("first", Value::Bool(first)));
        true
    }

    /// GIVE THE FLOUR TO THE BAKER, when it's what their job was for.
    /// None if it isn't.
    pub(crate) fn job_given(&mut self, item: EntityId, to: EntityId) -> Option<bool> {
        let job = self.world.job_taken(to).filter(|j| j.wants == Some(item))?;
        if self.world.location(item) != Some(PLAYER) {
            self.msg("give.not_held", &Ctx::new().entity("target", item));
            return Some(false);
        }
        self.world.unset(item, "worn");
        let _ = self.world.move_to(item, Some(to));
        self.job_done(to, &job, Some(item));
        Some(true)
    }

    /// The job's done: thanks, and pay.
    fn job_done(&mut self, who: EntityId, job: &Job, item: Option<EntityId>) {
        let mut ctx = Ctx::new().this(who).entity("npc", who);
        if let Some(i) = item {
            ctx = ctx.entity("target", i);
        }
        match &job.thanks {
            Some(t) => self.render_line(LineKind::Text, t, &ctx),
            None => self.msg("job.thanks", &ctx),
        }
        if job.pays > 0 {
            let money = self.world.money(PLAYER);
            self.world.set_money(PLAYER, money + job.pays);
            // They pay it out of their own money, when they keep count of it.
            if self.world.has_money(who) {
                let theirs = self.world.money(who);
                self.world.set_money(who, theirs - job.pays);
            }
            let amount = self.money_text(job.pays);
            self.msg("job.paid", &ctx.clone().text("amount", amount));
        }
        let gives: Vec<EntityId> = job.gives.iter().copied().filter(|&g| !self.world.is_within(g, PLAYER)).collect();
        if !gives.is_empty() {
            let list = self.list_a(&gives);
            self.msg("promise.reward", &ctx.clone().text("list", list));
            for g in gives {
                let _ = self.world.move_to(g, Some(PLAYER));
                self.world.state.known.insert(g);
            }
        }
        self.world.unset(who, "_job");
        let done = self.world.jobs_done(who) as i64 + 1;
        self.world.set(who, "_jobs_done", Value::Int(done));
        self.world.shift_disposition(who, 1);
        let what = self.job_what(who, job);
        self.msg_kind(LineKind::Notice, "job.done", &ctx.text("what", what));
    }

    /// After each turn: a job with `done` is finished once it's true and
    /// whoever gave it can see the player.
    pub(crate) fn jobs_turn(&mut self) {
        let givers: Vec<EntityId> = self.world.ids().filter(|&c| self.world.get(c, "_job").is_some()).collect();
        for c in givers {
            let Some(job) = self.world.job_taken(c) else { continue };
            let Some(done) = &job.done else { continue };
            if !self.world.is_active(c) || self.world.is_asleep(c) || !self.world.together(c, PLAYER) || !self.world.can_see(c, PLAYER) {
                continue;
            }
            if self.run(done, &Ctx::new().this(c)).truthy() {
                self.job_done(c, &job, None);
            }
        }
    }

    /// JOURNAL: jobs taken, and done. Whether there were any.
    pub(crate) fn journal_jobs(&mut self) -> bool {
        let mut lines: Vec<(EntityId, String, bool)> = Vec::new();
        for c in self.world.ids().collect::<Vec<_>>() {
            let jobs = self.world.jobs_of(c);
            if jobs.is_empty() {
                continue;
            }
            for j in jobs.iter().take(self.world.jobs_done(c)) {
                let what = self.job_what(c, j);
                lines.push((c, what, true));
            }
            if let Some(j) = self.world.job_taken(c) {
                let what = self.job_what(c, &j);
                lines.push((c, what, false));
            }
        }
        if lines.is_empty() {
            return false;
        }
        self.msg("journal.jobs", &Ctx::new());
        for (c, what, done) in lines {
            self.msg("journal.job", &Ctx::new().entity("npc", c).text("what", what).value("done", Value::Bool(done)));
        }
        true
    }
}

impl crate::World {
    /// A whole number kept in an entity's state, 0 when there's none.
    pub(crate) fn int(&self, id: EntityId, key: &str) -> i64 {
        match self.get(id, key) {
            Some(Value::Int(i)) => *i,
            _ => 0,
        }
    }

    pub fn jobs_of(&self, c: EntityId) -> Vec<Job> {
        self.def(c).npc.as_ref().map(|n| n.jobs.clone()).unwrap_or_default()
    }

    /// The job `c` has given the player, if any.
    pub(crate) fn job_taken(&self, c: EntityId) -> Option<Job> {
        let Some(Value::Str(key)) = self.get(c, "_job") else { return None };
        self.jobs_of(c).into_iter().find(|j| j.key == *key)
    }

    pub(crate) fn jobs_done(&self, c: EntityId) -> usize {
        self.int(c, "_jobs_done").max(0) as usize
    }

    /// Lua's `job_state`, for `x:job("flour")`: nil (not taken), "taken" or "done".
    pub fn job_state(&self, c: EntityId, key: &str) -> Option<&'static str> {
        let jobs = self.jobs_of(c);
        let i = jobs.iter().position(|j| j.key == key)?;
        if i < self.jobs_done(c) {
            Some("done")
        } else if self.job_taken(c).is_some_and(|j| j.key == key) {
            Some("taken")
        } else {
            None
        }
    }
}
