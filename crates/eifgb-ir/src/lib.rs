//! Story IR: the compiled, normalized form of a game.
//!
//! The compiler produces a [`Story`] and every runtime target reads one.
//! Compared with the source format, the IR:
//!
//! * contains no syntax sugar: every reference is a resolved numeric ID
//!   and every template has already been parsed into segments;
//! * is self-contained: the pinned standard library (verbs, lexicon and
//!   messages) is copied into it, so a story behaves the same no matter
//!   which engine release runs it;
//! * is language-neutral in structure: every piece of player-facing text
//!   is data, and none of it lives in engine code.
//!
//! Bump [`IR_VERSION`] whenever a change would make an older engine
//! misread a newer story.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const IR_VERSION: u32 = 1;

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(
    /// Index into [`Story::entities`].
    EntityId
);
id_type!(
    /// Index into [`Story::choices`].
    ChoiceId
);
id_type!(
    /// Index into [`Story::endings`].
    EndingId
);

/// The player is always entity 0.
pub const PLAYER: EntityId = EntityId(0);
pub const PLAYER_KEY: &str = "player";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub ir_version: u32,
    pub meta: Meta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<Text>,
    /// Initial values of global story variables.
    pub vars: BTreeMap<String, Value>,
    /// Every entity in the game: the player, rooms, objects and characters.
    /// Entity 0 is always the player.
    pub entities: Vec<Entity>,
    /// Verbs in the order they are matched: author verbs first, then the
    /// standard library's.
    pub verbs: Vec<Verb>,
    pub lexicon: Lexicon,
    /// Message catalog: the standard library's messages, with any author
    /// overrides already applied.
    pub messages: BTreeMap<String, Text>,
    pub choices: Vec<ChoiceGraph>,
    /// Global event handlers.
    pub handlers: Vec<Handler>,
    pub endings: Vec<Ending>,
    /// Lua chunks loaded once, in order, before any handler runs.
    pub scripts: Vec<Script>,
    /// Action categories from the stdlib: name → verb keys.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub categories: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timers: Vec<Timer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factions: Vec<Faction>,
    /// Hints for HINT, in the order they're offered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<Hint>,
    /// Goals for the JOURNAL, in the order they're listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<Goal>,
    /// The time of day, if the game keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<Clock>,
    /// The weather, if the game has some: outdoor rooms describe it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<Weather>,
}

/// The weather: kinds, the one it starts as, and how often it changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub start: String,
    pub kinds: Vec<WeatherKind>,
    /// Changes to another kind, at random, every this many actions (and up
    /// to `spread` more). None: only scripts change it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<u32>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub spread: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherKind {
    pub key: String,
    /// Added to outdoor rooms' descriptions: "Rain drums on the leaves."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describe: Option<Text>,
    /// Said, outdoors, when the weather turns to this: "It starts to rain."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub begins: Option<Text>,
}

/// Something the player is trying to do, listed by JOURNAL. It's added
/// once `when` is true (no `when`: from the start), and done once `done`
/// is true or a script calls `complete_goal`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub key: String,
    pub text: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<Code>,
    /// Points scored when it's done.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub points: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// Minutes in a day.
pub const DAY: u32 = 24 * 60;

/// A clock that moves with the turns. Times are minutes after midnight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    /// The time the game starts.
    pub start: u32,
    /// Minutes each action takes.
    pub per_turn: u32,
    /// Night is from `dusk` until `dawn`.
    pub dawn: u32,
    pub dusk: u32,
    /// Whether the status line shows the time.
    pub show: bool,
}

impl Clock {
    /// Whether a time of day (minutes after midnight) is at night.
    pub fn is_night(&self, at: u32) -> bool {
        let at = at % DAY;
        if self.dawn <= self.dusk { at < self.dawn || at >= self.dusk } else { at >= self.dusk && at < self.dawn }
    }

    /// "night", "morning", "afternoon" or "evening".
    pub fn period(&self, at: u32) -> &'static str {
        let at = at % DAY;
        if self.is_night(at) {
            "night"
        } else if at < 12 * 60 {
            "morning"
        } else if at < 17 * 60 {
            "afternoon"
        } else {
            "evening"
        }
    }
}

/// A time of day as authors write it, in minutes after midnight: "7:30",
/// "07:30", "19:00", "7:30 pm", "7pm", "noon", "midnight", or an hour ("7").
pub fn parse_time(text: &str) -> Result<u32, String> {
    let bad = |s: &str| format!("'{s}' isn't a time of day; write it like \"7:30\", \"19:00\" or \"7:30 pm\"");
    let s = text.trim().to_ascii_lowercase();
    match s.as_str() {
        "noon" | "midday" => return Ok(12 * 60),
        "midnight" => return Ok(0),
        _ => {}
    }
    let (clock, half) = if let Some(c) = s.strip_suffix("am") {
        (c.trim().trim_end_matches('.'), Some(false))
    } else if let Some(c) = s.strip_suffix("pm") {
        (c.trim().trim_end_matches('.'), Some(true))
    } else if let Some(c) = s.strip_suffix("a.m.").or(s.strip_suffix("p.m.")) {
        (c.trim(), Some(s.ends_with("p.m.")))
    } else {
        (s.as_str(), None)
    };
    let (h, m) = match clock.split_once(':').or(clock.split_once('.')) {
        Some((h, m)) if m.len() == 2 => (h.parse::<u32>().ok(), m.parse::<u32>().ok()),
        Some(_) => (None, None),
        None => (clock.parse::<u32>().ok(), Some(0)),
    };
    let (Some(mut h), Some(m)) = (h, m) else { return Err(bad(&s)) };
    if m > 59 {
        return Err(bad(&s));
    }
    match half {
        Some(pm) => {
            if h == 0 || h > 12 {
                return Err(bad(&s));
            }
            h = h % 12 + if pm { 12 } else { 0 };
        }
        None if h > 23 => return Err(bad(&s)),
        None => {}
    }
    Ok(h * 60 + m)
}

