//! Gossip: what the player did gets round. Robbing someone, attacking
//! them, a gift, sparing them, fooling them, and promises broken or kept
//! each start a tale, known to whoever it was done to and whoever saw.
//! Every few turns, each who knows it tells their `friends` (and those who
//! count them as friends), so it travels a friend at a time, as far as
//! the friendships reach. Someone who's heard says so when they next see
//! the player ("The miller's heard you robbed the baker."), or their own
//! `heard:` line for it; and those further off than the victim's own
//! friends (who get word of the victim's feelings anyway) think a little
//! better or worse of the player for it.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use crate::state::Tale;
use crate::World;
use eifgb_ir::{EntityId, EntityKind, PLAYER};

/// Turns between one telling and the next.
const GOSSIP_STEP: u64 = 3;
/// Tales kept at once: past this, the oldest are forgotten.
const TALES_KEPT: usize = 24;

/// What hearing of a deed does to how someone feels about the player.
fn weight(deed: &str) -> i64 {
    match deed {
        "gift" | "spared" | "kept_promise" => 1,
        _ => -1,
    }
}

impl World {
    /// Who `npc` tells: its friends, and those it's a friend of.
    pub(crate) fn friends_of(&self, npc: EntityId) -> Vec<EntityId> {
        let friends = self.def(npc).npc.as_ref().map(|n| n.friends.clone()).unwrap_or_default();
        self.ids()
            .filter(|&c| c != npc && c != PLAYER && self.def(c).kind == EntityKind::Character && self.is_active(c))
            .filter(|&c| friends.contains(&c) || self.def(c).npc.as_ref().is_some_and(|n| n.friends.contains(&npc)))
            .collect()
    }

    /// A tale begins: the player did `deed` to `about`, who knows, as does
    /// anyone awake who saw. Nothing in a game where nobody has friends.
    pub(crate) fn start_tale(&mut self, deed: &str, about: EntityId) {
        if about == PLAYER || self.def(about).kind != EntityKind::Character || !self.ids().any(|c| self.def(c).npc.as_ref().is_some_and(|n| !n.friends.is_empty())) {
            return;
        }
        let saw: Vec<EntityId> = self
            .ids()
            .filter(|&c| c != about && c != PLAYER && self.def(c).kind == EntityKind::Character && self.is_active(c) && !self.is_asleep(c))
            .filter(|&c| self.together(c, PLAYER) && self.can_see(c, PLAYER))
            .collect();
        let next = self.state.turn + GOSSIP_STEP;
        // The same again, while it's still going round: whoever saw knows too.
        if let Some(t) = self.state.tales.iter_mut().find(|t| t.deed == deed && t.about == about) {
            for c in std::iter::once(about).chain(saw) {
                if !t.knows.iter().any(|(k, _)| *k == c) {
                    t.knows.push((c, 0));
                }
            }
            t.done = false;
            t.next = t.next.min(next);
            return;
        }
        let knows = std::iter::once(about).chain(saw).map(|c| (c, 0)).collect();
        self.state.tales.push(Tale { deed: deed.to_string(), about, knows, mentioned: Vec::new(), next, done: false });
        if self.state.tales.len() > TALES_KEPT {
            self.state.tales.remove(0);
        }
    }

    /// Whether `npc` has heard of the player doing `deed` (to `about`, or
    /// to anyone), there or by word of mouth.
    pub fn has_heard(&self, npc: EntityId, deed: &str, about: Option<EntityId>) -> bool {
        self.state.tales.iter().any(|t| t.deed == deed && about.is_none_or(|a| a == t.about) && t.knows.iter().any(|(k, _)| *k == npc))
    }
}

impl<H: ScriptHost> Game<H> {
    /// After each turn: tales go one telling further, and whoever's heard
    /// one and sees the player says so.
    pub(crate) fn gossip_turn(&mut self) {
        if self.world.state.tales.is_empty() {
            return;
        }
        self.tell_tales();
        self.mention_tales();
    }

    fn tell_tales(&mut self) {
        let turn = self.world.state.turn;
        for i in 0..self.world.state.tales.len() {
            let t = &self.world.state.tales[i];
            if t.done || t.next > turn {
                continue;
            }
            let (deed, about, knows) = (t.deed.clone(), t.about, t.knows.clone());
            // The victim's own friends hear how they feel anyway (word gets round).
            let close = self.world.friends_of(about);
            let mut heard: Vec<(EntityId, u32)> = Vec::new();
            let mut someone_waits = false;
            for &(teller, hops) in &knows {
                if !self.world.is_active(teller) {
                    continue;
                }
                // Asleep: they'll tell it when they wake.
                if self.world.is_asleep(teller) {
                    someone_waits = true;
                    continue;
                }
                for f in self.world.friends_of(teller) {
                    if !knows.iter().chain(&heard).any(|(k, _)| *k == f) {
                        heard.push((f, hops + 1));
                    }
                }
            }
            for &(c, _) in &heard {
                if !close.contains(&c) && c != about {
                    self.world.shift_disposition_quietly(c, weight(&deed));
                }
            }
            let t = &mut self.world.state.tales[i];
            t.done = heard.is_empty() && !someone_waits;
            t.knows.extend(heard);
            t.next = turn + GOSSIP_STEP;
        }
    }

    /// Someone who's heard a tale (and wasn't there) says so, once, the
    /// next time they see the player: one tale at a time each.
    fn mention_tales(&mut self) {
        if self.world.state.choice.is_some() {
            return;
        }
        let mut spoke: Vec<EntityId> = Vec::new();
        for i in 0..self.world.state.tales.len() {
            let t = self.world.state.tales[i].clone();
            for &(c, hops) in &t.knows {
                if hops == 0 || t.mentioned.contains(&c) || spoke.contains(&c) {
                    continue;
                }
                if !self.world.is_active(c) || self.world.is_asleep(c) || !self.world.together(c, PLAYER) || !self.world.can_see(c, PLAYER) {
                    continue;
                }
                spoke.push(c);
                self.world.state.tales[i].mentioned.push(c);
                let ctx = Ctx::new().this(c).entity("npc", c).entity("about", t.about);
                match self.world.def(c).npc.as_ref().and_then(|n| n.heard.get(&t.deed).cloned()) {
                    Some(text) => self.render_line(LineKind::Text, &text, &ctx),
                    None => self.msg(&format!("gossip.{}", t.deed), &ctx),
                }
            }
        }
    }
}
