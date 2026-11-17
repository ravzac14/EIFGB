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
    #[serde(default)]
    pub timers: IndexMap<String, Timer>,
    #[serde(default)]
    pub factions: IndexMap<String, Faction>,
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
    /// How many actions tracks last. 0 turns tracks off. Defaults to 30.
    #[serde(default)]
    pub track_life: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { describe_state: true, track_life: None }
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
    /// How much the player can carry: a number (0 means nothing at all),
    /// or `none` (the default) for no limit.
    #[serde(default)]
    pub capacity: Option<Capacity>,
    #[serde(default)]
    pub size: Option<u32>,
    /// Full health. Without it, enemies' attacks do nothing to the player.
    #[serde(default)]
    pub health: Option<u32>,
    /// Bare-handed damage. Defaults to 1.
    #[serde(default)]
    pub damage: Option<u32>,
    /// Starting money, for buying from merchants.
    #[serde(default)]
    pub money: Option<u32>,
    /// How many rooms away SHOUT can be heard. Defaults to 2.
    #[serde(default)]
    pub voice: Option<u32>,
}

/// A carrying capacity: a whole number, or `none` (or null) for no limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capacity {
    Limit(u32),
    Unlimited,
}

impl Capacity {
    pub fn limit(self) -> Option<u32> {
        match self {
            Capacity::Limit(n) => Some(n),
            Capacity::Unlimited => None,
        }
    }
}