/// A time of day as players read it: "7:05 am", "12:30 pm".
pub fn time_text(at: u32) -> String {
    let at = at % DAY;
    let (h, m) = (at / 60, at % 60);
    let h12 = match h % 12 {
        0 => 12,
        n => n,
    };
    format!("{h12}:{m:02} {}", if h < 12 { "am" } else { "pm" })
}

/// A puzzle's hints, from a nudge to the answer. HINT shows the next one
/// for the first puzzle whose `when` is true (no `when`: always).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hint {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    pub steps: Vec<Text>,
}

/// A side in a fight. Hunters and guards attack whoever their faction is
/// hostile to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faction {
    pub key: String,
    /// Faction keys, and/or [`PLAYER_FACTION`] for the player.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hostile_to: Vec<String>,
    /// Members eat the corpses of the fallen.
    #[serde(default)]
    pub eats_corpses: bool,
    /// How members' victims look, unless the weapon or character says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kills: Option<String>,
    /// What members' tracks look like ("paw prints").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prints: Option<String>,
    /// Members see in unlit rooms.
    #[serde(default)]
    pub sees_in_dark: bool,
    /// Members flee below this percent of their health, or when outnumbered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morale: Option<u32>,
    /// What members fear: object keys, or `fire`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fears: Vec<String>,
    /// The sounds members answer when the player mimics them ("howl").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<String>,
    /// The faction's starting attitude toward a side it isn't hostile to,
    /// including a player disguised as one of its own: how many of its
    /// members the player can turn into foes before the whole faction
    /// turns on them.
    #[serde(default = "two")]
    pub patience: u32,
}

fn two() -> u32 {
    2
}

/// The name `hostile_to` uses for the player.
pub const PLAYER_FACTION: &str = "player";

/// Something that happens after a number of actions (turns that took time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub key: String,
    pub schedule: Schedule,
    /// For `after` and `every`: up to this many more actions, picked at
    /// random each time it's set going (`every: 5-10` is 5 with a spread of 5).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub spread: u32,
    /// Whether it's counting from the start of the game.
    pub running: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub started_by: Vec<ActionRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stopped_by: Vec<ActionRule>,
    /// Checked when due; false skips this firing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say: Option<Text>,
    /// Said instead of `say` every time after the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say_again: Option<Text>,
    /// The chance, in percent, that it goes off when it's due. None: always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chance: Option<u32>,
    /// Rooms (or things, heard in whatever room they're in) where `say`
    /// is shown. Empty: everywhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heard_in: Vec<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<EndingId>,
    /// A noise that hunters within earshot come to investigate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise: Option<Noise>,
}

/// A noise: where it is, and how many rooms away it can be heard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Noise {
    /// A room, or a thing whose room it is when the noise happens. None
    /// means wherever the player is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<EntityId>,
    pub loudness: u32,
    /// Only this faction's hunters answer it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    /// Once, this many actions after starting.
    After(u32),
    /// Every this many actions, until stopped.
    Every(u32),
    /// Each day when the clock reaches this time (minutes after midnight).
    At(u32),
}

impl Schedule {
    pub fn actions(self) -> u32 {
        match self {
            Schedule::After(n) | Schedule::Every(n) => n,
            Schedule::At(_) => 0,
        }
    }
}

impl Story {
    pub fn entity(&self, id: EntityId) -> &Entity {
        &self.entities[id.index()]
    }

    pub fn entity_by_key(&self, key: &str) -> Option<EntityId> {
        self.entities
            .iter()
            .position(|e| e.key == key)
            .map(|i| EntityId(i as u32))
    }

    pub fn choice(&self, id: ChoiceId) -> &ChoiceGraph {
        &self.choices[id.index()]
    }

    pub fn ending_by_key(&self, key: &str) -> Option<EndingId> {
        self.endings
            .iter()
            .position(|e| e.key == key)
            .map(|i| EndingId(i as u32))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub version: String,
    /// BCP 47-style language tag. It chooses the parser grammar module.
    pub language: String,
    /// The standard library version that was copied into this story.
    pub stdlib: u32,
    pub start: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_score: Option<i64>,
    #[serde(default)]
    pub ai_fallback: bool,
    #[serde(default)]
    pub settings: Settings,
    /// How the game looks on the website (its colour theme and accent).
    /// Only the web player reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Presentation>,
}

/// The website's look for a game, from the source's `presentation:` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

/// The website's colour themes a game can choose.
pub const THEMES: [&str; 7] = ["light", "dark", "sepia", "parchment", "terminal", "amber", "contrast"];

/// Game-wide switches from the source's `settings:` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Describe built-in states (open, closed, locked, worn) automatically
    /// when examining, in room descriptions and in lists of things.
    pub describe_state: bool,
    /// How many rooms away the player's SHOUT carries (`player.voice`).
    #[serde(default = "two")]
    pub voice: u32,
    /// How many actions tracks last before fading. 0 means no tracks.
    #[serde(default = "thirty")]
    pub track_life: u32,
    /// End room descriptions with the ways out ("Known exits: north.").
    #[serde(default = "yes")]
    pub list_exits: bool,
}

