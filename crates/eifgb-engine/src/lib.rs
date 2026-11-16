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
//! * [`rng`]: a deterministic, serializable random number generator.
//!
//! Still to come: the parser pipeline, the action rules and the Lua host.

pub mod rng;
pub mod state;
pub mod world;

pub use eifgb_ir as ir;
pub use state::{GameState, SaveError, SaveFile};
pub use world::{MoveError, World};
