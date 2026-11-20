//! The puzzle chain: what the player has to find, unlock and reach, in
//! order, from the start to each ending; and what can never happen.
//!
//! This is a reading of the game, not a playthrough. It follows exits,
//! doors and their keys, containers, `revealed_by` rules, characters'
//! conversations, timers, and Lua code that names things (`world.cellar`)
//! or ends the game (`end_game("won")`). Code counts once whatever owns it
//! can be reached: a room's handlers once the room can, a conversation's
//! once someone who has it can be talked to, and global handlers and
//! script files always. Conditions (`when`) are assumed to come true
//! sooner or later, so what it finds can't happen really can't, unless
//! a script does something it can't see (a name built at run time).

use crate::util::literal_calls;
use eifgb_ir::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Puzzles {
    /// What the player does, in an order that works.
    pub steps: Vec<Step>,
    /// How each ending can come about, or why it can't.
    pub endings: Vec<EndingReach>,
    /// What can never happen: rooms nobody reaches, keys that open nothing…
    pub problems: Vec<PuzzleProblem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// `start`, `find` (something hidden or shut away), `unlock`, `reach` or `ending`.
    pub kind: &'static str,
    /// What it's about: a room's, thing's or ending's ID.
    pub id: String,
    /// What the player does, in words: "Search the tide pool: a brass key."
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EndingReach {
    pub key: String,
    pub kind: EndingKind,
    /// Whether the player can get there.
    pub reachable: bool,
    /// What brings it about: "rooms.cellar.on.enter", "timers.storm"…
    pub from: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PuzzleProblem {
    /// The ID it's about (a room, thing or ending), for the editor to select.
    pub id: String,
    /// `room`, `thing`, `character` or `ending`.
    pub what: &'static str,
    pub message: String,
}

/// Who a piece of code belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    Always,
    Entity(EntityId),
    Choice(usize),
    Timer(usize),
}

struct Snippet<'a> {
    owner: Owner,
    origin: String,
    src: &'a str,
}

fn snippets(story: &Story) -> Vec<Snippet<'_>> {
    fn text<'a>(owner: Owner, t: &'a Text, origin: &str, out: &mut Vec<Snippet<'a>>) {
        for seg in &t.0 {
            match seg {
                Segment::Lit(_) => {}
                Segment::Expr(c) => out.push(Snippet { owner, origin: origin.into(), src: &c.src }),
                Segment::If { cond, then, otherwise } => {
                    out.push(Snippet { owner, origin: origin.into(), src: &cond.src });
                    text(owner, then, origin, out);
                    text(owner, otherwise, origin, out);
                }
            }
        }
    }
    fn push<'a>(owner: Owner, c: &'a Code, out: &mut Vec<Snippet<'a>>) {
        out.push(Snippet { owner, origin: c.origin.clone(), src: &c.src });
    }
    let mut out = Vec::new();
    for (i, e) in story.entities.iter().enumerate() {
        let owner = Owner::Entity(EntityId(i as u32));
        for h in &e.handlers {
            push(owner, &h.code, &mut out);
        }
        if let Some(d) = &e.description {
            text(owner, d, &format!("{}.description", e.key), &mut out);
        }
    }
    for (i, c) in story.choices.iter().enumerate() {
        let owner = Owner::Choice(i);
        for n in &c.nodes {
            if let Some(r) = &n.run {
                push(owner, r, &mut out);
            }
            for o in &n.options {
                if let Some(r) = &o.run {
                    push(owner, r, &mut out);
                }
            }
        }
    }
    for (i, t) in story.timers.iter().enumerate() {
        if let Some(r) = &t.run {
            push(Owner::Timer(i), r, &mut out);
        }
    }
    for h in &story.handlers {
        push(Owner::Always, &h.code, &mut out);
    }
    for s in &story.scripts {
        out.push(Snippet { owner: Owner::Always, origin: s.name.clone(), src: &s.src });
    }
    out
}

