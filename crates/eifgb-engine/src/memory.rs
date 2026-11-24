//! The past and the end: flashbacks the player plays through (what they
//! carry set aside, somewhere else for a while, and what they leave there
//! found in the present afterwards), and the epilogue after an ending.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{EndingId, EntityId, EntityKind, Value, PLAYER};

impl crate::World {
    /// The flashback the player is in.
    pub fn in_flashback(&self) -> Option<String> {
        match self.get(PLAYER, "_fb") {
            Some(Value::Str(k)) => Some(k.clone()),
            _ => None,
        }
    }
}

impl<H: ScriptHost> Game<H> {
    /// Into the memory: what's carried set aside, and the player there.
    pub(crate) fn start_flashback(&mut self, key: &str) {
        if self.world.in_flashback().is_some() {
            return;
        }
        let Some(fb) = self.world.story().flashbacks.iter().find(|f| f.key == key).cloned() else { return };
        let Some(from) = self.world.room_of(PLAYER) else { return };
        let items = self.world.contents(PLAYER);
        let keys: Vec<Value> = items.iter().map(|&t| Value::Str(self.world.def(t).key.clone())).collect();
        for &t in &items {
            let _ = self.world.move_to(t, None);
        }
        self.world.unset(PLAYER, "_riding");
        self.world.set(PLAYER, "_fb", Value::Str(key.to_string()));
        self.world.set(PLAYER, "_fb_from", Value::Str(self.world.def(from).key.clone()));
        self.world.set(PLAYER, "_fb_items", Value::List(keys));
        // The turn it began in doesn't count.
        self.world.set(PLAYER, "_fb_turn", Value::Int(self.world.state.turn as i64 + 1));
        let _ = self.world.move_to(PLAYER, Some(fb.at));
        for &t in &fb.carries {
            let _ = self.world.move_to(t, Some(PLAYER));
        }
        match &fb.begins {
            Some(t) => self.render_line(LineKind::Text, t, &Ctx::new()),
            None => self.msg("flashback.begins", &Ctx::new()),
        }
        self.look();
    }

    /// Back to the present: what was left in the past is found now, where
    /// it's `now`, and the player has what they had.
    pub(crate) fn end_flashback(&mut self) {
        let Some(key) = self.world.in_flashback() else { return };
        let fb = self.world.story().flashbacks.iter().find(|f| f.key == key).cloned();
        // What they carry stays in the past, where they stand.
        let here = self.world.room_of(PLAYER);
        for t in self.world.contents(PLAYER) {
            let _ = self.world.move_to(t, here);
        }
        let kept: Vec<EntityId> = match self.world.get(PLAYER, "_fb_items") {
            Some(Value::List(l)) => l.iter().filter_map(|v| if let Value::Str(k) = v { self.world.id(k) } else { None }).collect(),
            _ => Vec::new(),
        };
        // The past's things, in the present.
        let pairs: Vec<(EntityId, EntityId)> = self.world.ids().filter_map(|t| Some((t, self.world.def(t).now?))).filter(|(t, _)| self.world.def(*t).kind == EntityKind::Object).collect();
        for (then, now) in pairs {
            if kept.contains(&now) {
                continue;
            }
            if let Some(room_now) = self.world.room_of(then).and_then(|r| self.world.def(r).now) {
                let _ = self.world.move_to(now, Some(room_now));
            }
        }
        let back = match self.world.get(PLAYER, "_fb_from") {
            Some(Value::Str(k)) => self.world.id(k),
            _ => None,
        };
        let _ = self.world.move_to(PLAYER, back);
        for t in kept {
            let _ = self.world.move_to(t, Some(PLAYER));
        }
        for f in ["_fb", "_fb_from", "_fb_items", "_fb_turn"] {
            self.world.unset(PLAYER, f);
        }
        match fb.and_then(|f| f.ends) {
            Some(t) => self.render_line(LineKind::Text, &t, &Ctx::new()),
            None => self.msg("flashback.ends", &Ctx::new()),
        }
        self.look();
    }

    /// Each turn: whether the memory is over.
    pub(crate) fn flashback_turn(&mut self) {
        let Some(key) = self.world.in_flashback() else { return };
        let Some(fb) = self.world.story().flashbacks.iter().find(|f| f.key == key).cloned() else { return };
        let began = match self.world.get(PLAYER, "_fb_turn") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        };
        let over = fb.turns.is_some_and(|n| self.world.state.turn as i64 - began >= i64::from(n)) || fb.until.as_ref().is_some_and(|c| self.run(c, &Ctx::new()).truthy());
        if over && self.end_requested.is_none() {
            self.end_flashback();
        }
    }

    /// Looking at something that brings back a memory, the first time.
    pub(crate) fn remembered_by(&mut self, t: EntityId) {
        let Some(key) = self.world.def(t).flashback.clone() else { return };
        if self.world.flag(t, "_remembered") || self.world.in_flashback().is_some() {
            return;
        }
        self.world.set(t, "_remembered", Value::Bool(true));
        self.start_flashback(&key);
    }

    /// After an ending: the epilogue lines for it whose conditions hold.
    pub(crate) fn epilogue(&mut self, ending: EndingId) {
        let lines = self.world.story().epilogue.clone();
        for l in lines {
            if !l.endings.is_empty() && !l.endings.contains(&ending) {
                continue;
            }
            if let Some(c) = &l.when
                && !self.run(c, &Ctx::new()).truthy()
            {
                continue;
            }
            self.render_line(LineKind::Ending, &l.text, &Ctx::new());
        }
    }
}
