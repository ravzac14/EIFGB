//! The world model: an immutable [`Story`] plus the mutable [`GameState`],
//! with the queries and changes that the parser, actions and scripts share.

use crate::state::{GameState, SaveError, SaveFile};
use eifgb_ir::{Direction, Entity, EntityId, EntityKind, Exit, Story, Trait, Value, PLAYER};
use std::collections::VecDeque;

/// How many turns UNDO can go back.
pub const UNDO_LIMIT: usize = 100;

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
}

impl World {
    pub fn new(story: Story, seed: u64) -> Self {
        let state = GameState::new(&story, seed);
        World { story, state, undo: VecDeque::new() }
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

    pub fn is_hidden(&self, id: EntityId) -> bool {
        self.flag(id, "hidden")
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

    // --------------------------------------------------------------- scope

    /// Everything the player can currently refer to: the room, its visible
    /// contents (descending into open containers and supporters), the
    /// player and what they carry. Hidden things are left out, except
    /// for things the player carries.
    pub fn scope(&self) -> Vec<EntityId> {
        let mut out = Vec::new();
        let here = self.here();
        out.push(here);
        self.collect_visible(here, &mut out);
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
            if self.exposes_contents(c) {
                self.collect_visible(c, out);
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