fn thirty() -> u32 {
    30
}

impl Default for Settings {
    fn default() -> Self {
        Settings { describe_state: true, voice: 2, track_life: 30, list_exits: true }
    }
}

// ------------------------------------------------------------------ entities

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Player,
    Room,
    Object,
    Character,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// The author's identifier. Lua scripts see the entity as `world.<key>`.
    pub key: String,
    pub kind: EntityKind,
    /// Display name, e.g. "brass key".
    pub name: String,
    #[serde(default, skip_serializing_if = "Vocab::is_empty")]
    pub vocab: Vocab,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Text>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<Trait>,
    /// Initial mutable state. Built-in traits read well-known fields:
    /// `open`, `locked`, `hidden`, `worn`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub state: BTreeMap<String, Value>,
    /// Initial location. `None` means off-stage, which is always true for rooms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exits: Vec<Exit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlocks_with: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub talk: Option<ChoiceId>,
    /// How much this can hold: the total `size` of what's directly in or
    /// on it (or, for the player, carried). No limit when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u32>,
    /// How much room this takes up in a container or in the player's hands.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub size: u32,
    /// A picture: shown above a room's description, or when a thing is examined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<Picture>,
    /// Actions that reveal this entity while it's `hidden`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revealed_by: Vec<ActionRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handlers: Vec<Handler>,
    /// How a character acts on its turn. Only characters have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npc: Option<Npc>,
    /// Full health. The current value is the `health` state field. Things
    /// without it can't be hurt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<u32>,
    /// Damage dealt by an attack: a character's or the player's bare hands,
    /// or a weapon's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<u32>,
    /// What a merchant sells this for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<u32>,
    /// A character's side, a key in `Story::factions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    /// Worn by the player, this faction treats them as one of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disguise: Option<String>,
    /// Factions whose members eat this when it's left lying in their room.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eaten_by: Vec<String>,
    /// How its victims look: "run through" for a spear, "mauled" for a
    /// bear. Completes "The goblin has been …".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kills: Option<String>,
    /// The player can give it a name (CALL THE DOG REX).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nameable: bool,
    /// Objects: up out of reach (a high shelf, and whatever is on it),
    /// unless the player is standing on something.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub high: bool,
    /// Objects: the player can find it by feel in the dark without having
    /// seen it first (a light switch by the door, a candle by the bed).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub findable_in_dark: bool,
    /// Light sources: lit only from a flame (a candle, a torch), not on
    /// their own (a lighter, a match, a torch with a switch).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_flame: bool,
    /// Light sources: how many actions it burns for; the `fuel` state
    /// counts down while it's lit. None: for ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<u32>,
    /// Light sources: gone when its fuel runs out (a match), rather than out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub burns_away: bool,
    /// Said when its fuel runs out, instead of the standard message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spent: Option<Text>,
    /// What LISTEN hears of it, here or (for a room) from next door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<Text>,
    /// What SMELL smells of it, here or (for a room) from next door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smell: Option<Text>,
    /// Where a liquid comes from (a stream, a well, a barrel): its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_source: Option<String>,
    /// Can be filled with a liquid (a bottle, a bucket); the `liquid` state
    /// says with what.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub holds_liquid: bool,
    /// How many times it can be used before it breaks (the `uses` state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uses: Option<u32>,
    /// Said when it breaks, instead of the standard message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breaks: Option<Text>,
    /// Locked with a code (a safe, a keypad door): what to type to unlock it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// What's written on it, for READ: a book, a note, a sign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writing: Option<Text>,
    /// Rooms: out of doors, where the weather is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outdoors: bool,
    /// What its tracks look like ("paw prints"). Characters only; None means
    /// its faction's, or "footprints".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prints: Option<String>,
    /// Characters: false for ghosts, birds and others that leave no tracks.
    /// Rooms: false for a stream or bare rock that holds none.
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub leaves_tracks: bool,
    /// Rooms: soft ground, where things can be buried.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub diggable: bool,
    /// Rooms: whether this room's description lists its exits, overriding
    /// `settings.list_exits`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_exits: Option<bool>,
    /// Rooms: unlit, so peeking in shows nothing unless a light is burning.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dark: bool,
    /// Objects: worn by the player, lets them see in the dark.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub night_vision: bool,
    /// Objects: the noise it makes when blown, sounded, played or rung
    /// (`at` is unused: it's wherever the object is).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<Noise>,
    /// Objects: a trap, which holds whoever walks into it while armed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap: Option<TrapSpec>,
    /// Objects: puts out a fire (water, sand), and is used up doing it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub douses: bool,
}

/// How a trap works. Its damage, if any, is the object's `damage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrapSpec {
    /// How many turns it holds what it catches.
    pub holds: u32,
}

