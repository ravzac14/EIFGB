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
    let meta = json!({ "id": m.id, "title": m.title, "author": m.author, "version": m.version, "presentation": m.presentation, "endings": story.endings.iter().map(|e| e.key.clone()).collect::<Vec<_>>(), "stdlib": m.stdlib, "ir": story.ir_version, "engine": env!("CARGO_PKG_VERSION") });
    let started = LuaHost::new().and_then(|h| Game::new(story, h, seed));
    match started {
        Ok(mut g) => {
            let opening = output(g.start());
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

/// The editor's live check: `{ diagnostics, walkthrough? }`. Compiles, and plays
/// `walkthrough.txt` if there is one (see [`walkthrough`]).
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_check(files: *const c_char, entry: *const c_char) -> *const c_char {
    let files: BTreeMap<String, String> = serde_json::from_str(&arg(files)).unwrap_or_default();
    let entry = arg(entry);
    let walkthrough_file = files.get(WALKTHROUGH).cloned();
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
    if let (Some(story), Some(text)) = (out.story, walkthrough_file)
        && !result["diagnostics"].as_array().is_some_and(|d| d.iter().any(|d| d["severity"] == "error"))
    {
        let played = walkthrough(story, &text);
        if let Some(problem) = &played["problem"].as_str() {
            result["diagnostics"].as_array_mut().unwrap().push(json!({ "severity": "warning", "path": WALKTHROUGH, "message": problem }));
        }
        result["walkthrough"] = played;
    }
    give(result)
}

/// The walkthrough the editor records from test play, and checks on every edit.
const WALKTHROUGH: &str = "walkthrough.txt";

/// Plays a walkthrough (one command a line; blank lines and `#` comments
/// skipped) on a game of its own, with seed 1, as the editor's test play
/// does, so the game being played isn't disturbed. It stops at the first
/// ending. `{ result: "win" | "loss" | "neutral" | "unfinished" | "error",
/// steps, line?, problem? }`: `problem` says what went wrong when it
/// doesn't win, and `line` is where (1-based).
fn walkthrough(story: eifgb_engine::ir::Story, text: &str) -> Value {
    let mut g = match LuaHost::new().and_then(|h| Game::new(story, h, 1)) {
        Ok(g) => g,
        Err(e) => return json!({ "result": "error", "steps": 0, "problem": e.to_string() }),
    };
    g.start();
    let mut steps = 0;
    for (i, line) in text.lines().enumerate() {
        let command = line.trim();
        if command.is_empty() || command.starts_with('#') {
            continue;
        }
        steps += 1;
        let out = g.input(command);
        let n = i + 1;
        if let Some(err) = out.lines.iter().find(|l| l.kind == eifgb_engine::LineKind::Error) {
            return json!({ "result": "error", "steps": steps, "line": n,
                "problem": format!("the walkthrough hits a script error at line {n} ({command}): {}", err.text) });
        }
        if let Some(kind) = out.ended {
            let kind = serde_json::to_value(kind).unwrap_or(Value::Null);
            let problem = (kind != "win").then(|| {
                let reply = out.text();
                let reply = reply.lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join(" ");
                format!("the walkthrough no longer wins: at line {n} ({command}) the game ends in a {}: {reply}", kind.as_str().unwrap_or("loss"))
            });
            return json!({ "result": kind, "steps": steps, "line": n, "problem": problem });
        }
    }
    json!({ "result": "unfinished", "steps": steps,
        "problem": if steps == 0 { "the walkthrough is empty".to_string() } else { format!("the walkthrough no longer wins: after its {steps} commands the game is still going") } })
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
/// exits: [{from, dir, to?, door?, closed?}] }`. Only rooms the player has
/// been in; an exit from one of them whose far side they haven't visited
/// has no `to`. Exits through secret doors that haven't been found are left out.
#[unsafe(no_mangle)]
pub extern "C" fn eifgb_map() -> *const c_char {
    give(with_game(|g| map(g.world())))
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
    json!({ "here": key(w.here()), "rooms": rooms, "exits": exits })
}

fn main() {}
