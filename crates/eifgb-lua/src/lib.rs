//! [`ScriptHost`] backed by C Lua 5.4, via mlua.
//!
//! Sandbox:
//! * Only the `string`, `table` and `math` libraries are loaded, plus a
//!   whitelist of base functions. There is no `io`, `os`, `require`,
//!   `load`, `dofile`, `debug`, `getmetatable`/`setmetatable` or
//!   `string.dump`.
//! * `math.random` is replaced by `random()`, which uses the engine's
//!   seeded RNG, so randomness is saved and undone with everything else.
//! * Each call has an instruction budget and the VM has a memory limit,
//!   so a runaway script fails with an error instead of hanging the game.
//! * Handlers can't create globals, and script files may only define
//!   functions. Story state therefore can't hide in Lua, where saves
//!   wouldn't see it.

use eifgb_engine::script::{Arg, Ctx, Effect, Outcome, ScriptError, ScriptHost, ScriptValue};
use eifgb_engine::World;
use eifgb_ir::{Code, EntityId, EntityKind, Story, Trait, Value};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, VmState};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

const PRELUDE: &str = include_str!("prelude.lua");

/// Instructions one call may run before it is stopped.
pub const INSTRUCTION_BUDGET: u64 = 5_000_000;
const HOOK_EVERY: u32 = 1_000;
/// Memory limit for the whole Lua VM.
pub const MEMORY_LIMIT: usize = 64 * 1024 * 1024;

fn language_module(lang: &str) -> Option<&'static str> {
    match lang {
        "en" => Some(include_str!("lang/en.lua")),
        _ => None,
    }
}

pub struct LuaHost {
    lua: Lua,
    /// Host primitives, refilled for every call.
    h: Table,
    env: Table,
    ent: Function,
    id_of: Function,
    block: Table,
    set_loading: Function,
    install: Function,
    budget: Rc<Cell<u64>>,
    cache: HashMap<(String, String), Function>,
}

fn rt(msg: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(msg.into())
}

/// Turns an mlua error into a message for authors: no Rust wrapper
/// layers, and no traceback.
fn clean(e: &mlua::Error) -> String {
    let msg = match e {
        mlua::Error::CallbackError { cause, .. } => return clean(cause),
        mlua::Error::RuntimeError(m) => m.clone(),
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        mlua::Error::MemoryError(m) => format!("out of memory ({m})"),
        other => other.to_string(),
    };
    match msg.find("\nstack traceback:") {
        Some(i) => msg[..i].to_string(),
        None => msg,
    }
}

impl LuaHost {
    pub fn new() -> Result<Self, ScriptError> {
        Self::build().map_err(|e| ScriptError { origin: "eifgb:prelude".into(), message: clean(&e) })
    }

    fn build() -> mlua::Result<Self> {
        let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH, LuaOptions::new())?;
        lua.set_memory_limit(MEMORY_LIMIT)?;

