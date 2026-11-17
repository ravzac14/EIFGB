//! What characters do on their turn: moving between rooms, finding their
//! way to the player, and fighting.
//!
//! Everything here only changes the world and reports what happened as
//! [`NpcEvent`]s. The turn loop turns events into text ("The peddler goes
//! north."), so the built-in behaviours and Lua scripts calling
//! `self:wander()` describe things the same way.

use crate::world::World;
use eifgb_ir::{Behavior, Direction, EntityId, EntityKind, Faction, Value, PLAYER, PLAYER_FACTION};

use std::collections::{HashMap, VecDeque};

/// How a gift went.
#[derive(Debug, Clone, PartialEq)]
pub enum Gift {
    NotHeld,
    NotPerson,
    Unwanted,
    /// Tamed. `eaten` if it ate the gift.
    Tamed { eaten: bool },
    /// A welcome gift. `calmed` is the faction that warmed, if any.
    Liked { calmed: Option<String> },
}

/// How a bribe went.
#[derive(Debug, Clone, PartialEq)]
pub enum Bribe {
    NotPerson,
    /// It can't be bought.
    Refused,
    Already,
    CantAfford,
    TooLittle,
    Taken,
}

/// How an apology went.
#[derive(Debug, Clone, PartialEq)]
pub enum Apology {
    /// Not someone who can forgive (not a character, or dead).
    NotPerson,
    /// It has nothing against the player.
    Nothing,
    /// It's hostile by nature, and no apology changes that.
    Refused,
    /// It's accepted all the apologies it ever will.
    Exhausted,
    /// Forgiven. `calmed` is the faction whose attitude went up, if any.
    Accepted { calmed: Option<String> },
}

/// A faction thinking less of the player.
#[derive(Debug, Clone, PartialEq)]
pub struct AttitudeChange {
    pub faction: String,
    /// What's left; at 0 or below, every member attacks the player.
    pub left: i64,
    /// The faction the player was disguised as, if any.
    pub disguise: Option<String>,
}

/// Something a character did that the player may see.
#[derive(Debug, Clone, PartialEq)]
pub enum NpcEvent {
    /// Moved from one room to another, going `direction`.
    Moved { npc: EntityId, from: EntityId, to: EntityId, direction: Direction },
    /// Attacked `target`, taking `damage` health. `weapon` is what was used,
    /// if anything. `tolerated` means the target let it go this time, but
    /// its patience is wearing thin. `attitude` is set when the attack
    /// soured the target's whole faction on the player.
    Attacked {
        attacker: EntityId,
        target: EntityId,
        weapon: Option<EntityId>,
        damage: u32,
        tolerated: bool,
        attitude: Option<AttitudeChange>,
    },
    /// Ate a corpse, which is now gone.
    Ate { npc: EntityId, corpse: EntityId, room: EntityId },
    /// Ate some of `food`; `finished` if it's all gone now.
    Fed { npc: EntityId, food: EntityId, room: EntityId, finished: bool },
    /// Reached where it last saw the player, and found nothing.
    Searched { npc: EntityId, room: EntityId },
    /// Found the body of one of its own that the player killed, and raised
    /// the alarm.
    Discovered { npc: EntityId, body: EntityId, room: EntityId, attitude: Option<AttitudeChange> },
    /// Fetching something the player threw: `Grabbed` it, or `Brought` it back.
    Fetched { npc: EntityId, item: EntityId, room: EntityId, brought: bool },
}

impl World {
    /// Current health, if it can be hurt at all.
    pub fn health(&self, id: EntityId) -> Option<i64> {
        self.def(id).health?;
        Some(match self.get(id, "health") {
            Some(Value::Int(i)) => *i,
            Some(Value::Float(f)) => f.floor() as i64,
            _ => 0,
        })
    }

    /// Money in hand: a whole number, 0 if it has none.
    pub fn money(&self, id: EntityId) -> i64 {
        match self.get(id, "money") {
            Some(Value::Int(i)) => *i,
            Some(Value::Float(f)) => f.floor() as i64,
            _ => 0,
        }
    }

    /// Whether `id` keeps track of money at all. A merchant without it can always pay.
    pub fn has_money(&self, id: EntityId) -> bool {
        matches!(self.get(id, "money"), Some(Value::Int(_) | Value::Float(_)))
    }

    pub fn set_money(&mut self, id: EntityId, amount: i64) {
        self.set(id, "money", Value::Int(amount.max(0)));
    }

    /// A character that's in the world and hasn't been defeated.
    pub fn is_active(&self, id: EntityId) -> bool {
        self.def(id).kind == EntityKind::Character
            && self.room_of(id).is_some()
            && !self.is_dead(id)
            && self.health(id).is_none_or(|h| h > 0)
    }

    /// Can't be hurt (`invincible`).
    pub fn is_invincible(&self, id: EntityId) -> bool {
        self.def(id).npc.as_ref().is_some_and(|n| n.invincible)
    }

    fn is_pacifist(&self, id: EntityId) -> bool {
        self.def(id).npc.as_ref().is_some_and(|n| n.pacifist)
    }