/// A character's turn-by-turn behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Npc {
    pub behavior: Behavior,
    /// Percent chance of moving to a neighbouring room on a turn with
    /// nothing better to do.
    #[serde(default)]
    pub wander: u32,
    /// Rooms it may move between. Empty means anywhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area: Vec<EntityId>,
    /// How many rooms away a hunter or follower notices the player.
    /// None means from anywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<u32>,
    /// Percent of an item's price a merchant pays when buying from the player.
    #[serde(default)]
    pub buys_at: u32,
    /// The author's own algorithm, run instead of the behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act: Option<Code>,
    /// Whether it leaves a body when defeated, instead of vanishing.
    #[serde(default = "yes")]
    pub corpse: bool,
    /// Whether it attacks back whoever attacked it (its `foes`), once its
    /// patience runs out.
    #[serde(default = "yes")]
    pub fights_back: bool,
    /// How many attacks it puts up with before fighting back. The
    /// remaining patience is the `patience` state field.
    #[serde(default)]
    pub patience: u32,
    /// Never fights: not with its faction, and not back. Attacks on it
    /// still count against the attacker with its faction.
    #[serde(default)]
    pub pacifist: bool,
    /// Can't be hurt: attacks do no damage. For characters the story needs.
    #[serde(default)]
    pub invincible: bool,
    /// How many apologies it accepts over the whole game.
    #[serde(default = "one_u32")]
    pub forgiveness: u32,
    /// How far below its prices a merchant will go when haggling, in
    /// percent. 0 means fixed prices.
    #[serde(default)]
    pub haggle: u32,
    /// Things that tame it when given: it then follows the player and
    /// fights the player's enemies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tame_with: Vec<EntityId>,
    /// Gifts it welcomes: like an apology, without the limit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub likes: Vec<EntityId>,
    /// Money that buys it off: it stops attacking the player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bribe: Option<u32>,
    /// Tamed or a follower, it fetches what the player throws.
    #[serde(default)]
    pub fetches: bool,
    /// Follows the player's fresh tracks when it loses sight of them.
    #[serde(default)]
    pub tracker: bool,
    /// Sees in unlit rooms. Without it, it's as blind there as the player.
    #[serde(default)]
    pub sees_in_dark: bool,
    /// Things it says to others when it thinks nobody's listening.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chatter: Vec<Chatter>,
    /// Takes orders from the player (tamed characters always do).
    #[serde(default)]
    pub obeys: bool,
    /// Flees below this percent of its health, or when outnumbered. None
    /// means its faction's, or never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morale: Option<u32>,
    /// Things it's afraid of: object keys, or `fire` for anything burning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fears: Vec<String>,
}

/// A line of chatter between characters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chatter {
    pub text: Text,
    /// Loud enough to hear from the next room.
    #[serde(default)]
    pub loud: bool,
    /// Said only while this holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    /// Said every time it comes round, instead of once.
    #[serde(default)]
    pub repeat: bool,
}

fn one_u32() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behavior {
    /// Never moves.
    Stationary,
    /// Never moves, so the player can always find them to talk to.
    Conversationalist,
    /// Wanders between rooms.
    Wanderer,
    /// Trades with the player (BUY, SELL, WARES), and may wander.
    Merchant,
    /// Seeks the player out and attacks them.
    Hunter,
    /// Stays put and attacks the player when they're in the same room.
    Guard,
    /// Follows the player from room to room.
    Follower,
    /// Never starts a fight, but fights back against anyone who attacks it.
    Defender,
}

impl Behavior {
    pub const ALL: &[Behavior] = &[
        Behavior::Stationary,
        Behavior::Conversationalist,
        Behavior::Wanderer,
        Behavior::Merchant,
        Behavior::Hunter,
        Behavior::Guard,
        Behavior::Follower,
        Behavior::Defender,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Behavior::Stationary => "stationary",
            Behavior::Conversationalist => "conversationalist",
            Behavior::Wanderer => "wanderer",
            Behavior::Merchant => "merchant",
            Behavior::Hunter => "hunter",
            Behavior::Guard => "guard",
            Behavior::Follower => "follower",
            Behavior::Defender => "defender",
        }
    }

    pub fn from_key(s: &str) -> Option<Behavior> {
        Behavior::ALL.iter().copied().find(|b| b.key() == s)
    }

    /// The `wander` chance when the author doesn't give one.
    pub fn default_wander(self) -> u32 {
        match self {
            Behavior::Wanderer => 50,
            Behavior::Merchant | Behavior::Hunter => 25,
            _ => 0,
        }
    }

    /// Whether it attacks the player.
    pub fn hostile(self) -> bool {
        matches!(self, Behavior::Hunter | Behavior::Guard)
    }

    /// Whether it stays out of its faction's fights with the player,
    /// unless the author says otherwise.
    pub fn default_pacifist(self) -> bool {
        matches!(self, Behavior::Merchant | Behavior::Conversationalist)
    }

    /// How many attacks it takes before fighting back, unless the author
    /// says otherwise: none for fighters, a couple for everyone else.
    pub fn default_patience(self) -> u32 {
        match self {
            Behavior::Hunter | Behavior::Guard | Behavior::Defender => 0,
            _ => 2,
        }
    }
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

/// A picture: a file in the game's folder (`pictures/hall.jpg`) or a web address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Picture {
    /// As written: a path relative to the game's folder, or an `https://` address.
    pub file: String,
    /// Words for players who can't see it: screen readers, and the terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

