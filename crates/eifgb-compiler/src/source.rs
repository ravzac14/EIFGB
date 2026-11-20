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
    /// How the game looks on the website. The engine ignores it.
    #[serde(default)]
    pub presentation: Option<Presentation>,
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
    #[serde(default)]
    pub hints: IndexMap<String, Hint>,
    #[serde(default)]
    pub goals: IndexMap<String, Goal>,
    #[serde(default)]
    pub clock: Option<Clock>,
    #[serde(default)]
    pub weather: Option<Weather>,
    /// Overrides for standard library messages, by key.
    #[serde(default)]
    pub messages: IndexMap<String, String>,
    /// The game's own words for words the game knows: `using: with`.
    #[serde(default)]
    pub aliases: IndexMap<String, String>,
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
pub struct Presentation {
    /// One of the website's colour themes.
    #[serde(default)]
    pub theme: Option<String>,
    /// A colour for links, the prompt and highlights: "#c33".
    #[serde(default)]
    pub accent: Option<String>,
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
    /// End room descriptions with "Known exits: …". On by default.
    #[serde(default = "yes")]
    pub list_exits: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { describe_state: true, track_life: None, list_exits: true }
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
    /// Sees in unlit rooms (the `sees_in_dark` state, which scripts can change).
    #[serde(default)]
    pub sees_in_dark: bool,
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
    /// Soft ground: BURY works here.
    #[serde(default)]
    pub diggable: bool,
    /// Unlit: peeking in shows nothing unless something is burning.
    #[serde(default)]
    pub dark: bool,
    /// Whether this room's description lists its exits (overrides `settings.list_exits`).
    #[serde(default)]
    pub list_exits: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exits: IndexMap<String, Exit>,
    #[serde(default)]
    pub on: IndexMap<String, String>,
    #[serde(default)]
    pub picture: Option<PictureSrc>,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub smell: Option<String>,
    #[serde(default)]
    pub outdoors: bool,
}

/// A code, written as a number or as text: `code: 4512`, `code: "0042"`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CodeText {
    Number(u64),
    Text(String),
}

/// `weather:` the kinds there are, the first, and how often it changes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weather {
    pub start: String,
    pub kinds: IndexMap<String, WeatherKind>,
    #[serde(default)]
    pub changes: Option<Actions>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherKind {
    #[serde(default)]
    pub describe: Option<String>,
    #[serde(default)]
    pub begins: Option<String>,
}

/// A chance in percent: `25`, or `"25%"`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Chance {
    Number(f64),
    Text(String),
}

/// A number of actions: `5`, or a range to pick from at random, `5-10`
/// (also `5 to 10`, `5..10`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Actions {
    Exactly(i64),
    Text(String),
}

/// `picture: hall.jpg`, or `picture: { file: hall.jpg, alt: "..." }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PictureSrc {
    File(String),
    Full {
        file: String,
        #[serde(default)]
        alt: Option<String>,
    },
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
    /// Things the way goes by: `via: stairs` lets CLIMB THE STAIRS go this way.
    #[serde(default)]
    pub via: Option<OneOrMany>,
}

/// An object or a character. Only characters may use `talk`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thing {
    pub name: String,
    #[serde(default)]
    pub picture: Option<PictureSrc>,
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
    /// Objects: up out of reach unless the player stands on something.
    #[serde(default)]
    pub high: bool,
    /// Objects: found by feel in the dark, even if never seen.
    #[serde(default)]
    pub findable_in_dark: bool,
    #[serde(default)]
    pub needs_flame: bool,
    #[serde(default)]
    pub fuel: Option<u32>,
    #[serde(default)]
    pub burns_away: bool,
    #[serde(default)]
    pub spent: Option<String>,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub smell: Option<String>,
    /// A liquid comes from it: `liquid: water`.
    #[serde(default)]
    pub liquid: Option<String>,
    #[serde(default)]
    pub holds_liquid: bool,
    /// Starts filled with this liquid.
    #[serde(default)]
    pub full_of: Option<String>,
    #[serde(default)]
    pub uses: Option<u32>,
    #[serde(default)]
    pub breaks: Option<String>,
    #[serde(default)]
    pub code: Option<CodeText>,
    /// What's written on it, for READ.
    #[serde(default)]
    pub text: Option<String>,
    /// What its tracks look like ("paw prints").
    #[serde(default)]
    pub prints: Option<String>,
    /// False for characters that leave no tracks.
    #[serde(default)]
    pub leaves_tracks: Option<bool>,
    /// Follows the player's fresh tracks.
    #[serde(default)]
    pub tracker: Option<bool>,
    /// Sees in unlit rooms.
    #[serde(default)]
    pub sees_in_dark: Option<bool>,
    /// Lines it says to others when it thinks nobody's listening.
    #[serde(default)]
    pub chatter: Vec<ChatterLine>,
    /// Takes orders from the player.
    #[serde(default)]
    pub obeys: Option<bool>,
    /// Flees below this percent of its health, or when outnumbered.
    #[serde(default)]
    pub morale: Option<u32>,
    /// Things it fears: object IDs, or `fire`.
    #[serde(default)]
    pub fears: Vec<String>,
    /// Objects: the noise it makes when blown, sounded, played or rung.
    #[serde(default)]
    pub calls: Option<Noise>,
    /// Objects: a trap. `{ holds: 3 }`: turns it holds what it catches.
    #[serde(default)]
    pub trap: Option<Trap>,
    /// Objects: puts out fires (water, sand), used up doing it.
    #[serde(default)]
    pub douses: bool,
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

