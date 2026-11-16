//! The interface between the engine and a script language.
//!
//! The engine never depends on a particular Lua implementation. Each
//! target supplies a [`ScriptHost`]: `eifgb-lua` wraps C Lua 5.4 via
//! mlua for native builds, and the web build may use a different host.
//! Any host must provide the same script API (`world`, `vars`, `say`,
//! `block`, …), and a host-independent test suite checks that it does.
//!
//! Scripts may change the world directly: entity state, locations and
//! vars. Everything else they do (printing text, scoring, ending the
//! game, opening a choice) comes back as [`Effect`]s, in order, for the
//! engine to carry out. The engine therefore keeps control of output
//! and turn flow, and a host never has to call back into the engine.

use crate::world::World;
use eifgb_ir::{Code, EntityId, Story, Value};

/// A value passed to a script as `self` or in `args`.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Entity(EntityId),
    Value(Value),
}

/// What a script can see besides the world: `self` and `args`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ctx {
    pub this: Option<EntityId>,
    pub args: Vec<(String, Arg)>,
}

impl Ctx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn this(mut self, id: EntityId) -> Self {
        self.this = Some(id);
        self
    }

    pub fn entity(mut self, name: &str, id: EntityId) -> Self {
        self.args.push((name.to_string(), Arg::Entity(id)));
        self
    }

    pub fn value(mut self, name: &str, v: Value) -> Self {
        self.args.push((name.to_string(), Arg::Value(v)));
        self
    }

    pub fn text(self, name: &str, s: impl Into<String>) -> Self {
        self.value(name, Value::Str(s.into()))
    }
}

/// The value a script snippet returned.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Entity(EntityId),
    /// `return block(...)`: stop the default behaviour.
    Block,
    /// A value that has no meaning to the engine, such as a table or function.
    Other(&'static str),
}

impl ScriptValue {
    /// Lua truthiness: only nil and false are false.
    pub fn truthy(&self) -> bool {
        !matches!(self, ScriptValue::Nil | ScriptValue::Bool(false))
    }

    /// How the value appears when inserted into text with `${...}`.
    pub fn to_text(&self, world: &World) -> String {
        match self {
            ScriptValue::Nil => String::new(),
            ScriptValue::Bool(b) => b.to_string(),
            ScriptValue::Int(i) => i.to_string(),
            ScriptValue::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
            ScriptValue::Float(f) => f.to_string(),
            ScriptValue::Str(s) => s.clone(),
            ScriptValue::Entity(id) => world.def(*id).name.clone(),
            ScriptValue::Block => String::new(),
            ScriptValue::Other(kind) => format!("[{kind}]"),
        }
    }
}

/// Something a script asked for that the engine carries out after the script returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Say(String),
    /// Points were added. `GameState::score` has already been updated, so
    /// the engine only needs to announce it.
    Score(i64),
    EndGame(String),
    OpenChoice(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub value: ScriptValue,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// Where the failing snippet came from, e.g. `objects.lamp.on.light`.
    pub origin: String,
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "script error in {}: {}", self.origin, self.message)
    }
}

impl std::error::Error for ScriptError {}

pub trait ScriptHost {
    /// Loads the story's script files. Call this once, before any `call`.
    fn load(&mut self, story: &Story) -> Result<(), ScriptError>;

    /// Runs a snippet and returns its value. A snippet that parses as an
    /// expression is evaluated as one; otherwise it runs as a block of
    /// statements, and its value is whatever it `return`s.
    fn call(&mut self, world: &mut World, code: &Code, ctx: &Ctx) -> Result<Outcome, ScriptError>;
}
