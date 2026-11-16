//! The source format as authors write it (format `eifgb/0.1`).
//!
//! These types mirror the YAML closely. They do no validation beyond
//! shape: that happens during lowering, where errors can give a path.
//! `deny_unknown_fields` everywhere means a typo in a field name is an
//! error, not a silently ignored field.

use eifgb_ir::Value;
use indexmap::IndexMap;
use serde::Deserialize;

pub const SUPPORTED_FORMATS: &[&str] = &["eifgb/0.1"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub format: String,
    pub game: Game,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub vars: IndexMap<String, Value>,
    #[serde(default)]
    pub player: Player,
    #[serde(default)]
    pub rooms: IndexMap<String, Room>,
    #[serde(default)]
    pub objects: IndexMap<String, Thing>,
    #[serde(default)]
    pub characters: IndexMap<String, Thing>,
    #[serde(default)]
    pub verbs: IndexMap<String, Verb>,
    #[serde(default)]
    pub choices: IndexMap<String, Choice>,
    #[serde(default)]
    pub on: IndexMap<String, String>,
    #[serde(default)]
    pub endings: IndexMap<String, Ending>,
    /// Overrides for standard library messages, by key.
    #[serde(default)]
    pub messages: IndexMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    pub version: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub stdlib: Option<u32>,
    pub start: String,
    #[serde(default)]
    pub max_score: Option<i64>,
    #[serde(default)]
    pub intro: Option<String>,
    #[serde(default)]
    pub ai_fallback: bool,
}

fn default_language() -> String {
    "en".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Describe open, closed, locked and worn automatically. On by default.
    #[serde(default = "yes")]
    pub describe_state: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { describe_state: true }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub capacity: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exits: IndexMap<String, Exit>,
    #[serde(default)]
    pub on: IndexMap<String, String>,
}

/// An exit is written as a room ID (`north: hall`), a blocked message
/// (`south: "The sea. Not tonight."`), a full map, or a list of these
/// for several exits in the same direction.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Exit {
    Short(String),
    Full(FullExit),
    Many(Vec<OneExit>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneExit {
    Short(String),
    Full(FullExit),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullExit {
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub through: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub blocked: Option<String>,
}

/// An object or a character. Only characters may use `talk`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thing {
    pub name: String,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub adjectives: Vec<String>,
    #[serde(default, rename = "in")]
    pub location: Option<String>,
    #[serde(default)]
    pub traits: Vec<String>,
    #[serde(default)]
    pub state: IndexMap<String, Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub unlocks_with: Option<String>,
    #[serde(default)]
    pub talk: Option<String>,
    #[serde(default)]
    pub capacity: Option<u32>,
    #[serde(default)]
    pub on: IndexMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verb {
    pub patterns: Vec<String>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub start: String,
    pub nodes: IndexMap<String, Node>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "do")]
    pub run: Option<String>,
    #[serde(default)]
    pub options: Vec<ChoiceOption>,
    #[serde(default)]
    pub goto: Option<String>,
    #[serde(default)]
    pub end: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    pub text: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub say: Option<String>,
    #[serde(default, rename = "do")]
    pub run: Option<String>,
    #[serde(default)]
    pub goto: Option<String>,
    #[serde(default)]
    pub end: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ending {
    #[serde(default = "default_ending_kind")]
    pub kind: String,
    pub text: String,
}

fn default_ending_kind() -> String {
    "neutral".to_string()
}