    /// A defeated character's body, still lying where it fell.
    pub fn is_dead(&self, id: EntityId) -> bool {
        self.def(id).kind == EntityKind::Character && self.flag(id, "dead")
    }

    pub fn faction(&self, id: EntityId) -> Option<&Faction> {
        let key = self.def(id).faction.as_ref()?;
        self.story().factions.iter().find(|f| &f.key == key)
    }

    /// Whether `a` would attack `b`. With a faction, that's whatever its
    /// `hostile_to` says; without one, hunters and guards go for the
    /// player only. Nobody attacks the dead, or themselves.
    pub fn is_hostile(&self, a: EntityId, b: EntityId) -> bool {
        if a == b {
            return false;
        }
        let b_alive = if b == PLAYER { self.health(PLAYER).is_none_or(|h| h > 0) } else { self.is_active(b) };
        if !b_alive {
            return false;
        }
        if self.is_foe(a, b) {
            return true;
        }
        // Tamed or bought off: leaves the player alone.
        if b == PLAYER && (self.is_tamed(a) || self.is_bribed(a)) {
            return false;
        }
        // A tamed animal fights whoever would fight the player.
        if self.is_tamed(a) && self.is_hostile(b, PLAYER) {
            return true;
        }
        match self.faction(a) {
            // Toward the player, it's the faction's attitude that counts.
            Some(f) if b == PLAYER => self.attitude(&f.key) <= 0,
            Some(f) => self.def(b).faction.as_deref().is_some_and(|s| f.hostile_to.iter().any(|h| h == s)),
            None => b == PLAYER && self.def(a).npc.as_ref().is_some_and(|n| n.behavior.hostile()),
        }
    }

    /// The side characters take the player for: a faction they're disguised
    /// as (by wearing something with `disguise`), or else `player`.
    pub fn player_side(&self) -> &str {
        self.contents(PLAYER)
            .into_iter()
            .filter(|&i| self.flag(i, "worn"))
            .find_map(|i| self.def(i).disguise.as_deref())
            .unwrap_or(PLAYER_FACTION)
    }

    /// Whether the player is out of sight: shut inside something closed
    /// they can't see out of, so nobody can see in either.
    pub fn player_hidden(&self) -> bool {
        self.ceiling() != self.here()
    }

    /// Food for `npc`'s faction lying loose in `room` (not carried).
    fn food_in(&self, npc: EntityId, room: EntityId) -> Vec<EntityId> {
        let Some(f) = self.def(npc).faction.as_deref() else { return Vec::new() };
        self.contents(room)
            .into_iter()
            .filter(|&i| self.def(i).kind == EntityKind::Object && self.def(i).eaten_by.iter().any(|e| e == f))
            .collect()
    }