        let budget = Rc::new(Cell::new(INSTRUCTION_BUDGET));
        let b = budget.clone();
        lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| {
            let left = b.get();
            if left <= u64::from(HOOK_EVERY) {
                b.set(0);
                return Err(rt("the script ran too long and was stopped (is there an endless loop?)"));
            }
            b.set(left - u64::from(HOOK_EVERY));
            Ok(VmState::Continue)
        })?;

        let h = lua.create_table()?;
        let b = budget.clone();
        let exhausted = lua.create_function(move |_, ()| Ok(b.get() == 0))?;
        let api: Table = lua.load(PRELUDE).set_name("=eifgb:prelude").call((h.clone(), exhausted))?;
        Ok(LuaHost {
            env: api.get("env")?,
            ent: api.get("ent")?,
            id_of: api.get("id_of")?,
            block: api.get("block")?,
            set_loading: api.get("set_loading")?,
            install: api.get("install")?,
            lua,
            h,
            budget,
            cache: HashMap::new(),
        })
    }

    /// Compiles a snippet, trying it as an expression first (`return <src>`)
    /// and then as statements. Line 1 of the wrapper is line 1 of the
    /// snippet, so line numbers in errors stay right.
    fn compile(&mut self, code: &Code) -> mlua::Result<Function> {
        let key = (code.origin.clone(), code.src.clone());
        if let Some(f) = self.cache.get(&key) {
            return Ok(f.clone());
        }
        let name = format!("={}", code.origin);
        let load = |src: String| {
            self.lua.load(src).set_name(name.clone()).set_environment(self.env.clone()).into_function()
        };
        let f = match load(format!("local self, args = ...; return {}", code.src)) {
            Ok(f) => f,
            Err(mlua::Error::SyntaxError { .. }) => load(format!("local self, args = ...; {}", code.src))?,
            Err(e) => return Err(e),
        };
        self.cache.insert(key, f.clone());
        Ok(f)
    }

    fn proxy(&self, id: EntityId) -> mlua::Result<mlua::Value> {
        self.ent.call(id.0)
    }

    fn script_value(&self, v: mlua::Value) -> mlua::Result<ScriptValue> {
        Ok(match v {
            mlua::Value::Nil => ScriptValue::Nil,
            mlua::Value::Boolean(b) => ScriptValue::Bool(b),
            mlua::Value::Integer(i) => ScriptValue::Int(i),
            mlua::Value::Number(n) => ScriptValue::Float(n),
            mlua::Value::String(s) => ScriptValue::Str(s.to_string_lossy()),
            mlua::Value::Table(t) => {
                if t.to_pointer() == self.block.to_pointer() {
                    ScriptValue::Block
                } else if let Some(id) = self.id_of.call::<Option<u32>>(t)? {
                    ScriptValue::Entity(EntityId(id))
                } else {
                    ScriptValue::Other("table")
                }
            }
            mlua::Value::Function(_) => ScriptValue::Other("function"),
            _ => ScriptValue::Other("value"),
        })
    }
}

// ---------------------------------------------------------- value conversion

fn to_lua(lua: &Lua, v: &Value) -> mlua::Result<mlua::Value> {
    Ok(match v {
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Int(i) => mlua::Value::Integer(*i),
        Value::Float(f) => mlua::Value::Number(*f),
        Value::Str(s) => mlua::Value::String(lua.create_string(s)?),
        Value::List(items) => {
            let t = lua.create_table_with_capacity(items.len(), 0)?;
            for (i, item) in items.iter().enumerate() {
                t.raw_set(i + 1, to_lua(lua, item)?)?;
            }
            mlua::Value::Table(t)
        }
        Value::Map(m) => {
            let t = lua.create_table_with_capacity(0, m.len())?;
            for (k, item) in m {
                t.raw_set(k.as_str(), to_lua(lua, item)?)?;
            }
            mlua::Value::Table(t)
        }
    })
}

/// Converts a Lua value into a storable [`Value`]. Tables become a list
/// when their keys are exactly 1..n, and otherwise a map with string keys.
/// `x.capacity = n`: a whole number, or nil / "none" for no limit.
fn capacity_from_lua(v: &mlua::Value) -> mlua::Result<Option<u32>> {
    let bad = || rt("capacity must be a whole number (0 or more), or nil or \"none\" for no limit");
    match v {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(s) if s.to_str()?.eq_ignore_ascii_case("none") => Ok(None),
        mlua::Value::Integer(i) => u32::try_from(*i).map(Some).map_err(|_| bad()),
        mlua::Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 && *n <= f64::from(u32::MAX) => Ok(Some(*n as u32)),
        _ => Err(bad()),
    }
}