impl Entity {
    pub fn has(&self, t: Trait) -> bool {
        self.traits.contains(&t)
    }
}

/// "Doing any of `verbs` to `target`", e.g. SEARCH MAT. Used by
/// `revealed_by` and by timers' `started_by`/`stopped_by`. A category
/// such as `(physical action)` is expanded into its verbs at compile
/// time, so the pinned stdlib decides what it covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRule {
    pub verbs: Vec<String>,
    /// The category the rule was written with, kept for display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub target: EntityId,
}

/// Words the parser accepts for an entity. Everything is lowercase.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vocab {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nouns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjectives: Vec<String>,
}

impl Vocab {
    pub fn is_empty(&self) -> bool {
        self.nouns.is_empty() && self.adjectives.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trait {
    Portable,
    Scenery,
    Container,
    Supporter,
    Openable,
    Lockable,
    Door,
    Wearable,
    LightSource,
    Edible,
    /// Something to drink: a potion, a flask of water, a fountain. Drinking
    /// a portable one uses it up.
    Drinkable,
    /// The player can get in or on it (a wardrobe, a bed, a boat). Needs
    /// `container` or `supporter`.
    Enterable,
    /// A container whose contents can be seen, but not reached, while closed.
    Transparent,
    /// Can be set alight.
    Flammable,
    /// Can be smashed, and burns away completely if it's flammable too.
    Destructible,
    /// Too heavy to carry, but can be pushed into the next room (PUSH CRATE NORTH).
    Pushable,
}

impl Trait {
    pub const ALL: [Trait; 16] = [
        Trait::Portable,
        Trait::Scenery,
        Trait::Container,
        Trait::Supporter,
        Trait::Openable,
        Trait::Lockable,
        Trait::Door,
        Trait::Wearable,
        Trait::LightSource,
        Trait::Edible,
        Trait::Drinkable,
        Trait::Enterable,
        Trait::Transparent,
        Trait::Flammable,
        Trait::Destructible,
        Trait::Pushable,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Trait::Portable => "portable",
            Trait::Scenery => "scenery",
            Trait::Container => "container",
            Trait::Supporter => "supporter",
            Trait::Openable => "openable",
            Trait::Lockable => "lockable",
            Trait::Door => "door",
            Trait::Wearable => "wearable",
            Trait::LightSource => "light_source",
            Trait::Edible => "edible",
            Trait::Drinkable => "drinkable",
            Trait::Enterable => "enterable",
            Trait::Transparent => "transparent",
            Trait::Flammable => "flammable",
            Trait::Destructible => "destructible",
            Trait::Pushable => "pushable",
        }
    }

    pub fn from_key(s: &str) -> Option<Trait> {
        // A common spelling, accepted too.
        if s == "destructable" {
            return Some(Trait::Destructible);
        }
        Trait::ALL.into_iter().find(|t| t.key() == s)
    }
}

/// A direction is an ID, not a word. Each language's lexicon maps its
/// words onto these IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Northeast,
    Northwest,
    Southeast,
    Southwest,
    Up,
    Down,
    In,
    Out,
}

impl Direction {
    pub const ALL: [Direction; 12] = [
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
        Direction::Northeast,
        Direction::Northwest,
        Direction::Southeast,
        Direction::Southwest,
        Direction::Up,
        Direction::Down,
        Direction::In,
        Direction::Out,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::South => "south",
            Direction::East => "east",
            Direction::West => "west",
            Direction::Northeast => "northeast",
            Direction::Northwest => "northwest",
            Direction::Southeast => "southeast",
            Direction::Southwest => "southwest",
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::In => "in",
            Direction::Out => "out",
        }
    }

    pub fn from_key(s: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.key() == s)
    }

    pub fn opposite(self) -> Direction {
        use Direction::*;
        match self {
            North => South,
            South => North,
            East => West,
            West => East,
            Northeast => Southwest,
            Southwest => Northeast,
            Northwest => Southeast,
            Southeast => Northwest,
            Up => Down,
            Down => Up,
            In => Out,
            Out => In,
        }
    }
}

/// One exit from a room.
///
/// * `to` set, no `when`: an ordinary exit, optionally through a door.
/// * `to` set, with `when`: the exit works only while `when` is true;
///   otherwise the game prints `blocked` (or a default message).
/// * `to` unset: the exit is always blocked, and the game prints `blocked`.
///
/// A room may have several exits in the same direction, in source order.
/// When the player goes that way, entries with `to` whose `when` holds are
/// candidates. One candidate is taken; several (each through a different
/// door) make the game ask which door is meant. With no candidates, the
/// first `blocked` message among the entries is shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exit {
    pub direction: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Text>,
    /// Things the way goes by, so the player can say how they go: CLIMB
    /// THE STAIRS, GO DOWN THE PATH, CROSS THE BRIDGE.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<EntityId>,
}

// --------------------------------------------------------------- text & code

/// A parsed text template. Single newlines inside a literal are soft
/// wraps from the source file, and the engine reflows them. A blank line
/// starts a new paragraph.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Text(pub Vec<Segment>);

impl Text {
    pub fn literal(s: impl Into<String>) -> Text {
        Text(vec![Segment::Lit(s.into())])
    }