/// Whether Lua source names an entity: `world.key` or `world["key"]`.
fn names(src: &str, key: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = src[from..].find(key) {
        let start = from + pos;
        from = start + key.len();
        let after_ok = src[from..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let before = src[..start].trim_end_matches(['"', '\'', '[']).trim_end();
        if after_ok && (before.ends_with("world.") || before.ends_with("world")) {
            return true;
        }
    }
    false
}

pub fn puzzles(story: &Story) -> Puzzles {
    let n = story.entities.len();
    let def = |i: usize| &story.entities[i];
    let id = |i: usize| EntityId(i as u32);
    let name = |i: usize| def(i).name.clone();
    let code = snippets(story);

    // What's been worked out so far.
    let mut rooms: BTreeSet<usize> = BTreeSet::new();
    let mut present: BTreeSet<usize> = BTreeSet::new();
    let mut live: BTreeSet<Owner> = BTreeSet::from([Owner::Always]);
    let mut steps: Vec<Step> = Vec::new();
    let mut endings: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let start = story.meta.start.index();
    rooms.insert(start);
    present.insert(PLAYER.index());
    steps.push(Step { kind: "start", id: def(start).key.clone(), text: format!("Start in {}.", name(start)) });
    for (i, t) in story.timers.iter().enumerate() {
        if t.running {
            live.insert(Owner::Timer(i));
        }
    }

    let flag = |i: usize, f: &str| matches!(def(i).state.get(f), Some(Value::Bool(true)));
    let keys_for = |i: usize| def(i).unlocks_with.map(|k| k.index());

    loop {
        let before = (rooms.len(), present.len(), live.len());
        let live_code: Vec<&Snippet> = code.iter().filter(|s| live.contains(&s.owner)).collect();
        let mentioned = |i: usize| live_code.iter().any(|s| names(s.src, &def(i).key));

        // Things: present when where they are can be reached, and they can be found.
        for i in 0..n {
            if present.contains(&i) || def(i).kind == EntityKind::Room {
                continue;
            }
            // The chain of holders up to a room (or the player).
            let mut at = def(i).location.map(|l| l.index());
            let mut shut_in: Option<usize> = None;
            let mut held_by: Option<usize> = None;
            let mut ok = true;
            let mut guard = 0;
            while let Some(h) = at {
                guard += 1;
                if guard > n || h == PLAYER.index() || def(h).kind == EntityKind::Room {
                    break;
                }
                if def(h).kind == EntityKind::Character {
                    held_by.get_or_insert(h);
                } else if flag(h, "locked") {
                    let key = keys_for(h);
                    if !(key.is_some_and(|k| present.contains(&k)) || mentioned(h) || def(h).code.is_some()) {
                        ok = false;
                    }
                    shut_in.get_or_insert(h);
                }
                at = def(h).location.map(|l| l.index());
            }
            let place_ok = match at {
                Some(r) if r == PLAYER.index() => true,
                Some(r) => rooms.contains(&r),
                // Off-stage: a script brings it in.
                None => mentioned(i),
            };
            // Doors are present where their exits are.
            let door_here = def(i).has(Trait::Door) && rooms.iter().any(|&r| def(r).exits.iter().any(|e| e.through == Some(id(i))));
            if !(ok && place_ok) && !door_here {
                continue;
            }
            let hidden = flag(i, "hidden");
            let revealer = def(i).revealed_by.iter().find(|r| present.contains(&r.target.index()) || rooms.contains(&r.target.index()));
            if hidden && revealer.is_none() && !mentioned(i) {
                continue;
            }
            present.insert(i);
            if let Some(r) = revealer.filter(|_| hidden) {
                let verb = r.category.clone().map(|c| format!("({c})")).or_else(|| r.verbs.first().cloned()).unwrap_or_default();
                steps.push(Step { kind: "find", id: def(i).key.clone(), text: format!("{} the {}: {}.", cap(&verb), name(r.target.index()), a(&name(i))) });
            } else if hidden {
                steps.push(Step { kind: "find", id: def(i).key.clone(), text: format!("{}: {}.", by_script(story, &live_code, &def(i).key), a(&name(i))) });
            } else if let Some(c) = shut_in {
                let how = match (keys_for(c), &def(c).code) {
                    (Some(k), _) => format!("with {}", the(&name(k))),
                    (None, Some(code)) => format!("with the code {code}"),
                    (None, None) => "by a script".to_string(),
                };
                steps.push(Step { kind: "find", id: def(i).key.clone(), text: format!("Unlock {} {how}: {}.", the(&name(c)), a(&name(i))) });
            } else if let Some(h) = held_by.filter(|_| def(i).kind != EntityKind::Character) {
                steps.push(Step { kind: "find", id: def(i).key.clone(), text: format!("Get {} from {}.", the(&name(i)), the(&name(h))) });
            } else if def(i).location.is_none() && def(i).kind != EntityKind::Character {
                steps.push(Step { kind: "find", id: def(i).key.clone(), text: format!("{}: {}.", by_script(story, &live_code, &def(i).key), a(&name(i))) });
            }
        }

        // Rooms: by exits from rooms already reached.
        let reached: Vec<usize> = rooms.iter().copied().collect();
        for r in reached {
            for e in &def(r).exits {
                let Some(to) = e.to.map(|t| t.index()) else { continue };
                if rooms.contains(&to) {
                    continue;
                }
                let mut how: Option<String> = None;
                if let Some(d) = e.through.map(|d| d.index()) {
                    let locked = flag(d, "locked");
                    let blocked = matches!(def(d).state.get("blocked"), Some(Value::Bool(true)));
                    if locked {
                        match keys_for(d) {
                            Some(k) if present.contains(&k) => how = Some(format!("Unlock {} with {}", the(&name(d)), the(&name(k)))),
                            _ if def(d).code.is_some() => how = Some(format!("Enter the code {} on {}", def(d).code.clone().unwrap_or_default(), the(&name(d)))),
                            _ if mentioned(d) => how = Some(format!("A script opens {}", the(&name(d)))),
                            _ => continue,
                        }
                    } else if blocked && !mentioned(d) {
                        continue;
                    }
                    if flag(d, "hidden") && !present.contains(&d) && !mentioned(d) {
                        continue;
                    }
                }
                rooms.insert(to);
                match how {
                    Some(h) => steps.push(Step { kind: "unlock", id: def(to).key.clone(), text: format!("{h}: {} opens up.", the_room(&name(to))) }),
                    None if e.when.is_some() => steps.push(Step { kind: "reach", id: def(to).key.clone(), text: format!("Once it's allowed, go {} from {} to {}.", e.direction.key(), the_room(&name(r)), the_room(&name(to))) }),
                    None => {}
                }
            }
        }
        // Rooms a live script moves the player to.
        for i in 0..n {
            if def(i).kind == EntityKind::Room && !rooms.contains(&i) && mentioned(i) && live_code.iter().any(|s| names(s.src, &def(i).key) && s.src.contains("move")) {
                rooms.insert(i);
                steps.push(Step { kind: "reach", id: def(i).key.clone(), text: format!("A script takes the player to {}.", the_room(&name(i))) });
            }
        }

        // Code that can run now.
        for i in 0..n {
            let here = rooms.contains(&i) || present.contains(&i);
            if here {
                live.insert(Owner::Entity(id(i)));
                if let Some(c) = def(i).talk.map(|c| c.0 as usize) {
                    live.insert(Owner::Choice(c));
                }
            }
        }
        for s in &live_code {
            for c in literal_calls(s.src, "open_choice") {
                if let Some(ci) = story.choices.iter().position(|x| x.key == c) {
                    live.insert(Owner::Choice(ci));
                }
            }
            for t in literal_calls(s.src, "start_timer") {
                if let Some(ti) = story.timers.iter().position(|x| x.key == t) {
                    live.insert(Owner::Timer(ti));
                }
            }
        }
        for (ti, t) in story.timers.iter().enumerate() {
            if t.started_by.iter().any(|r| present.contains(&r.target.index()) || rooms.contains(&r.target.index())) {
                live.insert(Owner::Timer(ti));
            }
        }

        if (rooms.len(), present.len(), live.len()) == before {
            break;
        }
    }

    // Endings: what can bring each about.
    for s in &code {
        for k in literal_calls(s.src, "end_game") {
            let tag = if live.contains(&s.owner) { s.origin.clone() } else { format!("!{}", s.origin) };
            endings.entry(k.to_string()).or_default().push(tag);
        }
    }
    // The engine's own: a player with health who runs out of it.
    if story.entity(PLAYER).health.is_some() && story.endings.iter().any(|e| e.key == "died") {
        endings.entry("died".into()).or_default().push("the player running out of health".into());
    }
    for (ti, t) in story.timers.iter().enumerate() {
        if let Some(e) = t.end {
            let k = story.endings[e.index()].key.clone();
            let tag = if live.contains(&Owner::Timer(ti)) { format!("timers.{}", t.key) } else { format!("!timers.{}", t.key) };
            endings.entry(k).or_default().push(tag);
        }
    }

    let mut problems = Vec::new();
    let mut reach = Vec::new();
    for e in &story.endings {
        let from = endings.get(&e.key).cloned().unwrap_or_default();
        let reachable = from.iter().any(|f| !f.starts_with('!'));
        if reachable {
            let how: Vec<&String> = from.iter().filter(|f| !f.starts_with('!')).collect();
            steps.push(Step { kind: "ending", id: e.key.clone(), text: format!("The {} ending: from {}.", e.key, how.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) });
        } else if from.is_empty() {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "ending", message: format!("Nothing ends the game with '{}': no end_game(\"{}\") and no timer.", e.key, e.key) });
        } else {
            let places: Vec<String> = from.iter().map(|f| f.trim_start_matches('!').to_string()).collect();
            problems.push(PuzzleProblem { id: e.key.clone(), what: "ending", message: format!("The '{}' ending can't happen: only {} end the game that way, and the player can't get there.", e.key, places.join(", ")) });
        }
        reach.push(EndingReach { key: e.key.clone(), kind: e.kind, reachable, from: from.into_iter().map(|f| f.trim_start_matches('!').to_string()).collect() });
    }

    let all_code: Vec<&str> = code.iter().map(|s| s.src).collect();
    let anywhere = |i: usize| all_code.iter().any(|s| names(s, &def(i).key));
    for i in 0..n {
        let e = def(i);
        if i == PLAYER.index() {
            continue;
        }
        match e.kind {
            EntityKind::Room if !rooms.contains(&i) => problems.push(PuzzleProblem { id: e.key.clone(), what: "room", message: format!("The player can't get to {}.", the_room(&e.name)) }),
            EntityKind::Object | EntityKind::Character if !present.contains(&i) && e.location.is_some_and(|l| l.index() != PLAYER.index()) => {
                let what = if e.kind == EntityKind::Character { "character" } else { "thing" };
                let why = if flag(i, "hidden") && e.revealed_by.is_empty() { " (it's hidden, and nothing reveals it)" } else { "" };
                problems.push(PuzzleProblem { id: e.key.clone(), what, message: format!("The player can't get to {}{why}.", the(&e.name)) });
            }
            _ => {}
        }
        // Keys that open nothing.
        let keyish = e.vocab.nouns.iter().any(|w| w == "key" || w == "keys");
        if e.kind == EntityKind::Object && keyish && !story.entities.iter().any(|o| o.unlocks_with == Some(id(i))) && !anywhere(i) {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("{} opens nothing: nothing has `unlocks_with: {}`.", cap(&the(&e.name)), e.key) });
        }
        // Locked, with no key and no script.
        if flag(i, "locked") && e.unlocks_with.is_none() && e.code.is_none() && !anywhere(i) {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("{} is locked, and there's no key for it.", cap(&the(&e.name))) });
        }
    }

    Puzzles { steps, endings: reach, problems }
}

