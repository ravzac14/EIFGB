//! The browser bridge. Built for `wasm32-unknown-emscripten`, it exports a
//! handful of C functions that take and return UTF-8 strings (JSON), so the
//! page can compile a game, play it, and save and restore it.
//!
//! One game runs at a time. Each returned string stays valid until the next
//! call; JavaScript copies it straight away (`UTF8ToString`).

use eifgb_compiler::{compile, MemFiles, Severity};
use eifgb_engine::Game;
use eifgb_lua::LuaHost;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{c_char, CStr, CString};

thread_local! {
    static GAME: RefCell<Option<Game<LuaHost>>> = const { RefCell::new(None) };
    static RETURNED: RefCell<CString> = RefCell::new(CString::default());
}

fn arg(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: the page passes NUL-terminated strings it allocated for the call.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn give(v: Value) -> *const c_char {
    let s = CString::new(v.to_string()).unwrap_or_default();
    RETURNED.with(|r| {
        *r.borrow_mut() = s;
        r.borrow().as_ptr()
    })
}

fn output(o: eifgb_engine::Output) -> Value {
    serde_json::to_value(o).unwrap_or(Value::Null)
}

/// Compiles a project and starts it. `files` is a JSON object of
/// project-relative paths to file contents; `entry` is usually `game.yaml`.
/// Returns `{ diagnostics: [{severity, path, message}], meta?, output? }`,
/// with `meta` (id, title, author, version) and `output` (the opening)
/// only when it compiled and started.
fn load(files: &str, entry: &str, seed: u64) -> Value {
    let files: BTreeMap<String, String> = match serde_json::from_str(files) {
        Ok(f) => f,
        Err(e) => return json!({ "diagnostics": [{ "severity": "error", "path": "", "message": format!("bad files: {e}") }] }),
    };
    let entry = if entry.is_empty() { "game.yaml" } else { entry };
    let out = compile(&MemFiles(files), entry);
    let diagnostics: Vec<Value> = out
        .diagnostics
        .iter()
        .map(|d| {
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            json!({ "severity": sev, "path": d.path, "message": d.message })
        })
        .collect();
    let Some(story) = out.story else { return json!({ "diagnostics": diagnostics }) };
    let mut diagnostics = diagnostics;
    diagnostics.extend(script_errors(&story));
    if diagnostics.iter().any(|d| d["severity"] == "error") {
        return json!({ "diagnostics": diagnostics });
    }
    let m = &story.meta;
    let mut meta = json!({ "id": m.id, "title": m.title, "author": m.author, "version": m.version, "language": m.language, "presentation": m.presentation, "endings": story.endings.iter().map(|e| e.key.clone()).collect::<Vec<_>>(), "stdlib": m.stdlib, "ir": story.ir_version, "engine": env!("CARGO_PKG_VERSION") });
    let started = LuaHost::new().and_then(|h| Game::new(story, h, seed));
    match started {
        Ok(mut g) => {
            let opening = output(g.start());
            // Characters to play, for taking turns (two players, one screen).
            let playable = g.playable_characters();
            if playable.len() > 1 {
                meta["playable"] = Value::Array(playable.into_iter().map(|(id, name)| json!({ "id": id, "name": name })).collect());
            }
            GAME.with(|cell| *cell.borrow_mut() = Some(g));
            json!({ "diagnostics": diagnostics, "meta": meta, "output": opening })
        }
        Err(e) => {
            diagnostics.push(json!({ "severity": "error", "path": "", "message": e.to_string() }));
            json!({ "diagnostics": diagnostics })
        }
    }
}

/// Script files that fail to load, and Lua syntax errors in any snippet,
/// as error diagnostics at the snippet's source path.
fn script_errors(story: &eifgb_engine::ir::Story) -> Vec<Value> {
    use eifgb_engine::ScriptHost;
    let mut host = match LuaHost::new() {
        Ok(h) => h,
        Err(e) => return vec![json!({ "severity": "error", "path": "", "message": e.to_string() })],
    };
    if let Err(e) = host.load(story) {
        return vec![json!({ "severity": "error", "path": "scripts", "message": e.to_string() })];
    }
    let mut codes = Vec::new();
    eifgb_compiler::visit_code(story, &mut |c| codes.push(c.clone()));
    host.syntax_errors(&codes)
        .into_iter()
        .map(|(origin, msg)| json!({ "severity": "error", "path": origin, "message": format!("Lua syntax error, {msg}") }))
        .collect()
}

fn with_game(f: impl FnOnce(&mut Game<LuaHost>) -> Value) -> Value {
    GAME.with(|cell| match cell.borrow_mut().as_mut() {
        Some(g) => f(g),
        None => json!({ "error": "no game is running" }),
    })
}

/// Compile and start; see [`load`]. The page picks the seed (e.g. from the clock).
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_load(files: *const c_char, entry: *const c_char, seed: f64) -> *const c_char {
    give(load(&arg(files), &arg(entry), seed as u64))
}

/// The editor's live check: `{ diagnostics, puzzles?, walkthrough?,
/// walkthroughs? }`. Compiles, and plays each walkthrough: `walkthrough.txt`
/// (to a win) and `walkthrough-<ending>.txt` (to that ending). See
/// [`eifgb_engine::testing`]. `walkthrough` is `walkthrough.txt`'s, as before.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_check(files: *const c_char, entry: *const c_char) -> *const c_char {
    let files: BTreeMap<String, String> = serde_json::from_str(&arg(files)).unwrap_or_default();
    let entry = arg(entry);
    let walks: Vec<(String, String)> = files.iter().filter(|(k, _)| is_walkthrough(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let out = compile(&MemFiles(files), if entry.is_empty() { "game.yaml" } else { &entry });
    let mut diagnostics: Vec<Value> = out
        .diagnostics
        .iter()
        .map(|d| json!({ "severity": if d.severity == Severity::Error { "error" } else { "warning" }, "path": d.path, "message": d.message }))
        .collect();
    if let Some(story) = &out.story {
        diagnostics.extend(script_errors(story));
    }
    let mut result = json!({ "diagnostics": diagnostics });
    if let Some(story) = &out.story {
        result["puzzles"] = serde_json::to_value(eifgb_compiler::puzzles::puzzles(story)).unwrap_or(Value::Null);
    }
    if let Some(story) = out.story
        && !walks.is_empty()
        && !result["diagnostics"].as_array().is_some_and(|d| d.iter().any(|d| d["severity"] == "error"))
    {
        let mut all = Vec::new();
        for (file, text) in walks {
            let mut played = walkthrough(story.clone(), &file, &text);
            if let Some(problem) = played["problem"].as_str() {
                result["diagnostics"].as_array_mut().unwrap().push(json!({ "severity": "warning", "path": file, "message": problem }));
            }
            played["file"] = json!(file);
            if file == WALKTHROUGH {
                result["walkthrough"] = played.clone();
            }
            all.push(played);
        }
        result["walkthroughs"] = json!(all);
    }
    give(result)
}

/// The walkthrough the editor records from test play, and checks on every edit.
const WALKTHROUGH: &str = "walkthrough.txt";

/// `walkthrough.txt`, or one for an ending: `walkthrough-feast.txt`.
fn is_walkthrough(path: &str) -> bool {
    path == WALKTHROUGH || (path.starts_with("walkthrough-") && path.ends_with(".txt") && !path.contains('/'))
}

/// Plays a walkthrough on a game of its own, with seed 1, as the editor's
/// test play does, so the game being played isn't disturbed. See
/// [`eifgb_engine::testing::Played`]: `{ result, steps, line?, problem?,
/// ending?, goal, never }`.
fn walkthrough(story: eifgb_engine::ir::Story, file: &str, text: &str) -> Value {
    let mut g = match LuaHost::new().and_then(|h| Game::new(story, h, 1)) {
        Ok(g) => g,
        Err(e) => return json!({ "result": "error", "steps": 0, "problem": e.to_string() }),
    };
    g.start();
    let goal = eifgb_engine::testing::Goal::of(file, text);
    let name = if file == WALKTHROUGH { "the walkthrough".to_string() } else { file.to_string() };
    serde_json::to_value(eifgb_engine::testing::play_walkthrough(&mut g, text, &goal, &name)).unwrap_or(Value::Null)
}

/// Dead ends along a walkthrough: things it needs, lost for good. See
/// [`eifgb_engine::testing::DeadEnd`]. `{ error }` when the game can't be played.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_dead_ends(files: *const c_char, entry: *const c_char, walkthrough: *const c_char) -> *const c_char {
    let files: BTreeMap<String, String> = serde_json::from_str(&arg(files)).unwrap_or_default();
    let entry = arg(entry);
    let walkthrough = arg(walkthrough);
    let out = compile(&MemFiles(files), if entry.is_empty() { "game.yaml" } else { &entry });
    let Some(story) = out.story else { return give(json!({ "error": "the game has mistakes to fix first" })) };
    let make = || LuaHost::new().and_then(|h| Game::new(story.clone(), h, 1)).ok();
    give(serde_json::to_value(eifgb_engine::testing::dead_ends(make, &walkthrough)).unwrap_or(Value::Null))
}

/// What each step of a walkthrough brings, for the pacing chart. See
/// [`eifgb_engine::testing::Beat`]. `{ error }` when the game can't be played.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_pacing(files: *const c_char, entry: *const c_char, walkthrough: *const c_char) -> *const c_char {
    let files: BTreeMap<String, String> = serde_json::from_str(&arg(files)).unwrap_or_default();
    let entry = arg(entry);
    let walkthrough = arg(walkthrough);
    let out = compile(&MemFiles(files), if entry.is_empty() { "game.yaml" } else { &entry });
    let Some(story) = out.story else { return give(json!({ "error": "the game has mistakes to fix first" })) };
    let make = || LuaHost::new().and_then(|h| Game::new(story, h, 1)).ok();
    give(serde_json::to_value(eifgb_engine::testing::pacing(make, &walkthrough)).unwrap_or(Value::Null))
}