    /// The text when it contains no script segments, otherwise `None`.
    pub fn as_literal(&self) -> Option<String> {
        let mut out = String::new();
        for seg in &self.0 {
            match seg {
                Segment::Lit(s) => out.push_str(s),
                _ => return None,
            }
        }
        Some(out)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Segment {
    Lit(String),
    /// `${expr}`: the value of a Lua expression, converted to text.
    Expr(Code),
    /// `${if cond}...${elseif c}...${else}...${end}`. An `elseif` chain
    /// is stored as nested `If`s in `otherwise`.
    If {
        cond: Code,
        then: Text,
        #[serde(default, skip_serializing_if = "is_empty_text")]
        otherwise: Text,
    },
}

fn is_empty_text(t: &Text) -> bool {
    t.0.is_empty()
}

/// A Lua snippet. Whether it is an expression or a block of statements
/// is settled when the snippet is loaded, the way Lua's REPL does it:
/// the script host first tries to load `return <src>`, and if that is a
/// syntax error, loads `<src>` as statements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Code {
    pub src: String,
    /// Where the snippet came from in the source, e.g.
    /// `objects.great_lamp.on.light`. Used in error messages.
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    pub name: String,
    pub src: String,
}

// --------------------------------------------------------------------- verbs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verb {
    /// Verb ID. Handlers refer to it as `before_<key>`, `<key>` and `after_<key>`.
    pub key: String,
    pub patterns: Vec<Pattern>,
    pub action: VerbAction,
    /// Reply when nothing handles a custom verb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Text>,
    pub origin: VerbOrigin,
    /// Whether the player must be able to touch the target and tool, not
    /// just see them. Things in a closed `transparent` container can be
    /// seen but not reached.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub reach: bool,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl BuiltinAction {
    /// Whether this built-in behaviour needs the player to touch the thing.
    /// Looking, listening and moving about don't.
    pub fn needs_reach(self) -> bool {
        use BuiltinAction as A;
        matches!(
            self,
            A::Take
                | A::PutIn
                | A::PutOn
                | A::Open
                | A::Close
                | A::Lock
                | A::Unlock
                | A::Wear
                | A::TakeOff
                | A::Eat
                | A::Enter
                | A::Empty
                | A::Attack
                | A::Light
                | A::Extinguish
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbOrigin {
    Stdlib,
    Author,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbAction {
    /// The engine has default behaviour for this verb, which handlers can
    /// wrap or replace.
    Builtin(BuiltinAction),
    /// No default behaviour: only handlers act, otherwise `default` is printed.
    Script,
}

/// Behaviour the engine implements itself. New stdlib releases can add
/// variants. Changing what an existing variant does needs a new stdlib
/// version, so that stories pinned to the old version keep its behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinAction {
    Look,
    Examine,
    Inventory,
    Go,
    Take,
    Drop,
    PutIn,
    PutOn,
    Open,
    Close,
    Lock,
    Unlock,
    Search,
    Talk,
    Wear,
    TakeOff,
    Eat,
    Wait,
    Score,
    Undo,
    Save,
    Restore,
    Restart,
    Help,
    /// Go through a door, or get into or onto an `enterable` thing.
    Enter,
    /// Get out of or off whatever the player is in or on.
    GetOut,
    /// Tip a container's contents out, or into/onto something else.
    Empty,
    /// Repeat the previous command.
    Again,
    Quit,
    /// Attack something with health, bare-handed or with a weapon.
    Attack,
    /// Buy something a merchant here is selling.
    Buy,
    /// Sell something to a merchant here.
    Sell,
    /// List what the merchants here are selling.
    Wares,
    /// Say how healthy the player is.
    Health,
    /// Make a noise that hunters within earshot come to investigate.
    Shout,
    /// Apologise to a character, to win back its patience and its faction's.
    Apologize,
    /// Offer a merchant a price for something.
    Offer,
    /// Take the deal a merchant has on the table.
    Accept,
    /// Give something to a character: a gift, or something that tames it.
    Give,
    /// Give a character money, to buy it off.
    Bribe,
    /// Throw something at someone, or in a direction.
    Throw,
    /// Give something `nameable` a name.
    Rename,
    /// Look for tracks here.
    Tracks,
    /// Follow the freshest tracks here.
    FollowTracks,
    /// Bury something (a body, usually) in soft ground.
    Bury,
    /// Move quietly, without waking sleepers.
    Sneak,
    /// Get an impression of the next room, through a door or a way out.
    Peek,
    /// Light a `light_source`.
    Light,
    /// Put out a `light_source`.
    Extinguish,
    /// Take something a character is carrying, without being caught.
    Steal,
    /// Listen for talk in the next room (or rooms).
    Eavesdrop,
    /// Tell a companion what to do: TELL REX TO STAY.
    Order,
    /// Wave something about, to frighten whoever fears it.
    Brandish,
    /// Set something flammable alight.
    Burn,
    /// Put a fire out with water, sand…
    Douse,
    /// Blow, sound, play or ring something that `calls`.
    Sound,
    /// Mimic a creature's call: HOWL, IMITATE A WOLF.
    Mimic,
    /// Set a trap here.
    SetTrap,
    /// Make a trap safe.
    DisarmTrap,
    /// Push something `pushable` into the next room, and follow it.
    PushTo,
    /// Swap something you carry for something a merchant sells.
    Trade,
    /// The next hint for the puzzle the player is on.
    Hint,
    /// The player's goals.
    Journal,
    /// What time it is.
    Time,
    /// Use something, alone or on something else: USE KEY ON DOOR. Does
    /// whatever the thing is for (unlock, light, wear, eat, open…).
    Use,
    /// Walk to a place visited before, by the way the player knows: GO TO
    /// THE STUDY. One turn a step, stopping if anything happens on the way.
    GoTo,
    /// Go back to the room the player was in before.
    GoBack,
    /// Wait for some turns, or minutes or hours on the clock: WAIT 3.
    WaitFor,
    /// Wait until a time of day: WAIT UNTIL DUSK.
    WaitUntil,
    /// List the ways out of here.
    Exits,
    /// Who else is here.
    Who,
    /// What the player could try here.
    Suggest,
    /// The game's title, author and version.
    About,
    /// Drink something `drinkable`.
    Drink,
    /// Climb something: a way out that goes `via` it, or something to get onto.
    Climb,
    /// Wake a sleeping character.
    Wake,
    /// Feel around in the dark for what's here.
    FeelAround,
    /// Feel around for something, and take it: FEEL FOR THE KEY.
    FeelFor,
    /// READ a book, a note, a sign: its `writing`.
    Read,
    /// LISTEN: sounds here and next door.
    Listen,
    /// SMELL: smells here and next door.
    Smell,
    /// FILL a bottle from a stream.
    Fill,
    /// POUR what's in something out, or on something.
    Pour,
    /// TYPE 4512 (on the keypad): a code.
    EnterCode,
    /// The player's notes.
    Notes,
    /// Erase a note (or all of them).
    EraseNote,
    /// WHERE IS THE KEY: where the player last saw something.
    WhereIs,
    /// OOPS LANTERN: the misspelled word in the last command, fixed.
    Oops,
    /// Describe every room in full, every time.
    Verbose,
    /// Describe rooms in full only the first time.
    Brief,
    /// What's happened so far, and where things stand.
    Recap,
}

impl BuiltinAction {
    /// Out-of-world commands: they never take a turn and can't be undone.
    pub fn is_meta(self) -> bool {
        matches!(
            self,
            BuiltinAction::Score
                | BuiltinAction::Undo
                | BuiltinAction::Save
                | BuiltinAction::Restore
                | BuiltinAction::Restart
                | BuiltinAction::Help
                | BuiltinAction::Hint
                | BuiltinAction::Journal
                | BuiltinAction::Time
                | BuiltinAction::Exits
                | BuiltinAction::Who
                | BuiltinAction::Suggest
                | BuiltinAction::About
                | BuiltinAction::Notes
                | BuiltinAction::EraseNote
                | BuiltinAction::WhereIs
                | BuiltinAction::Oops
                | BuiltinAction::Verbose
                | BuiltinAction::Brief
                | BuiltinAction::Recap
                | BuiltinAction::Again
                | BuiltinAction::Quit
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pattern(pub Vec<PatternToken>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternToken {
    Word(String),
    Slot(Slot),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    /// The direct object, available to scripts as `args.target`.
    Target,
    /// The indirect object, available as `args.tool`.
    Tool,
    /// Free text, available as `args.text`.
    Text,
    /// A direction word, available as `args.direction`.
    Direction,
}

impl Slot {
    pub fn key(self) -> &'static str {
        match self {
            Slot::Target => "target",
            Slot::Tool => "tool",
            Slot::Text => "text",
            Slot::Direction => "direction",
        }
    }

    pub fn from_key(s: &str) -> Option<Slot> {
        [Slot::Target, Slot::Tool, Slot::Text, Slot::Direction]
            .into_iter()
            .find(|k| k.key() == s)
    }
}

/// Language-specific word lists the parser uses. Everything is lowercase.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lexicon {
    /// Synonym phrase → canonical phrase, e.g. `grab` → `take` or
    /// `peer at` → `examine`. The parser tries the longest phrase first.
    #[serde(default)]
    pub phrases: BTreeMap<String, String>,
    #[serde(default)]
    pub directions: BTreeMap<String, Direction>,
    /// Words dropped before matching, e.g. "the" or "a".
    #[serde(default)]
    pub articles: Vec<String>,
    /// Phrases dropped from the start of input, e.g. "please" or "try to".
    #[serde(default)]
    pub filler: Vec<String>,
    /// Words that refer back to the most recently mentioned object.
    #[serde(default)]
    pub pronouns: Vec<String>,
    /// Words that mean "everything", e.g. "all".
    #[serde(default)]
    pub all_words: Vec<String>,
    /// Words that refer to the player, e.g. "me" or "myself".
    #[serde(default)]
    pub self_words: Vec<String>,
    /// Common words ("is", "with", "there") that never refer to anything.
    /// Other words in descriptions let players refer to what they read:
    /// "x glint" after "Something glints at the bottom."
    #[serde(default)]
    pub stopwords: Vec<String>,
    /// Words for a dead character's body: "corpse", "body".
    #[serde(default)]
    pub corpse_words: Vec<String>,
    /// Words that join things or commands: "take the key and the lamp",
    /// "take the key and open the door".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub and_words: Vec<String>,
    /// Words that leave things out of "all": "take all but the key".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub except_words: Vec<String>,
    /// Words for the room the player is in: "x room", "search here".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub room_words: Vec<String>,
    /// Number words: "wait three turns".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub numbers: BTreeMap<String, u32>,
    /// Units of waiting: word → "turn", "minute" or "hour".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub durations: BTreeMap<String, String>,
    /// Times of day by name: word → "dawn", "dusk" or a time ("12:00").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub times: BTreeMap<String, String>,
    /// How a note starts: NOTE THE CODE IS 4512. The rest of the line, as
    /// typed, is the note.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_words: Vec<String>,
    /// "yes", "y": the answer to "Did you mean…?".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub yes_words: Vec<String>,
    /// "no", "nope": turning down a "Did you mean…?" (not "n", which is north).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_words: Vec<String>,
    /// "my hat": things the player has come first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub my_words: Vec<String>,
    /// "the brass one": stands in for a thing's name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub one_words: Vec<String>,
    /// "the other one": not the one just mentioned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_words: Vec<String>,
    /// "the second key": word → position, from 1.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ordinals: BTreeMap<String, u32>,
    /// Words that change nothing, dropped anywhere: "look closely at it".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adverbs: Vec<String>,
    /// Phrases dropped from the end of a command when it isn't understood
    /// with them: "save game", "go east please".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailing: Vec<String>,
    /// The game's own words for words the game knows, anywhere in a
    /// command: `using` → `with`, `port` → `west`. From the `aliases`
    /// section of a game.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
}

// ------------------------------------------------------------------ handlers

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Before,
    On,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handler {
    pub phase: Phase,
    /// A verb key, or one of the events in [`GLOBAL_EVENTS`] or [`ROOM_EVENTS`].
    pub event: String,
    pub code: Code,
}

/// Events that aren't verbs, and where each may be declared.
pub const GLOBAL_EVENTS: &[&str] = &["start", "turn", "parse_failed", "died"];
/// Events on a character: `defeated` runs when its health reaches 0.
pub const CHARACTER_EVENTS: &[&str] = &["defeated"];
pub const ROOM_EVENTS: &[&str] = &["enter", "leave", "turn"];

// ------------------------------------------------------------------- choices

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceGraph {
    pub key: String,
    /// Index into `nodes`.
    pub start: u32,
    pub nodes: Vec<ChoiceNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceNode {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Text>,
    /// Runs when the node is entered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Code>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ChoiceOption>,
    /// Only used when there are no options: where to go after showing the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Next>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub text: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Code>,
    pub next: Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Next {
    /// Index of a node in the same graph.
    Goto(u32),
    End,
    /// Show the current node again.
    Stay,
}

// ------------------------------------------------------------------- endings

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ending {
    pub key: String,
    pub kind: EndingKind,
    pub text: Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndingKind {
    Win,
    Loss,
    Neutral,
}

// -------------------------------------------------------------------- values

/// A story value. Variables and entity state use these, so this enum
/// defines everything that can be saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Float(_) => "number",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Map(_) => "map",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_through_json() {
        let v: Value = serde_json::from_str(r#"{"a": [1, 2.5, true, "x"]}"#).unwrap();
        let Value::Map(m) = &v else { panic!("expected map") };
        assert_eq!(
            m["a"],
            Value::List(vec![
                Value::Int(1),
                Value::Float(2.5),
                Value::Bool(true),
                Value::Str("x".into())
            ])
        );
        let back: Value = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn segments_serialize_compactly() {
        let t = Text(vec![
            Segment::Lit("It is ".into()),
            Segment::If {
                cond: Code { src: "self.open".into(), origin: "t".into() },
                then: Text::literal("open"),
                otherwise: Text::literal("shut"),
            },
        ]);
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.starts_with(r#"[{"lit":"It is "},{"if":"#), "{json}");
        let back: Text = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn trait_and_direction_keys_round_trip() {
        for t in Trait::ALL {
            assert_eq!(Trait::from_key(t.key()), Some(t));
        }
        for d in Direction::ALL {
            assert_eq!(Direction::from_key(d.key()), Some(d));
        }
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn times_are_read_the_ways_people_write_them() {
        for (s, m) in [
            ("7:30", 450),
            ("07:30", 450),
            ("19:00", 1140),
            ("7:30 pm", 1170),
            ("7pm", 1140),
            ("7:30 a.m.", 450),
            ("12 am", 0),
            ("12:15 pm", 735),
            ("noon", 720),
            ("Midnight", 0),
            ("7", 420),
        ] {
            assert_eq!(parse_time(s), Ok(m), "{s}");
        }
        for s in ["25:00", "7:60", "13 pm", "teatime", "7:5", ""] {
            assert!(parse_time(s).is_err(), "{s}");
        }
        assert_eq!(time_text(0), "12:00 am");
        assert_eq!(time_text(735), "12:15 pm");
        assert_eq!(time_text(1170 + DAY), "7:30 pm");
    }

    #[test]
    fn night_is_from_dusk_to_dawn() {
        let c = Clock { start: 0, per_turn: 1, dawn: 360, dusk: 1080, show: true };
        assert!(c.is_night(0) && c.is_night(359) && !c.is_night(360) && c.is_night(1080));
        assert_eq!([c.period(400), c.period(800), c.period(1050), c.period(1100)], ["morning", "afternoon", "evening", "night"]);
    }
}
