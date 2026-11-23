//! The mutable game state, and save files.

use crate::rng::Rng;
use eifgb_ir::{ChoiceId, EndingId, EntityId, Story, Value, IR_VERSION, PLAYER};
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
    /// Running timers: key → the turn number when it's next due.
    #[serde(default)]
    pub timers: BTreeMap<String, u64>,
    /// Things the player has examined or handled. Only these get state
    /// tags in lists: "a small box (empty)".
    #[serde(default)]
    pub known: BTreeSet<EntityId>,
    /// The clock, in minutes since midnight on the first day (0 when the
    /// game keeps no time).
    #[serde(default)]
    pub clock: u64,
    /// How far `at` timers have been checked, on the same scale as `clock`.
    #[serde(default)]
    pub clock_checked: u64,
    /// Goals in the journal: key → whether it's done.
    #[serde(default)]
    pub goals: BTreeMap<String, bool>,
    /// The room the player last walked out of, for GO BACK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub came_from: Option<EntityId>,
    /// Where the player knows things are: seen there in the light, felt
    /// there in the dark, or put down there. In the dark, they can still
    /// reach what they know is here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub located: BTreeMap<EntityId, EntityId>,
    /// Where the player first came to have each thing: the room (None: they
    /// had it from the start). For WHERE IS and INVENTORY BY PLACE.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub got: BTreeMap<EntityId, Option<EntityId>>,
    /// What was said in each conversation, the last time it was open (by
    /// choice key), and which was the last: WHAT DID SHE SAY?
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub talks: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_talk: Option<String>,
    /// Timers that have gone off at least once (for `say_again`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub fired: BTreeSet<String>,
    /// The weather now (None: the story's `start`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    /// The action when the weather next turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather_due: Option<u64>,
    /// The player's own notes (NOTE …), as typed. Kept with saves, and
    /// through UNDO and RESTART: they're the player's, not the story's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// BRIEF: rooms the player has been to are described in full only the
    /// first time. The player's choice, kept like the notes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub brief: bool,
    /// The last few commands that took a turn, as typed, for RECAP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub location: Option<EntityId>,
    pub state: BTreeMap<String, Value>,
    /// How much it can hold: starts as the story's `capacity` and can be
    /// changed by scripts. `None` means no limit for the player, and the
    /// "nothing bigger than the holder" rule for containers and supporters.
    pub capacity: Option<u32>,
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
        let start = story.clock.as_ref().map_or(0, |c| u64::from(c.start));
        // Timers running from the start; a range picks its first wait now.
        let mut rng = Rng::new(seed);
        let timers = story.timers.iter().filter(|t| t.running).map(|t| (t.key.clone(), crate::game::timer_wait(t, &mut rng))).collect();
        GameState {
            turn: 0,
            score: 0,
            vars: story.vars.clone(),
            entities: story
                .entities
                .iter()
                .map(|e| {
                    // A dark room's darkness is state too, so scripts can read and change it.
                    let mut state = e.state.clone();
                    if e.dark {
                        state.insert("dark".into(), Value::Bool(true));
                    }
                    EntityState { location: e.location, state, capacity: e.capacity }
                })
                .collect(),
            visited,
            rng,
            ending: None,
            choice: None,
            mentioned: Vec::new(),
            timers,
            // The player knows what they start out holding.
            known: story.entities.iter().enumerate().filter(|(_, e)| e.location == Some(PLAYER)).map(|(i, _)| EntityId(i as u32)).collect(),
            clock: start,
            clock_checked: start,
            goals: BTreeMap::new(),
            came_from: None,
            located: BTreeMap::new(),
            fired: BTreeSet::new(),
            weather: None,
            weather_due: None,
            notes: Vec::new(),
            brief: false,
            recent: Vec::new(),
            got: BTreeMap::new(),
            talks: BTreeMap::new(),
            last_talk: None,
        }
    }
}

pub const SAVE_FORMAT: &str = "eifgb-save/2";

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
