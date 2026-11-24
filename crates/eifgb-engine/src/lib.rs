//! EIFGB engine.
//!
//! The rule that keeps save, restore and undo simple is that **everything
//! that can change during play lives in [`GameState`]**. The [`Story`] is
//! immutable. Scripts may hold functions, but any story state they keep
//! must go through `vars` or entity state. So saving is serializing
//! `GameState`, and undo is restoring an earlier copy of it.
//!
//! Modules:
//! * [`state`]: the mutable game state, and save files;
//! * [`world`]: world-model queries and changes (location, scope, doors);
//! * [`rng`]: a deterministic, serializable random number generator;
//! * [`script`]: the [`ScriptHost`] interface that script languages implement;
//! * [`text`]: template rendering and reflowing prose;
//! * [`parser`]: turns input into commands, with typo correction and clarifying questions;
//! * [`game`]: the turn loop, which is the entry point for hosts;
//! * [`npc`]: what characters do on their turn: moving and fighting;
//! * [`journal`]: goals for the JOURNAL, and the time of day;
//! * `recall`: WHERE IS, RECAP, UNDO 3, OOPS, AGAIN WITH and ASK ABOUT a topic;
//! * [`lang`]: language rules such as stemming, articles and lists.

mod actions;
pub mod game;
pub mod journal;
pub mod lang;
pub mod npc;
mod people;
pub mod tracks;
mod peek;
mod fire;
mod errands;
mod recall;
mod nature;
pub mod parser;
pub mod rng;
pub mod script;
pub mod state;
pub mod testing;
pub mod text;
mod tricks;
mod cunning;
pub mod world;

pub use game::{Game, Line, LineKind, Missed, Output, Status};

pub use eifgb_ir as ir;
pub use script::{Arg, Ctx, Effect, Outcome, ScriptError, ScriptHost, ScriptValue};
pub use state::{GameState, SaveError, SaveFile};
pub use journal::ClockReading;
pub use npc::NpcEvent;
pub use world::{Blocked, Fit, MoveError, World};
