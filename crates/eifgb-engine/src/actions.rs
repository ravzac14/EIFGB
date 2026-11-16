//! Built-in behaviour for the standard library's verbs. Every reply
//! comes from the message catalog, so authors can override any of it
//! and other languages can translate it.
//!
//! Each action returns whether it succeeded. `after_` handlers run only
//! on success.

use crate::game::{Game, LineKind};
use crate::parser::Command;
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{BuiltinAction, EntityId, EntityKind, Trait, Value, PLAYER};

impl<H: ScriptHost> Game<H> {
    pub(crate) fn builtin(&mut self, a: BuiltinAction, target: Option<EntityId>, cmd: &Command, ctx: &Ctx) -> bool {
        use BuiltinAction as A;
        match (a, target) {
            (A::Look, _) => {
                self.look();
                true
            }
            (A::Inventory, _) => self.inventory(),
            (A::Wait, _) => {
                self.msg("wait.done", ctx);
                true
            }
            (A::Go, _) => self.go(cmd, ctx),
            (A::Examine, Some(t)) => self.examine(t),
            (A::Take, Some(t)) => self.take(t, ctx),
            (A::Drop, Some(t)) => self.drop_thing(t, ctx),
            (A::PutIn, Some(t)) => self.put(t, cmd.tool, false, ctx),
            (A::PutOn, Some(t)) => self.put(t, cmd.tool, true, ctx),
            (A::Open, Some(t)) => self.open(t, ctx),
            (A::Close, Some(t)) => self.close(t, ctx),
            (A::Unlock, Some(t)) => self.lock_or_unlock(t, cmd.tool, false),
            (A::Lock, Some(t)) => self.lock_or_unlock(t, cmd.tool, true),
            (A::Search, Some(t)) => self.search(t, ctx),
            (A::Talk, Some(t)) => self.talk(t, ctx),
            (A::Wear, Some(t)) => self.wear(t, ctx),
            (A::TakeOff, Some(t)) => self.take_off(t, ctx),
            (A::Eat, Some(t)) => self.eat(t, ctx),
            _ => {
                self.msg("action.nothing", ctx);
                false
            }
        }
    }

    // ------------------------------------------------------------ looking

    pub(crate) fn look(&mut self) {
        let here = self.world.here();
        let (name, description, exits) = {
            let d = self.world.def(here);
            (d.name.clone(), d.description.clone(), d.exits.clone())
        };
        self.push(LineKind::Title, name);
        if let Some(d) = description {
            self.render_line(LineKind::Text, &d, &Ctx::new().this(here));
        }
        let things = self.listable(here);
        if !things.is_empty() {
            let list = self.list_a(&things);
            self.msg("look.you_see", &Ctx::new().text("list", list));
        }
        for c in self.world.contents(here) {
            if !self.world.is_hidden(c) && c != PLAYER {
                self.describe_contents(c);
            }
        }
        let ways: Vec<String> = exits
            .iter()
            .filter(|x| x.to.is_some())
            .map(|x| self.grammar.direction_word(x.direction))
            .collect();
        if ways.is_empty() {
            self.msg("look.no_exits", &Ctx::new());
        } else {
            let list = (self.grammar.lang().list_and)(&ways);
            self.msg("look.exits", &Ctx::new().text("list", list));
        }
    }

    /// Things worth naming in a room description or a list of contents.
    fn listable(&self, holder: EntityId) -> Vec<EntityId> {
        self.world
            .contents(holder)
            .into_iter()
            .filter(|&e| e != PLAYER && !self.world.is_hidden(e) && !self.world.has(e, Trait::Scenery))
            .collect()
    }

    fn describe_contents(&mut self, holder: EntityId) {
        if self.world.def(holder).kind != EntityKind::Object || !self.world.exposes_contents(holder) {
            return;
        }
        let items = self.listable(holder);
        if items.is_empty() {
            return;
        }
        let key = if self.world.has(holder, Trait::Supporter) { "examine.on_top" } else { "examine.contents" };
        let list = self.list_a(&items);
        self.msg(key, &Ctx::new().entity("target", holder).text("list", list));
    }

    fn examine(&mut self, t: EntityId) -> bool {
        let def = self.world.def(t);
        if def.kind == EntityKind::Room {
            self.look();
            return true;
        }
        match def.description.clone() {
            Some(d) => self.render_line(LineKind::Text, &d, &Ctx::new().this(t)),
            None if t == PLAYER => self.msg("examine.self", &Ctx::new()),
            None => self.msg("examine.default", &Ctx::new().entity("target", t)),
        }
        self.describe_contents(t);
        true
    }