    /// A noise in `room` (or the room a thing is in): hunters that can reach
    /// it within `loudness` rooms come to investigate (it becomes where
    /// they "last saw" the player), whatever their `range`. Returns how
    /// many were drawn.
    pub fn make_noise(&mut self, at: EntityId, loudness: u32) -> usize {
        let Some(room) = self.room_of(at) else { return 0 };
        let key = Value::Str(self.def(room).key.clone());
        let hunters: Vec<EntityId> = (0..self.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| self.def(id).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Hunter))
            .filter(|&id| self.is_active(id))
            .filter(|&id| self.distance(id, room).is_some_and(|d| d <= loudness))
            .collect();
        for &h in &hunters {
            self.set(h, "_last_seen", key.clone());
        }
        hunters.len()
    }

    /// Sends every fetching companion after `item`, which the player threw.
    /// Returns the ones that set off, to be described.
    pub fn send_fetchers(&mut self, item: EntityId) -> Vec<EntityId> {
        let key = Value::Str(self.def(item).key.clone());
        let fetchers: Vec<EntityId> = (0..self.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| self.def(id).npc.as_ref().is_some_and(|n| n.fetches && (n.behavior == Behavior::Follower)) || (self.is_tamed(id) && self.def(id).npc.as_ref().is_some_and(|n| n.fetches)))
            .filter(|&id| self.is_active(id) && self.get(id, "_fetching").is_none())
            // Only after things it can get to.
            .filter(|&id| self.distance(id, item).is_some())
            .collect();
        for &f in &fetchers {
            self.set(f, "_fetching", key.clone());
        }
        fetchers
    }

    /// One step of fetching, if it's fetching something: go to it, grab it,
    /// bring it back to the player and drop it at their feet.
    fn fetch_step(&mut self, npc: EntityId) -> Option<Option<NpcEvent>> {
        let Some(Value::Str(key)) = self.get(npc, "_fetching").cloned() else { return None };
        let Some(item) = self.id(&key) else {
            self.unset(npc, "_fetching");
            return None;
        };
        let room = self.room_of(npc)?;
        if self.location(item) == Some(npc) {
            if self.together(npc, PLAYER) {
                let here = self.here();
                self.move_to(item, Some(here)).ok()?;
                self.unset(npc, "_fetching");
                return Some(Some(NpcEvent::Fetched { npc, item, room, brought: true }));
            }
            let step = self.npc_step_toward(npc, PLAYER);
            if step.is_none() {
                // Can't get back to the player: drop it here.
                let _ = self.move_to(item, Some(room));
                self.unset(npc, "_fetching");
            }
            return Some(step);
        }
        // Only things lying loose in a room can be fetched.
        let lying = self.location(item).filter(|&l| self.def(l).kind == EntityKind::Room);
        match lying {
            Some(r) if r == room => {
                self.move_to(item, Some(npc)).ok()?;
                Some(Some(NpcEvent::Fetched { npc, item, room, brought: false }))
            }
            Some(_) => match self.npc_step_toward(npc, item) {
                Some(step) => Some(Some(step)),
                // The way's been cut off: give up.
                None => {
                    self.unset(npc, "_fetching");
                    None
                }
            },
            None => {
                self.unset(npc, "_fetching");
                None
            }
        }
    }

    /// The name to show: one the player gave it, or its own.
    pub fn display_name(&self, id: EntityId) -> String {
        match self.get(id, "_name") {
            Some(Value::Str(n)) => n.clone(),
            _ => self.def(id).name.clone(),
        }
    }

    /// Named by the player, so it's a proper name: "Rex", not "the Rex".
    pub fn is_proper(&self, id: EntityId) -> bool {
        matches!(self.get(id, "_name"), Some(Value::Str(_)))
    }

    /// Tidies a name: up to three words of letters, digits, ' or -, each
    /// capitalised ("rex" → "Rex"). None if there's nothing usable.
    pub fn clean_name(text: &str) -> Option<String> {
        let words: Vec<String> = text
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c == '"' || c == '\''))
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
            })
            .collect();
        let ok = !words.is_empty()
            && words.len() <= 3
            && words.iter().all(|w: &String| w.chars().all(|c| c.is_alphanumeric() || c == '\'' || c == '-'));
        ok.then(|| words.join(" "))
    }

    /// Gives `id` a name, if it's `nameable` and the name is usable.
    pub fn rename(&mut self, id: EntityId, name: &str) -> Option<String> {
        if !self.def(id).nameable {
            return None;
        }
        let name = Self::clean_name(name)?;
        self.set(id, "_name", Value::Str(name.clone()));
        Some(name)
    }

    /// Whether `word` is part of a name the player has given `id`.
    pub fn named_word(&self, id: EntityId, word: &str) -> bool {
        matches!(self.get(id, "_name"), Some(Value::Str(n)) if n.split_whitespace().any(|w| w.eq_ignore_ascii_case(word)))
    }

    /// Takes a bite of `food`. It lasts as many bites as its size.
    pub fn npc_feed(&mut self, npc: EntityId, food: EntityId) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        if self.location(food) != Some(room) {
            return None;
        }
        let left = match self.get(food, "_bites") {
            Some(Value::Int(i)) => *i,
            _ => i64::from(self.size(food).max(1)),
        } - 1;
        let finished = left <= 0;
        if finished {
            self.move_to(food, None).ok()?;
            self.unset(food, "_bites");
        } else {
            self.set(food, "_bites", Value::Int(left));
        }
        Some(NpcEvent::Fed { npc, food, room, finished })
    }

    /// `attacker` attacked `target`. If the target fights back, that uses
    /// up a little of its patience, or, with none left, makes the attacker
    /// one of its foes. Returns whether it was let go this time.
    /// A member newly turned against the player also sours its faction.
    fn provoke(&mut self, target: EntityId, attacker: EntityId) -> (bool, Option<AttitudeChange>) {
        if target == attacker {
            return (false, None);
        }
        // A pacifist won't fight, but it remembers, and so does its faction.
        if self.is_pacifist(target) {
            let new_foe = self.add_foe(target, attacker);
            if new_foe && attacker == PLAYER {
                self.unset(target, "_tamed");
                self.unset(target, "_bribed");
            }
            let change = match (new_foe && attacker == PLAYER, self.def(target).faction.clone()) {
                (true, Some(f)) if self.witnessed(&f) => self.sour(&f),
                _ => None,
            };
            return (false, change);
        }
        if !self.def(target).npc.as_ref().is_some_and(|n| n.fights_back) {
            return (false, None);
        }
        let patience = match self.get(target, "patience") {
            Some(Value::Int(i)) => *i,
            Some(Value::Float(f)) => f.floor() as i64,
            _ => 0,
        };
        if patience > 0 {
            self.set(target, "patience", Value::Int(patience - 1));
            return (true, None);
        }
        let new_foe = self.add_foe(target, attacker);
        if new_foe && attacker == PLAYER {
            self.unset(target, "_tamed");
            self.unset(target, "_bribed");
        }
        let change = match (new_foe && attacker == PLAYER, self.def(target).faction.clone()) {
            (true, Some(f)) if self.witnessed(&f) => self.sour(&f),
            _ => None,
        };
        (false, change)
    }

    /// Whether any living member of `faction` saw the player just now: one
    /// in the player's room that can see them. A victim that survived the
    /// blow counts; one that didn't can't tell anyone.
    pub fn witnessed(&self, faction: &str) -> bool {
        if self.player_hidden() {
            return false;
        }
        (0..self.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&c| self.def(c).faction.as_deref() == Some(faction))
            .any(|c| self.is_active(c) && self.together(c, PLAYER))
    }

    /// Whether something can be seen where it is: not shut away inside
    /// anything closed that can't be seen into.
    pub fn in_plain_sight(&self, id: EntityId) -> bool {
        let mut cur = self.location(id);
        while let Some(c) = cur {
            if self.def(c).kind == EntityKind::Room {
                return true;
            }
            if self.has(c, eifgb_ir::Trait::Container) && !self.shows_contents(c) {
                return false;
            }
            cur = self.location(c);
        }
        false
    }

    /// A member of a faction comes across the body of one of its own that
    /// the player killed, lying in plain sight: the faction sours (once
    /// per body), and hunters within two rooms come running.
    pub fn discover_bodies(&mut self, npc: EntityId) -> Option<NpcEvent> {
        let faction = self.def(npc).faction.clone()?;
        let room = self.room_of(npc)?;
        let player = Value::Str(self.def(PLAYER).key.clone());
        let body = (0..self.story().entities.len() as u32).map(EntityId).find(|&b| {
            self.is_dead(b)
                && self.def(b).faction.as_deref() == Some(faction.as_str())
                && self.room_of(b) == Some(room)
                && !self.flag(b, "_found")
                && self.get(b, "killed_by") == Some(&player)
                && self.in_plain_sight(b)
        })?;
        self.set(body, "_found", Value::Bool(true));
        let attitude = self.sour(&faction);
        self.make_noise(room, 2);
        Some(NpcEvent::Discovered { npc, body, room, attitude })
    }

    /// The key under which a faction's attitude toward the player, as
    /// they present themselves now, is kept: "goblins>player", or
    /// "goblins>goblins" for a player in goblin disguise.
    fn attitude_key(&self, faction: &str) -> String {
        format!("{faction}>{}", self.player_side())
    }

    /// A faction's attitude toward the player as they present themselves
    /// now. It starts at 0 toward a side the faction is hostile to, and at
    /// the faction's `patience` otherwise (including toward its own kind,
    /// which is how a disguise works). At 0 or below, members attack.
    pub fn attitude(&self, faction: &str) -> i64 {
        let key = self.attitude_key(faction);
        if let Some(Value::Map(m)) = self.get(PLAYER, "_attitude")
            && let Some(Value::Int(v)) = m.get(&key)
        {
            return *v;
        }
        self.starting_attitude(faction)
    }

    /// Where a faction's attitude toward the player's current side starts,
    /// and the most an apology can bring it back to.
    fn starting_attitude(&self, faction: &str) -> i64 {
        let side = self.player_side();
        match self.story().factions.iter().find(|f| f.key == faction) {
            Some(f) if f.hostile_to.iter().any(|h| h == side) => 0,
            Some(f) => i64::from(f.patience),
            None => 0,
        }
    }

    fn set_attitude(&mut self, faction: &str, value: i64) {
        let key = self.attitude_key(faction);
        let mut map = match self.get(PLAYER, "_attitude") {
            Some(Value::Map(m)) => m.clone(),
            _ => Default::default(),
        };
        map.insert(key, Value::Int(value));
        self.set(PLAYER, "_attitude", Value::Map(map));
    }

    /// The player apologises to `npc`. If it accepts, it forgets its
    /// grudge, its patience is restored, and its faction's attitude toward
    /// the player (as they present themselves now) goes up by one, but
    /// never above where it started.
    pub fn apologize(&mut self, npc: EntityId) -> Apology {
        let Some(n) = self.def(npc).npc.clone() else { return Apology::NotPerson };
        if !self.is_active(npc) {
            return Apology::NotPerson;
        }
        let faction = self.def(npc).faction.clone();
        // Hostile to the player by nature: nothing an apology can change.
        let by_nature = match &faction {
            Some(f) => self.starting_attitude(f) <= 0,
            None => n.behavior.hostile(),
        };
        if by_nature {
            return Apology::Refused;
        }
        let key = Value::Str(self.def(PLAYER).key.clone());
        let foes = match self.get(npc, "foes") {
            Some(Value::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        let grudge = foes.contains(&key);
        let patience = match self.get(npc, "patience") {
            Some(Value::Int(i)) => Some(*i),
            _ => None,
        };
        let short_of_patience = patience.is_some_and(|p| p < i64::from(n.patience));
        let faction_low = faction.as_deref().is_some_and(|f| self.attitude(f) < self.starting_attitude(f));
        let refuses = self.flag(npc, "_refuses");
        if !grudge && !short_of_patience && !faction_low && !refuses {
            return Apology::Nothing;
        }
        let forgiven = match self.get(npc, "_forgiven") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        if forgiven >= i64::from(n.forgiveness) {
            return Apology::Exhausted;
        }
        self.set(npc, "_forgiven", Value::Int(forgiven + 1));
        Apology::Accepted { calmed: self.pacify(npc) }
    }

    /// Makes peace with the player: no grudge, full patience, trading again,
    /// and its faction's attitude up by one if it's below its start.
    /// Returns the faction that warmed, if any.
    fn pacify(&mut self, npc: EntityId) -> Option<String> {
        let n = self.def(npc).npc.clone()?;
        let key = Value::Str(self.def(PLAYER).key.clone());
        if let Some(Value::List(foes)) = self.get(npc, "foes").cloned() {
            self.set(npc, "foes", Value::List(foes.into_iter().filter(|v| *v != key).collect()));
        }
        if matches!(self.get(npc, "patience"), Some(Value::Int(_))) {
            self.set(npc, "patience", Value::Int(i64::from(n.patience)));
        }
        self.unset(npc, "_refuses");
        let f = self.def(npc).faction.clone()?;
        if self.attitude(&f) >= self.starting_attitude(&f) {
            return None;
        }
        let now = self.attitude(&f) + 1;
        self.set_attitude(&f, now);
        Some(f)
    }

    /// Tamed: follows the player and fights the player's enemies.
    pub fn is_tamed(&self, id: EntityId) -> bool {
        self.flag(id, "_tamed")
    }

    /// Bought off: won't attack the player.
    pub fn is_bribed(&self, id: EntityId) -> bool {
        self.flag(id, "_bribed")
    }

    /// GIVE `item` TO `npc`.
    pub fn give(&mut self, item: EntityId, npc: EntityId) -> Gift {
        if self.location(item) != Some(PLAYER) {
            return Gift::NotHeld;
        }
        let Some(n) = self.def(npc).npc.clone() else { return Gift::NotPerson };
        if !self.is_active(npc) {
            return Gift::NotPerson;
        }
        let tames = n.tame_with.contains(&item);
        if !tames && !n.likes.contains(&item) {
            return Gift::Unwanted;
        }
        self.unset(item, "worn");
        // Food its faction eats is eaten; anything else is kept.
        let food = self.def(npc).faction.as_ref().is_some_and(|f| self.def(item).eaten_by.contains(f));
        let _ = self.move_to(item, if food { None } else { Some(npc) });
        let calmed = self.pacify(npc);
        if tames {
            self.set(npc, "_tamed", Value::Bool(true));
            Gift::Tamed { eaten: food }
        } else {
            Gift::Liked { calmed }
        }
    }

    /// GIVE `amount` COINS TO `npc`.
    pub fn bribe(&mut self, npc: EntityId, amount: i64) -> Bribe {
        let Some(n) = self.def(npc).npc.clone() else { return Bribe::NotPerson };
        if !self.is_active(npc) {
            return Bribe::NotPerson;
        }
        let Some(price) = n.bribe else { return Bribe::Refused };
        if self.is_bribed(npc) {
            return Bribe::Already;
        }
        if amount > self.money(PLAYER) {
            return Bribe::CantAfford;
        }
        if amount < i64::from(price) {
            return Bribe::TooLittle;
        }
        let have = self.money(PLAYER);
        self.set_money(PLAYER, have - amount);
        if self.has_money(npc) {
            let m = self.money(npc);
            self.set_money(npc, m + amount);
        }
        self.set(npc, "_bribed", Value::Bool(true));
        self.pacify(npc);
        Bribe::Taken
    }

    /// Lowers a faction's attitude toward the player (as they present
    /// themselves now) by one, unless it's already at 0.
    fn sour(&mut self, faction: &str) -> Option<AttitudeChange> {
        let now = self.attitude(faction);
        if now <= 0 {
            return None;
        }
        self.set_attitude(faction, now - 1);
        let side = self.player_side();
        let disguise = (side != PLAYER_FACTION).then(|| side.to_string());
        Some(AttitudeChange { faction: faction.to_string(), left: now - 1, disguise })
    }

    /// Remembers that `attacker` attacked `target`, in the target's `foes`
    /// (a list of keys, which scripts can read or clear).
    /// Returns whether `attacker` is a new foe.
    fn add_foe(&mut self, target: EntityId, attacker: EntityId) -> bool {
        if target == attacker || self.def(target).kind != EntityKind::Character {
            return false;
        }
        let key = Value::Str(self.def(attacker).key.clone());
        let mut foes = match self.get(target, "foes") {
            Some(Value::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        if foes.contains(&key) {
            return false;
        }
        foes.push(key);
        self.set(target, "foes", Value::List(foes));
        true
    }

    /// Whether `a` holds a grudge against `b` and fights back.
    pub fn is_foe(&self, a: EntityId, b: EntityId) -> bool {
        let fights_back = self.def(a).npc.as_ref().is_some_and(|n| n.fights_back && !n.pacifist);
        let key = &self.def(b).key;
        fights_back
            && matches!(self.get(a, "foes"), Some(Value::List(l)) if l.iter().any(|v| matches!(v, Value::Str(s) if s == key)))
    }

    /// Everyone `npc` would attack: the player first, then characters in
    /// the order they're declared.
    fn enemies_of(&self, npc: EntityId) -> Vec<EntityId> {
        let chars = (0..self.story().entities.len() as u32).map(EntityId).filter(|&id| self.def(id).kind == EntityKind::Character);
        std::iter::once(PLAYER).chain(chars).filter(|&b| self.is_hostile(npc, b)).collect()
    }

    /// Foes of `npc` in its own room, the player first.
    fn foes_here(&self, npc: EntityId) -> Vec<EntityId> {
        let chars = (0..self.story().entities.len() as u32).map(EntityId).filter(|&id| self.def(id).kind == EntityKind::Character);
        std::iter::once(PLAYER)
            .chain(chars)
            .filter(|&b| self.together(npc, b) && self.is_foe(npc, b) && self.is_hostile_or_foe_alive(b))
            .filter(|&b| b != PLAYER || !self.player_hidden())
            .collect()
    }

    fn is_hostile_or_foe_alive(&self, b: EntityId) -> bool {
        if b == PLAYER { self.health(PLAYER).is_none_or(|h| h > 0) } else { self.is_active(b) }
    }

    /// Bodies in `room`.
    fn corpses_in(&self, room: EntityId) -> Vec<EntityId> {
        self.contents(room).into_iter().filter(|&c| self.is_dead(c)).collect()
    }

    /// Eats a corpse in the same room: it's gone for good.
    pub fn npc_eat(&mut self, npc: EntityId, corpse: EntityId) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        if !self.is_dead(corpse) || self.room_of(corpse) != Some(room) {
            return None;
        }
        self.move_to(corpse, None).ok()?;
        Some(NpcEvent::Ate { npc, corpse, room })
    }

    /// Whether `a` and `b` are in the same room.
    pub fn together(&self, a: EntityId, b: EntityId) -> bool {
        matches!((self.room_of(a), self.room_of(b)), (Some(x), Some(y)) if x == y)
    }

    /// Merchants in the player's room.
    pub fn merchants_here(&self) -> Vec<EntityId> {
        let here = self.here();
        (0..self.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| {
                self.def(id).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Merchant)
                    && self.is_active(id)
                    && self.room_of(id) == Some(here)
            })
            .collect()
    }

    /// Where a character can go from `room`: exits to another room that
    /// aren't conditional, and whose door (if any) isn't locked, blocked
    /// or hidden from this side. Characters slip through closed doors
    /// without changing them. With an `area`, only rooms in it.
    pub fn npc_exits(&self, npc: EntityId, room: EntityId) -> Vec<(Direction, EntityId)> {
        let area = self.def(npc).npc.as_ref().map(|n| n.area.as_slice()).unwrap_or(&[]);
        self.def(room)
            .exits
            .iter()
            .filter(|e| e.when.is_none())
            .filter_map(|e| Some((e, e.to?)))
            .filter(|(e, _)| {
                e.through.is_none_or(|d| {
                    !self.flag(d, "locked") && self.door_blocked(d, room).is_none() && !self.hidden_from(d, room)
                })
            })
            .filter(|(_, to)| area.is_empty() || area.contains(to))
            .map(|(e, to)| (e.direction, to))
            .collect()
    }

    /// The first step of a shortest route from `npc`'s room to `goal`, and
    /// how many rooms away `goal` is. `(None, 0)` when already there.
    fn route(&self, npc: EntityId, goal: EntityId) -> Option<(Option<(Direction, EntityId)>, u32)> {
        let start = self.room_of(npc)?;
        if start == goal {
            return Some((None, 0));
        }
        let mut first: HashMap<EntityId, ((Direction, EntityId), u32)> = HashMap::new();
        let mut queue = VecDeque::from([start]);
        while let Some(room) = queue.pop_front() {
            let (step, dist) = match first.get(&room) {
                Some(&(s, d)) => (Some(s), d),
                None => (None, 0),
            };
            for (dir, to) in self.npc_exits(npc, room) {
                if to == start || first.contains_key(&to) {
                    continue;
                }
                let step = step.unwrap_or((dir, to));
                if to == goal {
                    return Some((Some(step), dist + 1));
                }
                first.insert(to, (step, dist + 1));
                queue.push_back(to);
            }
        }
        None
    }

    /// How many moves `npc` needs to reach `target`'s room, or None if it can't.
    pub fn distance(&self, npc: EntityId, target: EntityId) -> Option<u32> {
        let goal = self.room_of(target)?;
        self.route(npc, goal).map(|(_, d)| d)
    }

    fn npc_move(&mut self, npc: EntityId, direction: Direction, to: EntityId) -> Option<NpcEvent> {
        let from = self.room_of(npc)?;
        self.move_to(npc, Some(to)).ok()?;
        self.leave_tracks(npc, from, direction);
        Some(NpcEvent::Moved { npc, from, to, direction })
    }

    /// Goes through the exit in `direction`, if it can.
    pub fn npc_go(&mut self, npc: EntityId, direction: Direction) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        let (_, to) = self.npc_exits(npc, room).into_iter().find(|(d, _)| *d == direction)?;
        self.npc_move(npc, direction, to)
    }

    /// Moves to a random neighbouring room, if there's one it can reach.
    pub fn npc_wander(&mut self, npc: EntityId) -> Option<NpcEvent> {
        let room = self.room_of(npc)?;
        let exits = self.npc_exits(npc, room);
        if exits.is_empty() {
            return None;
        }
        let pick = self.state.rng.range(0, exits.len() as i64 - 1) as usize;
        let (dir, to) = exits[pick];
        self.npc_move(npc, dir, to)
    }

    /// Takes one step along the shortest route to `target`'s room.
    pub fn npc_step_toward(&mut self, npc: EntityId, target: EntityId) -> Option<NpcEvent> {
        let goal = self.room_of(target)?;
        let (step, _) = self.route(npc, goal)?;
        let (dir, to) = step?;
        self.npc_move(npc, dir, to)
    }

    /// `attacker` hits `target`, which must be able to be hurt. The damage
    /// is the weapon's, or else the attacker's own, or else 1.
    pub fn attack(&mut self, attacker: EntityId, target: EntityId, weapon: Option<EntityId>) -> Option<NpcEvent> {
        if self.is_invincible(target) {
            let (tolerated, attitude) = self.provoke(target, attacker);
            return Some(NpcEvent::Attacked { attacker, target, weapon, damage: 0, tolerated, attitude });
        }
        let health = self.health(target)?;
        let damage = weapon
            .and_then(|w| self.def(w).damage)
            .or(self.def(attacker).damage)
            .unwrap_or(1);
        self.set(target, "health", Value::Int((health - i64::from(damage)).max(0)));
        // The killing blow is remembered, so the body can tell the story.
        if health > 0 && health - i64::from(damage) <= 0 && target != PLAYER {
            self.set(target, "killed_by", Value::Str(self.def(attacker).key.clone()));
            match weapon {
                Some(w) => self.set(target, "killed_with", Value::Str(self.def(w).key.clone())),
                None => self.unset(target, "killed_with"),
            }
        }
        let (tolerated, attitude) = self.provoke(target, attacker);
        // Killed in front of its own: there's no body left to discover.
        if attacker == PLAYER
            && self.health(target).is_some_and(|h| h <= 0)
            && let Some(f) = self.def(target).faction.clone()
            && self.witnessed(&f)
        {
            self.set(target, "_found", Value::Bool(true));
        }
        Some(NpcEvent::Attacked { attacker, target, weapon, damage, tolerated, attitude })
    }

    /// The best weapon `id` is carrying: the one that does the most damage.
    pub fn best_weapon(&self, id: EntityId) -> Option<EntityId> {
        self.contents(id).into_iter().filter(|&w| self.def(w).damage.is_some()).max_by_key(|&w| self.def(w).damage)
    }

    /// Rolls the character's `wander` chance, and moves if it comes up.
    fn maybe_wander(&mut self, npc: EntityId, chance: u32) -> Option<NpcEvent> {
        if chance == 0 || self.state.rng.range(1, 100) > i64::from(chance) {
            return None;
        }
        self.npc_wander(npc)
    }

    /// One turn of a character's built-in behaviour.
    pub fn npc_behave(&mut self, npc: EntityId) -> Vec<NpcEvent> {
        let Some(n) = self.def(npc).npc.clone() else { return Vec::new() };
        if !self.is_active(npc) {
            return Vec::new();
        }
        let Some(room) = self.room_of(npc) else { return Vec::new() };
        let in_range = |w: &World, target: EntityId| w.distance(npc, target).filter(|&d| n.range.is_none_or(|r| d <= r));
        let hidden = self.player_hidden();
        // The nearest enemy it can see and reach; the player wins a tie.
        let nearest = self
            .enemies_of(npc)
            .into_iter()
            .filter(|&e| e != PLAYER || !hidden)
            .filter_map(|e| in_range(self, e).map(|d| (d, e)))
            .min_by_key(|&(d, _)| d);
        // A hunter keeps track of where it last saw the player.
        if n.behavior == Behavior::Hunter
            && !hidden
            && self.is_hostile(npc, PLAYER)
            && in_range(self, PLAYER).is_some()
        {
            let seen = self.def(self.here()).key.clone();
            self.set(npc, "_last_seen", Value::Str(seen));
        }
        let eater = self.faction(npc).is_some_and(|f| f.eats_corpses);

        // Someone who attacked it is here: fight back, before anything else
        // (even food).
        if let Some(&foe) = self.foes_here(npc).first() {
            let weapon = self.best_weapon(npc);
            return self.attack(npc, foe, weapon).into_iter().collect();
        }
        // Tamed: fight the player's enemies here, else stay with the player.
        if self.is_tamed(npc) {
            let chars: Vec<EntityId> = (0..self.story().entities.len() as u32)
                .map(EntityId)
                .filter(|&c| c != npc && self.def(c).kind == EntityKind::Character && self.together(npc, c))
                .filter(|&c| self.is_hostile(npc, c))
                .collect();
            if let Some(&enemy) = chars.first() {
                let weapon = self.best_weapon(npc);
                return self.attack(npc, enemy, weapon).into_iter().collect();
            }
            if let Some(step) = self.fetch_step(npc) {
                return step.into_iter().collect();
            }
            if !self.together(npc, PLAYER) {
                return self.npc_step_toward(npc, PLAYER).into_iter().collect();
            }
            return Vec::new();
        }
        // A faction that's had enough of the player: every member attacks on
        // sight, whatever its behaviour, except pacifists.
        if !n.behavior.hostile()
            && !n.pacifist
            && self.faction(npc).is_some()
            && !hidden
            && self.together(npc, PLAYER)
            && self.is_hostile(npc, PLAYER)
        {
            let weapon = self.best_weapon(npc);
            return self.attack(npc, PLAYER, weapon).into_iter().collect();
        }
        // Food beats everything, even a fight.
        if let Some(&food) = self.food_in(npc, room).first() {
            return self.npc_feed(npc, food).into_iter().collect();
        }
        let corpse_here = self.corpses_in(room).into_iter().next();
        let event = match n.behavior {
            Behavior::Hunter | Behavior::Guard if matches!(nearest, Some((0, _))) => {
                let target = nearest.map(|(_, e)| e).unwrap_or(PLAYER);
                let weapon = self.best_weapon(npc);
                self.attack(npc, target, weapon).or(Some(NpcEvent::Attacked {
                    attacker: npc,
                    target,
                    weapon,
                    damage: 0,
                    tolerated: false,
                    attitude: None,
                }))
            }
            // Nothing to fight here: a scavenger eats.
            _ if eater && corpse_here.is_some() => self.npc_eat(npc, corpse_here.unwrap_or(npc)),
            Behavior::Hunter => self.hunt(npc, nearest, eater, &in_range, n.wander),
            Behavior::Stationary | Behavior::Conversationalist | Behavior::Wanderer | Behavior::Defender => {
                self.maybe_wander(npc, n.wander)
            }
            // Merchants don't walk off in the middle of a sale.
            Behavior::Merchant if self.together(npc, PLAYER) => None,
            Behavior::Merchant => self.maybe_wander(npc, n.wander),
            Behavior::Guard => self.maybe_wander(npc, n.wander),
            Behavior::Follower if self.get(npc, "_fetching").is_some() => self.fetch_step(npc).flatten(),
            Behavior::Follower if self.together(npc, PLAYER) => None,
            Behavior::Follower if in_range(self, PLAYER).is_some() => self.npc_step_toward(npc, PLAYER),
            Behavior::Follower => self.maybe_wander(npc, n.wander),
        };
        event.into_iter().collect()
    }

    /// A hunter with nobody to fight in its room: heads for the nearest
    /// enemy or food (food wins a tie), else to where it last saw the
    /// player, else (if it eats corpses) the nearest body, else wanders.
    fn hunt(
        &mut self,
        npc: EntityId,
        nearest: Option<(u32, EntityId)>,
        eater: bool,
        in_range: &dyn Fn(&World, EntityId) -> Option<u32>,
        wander: u32,
    ) -> Option<NpcEvent> {
        let food = self
            .def(npc)
            .faction
            .clone()
            .map(|f| {
                (0..self.story().entities.len() as u32)
                    .map(EntityId)
                    .filter(|&i| self.def(i).eaten_by.contains(&f))
                    .filter(|&i| self.location(i).is_some_and(|l| self.def(l).kind == EntityKind::Room))
                    .filter_map(|i| in_range(self, i).map(|d| (d, i)))
                    .min_by_key(|&(d, _)| d)
            })
            .unwrap_or(None);
        let target = match (food, nearest) {
            (Some((fd, f)), Some((ed, _))) if fd <= ed => Some(f),
            (Some((_, f)), None) => Some(f),
            (_, Some((_, e))) => Some(e),
            (None, None) => None,
        };
        if let Some(t) = target {
            return self.npc_step_toward(npc, t);
        }
        // A tracker picks up the player's fresh trail.
        if self.def(npc).npc.as_ref().is_some_and(|n| n.tracker)
            && self.is_hostile(npc, PLAYER)
            && let Some(room) = self.room_of(npc)
            && let Some(t) = self
                .tracks_in(room)
                .into_iter()
                .find(|t| t.who == PLAYER && t.age <= crate::tracks::FRESH)
            && let Some(step) = self.npc_go(npc, t.direction)
        {
            return Some(step);
        }
        if let Some(Value::Str(key)) = self.get(npc, "_last_seen").cloned()
            && let Some(spot) = self.id(&key)
        {
            if self.room_of(npc) == Some(spot) {
                self.unset(npc, "_last_seen");
                return Some(NpcEvent::Searched { npc, room: spot });
            }
            if let Some(step) = self.npc_step_toward(npc, spot) {
                return Some(step);
            }
            self.unset(npc, "_last_seen");
        }
        if eater {
            let body = (0..self.story().entities.len() as u32)
                .map(EntityId)
                .filter(|&c| self.is_dead(c) && self.room_of(c).is_some())
                .filter_map(|c| in_range(self, c).map(|d| (d, c)))
                .min_by_key(|&(d, _)| d);
            if let Some((_, c)) = body {
                return self.npc_step_toward(npc, c);
            }
        }
        self.maybe_wander(npc, wander)
    }
}