/// What a script does to bring something out, from where it's written:
/// `objects.tide_pool.on.search` is "Search the tide pool".
fn by_script(story: &Story, live: &[&Snippet], key: &str) -> String {
    let Some(s) = live.iter().find(|s| names(s.src, key)) else { return "A script brings it out".into() };
    let parts: Vec<&str> = s.origin.split('.').collect();
    if let [section, owner, "on", event, ..] = parts[..]
        && let Some(o) = story.entity_by_key(owner)
    {
        let owner = &story.entity(o).name;
        let event = event.trim_start_matches("before_").trim_start_matches("after_");
        return match (section, event) {
            ("rooms", "enter") => format!("Go to {owner}"),
            ("rooms", _) => format!("In {owner}, a script"),
            (_, "turn") => format!("In time, {}", the(owner)),
            (_, verb) => format!("{} {}", cap(&verb.replace('_', " ")), the(owner)),
        };
    }
    format!("A script ({}) brings it out", s.origin)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn the(name: &str) -> String {
    format!("the {name}")
}

fn the_room(name: &str) -> String {
    name.to_string()
}

fn a(name: &str) -> String {
    let an = name.chars().next().is_some_and(|c| "aeiouAEIOU".contains(c));
    format!("{} {name}", if an { "an" } else { "a" })
}