/// A line of chatter: just the words, or `{ say, loud, when, repeat }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ChatterLine {
    Plain(String),
    Full(ChatterFull),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatterFull {
    pub say: String,
    #[serde(default)]
    pub loud: bool,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub repeat: bool,
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
    /// Members see in unlit rooms.
    #[serde(default)]
    pub sees_in_dark: bool,
    /// Members flee below this percent of their health, or when outnumbered.
    #[serde(default)]
    pub morale: Option<u32>,
    /// What members fear: object IDs, or `fire`.
    #[serde(default)]
    pub fears: Vec<String>,
    /// Sounds members answer when the player mimics them: [howl].
    #[serde(default)]
    pub calls: Vec<String>,
    /// How many members the player can provoke before the faction turns
    /// on them (as themselves, or in its disguise). Defaults to 2.
    #[serde(default)]
    pub patience: Option<u32>,
}

/// A puzzle's hints: `steps` from a nudge to the answer, offered while
/// `when` (Lua) is true.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hint {
    #[serde(default)]
    pub when: Option<String>,
    pub steps: Vec<String>,
}

/// A goal for the JOURNAL: `text`, listed once `when` (Lua) is true, and
/// done once `done` (Lua) is true.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub text: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub done: Option<String>,
    #[serde(default)]
    pub points: Option<i64>,
}

/// The time of day. Times are written like "7:30", "19:00", "7:30 pm",
/// "noon" or "midnight".
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    pub start: TimeOfDay,
    #[serde(default)]
    pub minutes_per_turn: Option<u32>,
    #[serde(default)]
    pub dawn: Option<TimeOfDay>,
    #[serde(default)]
    pub dusk: Option<TimeOfDay>,
    /// Shows the time on the status line (the default).
    #[serde(default)]
    pub show: Option<bool>,
}

/// A time as written: "7:30", or a bare hour like 7 (YAML reads 7:30 as text).
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum TimeOfDay {
    Text(String),
    Hour(u32),
}

/// Something that happens after a number of actions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timer {
    /// Fires once, this many actions after it starts: `5`, or a random
    /// number in a range, `5-10`.
    #[serde(default)]
    pub after: Option<Actions>,
    /// Fires every this many actions (or a random number in a range, each
    /// time), until stopped.
    #[serde(default)]
    pub every: Option<Actions>,
    /// Fires each day when the clock reaches this time ("19:00", "dusk").
    #[serde(default)]
    pub at: Option<TimeOfDay>,
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
    /// Said instead of `say` from the second time it goes off.
    #[serde(default)]
    pub say_again: Option<String>,
    /// The chance it goes off when due: `25%` or `25`.
    #[serde(default)]
    pub chance: Option<Chance>,
    /// Where it's heard: a room, or a thing (heard wherever it is), or a
    /// list. Anywhere else it goes off unannounced.
    #[serde(default)]
    pub heard_in: Option<OneOrMany>,
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
pub struct Trap {
    pub holds: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    /// A room or thing. Defaults to wherever the player is.
    #[serde(default)]
    pub at: Option<String>,
    /// How many rooms away it carries.
    pub loudness: u32,
    /// Only this faction's hunters answer it.
    #[serde(default)]
    pub faction: Option<String>,
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