/// The wanderer: `moves` random moves on new games of `files`, the moves
/// picked from `seed`. Every game uses seed 1, as test play does, so what
/// it finds plays the same way there. See
/// [`eifgb_engine::testing::Wandered`]. `{ error }` when the game can't be
/// played.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_wander(files: *const c_char, entry: *const c_char, seed: f64, moves: f64) -> *const c_char {
    let files: BTreeMap<String, String> = serde_json::from_str(&arg(files)).unwrap_or_default();
    let entry = arg(entry);
    let out = compile(&MemFiles(files), if entry.is_empty() { "game.yaml" } else { &entry });
    let Some(story) = out.story else { return give(json!({ "error": "the game has mistakes to fix first" })) };
    let make = || {
        LuaHost::new().and_then(|h| Game::new(story.clone(), h, 1)).ok()
    };
    give(serde_json::to_value(eifgb_engine::testing::wander(make, seed as u64, moves as usize)).unwrap_or(Value::Null))
}

/// One line of player input: the game's output as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_input(line: *const c_char) -> *const c_char {
    let line = arg(line);
    give(with_game(|g| output(g.input(&line))))
}

/// The whole game state, for autosaves and save slots.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_save() -> *const c_char {
    give(with_game(|g| Value::String(g.world().save())))
}