fn from_lua(v: mlua::Value, depth: usize) -> mlua::Result<Value> {
    if depth > 32 {
        return Err(rt("value is nested too deeply to store (does a table contain itself?)"));
    }
    Ok(match v {
        mlua::Value::Boolean(b) => Value::Bool(b),
        mlua::Value::Integer(i) => Value::Int(i),
        mlua::Value::Number(n) => Value::Float(n),
        mlua::Value::String(s) => Value::Str(s.to_str()?.to_string()),
        mlua::Value::Table(t) => {
            if t.metatable().is_some() {
                return Err(rt("can't store an object or special table in state; store its key instead"));
            }
            let len = t.raw_len();
            let mut count = 0usize;
            let mut map = BTreeMap::new();
            for pair in t.pairs::<mlua::Value, mlua::Value>() {
                let (k, val) = pair?;
                count += 1;
                if let mlua::Value::String(s) = &k {
                    map.insert(s.to_str()?.to_string(), from_lua(val, depth + 1)?);
                }
            }
            if count == len {
                let mut list = Vec::with_capacity(len);
                for i in 1..=len {
                    list.push(from_lua(t.raw_get(i)?, depth + 1)?);
                }
                Value::List(list)
            } else if map.len() == count {
                Value::Map(map)
            } else {
                return Err(rt("tables stored in state must be lists or have only string keys"));
            }
        }
        mlua::Value::Nil => return Err(rt("can't store nil here")),
        other => return Err(rt(format!("can't store a {} in state", other.type_name()))),
    })
}

fn kind_name(k: EntityKind) -> &'static str {
    match k {
        EntityKind::Player => "player",
        EntityKind::Room => "room",
        EntityKind::Object => "object",
        EntityKind::Character => "character",
    }
}

/// Fields every entity has. Scripts can read them but not assign them.
const BUILTIN_FIELDS: &[&str] = &["key", "name", "kind", "location", "proper"];

impl ScriptHost for LuaHost {
    fn load(&mut self, story: &Story) -> Result<(), ScriptError> {
        let lang = &story.meta.language;
        let module = language_module(lang).ok_or_else(|| ScriptError {
            origin: "game.language".into(),
            message: format!("no Lua text helpers for language '{lang}'"),
        })?;
        let wrap = |origin: &str| {
            let origin = origin.to_string();
            move |e: mlua::Error| ScriptError { origin: origin.clone(), message: clean(&e) }
        };
        self.budget.set(INSTRUCTION_BUDGET);
        let helpers: Table = self
            .lua
            .load(module)
            .set_name(format!("=eifgb:lang/{lang}"))
            .call(self.id_of.clone())
            .map_err(wrap("eifgb:lang"))?;
        self.install.call::<()>(helpers).map_err(wrap("eifgb:lang"))?;

        self.set_loading.call::<()>(true).map_err(wrap("eifgb:prelude"))?;
        let result = story.scripts.iter().try_for_each(|s| {
            self.budget.set(INSTRUCTION_BUDGET);
            self.lua
                .load(s.src.as_str())
                .set_name(format!("={}", s.name))
                .set_environment(self.env.clone())
                .exec()
                .map_err(wrap(&format!("scripts:{}", s.name)))
        });
        self.set_loading.call::<()>(false).map_err(wrap("eifgb:prelude"))?;
        result
    }