    fn search(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let items = if self.world.exposes_contents(t) { self.listable(t) } else { Vec::new() };
        if items.is_empty() {
            self.msg("search.nothing", ctx);
        } else {
            let list = self.list_a(&items);
            self.msg("search.contents", &ctx.clone().text("list", list));
        }
        true
    }

    fn inventory(&mut self) -> bool {
        let items = self.world.inventory();
        if items.is_empty() {
            self.msg("inventory.empty", &Ctx::new());
        } else {
            let list = self.list_a(&items);
            self.msg("inventory.list", &Ctx::new().text("list", list));
        }
        true
    }

    // ----------------------------------------------------------- movement

    fn go(&mut self, cmd: &Command, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let Some(dir) = cmd.direction else {
            self.msg("go.no_exit", ctx);
            return false;
        };
        let Some(exit) = self.world.exit(here, dir).cloned() else {
            self.msg("go.no_exit", ctx);
            return false;
        };
        let blocked = |g: &mut Self, key: &str| {
            match &exit.blocked {
                Some(t) => g.render_line(LineKind::Text, t, &Ctx::new().this(here)),
                None => g.msg(key, &Ctx::new()),
            }
            false
        };
        let Some(to) = exit.to else { return blocked(self, "go.no_exit") };
        if let Some(when) = &exit.when
            && !self.run(when, &Ctx::new().this(here)).truthy()
        {
            return blocked(self, "go.blocked");
        }
        if let Some(door) = exit.through
            && !self.world.is_open(door)
        {
            if self.world.flag(door, "locked") {
                self.msg("open.locked", &Ctx::new().entity("target", door));
                return false;
            }
            // An unlocked door opens itself on the way through, which saves a step.
            self.msg_kind(LineKind::Notice, "go.opening_first", &Ctx::new().entity("door", door));
            self.world.set(door, "open", Value::Bool(true));
        }
        self.room_event(here, "leave");
        if self.world.move_to(PLAYER, Some(to)).is_err() {
            return false;
        }
        self.look();
        self.room_event(to, "enter");
        true
    }

    // ---------------------------------------------------- taking and dropping

    fn take(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let w = &self.world;
        let key = if t == PLAYER {
            Some("take.self")
        } else if w.location(t) == Some(PLAYER) {
            Some("take.already")
        } else if w.def(t).kind == EntityKind::Character {
            Some("take.person")
        } else if w.def(t).kind == EntityKind::Room || w.has(t, Trait::Scenery) {
            Some("take.scenery")
        } else if !w.has(t, Trait::Portable) {
            Some("take.fixed")
        } else if w.def(PLAYER).capacity.is_some_and(|cap| w.inventory().len() >= cap as usize) {
            Some("take.full")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
        }
        let _ = self.world.move_to(t, Some(PLAYER));
        self.world.unset(t, "hidden");
        self.msg("take.done", ctx);
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
        let w = &self.world;
        let key = if dest == t || w.is_within(dest, t) {
            Some("put.into_self")
        } else if w.location(t) != Some(PLAYER) {
            Some("put.not_held")
        } else if on && !w.has(dest, Trait::Supporter) {
            Some("put.not_supporter")
        } else if !on && !w.has(dest, Trait::Container) {
            Some("put.not_container")
        } else if !on && !w.is_open(dest) {
            Some("put.closed")
        } else if w.def(dest).capacity.is_some_and(|cap| w.contents(dest).len() >= cap as usize) {
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
        let key = if !self.world.has(t, Trait::Openable) {
            Some("open.cannot")
        } else if self.world.flag(t, "open") {
            Some("open.already")
        } else if self.world.flag(t, "locked") {
            Some("open.locked")
        } else {
            None
        };
        if let Some(k) = key {
            self.msg(k, ctx);
            return false;
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

    // ------------------------------------------------------------- people

    fn talk(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.def(t).kind != EntityKind::Character {
            self.msg("talk.not_person", ctx);
            return false;
        }
        match self.world.def(t).talk {
            Some(c) => {
                let key = self.world.story().choice(c).key.clone();
                self.choice_requested = Some(key);
                true
            }
            None => {
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
