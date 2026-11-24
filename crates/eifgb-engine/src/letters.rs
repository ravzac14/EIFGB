//! Notes the player writes: WRITE "MEET ME AT THE MILL" ON THE PAPER, then
//! give it to someone, or leave it where they'll find it. Someone who
//! reads, and believes it, goes where it says and waits there a while.

use crate::game::Game;
use crate::script::{Ctx, ScriptHost, ScriptValue};
use eifgb_ir::{EntityId, EntityKind, Phase, Value, PLAYER};

/// How long someone lured by a note waits where it said.
const LURED: i64 = 20;

impl crate::World {
    /// What the player wrote on it.
    pub fn written(&self, t: EntityId) -> Option<String> {
        match self.get(t, "written") {
            Some(Value::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    /// The room a note names: its whole name, its ID, or the last word of
    /// its name ("the mill" for "The Old Mill").
    pub fn room_named_in(&self, text: &str) -> Option<EntityId> {
        let text = format!(" {} ", text.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>());
        let has = |w: &str| !w.is_empty() && text.contains(&format!(" {w} "));
        let rooms: Vec<EntityId> = self.ids().filter(|&r| self.def(r).kind == EntityKind::Room).collect();
        rooms
            .iter()
            .copied()
            .find(|&r| has(&self.def(r).name.to_lowercase()) || has(&self.def(r).key.replace('_', " ")))
            .or_else(|| {
                rooms.into_iter().find(|&r| {
                    let name = self.def(r).name.to_lowercase();
                    name.split_whitespace().last().is_some_and(|w| w.len() >= 4 && has(w))
                })
            })
    }

    /// Where a lured character is waiting, and until when.
    pub fn lured_to(&self, npc: EntityId) -> Option<EntityId> {
        let until = match self.get(npc, "_lured_until") {
            Some(Value::Int(n)) => *n,
            _ => return None,
        };
        if until < self.state.turn as i64 {
            return None;
        }
        match self.get(npc, "_lured") {
            Some(Value::Str(k)) => self.id(k),
            _ => None,
        }
    }
}

impl<H: ScriptHost> Game<H> {
    /// WRITE "MEET ME AT THE MILL" ON THE PAPER (WITH THE PEN).
    pub(crate) fn write(&mut self, on: Option<EntityId>, with: Option<EntityId>, words: Option<&str>, ctx: &Ctx) -> bool {
        let w = &self.world;
        let words = words.unwrap_or_default().trim().trim_matches(|c| c == '"' || c == '\'' || c == '“' || c == '”').trim().to_string();
        let paper = on.or_else(|| w.inventory_all().into_iter().find(|&t| w.def(t).blank && w.written(t).is_none()));
        let Some(paper) = paper else {
            self.msg("write.no_paper", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("target", paper);
        if !w.def(paper).blank {
            self.msg("write.cant", &ctx);
            return false;
        }
        if w.written(paper).is_some() {
            self.msg("write.full", &ctx);
            return false;
        }
        if words.is_empty() {
            self.no_turn = true;
            self.msg("write.what", &ctx);
            return false;
        }
        let pen = with.or_else(|| w.inventory_all().into_iter().find(|&t| w.def(t).writes));
        let Some(pen) = pen.filter(|&p| w.def(p).writes) else {
            self.msg("write.no_pen", &ctx);
            return false;
        };
        let mut c = words.chars();
        let words: String = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
        self.world.set(paper, "written", Value::Str(words.clone()));
        self.world.learn(paper);
        self.msg("write.done", &ctx.entity("tool", pen).text("words", words));
        true
    }

    /// READ of a note the player wrote.
    pub(crate) fn read_written(&mut self, t: EntityId, ctx: &Ctx) -> Option<bool> {
        let words = self.world.written(t)?;
        self.msg("write.read", &ctx.clone().entity("target", t).text("words", words));
        Some(true)
    }

    /// Given a note: whether `npc` reads it (true), or it's an ordinary gift.
    pub(crate) fn hand_note(&mut self, note: EntityId, npc: EntityId) -> bool {
        if self.world.written(note).is_none() || !self.world.def(npc).reads || !self.world.is_active(npc) || self.world.is_asleep(npc) {
            return false;
        }
        let _ = self.world.move_to(note, Some(npc));
        self.read_note(npc, note);
        true
    }

    /// Each turn: characters who read find notes lying in their room.
    pub(crate) fn notes_turn(&mut self) {
        let notes: Vec<EntityId> = self.world.ids().filter(|&t| self.world.written(t).is_some() && self.world.def(self.world.location(t).unwrap_or(PLAYER)).kind == EntityKind::Room).collect();
        for note in notes {
            let Some(room) = self.world.location(note) else { continue };
            let key = Value::Str(self.world.def(note).key.clone());
            let reader = self.world.ids().find(|&c| {
                self.world.def(c).reads
                    && self.world.room_of(c) == Some(room)
                    && self.world.is_active(c)
                    && !self.world.is_asleep(c)
                    && !self.world.is_bound(c)
                    && !matches!(self.world.get(c, "_notes_read"), Some(Value::List(l)) if l.contains(&key))
            });
            if let Some(c) = reader {
                self.read_note(c, note);
            }
        }
    }

    /// `npc` reads the note: its own `read_note` handler, or it goes where
    /// the note says, if it believes it.
    fn read_note(&mut self, npc: EntityId, note: EntityId) {
        let words = self.world.written(note).unwrap_or_default();
        let key = Value::Str(self.world.def(note).key.clone());
        let mut read: Vec<Value> = match self.world.get(npc, "_notes_read") {
            Some(Value::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        read.push(key);
        self.world.set(npc, "_notes_read", Value::List(read));
        let here = self.world.here();
        let seen = self.world.room_of(npc) == Some(here) && !self.world.in_darkness();
        let ctx = Ctx::new().entity("npc", npc).entity("target", note).text("text", words.clone()).this(npc);
        let codes: Vec<_> = self.world.def(npc).handlers.iter().filter(|h| h.phase == Phase::On && h.event == "read_note").map(|h| h.code.clone()).collect();
        if !codes.is_empty() {
            for c in codes {
                if self.run(&c, &ctx) == ScriptValue::Block {
                    break;
                }
            }
            return;
        }
        // Believed: from someone it doesn't mind, or signed by a friend.
        let w = &self.world;
        let text = words.to_lowercase();
        let signed = w.def(npc).npc.as_ref().is_some_and(|n| n.friends.iter().chain(&n.answers_to).any(|&f| text.contains(&w.display_name(f).to_lowercase())));
        let believed = signed || w.disposition(npc) >= 0 && !w.is_hostile(npc, PLAYER);
        let place = w.room_named_in(&words);
        let key = match (believed, place) {
            (false, _) => "note.scorned",
            (true, None) => "note.shrugs",
            (true, Some(p)) => {
                let k = self.world.def(p).key.clone();
                self.world.set(npc, "_lured", Value::Str(k));
                self.world.set(npc, "_lured_until", Value::Int(self.world.state.turn as i64 + LURED));
                if self.world.room_of(npc) == Some(p) { "note.here_already" } else { "note.lured" }
            }
        };
        if seen {
            let ctx = match place {
                Some(p) => ctx.text("place", self.world.def(p).name.clone()),
                None => ctx,
            };
            self.msg(key, &ctx);
        }
    }

    /// A lured character's turn: on its way, or waiting. True if that was its turn.
    pub(crate) fn lured_turn(&mut self, npc: EntityId) -> bool {
        let Some(place) = self.world.lured_to(npc) else {
            if self.world.get(npc, "_lured").is_some() {
                self.world.unset(npc, "_lured");
                self.world.unset(npc, "_lured_until");
            }
            return false;
        };
        if self.world.room_of(npc) == Some(place) {
            return true;
        }
        if let Some(event) = self.world.npc_step_toward(npc, place) {
            self.narrate(&event);
        }
        true
    }
}