impl<'de> Deserialize<'de> for Capacity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{Error, Unexpected, Visitor};
        struct V;
        impl Visitor<'_> for V {
            type Value = Capacity;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a whole number, or `none` for no limit")
            }
            fn visit_u64<E: Error>(self, v: u64) -> Result<Capacity, E> {
                u32::try_from(v).map(Capacity::Limit).map_err(|_| E::custom("capacity is too large"))
            }
            fn visit_i64<E: Error>(self, v: i64) -> Result<Capacity, E> {
                match u64::try_from(v) {
                    Ok(v) => self.visit_u64(v),
                    Err(_) => Err(E::custom("capacity can't be negative; use 0 to carry nothing, or `none` for no limit")),
                }
            }
            fn visit_str<E: Error>(self, v: &str) -> Result<Capacity, E> {
                if v.eq_ignore_ascii_case("none") {
                    Ok(Capacity::Unlimited)
                } else {
                    Err(E::invalid_value(Unexpected::Str(v), &self))
                }
            }
            fn visit_unit<E: Error>(self) -> Result<Capacity, E> {
                Ok(Capacity::Unlimited)
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub name: String,
    /// A floor that holds no tracks: a stream, bare rock.
    #[serde(default)]
    pub trackless: bool,
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
    pub size: Option<u32>,
    /// `"search mat"` or a list of them: the actions that reveal this
    /// thing while it's hidden.
    #[serde(default)]
    pub revealed_by: Option<OneOrMany>,
    #[serde(default)]
    pub on: IndexMap<String, String>,

    // Characters: how they act, fight and trade.
    /// A built-in behaviour: stationary, conversationalist, wanderer,
    /// merchant, hunter, guard or follower.
    #[serde(default)]
    pub behavior: Option<String>,
    /// Lua run on the character's turn instead of its behaviour.
    #[serde(default)]
    pub act: Option<String>,
    /// Percent chance of moving on a turn with nothing better to do.
    #[serde(default)]
    pub wander: Option<u32>,
    /// Rooms the character keeps to.
    #[serde(default)]
    pub area: Vec<String>,
    /// How many rooms away a hunter or follower notices the player.
    #[serde(default)]
    pub range: Option<u32>,
    /// Percent of the price a merchant pays for things the player sells.
    #[serde(default)]
    pub buys_at: Option<u32>,
    #[serde(default)]
    pub health: Option<u32>,
    /// A character's attack, or a weapon's.
    #[serde(default)]
    pub damage: Option<u32>,
    /// What a merchant charges for this object.
    #[serde(default)]
    pub price: Option<u32>,
    /// A merchant's money. Without it they can always pay.
    #[serde(default)]
    pub money: Option<u32>,
    /// The character's side, from `factions`.
    #[serde(default)]
    pub faction: Option<String>,
    /// Whether it leaves a body when defeated. Defaults to true.
    #[serde(default)]
    pub corpse: Option<bool>,
    /// Whether it attacks back whoever attacks it. Defaults to true.
    #[serde(default)]
    pub fights_back: Option<bool>,
    /// How many attacks it puts up with before fighting back.
    #[serde(default)]
    pub patience: Option<u32>,
    /// Never fights, not even back. Defaults to true for merchants and
    /// conversationalists.
    #[serde(default)]
    pub pacifist: Option<bool>,
    /// Can't be hurt. For characters the story can't do without.
    #[serde(default)]
    pub invincible: Option<bool>,
    /// How many apologies it accepts. Defaults to 1; 0 never forgives.
    #[serde(default)]
    pub forgiveness: Option<u32>,
    /// How far below its prices a merchant goes when haggling (percent).
    #[serde(default)]
    pub haggle: Option<u32>,
    /// Objects that tame it when given to it.
    #[serde(default)]
    pub tame_with: Vec<String>,
    /// Objects it welcomes as gifts.
    #[serde(default)]
    pub likes: Vec<String>,
    /// Money that buys it off.
    #[serde(default)]
    pub bribe: Option<u32>,
    /// Fetches what the player throws (when tamed, or a follower).
    #[serde(default)]
    pub fetches: Option<bool>,
    /// The player can name it: CALL THE DOG REX.
    #[serde(default)]
    pub nameable: bool,
    /// What its tracks look like ("paw prints").
    #[serde(default)]
    pub prints: Option<String>,
    /// False for characters that leave no tracks.
    #[serde(default)]
    pub leaves_tracks: Option<bool>,
    /// Follows the player's fresh tracks.
    #[serde(default)]
    pub tracker: Option<bool>,
    /// Wearable disguise: this faction takes the wearer for one of its own.
    #[serde(default)]
    pub disguise: Option<String>,
    /// Factions that eat this when it's left in their room.
    #[serde(default)]
    pub eaten_by: Vec<String>,
    /// How victims of this weapon or character look: "run through".
    #[serde(default)]
    pub kills: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn items(&self) -> Vec<&str> {
        match self {
            OneOrMany::One(s) => vec![s.as_str()],
            OneOrMany::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verb {
    pub patterns: Vec<String>,
    #[serde(default)]
    pub default: Option<String>,
    /// Whether the player must be able to touch the thing. On by default;
    /// set it to false for verbs like READ that work through glass.
    #[serde(default)]
    pub reach: Option<bool>,
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

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    /// Other factions, or `player`.
    #[serde(default)]
    pub hostile_to: Vec<String>,
    #[serde(default)]
    pub eats_corpses: bool,
    /// How members' victims look: "torn apart by teeth".
    #[serde(default)]
    pub kills: Option<String>,
    /// What members' tracks look like: "paw prints".
    #[serde(default)]
    pub prints: Option<String>,
    /// How many members the player can provoke before the faction turns
    /// on them (as themselves, or in its disguise). Defaults to 2.
    #[serde(default)]
    pub patience: Option<u32>,
}

/// Something that happens after a number of actions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timer {
    /// Fires once, this many actions after it starts.
    #[serde(default)]
    pub after: Option<u32>,
    /// Fires every this many actions, until stopped.
    #[serde(default)]
    pub every: Option<u32>,
    /// Running from the start of the game. Defaults to true unless the
    /// timer has `started_by`.
    #[serde(default)]
    pub running: Option<bool>,
    #[serde(default)]
    pub started_by: Option<OneOrMany>,
    #[serde(default)]
    pub stopped_by: Option<OneOrMany>,
    /// Lua condition checked when the timer is due; false skips it.
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub say: Option<String>,
    #[serde(default, rename = "do")]
    pub run: Option<String>,
    /// Ends the game with this ending.
    #[serde(default)]
    pub end: Option<String>,
    /// A noise hunters come to investigate.
    #[serde(default)]
    pub noise: Option<Noise>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    /// A room or thing. Defaults to wherever the player is.
    #[serde(default)]
    pub at: Option<String>,
    /// How many rooms away it carries.
    pub loudness: u32,
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
