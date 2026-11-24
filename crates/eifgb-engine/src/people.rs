//! Characters in the turn loop: their turns, fighting, and trading.
//!
//! [`crate::npc`] decides what characters do and changes the world. This
//! module runs their turns, describes what the player can see, and
//! carries out ATTACK, BUY, SELL, WARES and HEALTH.

use crate::game::{Game, LineKind};
use crate::npc::NpcEvent;
use crate::script::{Ctx, ScriptHost, ScriptValue};
use crate::world::{Fit, World};
use eifgb_ir::{Behavior, Code, EntityId, EntityKind, Phase, Value, PLAYER};

impl<H: ScriptHost> Game<H> {
    /// Every active character takes a turn, in the order they're declared.
    pub(crate) fn npc_turns(&mut self) {
        let npcs: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| self.world.def(id).npc.is_some())
            .collect();
        for npc in npcs {
            if self.end_requested.is_some() {
                return;
            }
            // Woken by something earlier this turn: say so before it acts.
            self.report_woken();
            if !self.world.is_active(npc) || self.world.is_asleep(npc) || self.world.is_bound(npc) || self.world.riding() == Some(npc) {
                continue;
            }
            // Gone where a note said: on its way, or waiting there.
            if self.lured_turn(npc) {
                continue;
            }
            // A body of its own, left in plain sight: raising the alarm is
            // its whole turn, whatever its behaviour or script.
            if let Some(event) = self.world.discover_bodies(npc) {
                self.narrate(&event);
                self.check_defeats();
                continue;
            }
            match self.world.def(npc).npc.as_ref().and_then(|n| n.act.clone()) {
                Some(code) => {
                    self.run(&code, &Ctx::new().this(npc));
                }
                None => {
                    for event in self.world.npc_behave(npc) {
                        self.narrate(&event);
                    }
                }
            }
            self.check_defeats();
        }
    }

    /// Characters talking among themselves: at most one line per room each
    /// turn, by a character with someone awake to talk to, who can't see
    /// the player. The player overhears lines in their own room (unseen),
    /// loud ones from the next room, and quiet ones from rooms they're
    /// EAVESDROPping on.
    pub(crate) fn chatter(&mut self) {
        let here = self.world.here();
        let listening = std::mem::take(&mut self.listening);
        let mut heard = false;
        let mut rooms_done: Vec<EntityId> = Vec::new();
        let talkers: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| self.world.def(id).npc.as_ref().is_some_and(|n| !n.chatter.is_empty()))
            .collect();
        for npc in talkers {
            if !self.world.is_active(npc) || self.world.is_asleep(npc) || self.world.is_bound(npc) {
                continue;
            }
            let Some(room) = self.world.room_of(npc) else { continue };
            if rooms_done.contains(&room) {
                continue;
            }
            let partner = (0..self.world.story().entities.len() as u32).map(EntityId).find(|&c| {
                c != npc
                    && c != PLAYER
                    && self.world.def(c).kind == EntityKind::Character
                    && self.world.is_active(c)
                    && !self.world.is_asleep(c)
                    && self.world.room_of(c) == Some(room)
            });
            let Some(partner) = partner else { continue };
            // They hush when they can see the player.
            if self.world.together(npc, PLAYER) && self.world.can_see(npc, PLAYER) {
                continue;
            }
            let Some((index, text, loud, repeat)) = self.next_chatter(npc) else { continue };
            rooms_done.push(room);
            let line = self.render_str(&text, &Ctx::new().this(npc));
            if room == here {
                // Overheard, unseen: by name, unless it's too dark to tell.
                let dark = self.world.in_darkness();
                let mut ctx = Ctx::new().text("line", line);
                ctx = if dark { ctx.text("npc", "someone").text("listener", "someone else") } else { ctx.entity("npc", npc).entity("listener", partner) };
                self.msg("chatter.overheard", &ctx);
                if !dark {
                    self.hear_voice(npc);
                }
                heard = true;
                self.mark_said(npc, index, repeat);
            } else if self.world.def(here).hears.contains(&room) {
                // A whispering gallery: as clear as if they were here.
                self.msg("chatter.carried", &Ctx::new().text("line", line));
                heard = true;
                self.mark_said(npc, index, repeat);
            } else if let Some(dir) = self.world.def(here).exits.iter().find(|e| e.to == Some(room)).map(|e| e.direction)
                && (loud || listening.contains(&room))
            {
                let from = self.msg_str(&format!("from.{}", dir.key()), &Ctx::new());
                self.msg("chatter.nearby", &Ctx::new().text("line", line).text("from", from));
                self.hear_voice(npc);
                heard = true;
                self.mark_said(npc, index, repeat);
            }
        }
        if !listening.is_empty() && !heard {
            self.msg("eavesdrop.nothing", &Ctx::new());
        }
    }

    /// A line the player heard is done with, unless it repeats. Lines nobody
    /// heard come round again, so a clue isn't lost for good.
    fn mark_said(&mut self, npc: EntityId, index: usize, repeat: bool) {
        if repeat {
            return;
        }
        let mut l = match self.world.get(npc, "_said") {
            Some(Value::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        l.push(Value::Int(index as i64));
        self.world.set(npc, "_said", Value::List(l));
    }

    /// The next line `npc` will say: the first, from where it left off,
    /// that the player hasn't heard (unless it repeats) and whose `when` holds.
    fn next_chatter(&mut self, npc: EntityId) -> Option<(usize, eifgb_ir::Text, bool, bool)> {
        let lines = self.world.def(npc).npc.as_ref()?.chatter.clone();
        let n = lines.len();
        let start = match self.world.get(npc, "_chatter") {
            Some(Value::Int(i)) => *i as usize,
            _ => 0,
        };
        let said: Vec<i64> = match self.world.get(npc, "_said") {
            Some(Value::List(l)) => l.iter().filter_map(|v| if let Value::Int(i) = v { Some(*i) } else { None }).collect(),
            _ => Vec::new(),
        };
        for k in 0..n {
            let i = (start + k) % n;
            let line = &lines[i];
            if !line.repeat && said.contains(&(i as i64)) {
                continue;
            }
            if let Some(w) = &line.when
                && !self.run(w, &Ctx::new().this(npc)).truthy()
            {
                continue;
            }
            self.world.set(npc, "_chatter", Value::Int((i + 1) as i64));
            return Some((i, line.text.clone(), line.loud, line.repeat));
        }
        None
    }

    /// EAVESDROP / LISTEN AT THE DOOR / LISTEN NORTH: hears quiet talk in
    /// the room behind (or every room next door) at the end of this turn.
    pub(crate) fn eavesdrop_action(&mut self, door: Option<EntityId>, dir: Option<eifgb_ir::Direction>, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let exits = self.world.def(here).exits.clone();
        let rooms: Vec<EntityId> = match (door, dir) {
            (Some(d), _) => exits.iter().filter(|e| e.through == Some(d)).filter_map(|e| e.to).collect(),
            (None, Some(dir)) => exits.iter().filter(|e| e.direction == dir).filter_map(|e| e.to).collect(),
            (None, None) => exits.iter().filter_map(|e| e.to).collect(),
        };
        if rooms.is_empty() {
            self.no_turn = true;
            self.msg("eavesdrop.no_way", ctx);
            return false;
        }
        match (door, dir) {
            (Some(d), _) => self.msg("eavesdrop.door", &ctx.clone().entity("door", d)),
            (None, Some(d)) => {
                let from = self.msg_str(&format!("from.{}", d.key()), &Ctx::new());
                self.msg("eavesdrop.direction", &ctx.clone().text("from", from))
            }
            (None, None) => self.msg("eavesdrop.all", ctx),
        }
        self.listening = rooms;
        true
    }

    /// "The wolf is caught in the snare!" for traps sprung where the player is.
    pub(crate) fn report_traps(&mut self) {
        let here = self.world.here();
        for (who, trap) in std::mem::take(&mut self.world.sprung) {
            if who == PLAYER {
                self.msg("trap.catches_you", &Ctx::new().entity("trap", trap));
            } else if self.world.room_of(who) == Some(here) {
                self.msg("trap.catches", &Ctx::new().entity("npc", who).entity("trap", trap));
            }
        }
    }

    /// SET SNARE: arms a trap, here (dropping it if held).
    pub(crate) fn set_trap(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.def(t).trap.is_none() {
            self.msg("trap.not_trap", ctx);
            return false;
        }
        if self.world.flag(t, "armed") {
            self.msg("trap.already", ctx);
            return false;
        }
        let here = self.world.here();
        self.world.unset(t, "worn");
        let _ = self.world.move_to(t, Some(here));
        self.world.set(t, "armed", Value::Bool(true));
        self.world.set(t, "_players", Value::Bool(true));
        self.world.learn(t);
        self.msg("trap.set", ctx);
        true
    }

    /// DISARM SNARE: makes an armed trap safe.
    pub(crate) fn disarm_trap(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        if self.world.def(t).trap.is_none() || !self.world.flag(t, "armed") {
            self.msg("trap.not_set", ctx);
            return false;
        }
        self.world.set(t, "armed", Value::Bool(false));
        self.world.unset(t, "_players");
        self.msg("trap.disarm", ctx);
        true
    }

    /// "The guard wakes with a start!" for anyone woken where the player is.
    pub(crate) fn report_woken(&mut self) {
        let here = self.world.here();
        for id in std::mem::take(&mut self.world.woken) {
            self.world.unset(id, "_caught_asleep");
            if self.world.room_of(id) == Some(here) && self.world.is_active(id) {
                self.msg("npc.wakes", &Ctx::new().entity("npc", id));
            }
        }
    }

    /// SNEAK NORTH: moves like GO, but without waking anyone where it arrives.
    pub(crate) fn sneak_action(&mut self, cmd: &crate::parser::Command, ctx: &Ctx) -> bool {
        self.sneaking = true;
        let moved = self.go(cmd, ctx);
        self.sneaking = false;
        moved
    }

    /// Describes what a character did, if the player is there to see it.
    pub(crate) fn narrate(&mut self, event: &NpcEvent) {
        let here = self.world.here();
        // In the dark, whoever it is, it's just "something"; but the player
        // knows their own companions.
        let dark = self.world.in_darkness();
        let known: Vec<EntityId> = (0..self.world.story().entities.len() as u32).map(EntityId).filter(|&c| self.world.companion(c)).collect();
        let who = |c: Ctx, key: &str, id: EntityId| if dark && id != PLAYER && !known.contains(&id) { c.text(key, "something") } else { c.entity(key, id) };
        match *event {
            NpcEvent::Moved { npc, from, to, direction } => {
                if from == here {
                    let dir = self.grammar.direction_word(direction);
                    self.msg("npc.leaves", &who(Ctx::new(), "npc", npc).text("direction", dir));
                } else if to == here {
                    self.msg("npc.arrives", &who(Ctx::new(), "npc", npc));
                }
            }
            NpcEvent::Attacked { attacker, target, weapon, tolerated, ref attitude, .. } => {
                let mut ctx = who(who(Ctx::new(), "npc", attacker), "target", target);
                if let Some(w) = weapon {
                    ctx = ctx.entity("weapon", w);
                }
                if target == PLAYER {
                    self.world.learn(attacker);
                    self.msg("npc.attacks_you", &ctx);
                } else if attacker == PLAYER {
                    self.world.learn(target);
                    if self.world.flag(target, "_caught_asleep") {
                        self.world.unset(target, "_caught_asleep");
                        self.msg("attack.sneak", &ctx);
                    }
                    self.msg("attack.done", &ctx);
                    if self.world.is_invincible(target) {
                        self.msg("attack.invincible", &ctx);
                    }
                } else if self.world.room_of(attacker) == Some(here) {
                    self.msg("npc.attacks", &ctx);
                } else if let Some(room) = self.world.room_of(attacker) {
                    self.hear(room, "npc.fight_nearby");
                }
                // The whole faction thinks less of the player.
                if let Some(a) = attitude {
                    let words = a.faction.replace('_', " ");
                    let key = match (a.left <= 0, a.disguise.as_deref() == Some(a.faction.as_str())) {
                        (true, true) => "faction.sees_through",
                        (true, false) => "faction.turns",
                        (false, _) => "faction.wary",
                    };
                    self.msg(key, &Ctx::new().text("faction", words).entity("target", target));
                }
                // Patience wearing thin: worth a warning, if the player is there.
                if tolerated && self.world.room_of(target) == Some(here) && self.world.health(target).is_some_and(|h| h > 0) {
                    let key = if attacker == PLAYER { "npc.losing_patience_you" } else { "npc.losing_patience" };
                    self.msg(key, &Ctx::new().entity("target", target).entity("npc", attacker));
                }
            }
            NpcEvent::Fed { npc, food, room, finished } => {
                if room == here {
                    let key = if finished { "npc.finishes" } else { "npc.feeds" };
                    self.msg(key, &Ctx::new().entity("npc", npc).entity("target", food));
                } else {
                    self.hear(room, "npc.eating_nearby");
                }
            }
            NpcEvent::Struggled { npc, trap, room, free } => {
                if room == here {
                    let key = if free { "trap.breaks_free" } else { "trap.struggles" };
                    self.msg(key, &who(Ctx::new(), "npc", npc).entity("trap", trap));
                }
            }
            NpcEvent::Fled { npc, from, to, direction } => {
                if from == here {
                    let dir = self.grammar.direction_word(direction);
                    self.msg("npc.flees", &who(Ctx::new(), "npc", npc).text("direction", dir));
                } else if to == here {
                    self.msg("npc.arrives", &who(Ctx::new(), "npc", npc));
                }
            }
            NpcEvent::Discovered { npc, body, room, ref attitude } => {
                if room == here && !self.world.player_hidden() {
                    self.msg("npc.discovers", &Ctx::new().entity("npc", npc).entity("target", body));
                    if let Some(a) = attitude {
                        let words = a.faction.replace('_', " ");
                        let key = match (a.left <= 0, a.disguise.as_deref() == Some(a.faction.as_str())) {
                            (true, true) => "faction.sees_through",
                            (true, false) => "faction.turns",
                            (false, _) => "faction.wary",
                        };
                        self.msg(key, &Ctx::new().text("faction", words));
                    }
                } else {
                    self.hear(room, "npc.alarm_nearby");
                }
            }
            NpcEvent::Fetched { npc, item, room, brought } => {
                if room == here {
                    let key = if brought { "npc.brings" } else { "npc.grabs" };
                    self.msg(key, &Ctx::new().entity("npc", npc).entity("target", item));
                    // At the player's feet: they know where it is, light or dark.
                    if brought {
                        self.world.state.located.insert(item, here);
                    }
                }
            }
            NpcEvent::Searched { npc, room } => {
                if room == here {
                    self.msg("npc.searches", &Ctx::new().entity("npc", npc));
                }
            }
            NpcEvent::Ate { npc, corpse, room } => {
                if room == here {
                    self.msg("npc.eats", &Ctx::new().entity("npc", npc).entity("target", corpse));
                } else {
                    self.hear(room, "npc.eating_nearby");
                }
            }
        }
    }

    /// Something in a neighbouring room can be heard, though not seen:
    /// at most one such message per turn.
    fn hear(&mut self, room: EntityId, key: &str) {
        let here = self.world.here();
        let Some(dir) = self.world.def(here).exits.iter().find(|e| e.to == Some(room)).map(|e| e.direction) else {
            return;
        };
        if self.heard_nearby == Some(self.world.state.turn) {
            return;
        }
        self.heard_nearby = Some(self.world.state.turn);
        let dir = self.grammar.direction_word(dir);
        self.msg(key, &Ctx::new().text("direction", dir));
    }

    /// Characters whose health has run out are defeated, and a player
    /// whose health has run out dies.
    pub(crate) fn check_defeats(&mut self) {
        let fallen: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| {
                self.world.def(id).kind == EntityKind::Character
                    && self.world.room_of(id).is_some()
                    && !self.world.is_dead(id)
                    && self.world.health(id).is_some_and(|h| h <= 0)
            })
            .collect();
        for id in fallen {
            self.defeat(id);
        }
        if self.world.health(PLAYER).is_some_and(|h| h <= 0) && self.end_requested.is_none() {
            for code in self.global_handlers(Phase::On, "died") {
                self.run(&code, &Ctx::new());
            }
            // A `died` handler may have saved the player, or ended the game its own way.
            if self.end_requested.is_none() && self.world.health(PLAYER).is_some_and(|h| h <= 0) {
                self.end_requested = Some("died".into());
            }
        }
    }

    /// Runs the character's `defeated` handlers. Unless one returns
    /// `block()`, "X is defeated." and it drops what it carries. Then,
    /// unless a handler healed it or moved it away, it becomes a corpse
    /// (`dead`), or with `corpse: false`, leaves the game.
    fn defeat(&mut self, id: EntityId) {
        let codes: Vec<Code> = self
            .world
            .def(id)
            .handlers
            .iter()
            .filter(|h| h.phase == Phase::On && h.event == "defeated")
            .map(|h| h.code.clone())
            .collect();
        let mut blocked = false;
        for code in codes {
            blocked |= self.run(&code, &Ctx::new().this(id)) == ScriptValue::Block;
        }
        // A handler may have healed it or moved it away.
        if self.world.room_of(id).is_none() || self.world.health(id).is_some_and(|h| h > 0) {
            return;
        }
        let room = self.world.room_of(id);
        if !blocked {
            let seen = room == Some(self.world.here());
            if seen {
                self.msg("npc.defeated", &Ctx::new().entity("target", id));
            }
            let items = self.world.contents(id);
            for &item in &items {
                self.world.unset(item, "worn");
                let _ = self.world.move_to(item, room);
            }
            if seen && !items.is_empty() {
                let list = self.list_a(&items);
                self.msg("npc.drops", &Ctx::new().entity("target", id).text("list", list));
            }
        }
        if self.world.def(id).npc.as_ref().is_none_or(|n| n.corpse) {
            self.world.set(id, "dead", Value::Bool(true));
            self.world.learn(id);
        } else {
            let _ = self.world.move_to(id, None);
        }
    }

    /// How a body came to be one: "The goblin chief has been torn apart
    /// by teeth." The words come from the weapon's `kills`, else the
    /// killer's, else the killer's faction's.
    pub(crate) fn kill_story(&mut self, id: EntityId) -> Option<String> {
        let key = |w: &crate::World, field: &str| match w.get(id, field) {
            Some(Value::Str(k)) => w.id(k),
            _ => None,
        };
        if self.world.flag(id, "_burnt") {
            return Some(self.msg_str("state.killed_burnt", &Ctx::new().entity("target", id)));
        }
        let killer = key(&self.world, "killed_by")?;
        let weapon = key(&self.world, "killed_with");
        let how = weapon
            .and_then(|w| self.world.def(w).kills.clone())
            .or_else(|| self.world.def(killer).kills.clone())
            .or_else(|| self.world.faction(killer).and_then(|f| f.kills.clone()));
        let ctx = Ctx::new().entity("target", id).entity("by", killer);
        Some(match how {
            Some(how) => self.msg_str("state.killed_how", &ctx.text("how", how)),
            None if killer == PLAYER => self.msg_str("state.killed_by_you", &ctx),
            None => self.msg_str("state.killed_by", &ctx),
        })
    }

    pub(crate) fn apologize_action(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        use crate::npc::Apology;
        let (key, calmed) = match self.world.apologize(t) {
            Apology::NotPerson => ("apologize.not_person", None),
            Apology::Nothing => ("apologize.nothing", None),
            Apology::Refused => ("apologize.refused", None),
            Apology::Exhausted => ("apologize.exhausted", None),
            Apology::Accepted { calmed } => ("apologize.accepted", calmed),
        };
        self.msg(key, ctx);
        if let Some(f) = &calmed {
            self.msg("faction.calmer", &Ctx::new().text("faction", f.replace('_', " ")));
        }
        key == "apologize.accepted"
    }

    /// THROW X (AT Y / DIRECTION). At something that can be hurt, it's an
    /// attack with X as the weapon; X then lands in the room. In a
    /// direction, X lands in the next room that way (over an exit the
    /// player may not be able to walk through, but not through a closed
    /// door), making a small noise there. Companions that fetch go after it.
    pub(crate) fn throw_action(
        &mut self,
        t: eifgb_ir::EntityId,
        at: Option<EntityId>,
        dir: Option<eifgb_ir::Direction>,
        ctx: &Ctx,
    ) -> bool {
        if self.world.location(t) != Some(PLAYER) {
            self.msg("throw.not_held", ctx);
            return false;
        }
        if self.world.weight(t) > 3 {
            self.msg("throw.too_heavy", ctx);
            return false;
        }
        let here = self.world.here();
        // Water or sand thrown at a fire puts it out.
        if let Some(y) = at
            && self.world.def(t).douses
            && self.world.on_fire(y)
        {
            return self.douse(y, Some(t), &ctx.clone().entity("target", y).entity("tool", t));
        }
        if let Some(y) = at {
            if y == PLAYER {
                self.msg("attack.self", ctx);
                return false;
            }
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(here));
            self.msg("throw.at", ctx);
            let hurtable = !self.world.is_dead(y) && (self.world.health(y).is_some() || self.world.is_invincible(y));
            match hurtable.then(|| self.world.attack(PLAYER, y, Some(t))).flatten() {
                Some(event) => self.narrate(&event),
                None => self.msg("throw.bounces", ctx),
            }
        } else if let Some(d) = dir {
            let exit = self.world.def(here).exits.iter().find(|e| e.direction == d && e.to.is_some() && self.world.exit_visible(e)).cloned();
            let Some(exit) = exit else {
                self.no_turn = true;
                self.msg("throw.no_way", ctx);
                return false;
            };
            if let Some(door) = exit.through
                && !self.world.is_open(door)
            {
                self.msg("throw.door", &ctx.clone().entity("door", door));
                return false;
            }
            let to = exit.to.unwrap_or(here);
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(to));
            let words = self.grammar.direction_word(d);
            self.msg("throw.direction", &ctx.clone().text("direction", words));
            self.world.make_noise(to, 1);
            self.distract_onlookers();
        } else {
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(here));
            self.msg("throw.away", ctx);
            self.distract_onlookers();
        }
        for f in self.world.send_fetchers(t) {
            if self.world.together(f, PLAYER) {
                self.msg("npc.fetches", &Ctx::new().entity("npc", f).entity("target", t));
            }
        }
        true
    }

    /// BURY X: in soft ground, it's gone for good. Bodies most of all.
    pub(crate) fn bury_action(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let here = self.world.here();
        if !self.world.def(here).diggable {
            self.msg("bury.cant", ctx);
            return false;
        }
        let movable = self.world.is_dead(t) || self.world.has(t, eifgb_ir::Trait::Portable);
        if !movable || self.world.room_of(t) != Some(here) {
            self.msg("bury.not_that", ctx);
            return false;
        }
        self.world.unset(t, "worn");
        let _ = self.world.move_to(t, None);
        self.msg("bury.done", ctx);
        true
    }

    /// STEAL X FROM Y, TAKE X FROM Y (a character), PICKPOCKET Y. It works
    /// if Y is asleep, busy eating, or fooled by a disguise; otherwise Y
    /// catches the player, which provokes it like an attack, and a merchant
    /// stops trading.
    pub(crate) fn steal_action(&mut self, item: Option<EntityId>, victim: Option<EntityId>, ctx: &Ctx) -> bool {
        let victim = victim.or_else(|| {
            item.and_then(|i| self.world.location(i)).filter(|&l| self.world.def(l).kind == EntityKind::Character)
        });
        let Some(victim) = victim.filter(|&v| v != PLAYER) else {
            // Nobody to steal from: it's lying about, so just take it.
            return match item {
                Some(i) => self.take(i, None, ctx),
                None => {
                    self.msg("steal.nobody", ctx);
                    false
                }
            };
        };
        let ctx = ctx.clone().entity("tool", victim);
        if !self.world.is_active(victim) {
            self.msg("steal.nobody", &ctx);
            return false;
        }
        let asleep = self.world.is_asleep(victim);
        // PICKPOCKET Y: the most valuable thing it has that can be lifted.
        let item = item.or_else(|| {
            self.world
                .contents(victim)
                .into_iter()
                .filter(|&i| !self.world.is_hidden(i) && (asleep || !self.world.flag(i, "worn")))
                .max_by_key(|&i| self.world.def(i).price.unwrap_or(0))
        });
        let Some(item) = item else {
            self.msg("steal.nothing", &ctx);
            return false;
        };
        let ctx = ctx.entity("target", item);
        if self.world.location(item) != Some(victim) {
            self.msg("steal.not_carrying", &ctx);
            return false;
        }
        if self.world.flag(item, "worn") && !asleep {
            self.msg("steal.worn", &ctx);
            return false;
        }
        let too_much = match self.world.fits(item, PLAYER) {
            Fit::Yes => None,
            Fit::TooBig => Some("take.too_big"),
            Fit::TooHeavy => Some("take.too_heavy"),
            Fit::Full => Some("take.full"),
        };
        if let Some(k) = too_much {
            self.msg(k, &ctx);
            return false;
        }
        // In a crowd, nobody notices; unless somebody does.
        let crowd = self.world.room_of(PLAYER).is_some_and(|r| self.world.is_crowded(r));
        if (!crowd && self.world.notices_theft(victim)) || (crowd && self.thief_spotted_in_crowd()) {
            self.msg("steal.caught", &ctx);
            if self.world.def(victim).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Merchant) {
                self.world.set(victim, "_refuses", Value::Bool(true));
            }
            let (tolerated, attitude) = self.world.caught_stealing(victim);
            if let Some(a) = attitude {
                self.announce_attitude(&a);
            }
            if tolerated {
                self.msg("npc.losing_patience_you", &Ctx::new().entity("target", victim));
            }
            return false;
        }
        self.world.unset(item, "worn");
        let _ = self.world.move_to(item, Some(PLAYER));
        self.world.learn(item);
        let key = self.world.def(victim).key.clone();
        self.world.set(item, "_stolen_from", Value::Str(key));
        self.msg(if asleep { "steal.done_asleep" } else { "steal.done" }, &ctx);
        true
    }

    /// "The goblins are growing wary of you." and the like.
    fn announce_attitude(&mut self, a: &crate::npc::AttitudeChange) {
        let words = a.faction.replace('_', " ");
        let key = match (a.left <= 0, a.disguise.as_deref() == Some(a.faction.as_str())) {
            (true, true) => "faction.sees_through",
            (true, false) => "faction.turns",
            (false, _) => "faction.wary",
        };
        self.msg(key, &Ctx::new().text("faction", words));
    }

    /// TELL REX TO ATTACK THE GOBLIN (also ORDER/ASK/COMMAND …, and "REX,
    /// …"). Tamed characters, and those with `obeys`, take orders: attack,
    /// stay, follow, go <direction>, fetch, drop.
    pub(crate) fn order_action(&mut self, npc: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        let ctx = ctx.clone().entity("npc", npc);
        if self.world.def(npc).kind != EntityKind::Character || !self.world.is_active(npc) {
            self.msg("order.not_person", &ctx);
            return false;
        }
        if self.world.is_asleep(npc) {
            self.msg("talk.asleep", &ctx.clone().entity("target", npc));
            return false;
        }
        if !self.world.obeys(npc) {
            self.msg("order.ignored", &ctx);
            return false;
        }
        let words: Vec<String> = self.grammar.split_commands(text.unwrap_or("")).concat();
        let Some((verb, rest)) = words.split_first() else {
            self.no_turn = true;
            self.msg("order.confused", &ctx);
            return false;
        };
        let attackish = matches!(verb.as_str(), "attack" | "kill" | "fight" | "bite" | "maul" | "sic" | "get" | "charge");
        if attackish && rest.iter().any(|w| w == "me" || w == "myself") {
            self.msg("order.wont_attack_you", &ctx);
            return false;
        }
        let rest: Vec<String> = rest.iter().filter(|w| !matches!(w.as_str(), "it" | "me" | "here" | "up")).cloned().collect();
        // Someone to attack can be elsewhere: the companion goes after them.
        let everyone: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&c| self.world.def(c).kind == EntityKind::Character && self.world.is_active(c))
            .collect();
        let named = self
            .grammar
            .name_matches(&self.world, &rest)
            .into_iter()
            .find(|&e| e != npc)
            .or_else(|| self.grammar.name_matches_in(&self.world, &rest, &everyone).into_iter().find(|&e| e != npc))
            .or_else(|| {
                // A companion that sees in the dark finds what the player can't.
                if !(self.world.in_darkness() && self.world.sees_in_dark(npc)) {
                    return None;
                }
                let here = self.world.here();
                let around: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
                    .map(EntityId)
                    .filter(|&i| i != npc && self.world.room_of(i) == Some(here) && !self.world.is_hidden(i) && self.world.def(i).kind != EntityKind::Room)
                    .collect();
                self.grammar.name_matches_in(&self.world, &rest, &around).into_iter().next()
            });
        let dir = rest.iter().find_map(|w| self.grammar_direction(w));
        let pacifist = self.world.def(npc).npc.as_ref().is_some_and(|n| n.pacifist);
        match verb.as_str() {
            "attack" | "kill" | "fight" | "bite" | "maul" | "sic" | "get" | "charge"
                if named.is_some_and(|t| self.world.def(t).kind == EntityKind::Character) =>
            {
                let t = named.unwrap_or(npc);
                if t == PLAYER {
                    self.msg("order.wont_attack_you", &ctx);
                    return false;
                }
                if pacifist {
                    self.msg("order.pacifist", &ctx);
                    return false;
                }
                if self.world.is_demoralized(npc) || self.world.is_frightened(npc) {
                    self.msg("order.too_scared", &ctx);
                    return false;
                }
                let key = self.world.def(t).key.clone();
                self.world.set_order(npc, Some("attack"), Some(key));
                self.msg("order.attack", &ctx.entity("target", t));
            }
            "fetch" | "bring" | "retrieve" | "get" if named.is_some() => {
                let t = named.unwrap_or(npc);
                let key = Value::Str(self.world.def(t).key.clone());
                self.world.set_order(npc, None, None);
                self.world.set(npc, "_fetching", key);
                self.msg("order.fetch", &ctx.entity("target", t));
            }
            "stay" | "wait" | "sit" | "halt" | "stop" | "guard" | "down" => {
                self.world.set_order(npc, Some("stay"), None);
                self.msg("order.stay", &ctx);
            }
            "follow" | "come" | "heel" | "return" => {
                self.world.set_order(npc, None, None);
                self.msg("order.follow", &ctx);
            }
            "go" | "run" | "head" | "move" | "walk" if dir.is_some() => {
                let d = dir.unwrap_or(eifgb_ir::Direction::North);
                self.world.set_order(npc, Some("go"), Some(d.key().to_string()));
                let words = self.grammar.direction_word(d);
                self.msg("order.go", &ctx.text("direction", words));
            }
            "drop" | "release" | "leave" => {
                let room = self.world.room_of(npc);
                let items = self.world.contents(npc);
                if items.is_empty() {
                    self.msg("order.nothing_to_drop", &ctx);
                    return false;
                }
                for &i in &items {
                    self.world.unset(i, "worn");
                    let _ = self.world.move_to(i, room);
                }
                let list = self.list_a(&items);
                self.msg("order.drop", &ctx.text("list", list));
            }
            // A bare direction: GO that way.
            w if self.grammar_direction(w).is_some() => {
                let d = self.grammar_direction(w).unwrap_or(eifgb_ir::Direction::North);
                self.world.set_order(npc, Some("go"), Some(d.key().to_string()));
                let words = self.grammar.direction_word(d);
                self.msg("order.go", &ctx.text("direction", words));
            }
            _ => {
                self.no_turn = true;
                self.msg("order.confused", &ctx);
                return false;
            }
        }
        true
    }

    fn grammar_direction(&self, w: &str) -> Option<eifgb_ir::Direction> {
        self.grammar.direction_of(w)
    }

    /// BLOW HORN / SOUND / PLAY / RING: something that `calls` makes its
    /// noise, wherever it is, answered by its faction's hunters (or all).
    pub(crate) fn sound_action(&mut self, t: EntityId, cmd: &crate::parser::Command, ctx: &Ctx) -> bool {
        let Some(call) = self.world.def(t).calls.clone() else {
            self.msg("sound.cant", ctx);
            return false;
        };
        let verb = cmd.words.first().cloned().unwrap_or_else(|| "sound".into());
        self.msg("sound.done", &ctx.clone().text("verb", verb));
        self.world.make_call(t, call.loudness, call.faction.as_deref());
        true
    }

    /// HOWL, BARK, IMITATE A WOLF: a call that only the faction whose
    /// `calls` (or name, or members' names) it matches answers, carrying as
    /// far as the player's voice. No match: just a strange noise.
    pub(crate) fn mimic_action(&mut self, cmd: &crate::parser::Command, ctx: &Ctx) -> bool {
        let words: Vec<String> = match &cmd.text {
            Some(t) => t.split_whitespace().map(str::to_lowercase).filter(|w| !matches!(w.as_str(), "a" | "an" | "the" | "like" | "call")).collect(),
            None => cmd.words.first().cloned().into_iter().collect(),
        };
        let singular = |w: &str| w.trim_end_matches('s').to_string();
        let story = self.world.story();
        let faction = story.factions.iter().find(|f| {
            words.iter().any(|w| {
                f.calls.iter().any(|c| c == w)
                    || singular(&f.key) == singular(w)
                    || story.entities.iter().any(|e| {
                        e.faction.as_deref() == Some(f.key.as_str()) && e.vocab.nouns.iter().any(|n| singular(n) == singular(w))
                    })
            })
        });
        let faction = faction.map(|f| f.key.clone());
        let voice = self.world.story().meta.settings.voice;
        // Shown as the player put it: "an impression of a goblin".
        let what = cmd.text.clone().unwrap_or_else(|| words.join(" "));
        match &faction {
            Some(_) if cmd.text.is_some() => self.msg("mimic.impression", &ctx.clone().text("what", what)),
            Some(_) => self.msg("mimic.call", &ctx.clone().text("sound", what)),
            None => self.msg("mimic.strange", ctx),
        }
        self.world.make_call(PLAYER, voice, faction.as_deref());
        true
    }

    /// BRANDISH TORCH (AT WOLF): whoever here fears it, and can see it,
    /// panics and runs.
    pub(crate) fn brandish_action(&mut self, t: EntityId, at: Option<EntityId>, ctx: &Ctx) -> bool {
        if self.world.location(t) != Some(PLAYER) {
            self.msg("throw.not_held", ctx);
            return false;
        }
        // SHOW X TO Y: holding it up for someone to see.
        let showing = ctx.text_arg("verb") == Some("show");
        if showing && at.is_none() {
            let w = &self.world;
            let who: Vec<EntityId> = w
                .scope()
                .into_iter()
                .filter(|&c| c != PLAYER && w.def(c).kind == eifgb_ir::EntityKind::Character && !w.is_dead(c) && !w.is_asleep(c) && !w.is_hidden(c))
                .collect();
            match who[..] {
                [c] => return self.brandish_action(t, Some(c), &ctx.clone().entity("tool", c)),
                [] => {}
                _ => {
                    self.msg("show.who", ctx);
                    return false;
                }
            }
        }
        if !showing {
            self.msg("brandish.done", ctx);
        }
        let scared: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&c| at.is_none_or(|a| a == c))
            .filter(|&c| self.world.is_active(c) && self.world.together(c, PLAYER) && !self.world.is_asleep(c))
            .filter(|&c| self.world.can_see(c, PLAYER) && self.world.fears(c, t))
            .collect();
        if scared.is_empty() {
            if showing && at.is_some() {
                self.msg("show.done", ctx);
            } else {
                self.msg("brandish.nothing", ctx);
            }
        }
        for c in scared {
            self.world.panic(c);
            self.msg("npc.afraid", &Ctx::new().entity("npc", c));
        }
        true
    }

    /// CALL X Y: names something `nameable`.
    pub(crate) fn rename_action(&mut self, t: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        if !self.world.def(t).nameable {
            self.msg("name.cant", ctx);
            return false;
        }
        let Some(name) = text.and_then(World::clean_name) else {
            self.no_turn = true;
            self.msg("name.bad", ctx);
            return false;
        };
        let old = self.the(t);
        self.world.rename(t, &name);
        self.msg("name.done", &ctx.clone().text("old", old).text("name", name));
        true
    }

    /// GIVE X TO Y.
    pub(crate) fn give_action(&mut self, item: EntityId, to: Option<EntityId>, ctx: &Ctx) -> bool {
        use crate::npc::Gift;
        let Some(to) = to else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("give.whom", ctx);
            return false;
        };
        if let Some(ok) = self.keep_favour(item, to) {
            return ok;
        }
        let (key, calmed) = match self.world.give(item, to) {
            Gift::NotHeld => ("give.not_held", None),
            Gift::NotPerson => ("give.not_person", None),
            Gift::Unwanted => ("give.unwanted", None),
            Gift::Tamed { eaten: true } => ("give.tamed_eaten", None),
            Gift::Tamed { eaten: false } => ("give.tamed", None),
            Gift::Liked { calmed } => ("give.liked", calmed),
        };
        self.msg(key, ctx);
        if let Some(f) = calmed {
            self.msg("faction.calmer", &Ctx::new().text("faction", f.replace('_', " ")));
        }
        matches!(key, "give.tamed" | "give.tamed_eaten" | "give.liked")
    }

    /// GIVE 5 COINS TO GUARD, PAY GUARD 5, BRIBE GUARD WITH 5.
    pub(crate) fn bribe_action(&mut self, npc: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        use crate::npc::Bribe;
        let Some(amount) = text.and_then(first_number) else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("bribe.how_much", ctx);
            return false;
        };
        let price = self.money_text(amount);
        let have = self.money_text(self.world.money(PLAYER));
        let key = match self.world.bribe(npc, amount) {
            Bribe::NotPerson => "give.not_person_money",
            Bribe::Refused => "bribe.refused",
            Bribe::Already => "bribe.already",
            Bribe::CantAfford => "bribe.cant_afford",
            Bribe::TooLittle => "bribe.too_little",
            Bribe::Taken => "bribe.taken",
        };
        self.msg(key, &ctx.clone().text("price", price).text("amount", have));
        key == "bribe.taken"
    }

    /// "12 coins"
    pub(crate) fn money_text(&mut self, amount: i64) -> String {
        self.msg_str("money.amount", &Ctx::new().value("amount", Value::Int(amount)))
    }

    // --------------------------------------------------------------- fighting

    pub(crate) fn attack_action(&mut self, t: EntityId, tool: Option<EntityId>, ctx: &Ctx) -> bool {
        if t == PLAYER {
            self.msg("attack.self", ctx);
            return false;
        }
        if let Some(w) = tool {
            if !self.world.is_within(w, PLAYER) {
                self.msg("attack.not_held", ctx);
                return false;
            }
            if self.world.def(w).damage.is_none() {
                self.msg("attack.not_weapon", ctx);
                return false;
            }
        }
        if self.world.is_dead(t) {
            self.msg("attack.dead", ctx);
            return false;
        }
        if self.world.health(t).is_none() && !self.world.is_invincible(t) {
            // Something that can be broken, is.
            if self.world.has(t, eifgb_ir::Trait::Destructible) && self.world.def(t).kind == EntityKind::Object {
                return self.smash(t, ctx);
            }
            self.msg("attack.pointless", ctx);
            return false;
        }
        match self.world.attack(PLAYER, t, tool) {
            Some(event) => {
                self.narrate(&event);
                true
            }
            None => false,
        }
    }

    /// Doesn't take a turn: checking on yourself shouldn't give an enemy a free hit.
    pub(crate) fn health_action(&mut self) -> bool {
        self.no_turn = true;
        let max = self.world.def(PLAYER).health;
        match (self.world.health(PLAYER), max) {
            (Some(h), Some(m)) if h >= i64::from(m) => self.msg("health.full", &Ctx::new()),
            (Some(h), Some(m)) => self.msg(
                "health.hurt",
                &Ctx::new().value("health", Value::Int(h)).value("max", Value::Int(i64::from(m))),
            ),
            _ => self.msg("health.none", &Ctx::new()),
        }
        true
    }

    // ---------------------------------------------------------------- trading

    /// The merchant named as the second object, or else the first one here.
    fn pick_merchant(&self, named: Option<EntityId>) -> Option<EntityId> {
        let here = self.world.merchants_here();
        match named {
            Some(m) => here.contains(&m).then_some(m),
            None => here.first().copied(),
        }
    }

    /// The merchant here selling `t`, and its asking price, or the reason
    /// it can't be bought (already reported).
    fn seller_of(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> Option<(EntityId, i64)> {
        let merchants = self.world.merchants_here();
        if merchants.is_empty() || merchant.is_some_and(|m| !merchants.contains(&m)) {
            self.msg("buy.no_merchant", ctx);
            return None;
        }
        let seller = merchants.iter().copied().find(|&m| self.world.location(t) == Some(m) && merchant.is_none_or(|n| n == m));
        let (Some(seller), Some(price)) = (seller, self.world.def(t).price) else {
            self.msg("buy.not_for_sale", ctx);
            return None;
        };
        if self.refuses_trade(seller, ctx) {
            return None;
        }
        // A friend pays less; someone wary of the player, more.
        Some((seller, self.world.asking_price(seller, i64::from(price))))
    }

    /// A merchant pushed too far won't trade with the player at all.
    fn refuses_trade(&mut self, merchant: EntityId, ctx: &Ctx) -> bool {
        if self.world.flag(merchant, "_refuses") {
            self.world.unset(merchant, "_deal");
            self.msg("trade.refused", &ctx.clone().entity("merchant", merchant));
            return true;
        }
        false
    }

    pub(crate) fn buy(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some((seller, price)) = self.seller_of(t, merchant, ctx) else { return false };
        let have = self.world.money(PLAYER);
        if have < price {
            let ctx = ctx.clone().text("price", self.money_text(price)).text("amount", self.money_text(have));
            self.msg("buy.too_poor", &ctx);
            return false;
        }
        self.purchase(t, seller, price, "buy.done", ctx)
    }

    /// Hands `t` over for `price`, if the player can carry it.
    fn purchase(&mut self, t: EntityId, seller: EntityId, price: i64, key: &str, ctx: &Ctx) -> bool {
        let refusal = match self.world.fits(t, PLAYER) {
            Fit::Yes => None,
            Fit::TooBig => Some("take.too_big"),
            Fit::TooHeavy => Some("take.too_heavy"),
            Fit::Full => Some("take.full"),
        };
        if let Some(k) = refusal {
            self.msg(k, ctx);
            return false;
        }
        let have = self.world.money(PLAYER);
        self.world.set_money(PLAYER, have - price);
        if self.world.has_money(seller) {
            let m = self.world.money(seller);
            self.world.set_money(seller, m + price);
        }
        let _ = self.world.move_to(t, Some(PLAYER));
        self.world.learn(t);
        let ctx = ctx.clone().entity("merchant", seller).text("price", self.money_text(price));
        self.msg(key, &ctx);
        true
    }

    /// OFFER 5 FOR LAMP. The asking price or more buys it. Less is accepted
    /// down to the merchant's floor (`haggle` percent off); below that, the
    /// merchant counters, and each lowball offer costs a point of patience.
    /// Out of patience, the merchant refuses to trade at all.
    pub(crate) fn offer(&mut self, t: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some((seller, price)) = self.seller_of(t, None, ctx) else { return false };
        let ctx = ctx.clone().entity("merchant", seller);
        let amount = text.and_then(first_number);
        let Some(amount) = amount else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("offer.how_much", &ctx);
            return false;
        };
        if amount > self.world.money(PLAYER) {
            let have = self.money_text(self.world.money(PLAYER));
            self.msg("offer.cant_afford", &ctx.text("amount", have));
            return false;
        }
        if amount >= price {
            return self.purchase(t, seller, price, "buy.done", &ctx);
        }
        let haggle = self.world.def(seller).npc.as_ref().map_or(0, |n| n.haggle);
        let price_text = self.money_text(price);
        if haggle == 0 {
            self.msg("offer.fixed", &ctx.text("price", price_text));
            return false;
        }
        // The least it'll take, rounded up.
        let floor = (price * i64::from(100 - haggle.min(100)) + 99) / 100;
        if amount >= floor {
            return self.purchase(t, seller, amount, "offer.accepted", &ctx);
        }
        let patience = match self.world.get(seller, "patience") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        // Like being attacked: it puts up with `patience` lowball offers.
        if patience <= 0 {
            self.world.set(seller, "_refuses", Value::Bool(true));
            self.world.shift_disposition(seller, -1);
            self.msg("offer.insulted", &ctx);
            return false;
        }
        self.world.set(seller, "patience", Value::Int(patience - 1));
        // "…and not a coin less": the counter is the least it'll take.
        let counter = floor;
        self.set_deal(seller, "buy", t, counter);
        let counter = self.money_text(counter);
        self.msg("offer.counter", &ctx.text("price", counter));
        false
    }

    /// TRADE X FOR Y: barter, with no money changing hands. The merchant
    /// values X at what it would pay for it (`buys_at`), and takes it for Y
    /// if that's at least the least it would take for Y (its price, less
    /// `haggle` percent). Otherwise it wants something more, which costs a
    /// point of patience, as a lowball offer does.
    pub(crate) fn trade(&mut self, t: EntityId, wanted: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(wanted) = wanted else {
            self.msg("trade.for_what", ctx);
            return false;
        };
        if self.world.location(t) != Some(PLAYER) {
            self.msg("sell.not_held", ctx);
            return false;
        }
        // Watched, or a fake of it: TRADE THE GLASS GEM FOR THE RUBY is a swap.
        let sold = self.world.merchants_here().iter().any(|&m| self.world.location(wanted) == Some(m));
        let watched = self.world.ids().any(|c| self.world.def(c).npc.as_ref().is_some_and(|n| n.watches.contains(&wanted)));
        if !sold && (watched || self.world.def(t).decoy_for == Some(wanted)) {
            return self.swap(wanted, Some(t), ctx);
        }
        let Some((seller, price)) = self.seller_of(wanted, None, &ctx.clone().entity("target", wanted)) else { return false };
        let ctx = ctx.clone().entity("merchant", seller);
        let (buys_at, haggle) = self.world.def(seller).npc.as_ref().map_or((0, 0), |n| (self.world.buying_at(seller, n.buys_at), n.haggle));
        let worth = self.world.def(t).price.map_or(0, i64::from) * i64::from(buys_at) / 100;
        if worth <= 0 {
            self.msg("sell.worthless", &ctx);
            return false;
        }
        let floor = (price * i64::from(100 - haggle.min(100)) + 99) / 100;
        if worth >= floor {
            // Handed over first, so its room is free for what comes back.
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(seller));
            if self.purchase(wanted, seller, 0, "trade.done", &ctx) {
                self.world.unset(seller, "_deal");
                return true;
            }
            let _ = self.world.move_to(t, Some(PLAYER));
            return false;
        }
        let patience = match self.world.get(seller, "patience") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        if patience <= 0 && haggle > 0 {
            self.world.set(seller, "_refuses", Value::Bool(true));
            self.world.shift_disposition(seller, -1);
            self.msg("offer.insulted", &ctx);
            return false;
        }
        if haggle > 0 {
            self.world.set(seller, "patience", Value::Int(patience - 1));
        }
        self.msg("trade.more", &ctx);
        false
    }

    /// SELL X (TO M) (FOR N). A merchant who haggles names its price first,
    /// so the player knows they can ask for more; SELL X again (or ACCEPT)
    /// takes it. FOR N asks for a price: anything up to `haggle` percent
    /// above the merchant's offer (never above its own selling price) is
    /// accepted; more gets a counter-offer and costs patience.
    pub(crate) fn sell(&mut self, t: EntityId, merchant: Option<EntityId>, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some(buyer) = self.pick_merchant(merchant) else {
            self.msg("sell.no_merchant", ctx);
            return false;
        };
        if self.refuses_trade(buyer, ctx) {
            return false;
        }
        let ctx = ctx.clone().entity("merchant", buyer);
        if self.world.location(t) != Some(PLAYER) {
            self.msg("sell.not_held", &ctx);
            return false;
        }
        let n = self.world.def(buyer).npc.clone();
        let (buys_at, haggle) = n.as_ref().map_or((0, 0), |n| (self.world.buying_at(buyer, n.buys_at), n.haggle));
        let price = self.world.def(t).price.map_or(0, i64::from);
        let offer = price * i64::from(buys_at) / 100;
        if offer <= 0 {
            self.msg("sell.worthless", &ctx);
            return false;
        }
        let purse = self.world.has_money(buyer).then(|| self.world.money(buyer));
        if purse.is_some_and(|m| m < offer) {
            self.msg("sell.merchant_poor", &ctx);
            return false;
        }
        let asked = text.and_then(first_number);
        if text.is_some() && asked.is_none() {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("sell.how_much", &ctx);
            return false;
        }
        // Fixed prices: just sell, at the price shown.
        if haggle == 0 {
            if asked.is_some_and(|a| a > offer) {
                let o = self.money_text(offer);
                self.msg("sell.fixed", &ctx.text("price", o));
                return false;
            }
            return self.complete_sale(t, buyer, asked.unwrap_or(offer), "sell.done", &ctx);
        }
        let Some(asked) = asked else {
            // A deal on the table for this: SELL again takes it.
            if let Some(p) = self.deal(buyer, "sell", t) {
                return self.complete_sale(t, buyer, p, "sell.done", &ctx);
            }
            self.set_deal(buyer, "sell", t, offer);
            let o = self.money_text(offer);
            let example = offer + (offer / 5).max(1);
            self.msg("sell.offer", &ctx.text("price", o).value("example", Value::Int(example)));
            return false;
        };
        // The most it'll pay, rounded down: never more than its own price,
        // or than it has.
        let ceiling = (offer * i64::from(100 + haggle) / 100).min(price.max(offer)).min(purse.unwrap_or(i64::MAX));
        if asked <= ceiling {
            let key = if asked > offer { "sell.accepted" } else { "sell.done" };
            return self.complete_sale(t, buyer, asked, key, &ctx);
        }
        let patience = match self.world.get(buyer, "patience") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        if patience <= 0 {
            self.world.set(buyer, "_refuses", Value::Bool(true));
            self.world.shift_disposition(buyer, -1);
            self.world.unset(buyer, "_deal");
            self.msg("offer.insulted", &ctx);
            return false;
        }
        self.world.set(buyer, "patience", Value::Int(patience - 1));
        // "…and not a coin more": the counter is the most it'll pay.
        let counter = ceiling;
        self.set_deal(buyer, "sell", t, counter);
        let c = self.money_text(counter);
        self.msg("sell.counter", &ctx.text("price", c));
        false
    }

    fn complete_sale(&mut self, t: EntityId, buyer: EntityId, price: i64, key: &str, ctx: &Ctx) -> bool {
        if self.world.has_money(buyer) {
            let m = self.world.money(buyer);
            self.world.set_money(buyer, m - price);
        }
        let have = self.world.money(PLAYER);
        self.world.set_money(PLAYER, have + price);
        self.world.unset(t, "worn");
        self.world.unset(buyer, "_deal");
        let _ = self.world.move_to(t, Some(buyer));
        let p = self.money_text(price);
        self.msg(key, &ctx.clone().entity("merchant", buyer).text("price", p));
        true
    }

    /// A deal on the table: the merchant's last offer or counter-offer,
    /// which ACCEPT (or repeating SELL) takes.
    fn set_deal(&mut self, merchant: EntityId, kind: &str, item: EntityId, price: i64) {
        let mut m = std::collections::BTreeMap::new();
        m.insert("kind".to_string(), Value::Str(kind.into()));
        m.insert("item".to_string(), Value::Str(self.world.def(item).key.clone()));
        m.insert("price".to_string(), Value::Int(price));
        self.world.set(merchant, "_deal", Value::Map(m));
    }

    /// The price of a deal of this kind on the table for `item`.
    fn deal(&self, merchant: EntityId, kind: &str, item: EntityId) -> Option<i64> {
        let Some(Value::Map(m)) = self.world.get(merchant, "_deal") else { return None };
        let same = m.get("kind") == Some(&Value::Str(kind.into()))
            && m.get("item") == Some(&Value::Str(self.world.def(item).key.clone()));
        match m.get("price") {
            Some(Value::Int(p)) if same => Some(*p),
            _ => None,
        }
    }

    /// ACCEPT: takes the deal a merchant here has on the table.
    pub(crate) fn accept(&mut self, ctx: &Ctx) -> bool {
        for m in self.world.merchants_here() {
            let Some(Value::Map(d)) = self.world.get(m, "_deal").cloned() else { continue };
            let (Some(Value::Str(kind)), Some(Value::Str(item)), Some(Value::Int(price))) =
                (d.get("kind"), d.get("item"), d.get("price"))
            else {
                continue;
            };
            let Some(item) = self.world.id(item) else { continue };
            let ctx = ctx.clone().entity("target", item).entity("merchant", m);
            if self.refuses_trade(m, &ctx) {
                return false;
            }
            return match kind.as_str() {
                // Taking the merchant's own price: a plain sale.
                "sell" if self.world.location(item) == Some(PLAYER) => self.complete_sale(item, m, *price, "sell.done", &ctx),
                "buy" if self.world.location(item) == Some(m) => {
                    if self.world.money(PLAYER) < *price {
                        let have = self.money_text(self.world.money(PLAYER));
                        self.msg("offer.cant_afford", &ctx.text("amount", have));
                        return false;
                    }
                    self.world.unset(m, "_deal");
                    self.purchase(item, m, *price, "buy.done", &ctx)
                }
                _ => {
                    self.world.unset(m, "_deal");
                    self.msg("accept.nothing", &ctx);
                    false
                }
            };
        }
        self.msg("accept.nothing", ctx);
        false
    }

    pub(crate) fn wares(&mut self, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let merchants: Vec<EntityId> = match merchant {
            Some(m) if self.world.def(m).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Merchant) => vec![m],
            Some(_) => Vec::new(),
            None => self.world.merchants_here(),
        };
        if merchants.is_empty() {
            self.msg("buy.no_merchant", ctx);
            return false;
        }
        for m in merchants {
            if self.refuses_trade(m, ctx) {
                continue;
            }
            let goods: Vec<EntityId> =
                self.world.contents(m).into_iter().filter(|&i| self.world.def(i).price.is_some()).collect();
            let ctx = Ctx::new().entity("merchant", m);
            if goods.is_empty() {
                self.msg("wares.empty", &ctx);
                continue;
            }
            let items: Vec<String> = goods
                .iter()
                .map(|&i| {
                    let price = self.money_text(self.world.asking_price(m, i64::from(self.world.def(i).price.unwrap_or(0))));
                    self.msg_str("wares.item", &Ctx::new().text("item", self.a(i)).text("price", price))
                })
                .collect();
            let list = (self.grammar.lang().list_and)(&items);
            self.msg("wares.list", &ctx.text("list", list));
            for g in goods {
                self.world.learn(g);
            }
        }
        true
    }

    /// A line for the inventory: "You have 12 coins.", if the player has money at all.
    pub(crate) fn report_money(&mut self) {
        if self.world.has_money(PLAYER) {
            let amount = self.money_text(self.world.money(PLAYER));
            self.msg_kind(LineKind::Text, "money.have", &Ctx::new().text("amount", amount));
        }
    }
}

/// The first whole number in some text: "6", "6 coins", "for 6".
fn first_number(text: &str) -> Option<i64> {
    text.split(|c: char| !c.is_ascii_digit()).find(|w| !w.is_empty()).and_then(|w| w.parse().ok())
}
