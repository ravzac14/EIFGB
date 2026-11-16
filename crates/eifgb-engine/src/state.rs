//! The mutable game state, and save files.

use crate::rng::Rng;
use eifgb_ir::{ChoiceId, EndingId, EntityId, Story, Value, IR_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Everything that can change during play. Cloning it is how undo works.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub turn: u64,
    pub score: i64,
    pub vars: BTreeMap<String, Value>,
    /// Indexed like `Story::entities`.
    pub entities: Vec<EntityState>,
    pub visited: BTreeSet<EntityId>,
    pub rng: Rng,
    /// Set once the game has ended.
    pub ending: Option<EndingId>,
    /// The choice graph currently open, if any. While one is open, input
    /// goes to it instead of to the parser.
    pub choice: Option<ChoiceCursor>,
    /// Objects the most recent command referred to. Pronouns like "it" resolve to these.
    pub mentioned: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub location: Option<EntityId>,
    pub state: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceCursor {
    pub choice: ChoiceId,
    pub node: u32,
}

impl GameState {
    pub fn new(story: &Story, seed: u64) -> Self {
        let mut visited = BTreeSet::new();
        visited.insert(story.meta.start);
        GameState {
            turn: 0,
            score: 0,
            vars: story.vars.clone(),
            entities: story
                .entities
                .iter()
                .map(|e| EntityState { location: e.location, state: e.state.clone() })
                .collect(),
            visited,
            rng: Rng::new(seed),
            ending: None,
            choice: None,
            mentioned: Vec::new(),
        }
    }
}

pub const SAVE_FORMAT: &str = "eifgb-save/1";

/// A save game: the state, plus enough about the story to refuse loading
/// it into a different game or an incompatible version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub format: String,
    pub story_id: String,
    pub story_version: String,
    pub ir_version: u32,
    pub state: GameState,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    Malformed(String),
    WrongFormat(String),
    WrongStory { expected: String, found: String },
    WrongVersion { expected: String, found: String },
    /// The entity count differs, so the save can't belong to this build of the story.
    Incompatible,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Malformed(e) => write!(f, "the save file is damaged: {e}"),
            SaveError::WrongFormat(v) => write!(f, "unsupported save format '{v}'"),
            SaveError::WrongStory { expected, found } => {
                write!(f, "this save is for '{found}', not '{expected}'")
            }
            SaveError::WrongVersion { expected, found } => {
                write!(f, "this save is from version {found} of the game; this is version {expected}")
            }
            SaveError::Incompatible => write!(f, "this save doesn't match this build of the game"),
        }
    }
}

impl std::error::Error for SaveError {}

impl SaveFile {
    pub fn new(story: &Story, state: &GameState) -> Self {
        SaveFile {
            format: SAVE_FORMAT.into(),
            story_id: story.meta.id.clone(),
            story_version: story.meta.version.clone(),
            ir_version: IR_VERSION,
            state: state.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("game state always serializes")
    }

    /// Parses a save and checks that it belongs to `story`.
    ///
    /// Saves from a different version of the same story are refused for
    /// now. Migrating them is possible later, by matching entities on
    /// their keys instead of their indices.
    pub fn from_json(story: &Story, json: &str) -> Result<SaveFile, SaveError> {
        let save: SaveFile = serde_json::from_str(json).map_err(|e| SaveError::Malformed(e.to_string()))?;
        if save.format != SAVE_FORMAT {
            return Err(SaveError::WrongFormat(save.format));
        }
        if save.story_id != story.meta.id {
            return Err(SaveError::WrongStory { expected: story.meta.id.clone(), found: save.story_id });
        }
        if save.story_version != story.meta.version {
            return Err(SaveError::WrongVersion {
                expected: story.meta.version.clone(),
                found: save.story_version,
            });
        }
        if save.state.entities.len() != story.entities.len() {
            return Err(SaveError::Incompatible);
        }
        Ok(save)
    }
}
