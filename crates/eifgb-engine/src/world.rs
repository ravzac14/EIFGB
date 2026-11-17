//! The world model: an immutable [`Story`] plus the mutable [`GameState`],
//! with the queries and changes that the parser, actions and scripts share.

use crate::state::{GameState, SaveError, SaveFile};
use eifgb_ir::{Direction, Entity, EntityId, EntityKind, Exit, Story, Trait, Value, PLAYER};
use std::collections::VecDeque;

/// How many turns UNDO can go back.
pub const UNDO_LIMIT: usize = 100;

/// Whether something fits in a holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Yes,
    /// It's bigger than the holder's whole capacity.
    TooBig,
    /// It's small enough, but what's inside it takes it over the capacity.
    TooHeavy,
    /// It would fit in an empty holder, but not with what's there now.
    Full,
}

/// How a door is blocked, seen from one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    Both,
    ThisSide,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MoveError {
    /// Rooms are fixed and the player always has a location.
    Immovable,
    /// The destination is the thing itself, or something inside it.
    WouldContainItself,
}

pub struct World {
    story: Story,
    pub state: GameState,
    undo: VecDeque<GameState>,
    /// Characters woken since the turn loop last looked, so it can say so.
    pub(crate) woken: Vec<EntityId>,
}

impl World {
    pub fn new(story: Story, seed: u64) -> Self {
        let state = GameState::new(&story, seed);
        World { story, state, undo: VecDeque::new(), woken: Vec::new() }
    }

    pub fn story(&self) -> &Story {
        &self.story
    }

    pub fn def(&self, id: EntityId) -> &Entity {
        self.story.entity(id)
    }

    pub fn id(&self, key: &str) -> Option<EntityId> {
        self.story.entity_by_key(key)
    }