    fn call(&mut self, world: &mut World, code: &Code, ctx: &Ctx) -> Result<Outcome, ScriptError> {
        let err = |e: mlua::Error| ScriptError { origin: code.origin.clone(), message: clean(&e) };
        let func = self.compile(code).map_err(err)?;
        self.budget.set(INSTRUCTION_BUDGET);

        let world = RefCell::new(world);
        let effects: RefCell<Vec<Effect>> = RefCell::new(Vec::new());
        let (world, effects) = (&world, &effects);
        let entity_count = world.borrow().story().entities.len() as u32;
        let check = move |id: u32| -> mlua::Result<EntityId> {
            if id < entity_count { Ok(EntityId(id)) } else { Err(rt(format!("no entity #{id}"))) }
        };

        let value = self
            .lua
            .scope(|scope| {
                let h = &self.h;

                h.set(
                    "ent_get",
                    scope.create_function(move |lua, (id, k): (u32, String)| {
                        let id = check(id)?;
                        let w = world.borrow();
                        let def = w.def(id);
                        let s = |s: &str| lua.create_string(s).map(mlua::Value::String);
                        Ok(match k.as_str() {
                            "key" => (s(&def.key)?, false),
                            "name" => (s(&w.display_name(id))?, false),
                            "proper" => (mlua::Value::Boolean(w.is_proper(id)), false),
                            "kind" => (s(kind_name(def.kind))?, false),
                            "capacity" => match w.capacity(id) {
                                Some(n) => (mlua::Value::Integer(i64::from(n)), false),
                                None => (mlua::Value::Nil, false),
                            },
                            "location" => match w.location(id) {
                                Some(l) => (mlua::Value::Integer(i64::from(l.0)), true),
                                None => (mlua::Value::Nil, false),
                            },
                            field => match w.get(id, field) {
                                Some(v) => (to_lua(lua, v)?, false),
                                None => (mlua::Value::Nil, false),
                            },
                        })
                    })?,
                )?;

                h.set(
                    "ent_set",
                    scope.create_function(move |_, (id, k, v): (u32, String, mlua::Value)| {
                        let id = check(id)?;
                        if BUILTIN_FIELDS.contains(&k.as_str()) {
                            let hint = if k == "location" { "; use :move_to(place)" } else { "" };
                            return Err(rt(format!("'{k}' can't be changed{hint}")));
                        }
                        let mut w = world.borrow_mut();
                        if k == "capacity" {
                            w.set_capacity(id, capacity_from_lua(&v)?);
                            return Ok(());
                        }
                        if v.is_nil() {
                            w.unset(id, &k);
                        } else {
                            w.set(id, &k, from_lua(v, 0)?);
                        }
                        Ok(())
                    })?,
                )?;

                h.set(
                    "move",
                    scope.create_function(move |_, (id, dest): (u32, Option<u32>)| {
                        let id = check(id)?;
                        let dest = dest.map(check).transpose()?;
                        let mut w = world.borrow_mut();
                        w.move_to(id, dest).map_err(|e| {
                            let name = w.def(id).key.clone();
                            rt(match e {
                                eifgb_engine::MoveError::Immovable => format!("'{name}' can't be moved"),
                                eifgb_engine::MoveError::WouldContainItself => {
                                    format!("can't move '{name}' into itself or something inside it")
                                }
                            })
                        })
                    })?,
                )?;

                h.set(
                    "is_within",
                    scope.create_function(move |_, (id, holder): (u32, u32)| {
                        Ok(world.borrow().is_within(check(id)?, check(holder)?))
                    })?,
                )?;

                h.set(
                    "can_hold",
                    scope.create_function(move |_, (holder, item): (u32, u32)| {
                        let (holder, item) = (check(holder)?, check(item)?);
                        let w = world.borrow();
                        Ok(w.location(item) == Some(holder) || w.fits(item, holder) == eifgb_engine::Fit::Yes)
                    })?,
                )?;

                h.set(
                    "contents",
                    scope.create_function(move |_, id: u32| {
                        Ok(world.borrow().contents(check(id)?).into_iter().map(|e| e.0).collect::<Vec<u32>>())
                    })?,
                )?;

                h.set(
                    "has_trait",
                    scope.create_function(move |_, (id, name): (u32, String)| {
                        let t = Trait::from_key(&name).ok_or_else(|| rt(format!("unknown trait '{name}'")))?;
                        Ok(world.borrow().has(check(id)?, t))
                    })?,
                )?;

                h.set(
                    "lookup",
                    scope.create_function(move |_, key: String| Ok(world.borrow().id(&key).map(|e| e.0)))?,
                )?;

                h.set(
                    "here",
                    scope.create_function(move |_, ()| Ok(world.borrow().here().0))?,
                )?;

                h.set(
                    "var_get",
                    scope.create_function(move |lua, k: String| {
                        let w = world.borrow();
                        match w.state.vars.get(&k) {
                            Some(v) => to_lua(lua, v),
                            None => Err(rt(format!("vars.{k} isn't declared; add it under 'vars:' in the game file"))),
                        }
                    })?,
                )?;

                h.set(
                    "var_set",
                    scope.create_function(move |_, (k, v): (String, mlua::Value)| {
                        let mut w = world.borrow_mut();
                        if !w.state.vars.contains_key(&k) {
                            return Err(rt(format!("vars.{k} isn't declared; add it under 'vars:' in the game file")));
                        }
                        let v = from_lua(v, 0).map_err(|e| rt(format!("vars.{k}: {}", clean(&e))))?;
                        w.state.vars.insert(k, v);
                        Ok(())
                    })?,
                )?;

                h.set(
                    "game_get",
                    scope.create_function(move |lua, k: String| {
                        let w = world.borrow();
                        Ok(match k.as_str() {
                            "turn" => mlua::Value::Integer(w.state.turn as i64),
                            "score" => mlua::Value::Integer(w.state.score),
                            "max_score" => w.story().meta.max_score.map_or(mlua::Value::Nil, mlua::Value::Integer),
                            "title" => mlua::Value::String(lua.create_string(&w.story().meta.title)?),
                            "id" => mlua::Value::String(lua.create_string(&w.story().meta.id)?),
                            other => return Err(rt(format!("game.{other} doesn't exist"))),
                        })
                    })?,
                )?;

                h.set(
                    "say",
                    scope.create_function(move |_, s: String| {
                        effects.borrow_mut().push(Effect::Say(s));
                        Ok(())
                    })?,
                )?;

                h.set(
                    "score",
                    scope.create_function(move |_, n: i64| {
                        world.borrow_mut().state.score += n;
                        effects.borrow_mut().push(Effect::Score(n));
                        Ok(())
                    })?,
                )?;

                h.set(
                    "end_game",
                    scope.create_function(move |_, key: String| {
                        if world.borrow().story().ending_by_key(&key).is_none() {
                            return Err(rt(format!("end_game: there is no ending called '{key}'")));
                        }
                        effects.borrow_mut().push(Effect::EndGame(key));
                        Ok(())
                    })?,
                )?;

                h.set(
                    "open_choice",
                    scope.create_function(move |_, key: String| {
                        if !world.borrow().story().choices.iter().any(|c| c.key == key) {
                            return Err(rt(format!("open_choice: there is no choice called '{key}'")));
                        }
                        effects.borrow_mut().push(Effect::OpenChoice(key));
                        Ok(())
                    })?,
                )?;

                for (name, start) in [("start_timer", true), ("stop_timer", false)] {
                    h.set(
                        name,
                        scope.create_function(move |_, key: String| {
                            if !world.borrow().story().timers.iter().any(|t| t.key == key) {
                                return Err(rt(format!("{name}: there is no timer called '{key}'")));
                            }
                            effects.borrow_mut().push(if start { Effect::StartTimer(key) } else { Effect::StopTimer(key) });
                            Ok(())
                        })?,
                    )?;
                }

                // Characters acting. Each returns whether something happened,
                // and hands the engine an event to describe.
                let push = move |ev: Option<eifgb_engine::NpcEvent>| -> bool {
                    match ev {
                        Some(ev) => {
                            effects.borrow_mut().push(Effect::Npc(ev));
                            true
                        }
                        None => false,
                    }
                };
                h.set(
                    "npc_wander",
                    scope.create_function(move |_, id: u32| {
                        let id = check(id)?;
                        let ev = world.borrow_mut().npc_wander(id);
                        Ok(push(ev))
                    })?,
                )?;
                h.set(
                    "npc_step_toward",
                    scope.create_function(move |_, (id, target): (u32, u32)| {
                        let (id, target) = (check(id)?, check(target)?);
                        let ev = world.borrow_mut().npc_step_toward(id, target);
                        Ok(push(ev))
                    })?,
                )?;
                h.set(
                    "npc_go",
                    scope.create_function(move |_, (id, dir): (u32, String)| {
                        let id = check(id)?;
                        let d = eifgb_engine::ir::Direction::from_key(&dir)
                            .ok_or_else(|| rt(format!("go: '{dir}' isn't a direction")))?;
                        let ev = world.borrow_mut().npc_go(id, d);
                        Ok(push(ev))
                    })?,
                )?;
                h.set(
                    "npc_attack",
                    scope.create_function(move |_, (id, target, weapon): (u32, u32, Option<u32>)| {
                        let (id, target) = (check(id)?, check(target)?);
                        let weapon = weapon.map(check).transpose()?;
                        let ev = world.borrow_mut().attack(id, target, weapon);
                        Ok(push(ev))
                    })?,
                )?;
                h.set(
                    "npc_behave",
                    scope.create_function(move |_, id: u32| {
                        let id = check(id)?;
                        let events = world.borrow_mut().npc_behave(id);
                        let acted = !events.is_empty();
                        effects.borrow_mut().extend(events.into_iter().map(Effect::Npc));
                        Ok(acted)
                    })?,
                )?;
                h.set(
                    "distance",
                    scope.create_function(move |_, (id, target): (u32, u32)| {
                        Ok(world.borrow().distance(check(id)?, check(target)?).map(i64::from))
                    })?,
                )?;
                h.set(
                    "npc_eat",
                    scope.create_function(move |_, (id, corpse): (u32, u32)| {
                        let (id, corpse) = (check(id)?, check(corpse)?);
                        let ev = world.borrow_mut().npc_eat(id, corpse);
                        Ok(push(ev))
                    })?,
                )?;
                h.set(
                    "hostile",
                    scope.create_function(move |_, (a, b): (u32, u32)| Ok(world.borrow().is_hostile(check(a)?, check(b)?)))?,
                )?;
                h.set(
                    "make_noise",
                    scope.create_function(move |_, (at, loudness, faction): (u32, i64, Option<String>)| {
                        let at = check(at)?;
                        let loudness = u32::try_from(loudness).map_err(|_| rt("make_noise: loudness can't be negative"))?;
                        Ok(world.borrow_mut().make_call(at, loudness, faction.as_deref()) as i64)
                    })?,
                )?;
                h.set(
                    "rename",
                    scope.create_function(move |_, (id, name): (u32, String)| {
                        let id = check(id)?;
                        let mut w = world.borrow_mut();
                        if !w.def(id).nameable {
                            return Err(rt(format!("rename: '{}' isn't nameable", w.def(id).key)));
                        }
                        w.rename(id, &name).ok_or_else(|| rt(format!("rename: '{name}' isn't a usable name")))
                    })?,
                )?;
                h.set(
                    "together",
                    scope.create_function(move |_, (a, b): (u32, u32)| Ok(world.borrow().together(check(a)?, check(b)?)))?,
                )?;

                h.set(
                    "timer_left",
                    scope.create_function(move |_, key: String| {
                        let w = world.borrow();
                        if !w.story().timers.iter().any(|t| t.key == key) {
                            return Err(rt(format!("timer_left: there is no timer called '{key}'")));
                        }
                        Ok(w.state.timers.get(&key).map(|due| due.saturating_sub(w.state.turn) as i64))
                    })?,
                )?;

                h.set(
                    "random",
                    scope.create_function(move |_, (lo, hi): (i64, i64)| {
                        Ok(world.borrow_mut().state.rng.range(lo, hi))
                    })?,
                )?;

                let this = match ctx.this {
                    Some(id) => self.proxy(id)?,
                    None => mlua::Value::Nil,
                };
                let args = self.lua.create_table()?;
                for (k, a) in &ctx.args {
                    let v = match a {
                        Arg::Entity(id) => self.proxy(*id)?,
                        Arg::Value(v) => to_lua(&self.lua, v)?,
                    };
                    args.raw_set(k.as_str(), v)?;
                }
                let ret: mlua::Value = func.call((this, args))?;
                self.script_value(ret)
            })
            .map_err(err)?;

        Ok(Outcome { value, effects: effects.take() })
    }
}
