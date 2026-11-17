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
}

/// Something that happens after a number of actions (turns that took time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub key: String,
    pub schedule: Schedule,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Code>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<EndingId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    /// Once, this many actions after starting.
    After(u32),
    /// Every this many actions, until stopped.
    Every(u32),
}

impl Schedule {
    pub fn actions(self) -> u32 {
        match self {
            Schedule::After(n) | Schedule::Every(n) => n,
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
}

/// Game-wide switches from the source's `settings:` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Describe built-in states (open, closed, locked, worn) automatically
    /// when examining, in room descriptions and in lists of things.
    pub describe_state: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { describe_state: true }
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
}

/// A character's turn-by-turn behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
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
    /// The player can get in or on it (a wardrobe, a bed, a boat). Needs
    /// `container` or `supporter`.
    Enterable,
    /// A container whose contents can be seen, but not reached, while closed.
    Transparent,
}

impl Trait {
    pub const ALL: [Trait; 12] = [
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
        Trait::Enterable,
        Trait::Transparent,
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
            Trait::Enterable => "enterable",
            Trait::Transparent => "transparent",
        }
    }

    pub fn from_key(s: &str) -> Option<Trait> {
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