    fn ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        (0..self.story.entities.len() as u32).map(EntityId)
    }

    // ------------------------------------------------------------ location

    pub fn location(&self, id: EntityId) -> Option<EntityId> {
        self.state.entities[id.index()].location
    }

    /// Things directly inside or on `holder`, in story order.
    pub fn contents(&self, holder: EntityId) -> Vec<EntityId> {
        self.ids().filter(|&e| self.location(e) == Some(holder)).collect()
    }

    /// True if `id` is somewhere inside `ancestor`, at any depth.
    pub fn is_within(&self, id: EntityId, ancestor: EntityId) -> bool {
        let mut cur = self.location(id);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.location(c);
        }
        false
    }

    /// The room that ultimately contains `id`. For a room, that's the room itself.
    pub fn room_of(&self, id: EntityId) -> Option<EntityId> {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if self.def(c).kind == EntityKind::Room {
                return Some(c);
            }
            cur = self.location(c);
        }
        None
    }

    pub fn here(&self) -> EntityId {
        self.room_of(PLAYER).unwrap_or(self.story.meta.start)
    }

    pub fn move_to(&mut self, id: EntityId, dest: Option<EntityId>) -> Result<(), MoveError> {
        if self.def(id).kind == EntityKind::Room || (id == PLAYER && dest.is_none()) {
            return Err(MoveError::Immovable);
        }
        if let Some(d) = dest
            && (d == id || self.is_within(d, id))
        {
            return Err(MoveError::WouldContainItself);
        }
        self.state.entities[id.index()].location = dest;
        if id == PLAYER
            && let Some(room) = self.room_of(PLAYER)
        {
            self.state.visited.insert(room);
        }
        Ok(())
    }

    // --------------------------------------------------------------- state

    pub fn get(&self, id: EntityId, field: &str) -> Option<&Value> {
        self.state.entities[id.index()].state.get(field)
    }

    pub fn set(&mut self, id: EntityId, field: &str, value: Value) {
        self.state.entities[id.index()].state.insert(field.to_string(), value);
    }

    pub fn unset(&mut self, id: EntityId, field: &str) {
        self.state.entities[id.index()].state.remove(field);
    }

    /// A boolean state field. A missing or non-boolean field counts as false.
    pub fn flag(&self, id: EntityId, field: &str) -> bool {
        self.get(id, field).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn has(&self, id: EntityId, t: Trait) -> bool {
        self.def(id).has(t)
    }

    /// Hidden from the player where they are now. A door's `hidden` may
    /// name the sides it's hidden from (see [`World::hidden_from`]).
    pub fn is_hidden(&self, id: EntityId) -> bool {
        if self.def(id).has(Trait::Door) {
            self.hidden_from(id, self.here())
        } else {
            self.flag(id, "hidden")
        }
    }

    /// Whether a door is hidden when seen from `room`: `hidden` is `true`
    /// (both sides), a room key, or a list of room keys.
    pub fn hidden_from(&self, door: EntityId, room: EntityId) -> bool {
        self.side_listed(door, "hidden", room)
    }

    fn side_listed(&self, door: EntityId, field: &str, room: EntityId) -> bool {
        let key = &self.def(room).key;
        match self.get(door, field) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Str(s)) => s == key,
            Some(Value::List(sides)) => sides.iter().any(|v| matches!(v, Value::Str(s) if s == key)),
            _ => false,
        }
    }

    /// Open, or not openable at all: containers without `openable` are
    /// always open.
    pub fn is_open(&self, id: EntityId) -> bool {
        !self.has(id, Trait::Openable) || self.flag(id, "open")
    }

    /// Whether you can see and reach the contents of `id`.
    pub fn exposes_contents(&self, id: EntityId) -> bool {
        match self.def(id).kind {
            EntityKind::Room | EntityKind::Player => true,
            // You can't rummage through someone else's pockets.
            EntityKind::Character => false,
            EntityKind::Object => {
                self.has(id, Trait::Supporter) || (self.has(id, Trait::Container) && self.is_open(id))
            }
        }
    }

    /// Whether you can see what's in or on `id`: everything that exposes
    /// its contents, plus closed `transparent` containers.
    pub fn shows_contents(&self, id: EntityId) -> bool {
        self.exposes_contents(id) || (self.has(id, Trait::Container) && self.has(id, Trait::Transparent))
    }

    // ------------------------------------------------------------- size

    pub fn size(&self, id: EntityId) -> u32 {
        self.def(id).size
    }

    /// The current capacity, which scripts may have changed from the story's.
    pub fn capacity(&self, id: EntityId) -> Option<u32> {
        self.state.entities[id.index()].capacity
    }

    pub fn set_capacity(&mut self, id: EntityId, capacity: Option<u32>) {
        self.state.entities[id.index()].capacity = capacity;
    }

    /// A thing's size plus the weight of everything in or on it: what it
    /// counts as when carried or put somewhere.
    pub fn weight(&self, id: EntityId) -> u32 {
        self.weight_depth(id, 0)
    }

    fn weight_depth(&self, id: EntityId, depth: usize) -> u32 {
        let inside: u32 = if depth < 16 {
            self.contents(id).iter().map(|c| self.weight_depth(*c, depth + 1)).sum()
        } else {
            0
        };
        self.size(id).saturating_add(inside)
    }

    /// Total weight of what's directly in or on `holder`.
    pub fn load(&self, holder: EntityId) -> u32 {
        self.contents(holder).iter().map(|c| self.weight(*c)).sum()
    }

    /// Whether `item` would fit in or on `holder`. With a `capacity`, the
    /// total weight of what's there can't go over it, where a thing's
    /// weight includes everything inside it. Without one, nothing bigger
    /// than the holder itself fits (no piano in a jar), except that the
    /// player can carry anything.
    pub fn fits(&self, item: EntityId, holder: EntityId) -> Fit {
        let size = self.size(item);
        let Some(cap) = self.capacity(holder) else {
            let bigger = holder != PLAYER && self.def(holder).kind == EntityKind::Object && size > self.size(holder);
            return if bigger { Fit::TooBig } else { Fit::Yes };
        };
        let weight = self.weight(item);
        if size > cap {
            Fit::TooBig
        } else if weight > cap {
            Fit::TooHeavy
        } else if self.load(holder).saturating_add(weight) > cap {
            Fit::Full
        } else {
            Fit::Yes
        }
    }

    /// Whether the player has examined or handled `id`, so that its
    /// state tags are shown in lists.
    pub fn known(&self, id: EntityId) -> bool {
        self.state.known.contains(&id)
    }

    pub fn learn(&mut self, id: EntityId) {
        self.state.known.insert(id);
    }

    // --------------------------------------------------------------- scope

    /// How far the player can see: the room, or the closed opaque
    /// container they're shut inside (a wardrobe with its door shut).
    pub fn ceiling(&self) -> EntityId {
        let mut cur = self.location(PLAYER);
        while let Some(c) = cur {
            if self.def(c).kind == EntityKind::Room {
                return c;
            }
            if self.has(c, Trait::Container) && !self.is_open(c) && !self.has(c, Trait::Transparent) {
                return c;
            }
            cur = self.location(c);
        }
        self.here()
    }

    /// The closed container that keeps the player from reaching `id`, if any.
    /// Things in a closed `transparent` container are in scope (you can see
    /// them) but not reachable.
    pub fn blocked_reach(&self, id: EntityId) -> Option<EntityId> {
        let ceiling = self.ceiling();
        let mut cur = self.location(id);
        while let Some(c) = cur {
            if c == ceiling || c == PLAYER || self.def(c).kind == EntityKind::Room {
                return None;
            }
            // The player inside a closed container can reach out to its walls.
            if self.is_within(PLAYER, c) {
                return None;
            }
            if self.has(c, Trait::Container) && !self.is_open(c) {
                return Some(c);
            }
            cur = self.location(c);
        }
        None
    }

    /// Everything the player can currently refer to: what's around them up
    /// to their [`World::ceiling`] (descending into containers they can see
    /// into, and supporters), the room's doors, the player and what they
    /// carry. Hidden things are left out, except for things the player carries.
    pub fn scope(&self) -> Vec<EntityId> {
        let mut out = Vec::new();
        // In the dark, only what the player is holding.
        if self.in_darkness() {
            out.push(PLAYER);
            self.collect_visible(PLAYER, &mut out);
            return out;
        }
        let here = self.here();
        let ceiling = self.ceiling();
        out.push(ceiling);
        self.collect_visible(ceiling, &mut out);
        if ceiling == here {
            for d in self.doors_of(here) {
                if !out.contains(&d) && !self.is_hidden(d) {
                    out.push(d);
                }
            }
        }
        if !out.contains(&PLAYER) {
            out.push(PLAYER);
            self.collect_visible(PLAYER, &mut out);
        }
        out
    }

    fn collect_visible(&self, holder: EntityId, out: &mut Vec<EntityId>) {
        for c in self.contents(holder) {
            // Carried things are always in scope, even if a script forgot to
            // clear `hidden`: the player is holding them.
            let carried = c == PLAYER || self.is_within(c, PLAYER);
            if (self.is_hidden(c) && !carried) || out.contains(&c) {
                continue;
            }
            out.push(c);
            if self.shows_contents(c) {
                self.collect_visible(c, out);
            }
        }
    }

    /// Doors that this room's exits go through. A door belongs to every
    /// room it connects, not just the one its `in:` names.
    pub fn doors_of(&self, room: EntityId) -> Vec<EntityId> {
        let mut out = Vec::new();
        for e in &self.def(room).exits {
            if let Some(d) = e.through
                && !out.contains(&d)
            {
                out.push(d);
            }
        }
        out
    }

    /// Everything physically present in a room: its contents, plus the
    /// doors its exits use (a door's other side is present too).
    pub fn present(&self, room: EntityId) -> Vec<EntityId> {
        let mut out = self.contents(room);
        for d in self.doors_of(room) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }

    /// Whether `door` is blocked when passing through it from `from`.
    /// `blocked` is `true` (both sides), a room key, or a list of room keys.
    pub fn door_blocked(&self, door: EntityId, from: EntityId) -> Option<Blocked> {
        let key = &self.def(from).key;
        match self.get(door, "blocked")? {
            Value::Bool(true) => Some(Blocked::Both),
            Value::Str(s) if s == key => Some(Blocked::ThisSide),
            Value::List(sides) if sides.iter().any(|v| matches!(v, Value::Str(s) if s == key)) => {
                Some(Blocked::ThisSide)
            }
            _ => None,
        }
    }

    /// Hidden entities that doing `verb` to `target` reveals.
    pub fn reveals(&self, verb: &str, target: EntityId) -> Vec<EntityId> {
        self.ids()
            .filter(|&id| self.is_hidden(id))
            .filter(|&id| self.def(id).revealed_by.iter().any(|r| r.target == target && r.verbs.iter().any(|v| v == verb)))
            .collect()
    }

    /// Whether an exit can be seen and used: one through a hidden (secret)
    /// door can't be, until the door is found.
    pub fn exit_visible(&self, e: &Exit) -> bool {
        e.through.is_none_or(|d| !self.is_hidden(d))
    }

    /// Whether a door is blocked from any side at all.
    pub fn blocked_anywhere(&self, door: EntityId) -> bool {
        match self.get(door, "blocked") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Str(s)) => !s.is_empty(),
            Some(Value::List(l)) => !l.is_empty(),
            _ => false,
        }
    }

    /// A blocked door is held shut, so a door that has become blocked
    /// while open (a script barred it) is closed. Called after scripts run.
    pub fn enforce_door_rules(&mut self) {
        for i in 0..self.story.entities.len() {
            let id = EntityId(i as u32);
            if self.def(id).has(Trait::Door) && self.flag(id, "open") && self.blocked_anywhere(id) {
                self.set(id, "open", Value::Bool(false));
            }
        }
    }

    pub fn in_scope(&self, id: EntityId) -> bool {
        self.scope().contains(&id)
    }

    /// Things the player is carrying directly.
    pub fn inventory(&self) -> Vec<EntityId> {
        self.contents(PLAYER)
    }

    // --------------------------------------------------------------- exits

    pub fn exit(&self, room: EntityId, dir: Direction) -> Option<&Exit> {
        self.def(room).exits.iter().find(|e| e.direction == dir)
    }

    // ---------------------------------------------------- undo, save, load

    /// Call before each turn that changes state. UNDO goes back to the most
    /// recent checkpoint.
    pub fn checkpoint(&mut self) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(self.state.clone());
    }

    /// Records a state taken before a turn, so UNDO can return to it.
    pub fn push_undo(&mut self, state: GameState) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(state);
    }

    /// Returns false when there's nothing to undo.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop_back() {
            Some(s) => {
                self.state = s;
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn save(&self) -> String {
        SaveFile::new(&self.story, &self.state).to_json()
    }

    /// Replaces the current state with a saved one. Undo history is cleared.
    pub fn restore(&mut self, json: &str) -> Result<(), SaveError> {
        let save = SaveFile::from_json(&self.story, json)?;
        self.state = save.state;
        self.undo.clear();
        Ok(())
    }

    pub fn restart(&mut self, seed: u64) {
        self.state = GameState::new(&self.story, seed);
        self.undo.clear();
    }
}