/// Restores a save made by SAVE or [`eifgb_save`]: the output as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_restore(save: *const c_char) -> *const c_char {
    let save = arg(save);
    give(with_game(|g| output(g.restore(&save))))
}

/// What the player has seen of the map: `{ here, rooms: [{id, name}],
/// exits: [{from, dir, to?, door?, closed?}], heard: [{id, name}] }`
/// (heard: places they've heard of but not been to). Only rooms the player has
/// been in; an exit from one of them whose far side they haven't visited
/// has no `to`. Exits through secret doors that haven't been found are left out.
/// Words that finish the last word of `line` (Tab in the player), from what
/// the player can see now: `["lantern", "ladder"]`.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_complete(line: *const c_char) -> *const c_char {
    let line = arg(line);
    give(with_game(|g| json!(g.completions(&line))))
}

#[unsafe(no_mangle)]
pub extern "C" fn eifgb_map() -> *const c_char {
    give(with_game(|g| map(g.world())))
}

/// Pass-and-play: be the character with key `key` now, without a turn
/// passing: the output (it says so, and looks around).
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_play_as(key: *const c_char) -> *const c_char {
    let key = arg(key);
    give(with_game(|g| output(g.play_as(&key))))
}

/// What JOURNAL says now (goals, promises and notes), for the notes panel:
/// `[{ kind, text }]`, empty when there's nothing yet. Changes nothing.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_journal() -> *const c_char {
    give(with_game(|g| serde_json::to_value(g.journal_lines()).unwrap_or(Value::Null)))
}

/// The characters and things the player has met, for following one through
/// the story: `[{ id, name, kind, words }]`, characters first.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_met() -> *const c_char {
    give(with_game(|g| met(g.world())))
}

fn met(w: &eifgb_engine::World) -> Value {
    use eifgb_engine::ir::{EntityKind, PLAYER};
    let st = &w.state;
    let mut ids: Vec<eifgb_engine::ir::EntityId> = st.located.keys().chain(st.got.keys()).chain(st.known.iter()).copied().filter(|&e| e != PLAYER && w.def(e).kind != EntityKind::Room).collect();
    ids.sort_by_key(|&e| (w.def(e).kind != EntityKind::Character, w.display_name(e).to_lowercase()));
    ids.dedup();
    Value::Array(
        ids.into_iter()
            .map(|e| {
                let d = w.def(e);
                json!({ "id": d.key, "name": w.display_name(e), "kind": if d.kind == EntityKind::Character { "character" } else { "thing" }, "words": d.vocab.nouns })
            })
            .collect(),
    )
}

fn map(w: &eifgb_engine::World) -> Value {
    let visited = &w.state.visited;
    let key = |id: eifgb_engine::ir::EntityId| w.def(id).key.clone();
    let rooms: Vec<Value> = visited.iter().map(|&r| json!({ "id": key(r), "name": w.def(r).name })).collect();
    let mut exits = Vec::new();
    for &r in visited {
        for e in w.def(r).exits.iter().filter(|e| w.exit_visible(e)) {
            let mut x = json!({ "from": key(r), "dir": e.direction });
            if let Some(to) = e.to.filter(|t| visited.contains(t)) {
                x["to"] = json!(key(to));
            }
            if let Some(d) = e.through {
                x["door"] = json!(w.def(d).name);
                x["closed"] = json!(!w.flag(d, "open"));
            }
            exits.push(x);
        }
    }
    // Places heard of, not yet found: drawn apart, with a "?".
    let heard: Vec<Value> = w
        .heard_of_only()
        .into_iter()
        .map(|r| json!({ "id": key(r), "name": w.def(r).name }))
        .collect();
    json!({ "here": key(w.here()), "rooms": rooms, "exits": exits, "heard": heard })
}

fn main() {}
