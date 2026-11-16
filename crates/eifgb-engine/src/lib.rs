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
//! * [`text`]: template rendering and reflowing prose.
//!
//! Still to come: the parser pipeline and the action rules.

pub mod rng;
pub mod script;
pub mod state;
pub mod text;
pub mod world;

pub use eifgb_ir as ir;
pub use script::{Arg, Ctx, Effect, Outcome, ScriptError, ScriptHost, ScriptValue};
pub use state::{GameState, SaveError, SaveFile};
pub use world::{MoveError, World};
