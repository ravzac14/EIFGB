//! The puzzle chain: what the player has to find, unlock and reach, from
//! the start to each ending; what each step needs first, other ways to do
//! it, what can never happen, and where a player could get stuck.
//!
//! It's a graph. Every way something can come about is a rule: the Cellar
//! is reached from the Hall, through the trapdoor, once the brass key is
//! at hand; the key is found by searching the mat, once the Kitchen is
//! reached. The rules are worked through from the start until nothing new
//! can happen. A step is a rule that took some doing (a find, an unlock);
//! its `needs` are the steps it waited on, so separate puzzles stay
//! separate, and an ending's `needs` are every step it takes.
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
    /// What the player does, in an order that works: each after what it needs.
    pub steps: Vec<Step>,
    /// How each ending can come about, or why it can't.
    pub endings: Vec<EndingReach>,
    /// What can never happen: rooms nobody reaches, keys that open nothing…
    pub problems: Vec<PuzzleProblem>,
    /// Where a player could get stuck on the way to a win: something they
    /// need can be eaten, burnt, broken, worn out or taken away, or a choice
    /// rules out the only way, and there's no other way.
    pub stuck: Vec<PuzzleProblem>,
    /// What the chain takes on trust, on the way to an ending: scripts whose
    /// inside it can't see, conditions it assumes come true.
    pub assumes: Vec<PuzzleProblem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// `start`, `find` (something hidden or shut away), `unlock`, `reach` or `ending`.
    pub kind: &'static str,
    /// What it's about: a room's, thing's or ending's ID.
    pub id: String,
    /// What the player does, in words: "Search the tide pool: a brass key."
    pub text: String,
    /// The steps that have to come first (indexes into `steps`).
    pub needs: Vec<usize>,
    /// Other ways to do it, in words: "Unlock the chest with the code 4512: a coin."
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub or: Vec<String>,
    /// For a choice: the others it rules out, in words.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub either: Vec<String>,
    /// The script it leans on (its origin), whose inside the chain can't see.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EndingReach {
    pub key: String,
    pub kind: EndingKind,
    /// Whether the player can get there.
    pub reachable: bool,
    /// What brings it about: "rooms.cellar.on.enter", "timers.storm"…
    pub from: Vec<String>,
    /// Every step it takes, in order (indexes into `steps`).
    pub needs: Vec<usize>,
    /// When it depends on an either-or choice: "only if the player chooses “Take the sword”".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub only_if: Vec<String>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Owner {
    Always,
    Entity(EntityId),
    Choice(usize),
    Timer(usize),
}

struct Snippet<'a> {
    /// Its place in the list of snippets.
    idx: usize,
    owner: Owner,
    origin: String,
    src: &'a str,
    /// Code that runs (a handler, a script), not an expression in text: only
    /// that can move, reveal or unlock anything.
    acts: bool,
    /// The condition it only runs under: a conversation option's `when`.
    guard: Option<&'a str>,
    /// What the player sees to choose it: a conversation option's text.
    label: Option<String>,
}

fn snippets(story: &Story) -> Vec<Snippet<'_>> {
    fn text<'a>(owner: Owner, t: &'a Text, origin: &str, out: &mut Vec<Snippet<'a>>) {
        for seg in &t.0 {
            match seg {
                Segment::Lit(_) => {}
                Segment::Expr(c) => out.push(Snippet { idx: 0, owner, origin: origin.into(), src: &c.src, acts: false, guard: None, label: None }),
                Segment::If { cond, then, otherwise } => {
                    out.push(Snippet { idx: 0, owner, origin: origin.into(), src: &cond.src, acts: false, guard: None, label: None });
                    text(owner, then, origin, out);
                    text(owner, otherwise, origin, out);
                }
                Segment::Vary { options, .. } => options.iter().for_each(|o| text(owner, o, origin, out)),
            }
        }
    }
    fn push<'a>(owner: Owner, c: &'a Code, out: &mut Vec<Snippet<'a>>) {
        out.push(Snippet { idx: 0, owner, origin: c.origin.clone(), src: &c.src, acts: true, guard: None, label: None });
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
                    let last = out.last_mut().expect("just pushed");
                    last.guard = o.when.as_ref().map(|w| w.src.as_str());
                    last.label = Some(literal(&o.text));
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
        out.push(Snippet { idx: 0, owner: Owner::Always, origin: s.name.clone(), src: &s.src, acts: true, guard: None, label: None });
    }
    for (i, s) in out.iter_mut().enumerate() {
        s.idx = i;
    }
    out
}

/// A text's words, without its `${…}` parts: an option as the player sees it, mostly.
fn literal(t: &Text) -> String {
    t.0.iter().filter_map(|seg| if let Segment::Lit(l) = seg { Some(l.as_str()) } else { None }).collect::<String>().trim().to_string()
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

/// Something that can come about: a room reached, a thing at hand (or
/// somewhere the player can get to it), code able to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Fact {
    Room(usize),
    Thing(usize),
    /// Everything an owner (a room, a thing, a conversation, a timer) has can run.
    Live(Owner),
    /// One piece of code has run: a conversation option chosen, a handler done.
    Code(usize),
}

/// One way for a fact to come about: once every one of `needs` has.
struct Rule {
    fact: Fact,
    needs: Vec<Fact>,
    /// Shown as a step, of this kind: it takes some doing.
    kind: Option<&'static str>,
    text: String,
    /// Things it uses up: a key that wears out each time it unlocks.
    wears: Option<usize>,
    /// The code it leans on, whose inside the chain can't see: its origin.
    script: Option<String>,
    /// What it takes on trust: "the way north opens (when: clock.night)".
    assume: Option<String>,
}

impl Rule {
    fn new(fact: Fact, needs: Vec<Fact>) -> Rule {
        Rule { fact, needs, kind: None, text: String::new(), wears: None, script: None, assume: None }
    }
}

/// One part of a rule's needs, with the words it adds.
#[derive(Clone)]
struct Part {
    needs: Vec<Fact>,
    text: String,
    script: Option<String>,
}

impl Part {
    fn new(needs: Vec<Fact>, text: impl Into<String>) -> Part {
        Part { needs, text: text.into(), script: None }
    }
    /// Once a piece of code has run.
    fn code(s: &Snippet, text: impl Into<String>) -> Part {
        Part { needs: vec![Fact::Code(s.idx)], text: text.into(), script: Some(s.origin.clone()) }
    }
}

/// Every combination of one part from each list (at most a few dozen).
fn combos(lists: &[Vec<Part>]) -> Vec<Vec<Part>> {
    let mut out: Vec<Vec<Part>> = vec![Vec::new()];
    for list in lists {
        let mut next = Vec::new();
        for c in &out {
            for p in list {
                if next.len() >= 48 {
                    break;
                }
                let mut c = c.clone();
                c.push(p.clone());
                next.push(c);
            }
        }
        out = next;
    }
    out
}

/// `vars.name` in Lua source: the names read.
fn vars_read(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find("vars.") {
        let start = from + pos + 5;
        let name: String = src[start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        from = start + name.len();
        let before_ok = src[..start - 5].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        if before_ok && !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// A Lua value at the start of `s`, as written: `"rebels"` → rebels, `true`, `3`.
fn token(s: &str) -> String {
    let s = s.trim_start();
    match s.chars().next() {
        Some(q @ ('"' | '\'')) => s[1..].split(q).next().unwrap_or("").to_string(),
        _ => s.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '-').collect(),
    }
}

/// Each `vars.name` in Lua source, and what follows it.
fn after_var<'a>(src: &'a str, name: &str) -> Vec<&'a str> {
    let pat = format!("vars.{name}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find(&pat) {
        let start = from + pos;
        let end = start + pat.len();
        from = end;
        let before_ok = src[..start].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        let after_ok = src[end..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            out.push(&src[end..]);
        }
    }
    out
}

/// What Lua source sets `vars.name` to: `vars.side = "rebels"` → rebels.
fn values_set(src: &str, name: &str) -> Vec<String> {
    after_var(src, name)
        .into_iter()
        .filter_map(|rest| {
            let rest = rest.trim_start();
            (rest.starts_with('=') && !rest.starts_with("==")).then(|| token(&rest[1..]))
        })
        .collect()
}

/// Whether Lua source sets `vars.name` to something (not `false` or `nil`).
fn sets_var(src: &str, name: &str) -> bool {
    values_set(src, name).iter().any(|v| v != "false" && v != "nil")
}

/// The value Lua source compares `vars.name` with: `vars.side == "rebels"`.
fn compared(src: &str, name: &str) -> Option<String> {
    after_var(src, name).into_iter().find_map(|rest| {
        let rest = rest.trim_start();
        rest.strip_prefix("==").map(token).filter(|v| v != "nil" && v != "false" && !v.is_empty())
    })
}

/// Whether Lua source checks `vars.name` hasn't been set yet: `not
/// vars.name`, `vars.name == nil`, `vars.name == false`.
fn checks_unset(src: &str, name: &str) -> bool {
    let pat = format!("not vars.{name}");
    let negated = src.match_indices(&pat).any(|(i, _)| src[i + pat.len()..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_')));
    negated
        || after_var(src, name).into_iter().any(|rest| {
            let rest = rest.trim_start();
            rest.strip_prefix("==").map(token).is_some_and(|v| v == "nil" || v == "false")
        })
}

pub fn puzzles(story: &Story) -> Puzzles {
    let n = story.entities.len();
    let def = |i: usize| &story.entities[i];
    let name = |i: usize| def(i).name.clone();
    let code = snippets(story);
    let flag = |i: usize, f: &str| matches!(def(i).state.get(f), Some(Value::Bool(true)));
    let keys_for = |i: usize| def(i).unlocks_with.map(|k| k.index());
    // Code naming an entity, as the parts of a rule: "a script does it".
    let naming = |i: usize| -> Vec<&Snippet> { code.iter().filter(|s| s.acts && names(s.src, &def(i).key)).collect() };
    let place_fact = |i: usize| if def(i).kind == EntityKind::Room { Fact::Room(i) } else { Fact::Thing(i) };

    let mut rules: Vec<Rule> = Vec::new();

    // ---------------------------------------------------------- things
    for i in 0..n {
        if def(i).kind == EntityKind::Room || i == PLAYER.index() {
            continue;
        }
        // The chain of holders up to a room (or the player).
        let mut at = def(i).location.map(|l| l.index());
        let mut locked: Vec<usize> = Vec::new();
        let mut held_by: Option<usize> = None;
        let mut guard = 0;
        while let Some(h) = at {
            guard += 1;
            if guard > n || h == PLAYER.index() || def(h).kind == EntityKind::Room {
                break;
            }
            if def(h).kind == EntityKind::Character {
                held_by.get_or_insert(h);
            } else if flag(h, "locked") {
                locked.push(h);
            }
            at = def(h).location.map(|l| l.index());
        }
        let mut lists: Vec<Vec<Part>> = Vec::new();
        // Where it is.
        let doors: Vec<usize> = if def(i).has(Trait::Door) {
            (0..n).filter(|&r| def(r).kind == EntityKind::Room && def(r).exits.iter().any(|e| e.through.map(|d| d.index()) == Some(i))).collect()
        } else {
            Vec::new()
        };
        if !doors.is_empty() {
            // A door is where its exits are.
            lists.push(doors.iter().map(|&r| Part::new(vec![Fact::Room(r)], "")).collect());
            locked.clear();
        } else {
            lists.push(match at {
                Some(r) if r == PLAYER.index() => vec![Part::new(vec![], "")],
                Some(r) => vec![Part::new(vec![Fact::Room(r)], "")],
                // Off-stage: a script brings it in.
                None => naming(i).iter().map(|s| Part::code(s, format!("offstage:{}", by_script(story, s)))).collect(),
            });
        }
        // Each locked thing it's in: its key, its code, or a script.
        for &h in &locked {
            let mut ways = Vec::new();
            if let Some(k) = keys_for(h) {
                ways.push(Part::new(vec![Fact::Thing(k)], format!("shut:{h}:with {}", the(&name(k)))));
            }
            if let Some(c) = &def(h).code {
                ways.push(Part::new(vec![], format!("shut:{h}:with the code {c}")));
            }
            for s in naming(h) {
                ways.push(Part::code(s, format!("shut:{h}:by a script")));
            }
            lists.push(ways);
        }
        // Hidden: what reveals it.
        if flag(i, "hidden") {
            let mut ways = Vec::new();
            for r in &def(i).revealed_by {
                let verb = r.category.clone().map(|c| format!("({c})")).or_else(|| r.verbs.first().cloned()).unwrap_or_default();
                ways.push(Part::new(vec![place_fact(r.target.index())], format!("reveal:{} the {}", cap(&verb), name(r.target.index()))));
            }
            for s in naming(i) {
                ways.push(Part::code(s, format!("reveal:{}", by_script(story, s))));
            }
            lists.push(ways);
        }
        // Watched by someone: a fake to swap for it, or taken on trust.
        let watcher = (0..n).find(|&c| def(c).npc.as_ref().is_some_and(|w| w.watches.contains(&EntityId(i as u32))));
        if let Some(c) = watcher {
            let decoys: Vec<usize> = (0..n).filter(|&d| def(d).decoy_for == Some(EntityId(i as u32))).collect();
            if !decoys.is_empty() {
                lists.push(decoys.iter().map(|&d| Part::new(vec![Fact::Thing(d)], format!("swap:Swap {} for {}", the(&name(d)), the(&name(i))))).collect());
            } else {
                lists.push(vec![Part::new(vec![], format!("watched:{c}"))]);
            }
        }
        for parts in combos(&lists) {
            let needs: Vec<Fact> = parts.iter().flat_map(|p| p.needs.clone()).collect();
            let tag = |t: &str| parts.iter().find_map(|p| p.text.strip_prefix(t).map(str::to_string));
            let (kind, text) = if let Some(s) = tag("swap:") {
                (Some("find"), format!("{s}, unseen by {}.", the(&name(watcher.unwrap_or(i)))))
            } else if let Some(c) = tag("watched:").and_then(|c| c.parse::<usize>().ok()) {
                (Some("find"), format!("Take {} while {} isn't looking.", the(&name(i)), the(&name(c))))
            } else if let Some(r) = tag("reveal:") {
                (Some("find"), format!("{r}: {}.", a(&name(i))))
            } else if let Some(sh) = tag("shut:") {
                let (h, how) = sh.split_once(':').unwrap_or(("", ""));
                let h: usize = h.parse().unwrap_or(0);
                (Some("find"), format!("Unlock {} {how}: {}.", the(&name(h)), a(&name(i))))
            } else if let Some(c) = held_by.filter(|_| def(i).kind != EntityKind::Character) {
                // A merchant sells what has a price.
                let sold = def(c).npc.as_ref().is_some_and(|x| x.behavior == Behavior::Merchant) && def(i).price.is_some();
                (Some("find"), format!("{} {} from {}.", if sold { "Buy" } else { "Get" }, the(&name(i)), the(&name(c))))
            } else if let Some(o) = tag("offstage:").filter(|_| def(i).kind != EntityKind::Character) {
                (Some("find"), format!("{o}: {}.", a(&name(i))))
            } else {
                (None, format!("Find {}.", the(&name(i))))
            };
            // Unlocking a box with a key wears the key.
            let wears = locked.first().and_then(|&h| keys_for(h)).filter(|&k| def(k).uses.is_some() && needs.contains(&Fact::Thing(k)));
            let script = parts.iter().find_map(|p| p.script.clone());
            let assume = held_by
                .filter(|_| kind == Some("find") && text.starts_with("Get "))
                .map(|c| format!("the player can get {} from {}, by asking, buying, stealing or fighting", the(&name(i)), the(&name(c))))
                .or_else(|| {
                    tag("watched:").and_then(|c| c.parse::<usize>().ok()).map(|c| format!("the player can take {} while {} isn't looking (it's watched, and there's no fake to swap for it)", the(&name(i)), the(&name(c))))
                });
            rules.push(Rule { fact: Fact::Thing(i), needs, kind, text, wears, script, assume });
        }
    }

    // ----------------------------------------------------------- rooms
    for r in 0..n {
        if def(r).kind != EntityKind::Room {
            continue;
        }
        for e in &def(r).exits {
            let Some(to) = e.to.map(|t| t.index()) else { continue };
            if to == r {
                continue;
            }
            let mut lists: Vec<Vec<Part>> = vec![vec![Part::new(vec![Fact::Room(r)], "")]];
            let mut wears = None;
            if let Some(d) = e.through.map(|d| d.index()) {
                if flag(d, "locked") {
                    let mut ways = Vec::new();
                    if let Some(k) = keys_for(d) {
                        ways.push(Part::new(vec![Fact::Thing(k)], format!("open:Unlock {} with {}", the(&name(d)), the(&name(k)))));
                        wears = Some(k).filter(|&k| def(k).uses.is_some());
                    }
                    if let Some(c) = &def(d).code {
                        ways.push(Part::new(vec![], format!("open:Enter the code {c} on {}", the(&name(d)))));
                    }
                    for s in naming(d) {
                        ways.push(Part::code(s, format!("open:A script opens {}", the(&name(d)))));
                    }
                    lists.push(ways);
                } else if flag(d, "blocked") {
                    lists.push(naming(d).iter().map(|s| Part::code(s, "")).collect());
                }
                if flag(d, "hidden") {
                    let mut ways = vec![Part::new(vec![Fact::Thing(d)], "")];
                    ways.extend(naming(d).iter().map(|s| Part::code(s, "")));
                    lists.push(ways);
                }
            }
            // A condition on the way: what sets the variables it reads (to the
            // value it wants, if it wants one). One it checks is still unset
            // ("not vars.alarm") holds from the start.
            let mut understood = e.when.is_some();
            if let Some(w) = &e.when {
                let read = vars_read(&w.src);
                understood = !read.is_empty();
                for v in read {
                    if checks_unset(&w.src, &v) {
                        continue;
                    }
                    let want = compared(&w.src, &v);
                    let setters: Vec<&Snippet> = code
                        .iter()
                        .filter(|s| s.acts && match &want {
                            Some(x) => values_set(s.src, &v).contains(x),
                            None => sets_var(s.src, &v),
                        })
                        .collect();
                    let to = want.as_ref().map(|x| format!(" to {x}")).unwrap_or_default();
                    if setters.is_empty() {
                        // Nothing sets it: as it starts (problems below say if that's never).
                        understood = false;
                        continue;
                    }
                    lists.push(
                        setters
                            .iter()
                            .map(|s| match s.label.as_ref().filter(|l| !l.is_empty()) {
                                Some(l) => Part::code(s, format!("var:once the player chooses “{l}”")),
                                None => Part::code(s, format!("var:once {} sets vars.{v}{to}", by_origin(story, s))),
                            })
                            .collect(),
                    );
                }
            }
            for parts in combos(&lists) {
                let needs: Vec<Fact> = parts.iter().flat_map(|p| p.needs.clone()).collect();
                let open = parts.iter().find_map(|p| p.text.strip_prefix("open:").map(str::to_string));
                let var = parts.iter().find_map(|p| p.text.strip_prefix("var:").map(str::to_string));
                let (kind, text) = match (open, &e.when, var) {
                    (Some(h), _, _) => (Some("unlock"), format!("{h}: {} opens up.", the_room(&name(to)))),
                    (None, Some(_), Some(v)) => (Some("reach"), format!("Go {} from {} to {}, {v}.", e.direction.key(), the_room(&name(r)), the_room(&name(to)))),
                    (None, Some(_), None) => (Some("reach"), format!("Once it's allowed, go {} from {} to {}.", e.direction.key(), the_room(&name(r)), the_room(&name(to)))),
                    (None, None, _) => (None, format!("Go {} from {} to {}.", e.direction.key(), the_room(&name(r)), the_room(&name(to)))),
                };
                let wears = wears.filter(|k| needs.contains(&Fact::Thing(*k)));
                let script = parts.iter().find_map(|p| p.script.clone());
                let assume = e.when.as_ref().filter(|_| !understood).map(|w| format!("the way {} from {} to {} opens (when: {})", e.direction.key(), the_room(&name(r)), the_room(&name(to)), w.src.trim()));
                rules.push(Rule { fact: Fact::Room(to), needs, kind, text, wears, script, assume });
            }
        }
    }
    // Things made from two others: COMBINE, TIE, PUT IN.
    for one in 0..n {
        for c in &def(one).combines {
            let (other, made) = (c.with.index(), c.makes.index());
            let mut r = Rule::new(Fact::Thing(made), vec![Fact::Thing(one), Fact::Thing(other)]);
            r.kind = Some("find");
            r.text = format!("Put {} and {} together: {}.", the(&name(one)), the(&name(other)), a(&name(made)));
            rules.push(r);
        }
    }
    // Rooms a script moves the player to.
    for i in 0..n {
        if def(i).kind != EntityKind::Room {
            continue;
        }
        for s in naming(i).into_iter().filter(|s| s.src.contains("move")) {
            let mut r = Rule::new(Fact::Room(i), vec![Fact::Code(s.idx)]);
            r.kind = Some("reach");
            r.text = format!("{} takes the player to {}.", cap(&by_origin(story, s)), the_room(&name(i)));
            r.script = Some(s.origin.clone());
            rules.push(r);
        }
    }

    // ------------------------------------------------------------ code
    // Either-or: pieces of code that each check a variable isn't set yet,
    // then set it ("Take the sword" and "Take the shield", each `when: not
    // vars.armed`, each setting it). Once one has run, the others can't.
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    let read: BTreeSet<String> = code.iter().filter(|s| s.acts).flat_map(|s| vars_read(s.src)).collect();
    for v in read {
        let branches: Vec<usize> = code
            .iter()
            .filter(|s| s.acts && sets_var(s.src, &v) && (s.guard.is_some_and(|g| checks_unset(g, &v)) || checks_unset(s.src, &v)))
            .map(|s| s.idx)
            .collect();
        if branches.len() >= 2 {
            groups.push((v, branches));
        }
    }
    let branch_text = |s: &Snippet| match &s.label {
        Some(l) if !l.is_empty() => format!("Choose “{l}” in {}.", by_origin(story, s)),
        _ => format!("{}.", by_script(story, s)),
    };

    rules.push(Rule::new(Fact::Live(Owner::Always), vec![]));
    for i in 0..n {
        rules.push(Rule::new(Fact::Live(Owner::Entity(EntityId(i as u32))), vec![place_fact(i)]));
        if let Some(c) = def(i).talk.map(|c| c.0 as usize) {
            rules.push(Rule::new(Fact::Live(Owner::Choice(c)), vec![place_fact(i)]));
        }
    }
    for s in &code {
        // A piece of code runs once its owner can, with what it checks for at hand.
        let mut r = Rule::new(Fact::Code(s.idx), vec![Fact::Live(s.owner)]);
        if s.acts
            && let Some((_, things)) = handler_action(story, s)
        {
            r.needs.extend(things.into_iter().map(Fact::Thing));
        }
        if groups.iter().any(|(_, b)| b.contains(&s.idx)) {
            r.kind = Some("choose");
            r.text = branch_text(s);
            r.script = Some(s.origin.clone());
        }
        rules.push(r);
        for c in literal_calls(s.src, "open_choice") {
            if let Some(ci) = story.choices.iter().position(|x| x.key == c) {
                rules.push(Rule::new(Fact::Live(Owner::Choice(ci)), vec![Fact::Code(s.idx)]));
            }
        }
        for t in literal_calls(s.src, "start_timer") {
            if let Some(ti) = story.timers.iter().position(|x| x.key == t) {
                rules.push(Rule::new(Fact::Live(Owner::Timer(ti)), vec![Fact::Code(s.idx)]));
            }
        }
    }
    for (ti, t) in story.timers.iter().enumerate() {
        if t.running {
            rules.push(Rule::new(Fact::Live(Owner::Timer(ti)), vec![]));
        }
        for r in &t.started_by {
            rules.push(Rule::new(Fact::Live(Owner::Timer(ti)), vec![place_fact(r.target.index())]));
        }
    }

    // --------------------------------------------------- working it out
    let start = story.meta.start.index();
    // Works the rules through from the start, never letting `banned` happen:
    // each fact, and the rule that first brought it about, in order.
    let solve = |banned: &BTreeSet<Fact>| {
        let mut known: BTreeMap<Fact, Option<usize>> = BTreeMap::from([(Fact::Room(start), None), (Fact::Thing(PLAYER.index()), None)]);
        let mut order: Vec<usize> = Vec::new();
        loop {
            let before = known.len();
            for (ri, r) in rules.iter().enumerate() {
                if !known.contains_key(&r.fact) && !banned.contains(&r.fact) && r.needs.iter().all(|f| known.contains_key(f)) {
                    known.insert(r.fact, Some(ri));
                    order.push(ri);
                }
            }
            if known.len() == before {
                break;
            }
        }
        (known, order)
    };
    let (known, order) = solve(&BTreeSet::new());
    let rooms: BTreeSet<usize> = known.keys().filter_map(|f| if let Fact::Room(r) = f { Some(*r) } else { None }).collect();
    let present: BTreeSet<usize> = known.keys().filter_map(|f| if let Fact::Thing(t) = f { Some(*t) } else { None }).collect();
    let live: BTreeSet<Owner> = known.keys().filter_map(|f| if let Fact::Live(o) = f { Some(*o) } else { None }).collect();

    // Whether `f` came about (as first worked out) only by way of `target`:
    // a way back from the Vault doesn't count as another way into it.
    let through = |f: Fact, target: Fact| {
        let mut seen = BTreeSet::new();
        let mut todo = vec![f];
        while let Some(x) = todo.pop() {
            if x == target {
                return true;
            }
            if seen.insert(x)
                && let Some(Some(ri)) = known.get(&x)
            {
                todo.extend(rules[*ri].needs.iter().copied());
            }
        }
        false
    };
    // The ways a fact could come about, by the end: each one possible without the fact itself.
    let ways_to = |fact: Fact| -> Vec<usize> {
        rules.iter().enumerate().filter(|(_, x)| x.fact == fact && x.needs.iter().all(|f| known.contains_key(f) && !through(*f, fact))).map(|(i, _)| i).collect()
    };

    // ------------------------------------------------------- the steps
    let mut steps: Vec<Step> = vec![Step { kind: "start", id: def(start).key.clone(), text: format!("Start in {}.", name(start)), needs: vec![], or: vec![], either: vec![], script: None }];
    // The step each shown rule became.
    let mut step_of: BTreeMap<usize, usize> = BTreeMap::new();
    let key_of = |f: Fact| match f {
        Fact::Room(i) | Fact::Thing(i) => def(i).key.clone(),
        Fact::Code(si) => match code[si].owner {
            Owner::Entity(e) => def(e.index()).key.clone(),
            Owner::Choice(c) => story.choices[c].key.clone(),
            Owner::Timer(t) => story.timers[t].key.clone(),
            Owner::Always => String::new(),
        },
        Fact::Live(_) => String::new(),
    };
    // The branches a piece of code rules out, in words.
    let rivals = |f: Fact| -> Vec<String> {
        let Fact::Code(si) = f else { return Vec::new() };
        groups.iter().filter(|(_, b)| b.contains(&si)).flat_map(|(_, b)| b.iter().filter(|&&x| x != si).map(|&x| branch_text(&code[x]))).collect()
    };
    for &ri in &order {
        let r = &rules[ri];
        let Some(kind) = r.kind else { continue };
        // Other ways that would do as well, with what's possible by the end.
        let or: Vec<String> = ways_to(r.fact)
            .into_iter()
            .filter(|&i| i != ri && rules[i].text != r.text)
            .map(|i| rules[i].text.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        step_of.insert(ri, steps.len());
        steps.push(Step { kind, id: key_of(r.fact), text: r.text.clone(), needs: vec![], or, either: rivals(r.fact), script: r.script.clone() });
    }
    // What each fact waited on, as steps: through plain rooms and code, back to the steps.
    fn deps(f: Fact, known: &BTreeMap<Fact, Option<usize>>, rules: &[Rule], step_of: &BTreeMap<usize, usize>, memo: &mut BTreeMap<Fact, BTreeSet<usize>>) -> BTreeSet<usize> {
        if let Some(d) = memo.get(&f) {
            return d.clone();
        }
        memo.insert(f, BTreeSet::new());
        let mut out = BTreeSet::new();
        if let Some(Some(ri)) = known.get(&f) {
            for &n in &rules[*ri].needs {
                match known.get(&n) {
                    Some(Some(nr)) if step_of.contains_key(nr) => {
                        out.insert(step_of[nr]);
                    }
                    _ => out.extend(deps(n, known, rules, step_of, memo)),
                }
            }
        }
        memo.insert(f, out.clone());
        out
    }
    let mut memo = BTreeMap::new();
    for (&ri, &si) in &step_of {
        steps[si].needs = deps(rules[ri].fact, &known, &rules, &step_of, &mut memo).into_iter().collect();
    }
    // Every step a fact takes, all the way back.
    let closure = |direct: BTreeSet<usize>, steps: &[Step]| {
        let mut all = BTreeSet::new();
        let mut todo: Vec<usize> = direct.into_iter().collect();
        while let Some(s) = todo.pop() {
            if all.insert(s) {
                todo.extend(steps[s].needs.iter().copied());
            }
        }
        all
    };

    // ------------------------------------------------------ endings
    // What can bring each about: code that calls end_game, timers that end.
    let mut sources: BTreeMap<String, Vec<(String, Fact)>> = BTreeMap::new();
    for s in &code {
        for k in literal_calls(s.src, "end_game") {
            sources.entry(k.to_string()).or_default().push((s.origin.clone(), Fact::Code(s.idx)));
        }
    }
    // The engine's own: a player with health who runs out of it.
    let dies = story.entity(PLAYER).health.is_some() && story.endings.iter().any(|e| e.key == "died");
    if dies {
        sources.entry("died".into()).or_default().push(("the player running out of health".into(), Fact::Live(Owner::Always)));
    }
    for (ti, t) in story.timers.iter().enumerate() {
        if let Some(e) = t.end {
            sources.entry(story.endings[e.index()].key.clone()).or_default().push((format!("timers.{}", t.key), Fact::Live(Owner::Timer(ti))));
        }
    }

    let mut problems = Vec::new();
    let mut reach = Vec::new();
    for e in &story.endings {
        let from = sources.get(&e.key).cloned().unwrap_or_default();
        let open: Vec<&(String, Fact)> = from.iter().filter(|(_, f)| known.contains_key(f)).collect();
        let mut needs = Vec::new();
        if let Some((_, f)) = open.first() {
            let direct = deps(*f, &known, &rules, &step_of, &mut memo);
            let all = closure(direct.clone(), &steps);
            needs = all.into_iter().collect();
            // What the player does, where the code says: "Give the loaf of bread to the child (objects.bread.on.after_give)".
            let how: Vec<String> = open
                .iter()
                .map(|(o, f)| match f {
                    Fact::Code(si) if code[*si].acts => match (&code[*si].label, handler_action(story, &code[*si])) {
                        (Some(l), _) if !l.is_empty() => format!("choosing “{l}” ({o})"),
                        (_, Some((words, _))) => format!("{words} ({o})"),
                        _ => o.clone(),
                    },
                    _ => o.clone(),
                })
                .collect();
            steps.push(Step { kind: "ending", id: e.key.clone(), text: format!("The {} ending: from {}.", e.key, how.join(", ")), needs: direct.into_iter().collect(), or: vec![], either: vec![], script: None });
        } else if from.is_empty() {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "ending", message: format!("Nothing ends the game with '{}': no end_game(\"{}\") and no timer.", e.key, e.key) });
        } else {
            let places: Vec<&str> = from.iter().map(|(o, _)| o.as_str()).collect();
            problems.push(PuzzleProblem { id: e.key.clone(), what: "ending", message: format!("The '{}' ending can't happen: only {} end the game that way, and the player can't get there.", e.key, places.join(", ")) });
        }
        reach.push(EndingReach { key: e.key.clone(), kind: e.kind, reachable: !open.is_empty(), from: from.into_iter().map(|(o, _)| o).collect(), needs, only_if: vec![] });
    }

    // ------------------------------------------------------- either-or
    // Each branch on its own, the others ruled out: which endings it leaves.
    let mut stuck = Vec::new();
    let reached = |k: &BTreeMap<Fact, Option<usize>>, key: &str| sources.get(key).is_some_and(|f| f.iter().any(|(_, x)| k.contains_key(x)));
    // "“Take the sword”", "“Pull the lever”": a branch, as the player sees it.
    let quoted = |si: usize| match &code[si].label {
        Some(l) if !l.is_empty() => format!("“{l}”"),
        _ => format!("“{}”", by_script(story, &code[si])),
    };
    // "chooses “Take the sword”", "does “Pull the lever”".
    let doing = |si: usize| format!("{} {}", if code[si].label.as_ref().is_some_and(|l| !l.is_empty()) { "chooses" } else { "does" }, quoted(si));
    let wins: Vec<&str> = story.endings.iter().filter(|e| e.kind == EndingKind::Win).map(|e| e.key.as_str()).collect();
    for (v, branches) in &groups {
        let branches: Vec<usize> = branches.iter().copied().filter(|b| known.contains_key(&Fact::Code(*b))).collect();
        if branches.len() < 2 {
            continue;
        }
        let with: Vec<(usize, BTreeSet<&str>)> = branches
            .iter()
            .map(|&b| {
                let banned = branches.iter().filter(|&&x| x != b).map(|&x| Fact::Code(x)).collect();
                let (k, _) = solve(&banned);
                (b, story.endings.iter().filter(|e| reached(&k, &e.key)).map(|e| e.key.as_str()).collect())
            })
            .collect();
        for e in reach.iter_mut().filter(|e| e.reachable) {
            let ok: Vec<usize> = with.iter().filter(|(_, ends)| ends.contains(e.key.as_str())).map(|(b, _)| *b).collect();
            if ok.is_empty() {
                e.reachable = false;
                let all: Vec<String> = branches.iter().map(|&b| quoted(b)).collect();
                problems.push(PuzzleProblem {
                    id: e.key.clone(),
                    what: "ending",
                    message: format!("The '{}' ending can't happen: it needs more than one of {}, and only one can happen (each sets vars.{v}).", e.key, all.join(" and ")),
                });
            } else if ok.len() < branches.len() {
                e.only_if.push(format!("only if the player {}", ok.iter().map(|&b| doing(b)).collect::<Vec<_>>().join(" or ")));
            }
        }
        // A branch that leaves no way to win, when another would.
        if with.iter().any(|(_, ends)| wins.iter().any(|w| ends.contains(w))) {
            for (b, ends) in &with {
                if !wins.iter().any(|w| ends.contains(w)) {
                    let others: Vec<String> = branches.iter().filter(|&&x| x != *b).map(|&x| quoted(x)).collect();
                    stuck.push(PuzzleProblem {
                        id: key_of(Fact::Code(*b)),
                        what: match code[*b].owner {
                            Owner::Choice(_) => "choice",
                            Owner::Entity(e) if def(e.index()).kind == EntityKind::Room => "room",
                            Owner::Entity(e) if def(e.index()).kind == EntityKind::Character => "character",
                            _ => "thing",
                        },
                        message: format!("If the player {}, they can't win: it rules out {}.", doing(*b), others.join(" and ")),
                    });
                }
            }
        }
    }

    // --------------------------------------------------- getting stuck
    // Steps on the way to a win, and the things each can't do without.
    let winning: BTreeSet<usize> = reach.iter().filter(|e| e.kind == EndingKind::Win && e.reachable).flat_map(|e| e.needs.iter().copied()).collect();
    let rule_of: BTreeMap<usize, usize> = step_of.iter().map(|(&r, &s)| (s, r)).collect();
    // A thing every way to do a step needs (not just the one found first).
    let must_have = |si: usize| -> Vec<usize> {
        let Some(&ri) = rule_of.get(&si) else { return Vec::new() };
        let ways: Vec<&Rule> = ways_to(rules[ri].fact).into_iter().map(|i| &rules[i]).collect();
        rules[ri]
            .needs
            .iter()
            .filter_map(|f| if let Fact::Thing(t) = f { Some(*t) } else { None })
            .filter(|t| def(*t).kind == EntityKind::Object && ways.iter().all(|w| w.needs.contains(&Fact::Thing(*t))))
            .collect()
    };
    let mut warned: BTreeSet<usize> = BTreeSet::new();
    let lower = |s: &str| {
        let mut c = s.chars();
        c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default().trim_end_matches('.').to_string()
    };
    for &si in &winning {
        for t in must_have(si) {
            if warned.contains(&t) {
                continue;
            }
            let e = def(t);
            let lose = if e.has(Trait::Edible) {
                Some("eat")
            } else if e.has(Trait::Flammable) {
                Some("burn")
            } else if e.has(Trait::Destructible) {
                Some("smash")
            } else {
                None
            };
            let task = lower(&steps[si].text);
            if let Some(verb) = lose {
                warned.insert(t);
                stuck.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("The player can {verb} {}, which they need ({task}). If they do, there's no other way to win.", the(&e.name)) });
                continue;
            }
            // A script that does away with it: `world.key:move_to(nil)`.
            let gone = |l: &str| names(l, &e.key) && l.split(":move_to(").skip(1).any(|a| a.trim_start().starts_with("nil") || a.trim_start().starts_with(')'));
            let away = code.iter().find(|s| s.acts && live.contains(&s.owner) && s.src.lines().any(gone));
            if let Some(s) = away {
                warned.insert(t);
                stuck.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("A script ({}) can take {} away, and the player needs it ({task}). If that happens first, there's no other way to win.", s.origin, the(&e.name)) });
            }
        }
    }
    // Keys that wear out, against how many locks on the way need them.
    let mut uses_needed: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &si in &winning {
        if let Some(k) = rule_of.get(&si).and_then(|&ri| rules[ri].wears)
            && must_have(si).contains(&k)
        {
            uses_needed.entry(k).or_default().push(si);
        }
    }
    for (k, needing) in uses_needed {
        let uses = def(k).uses.unwrap_or(0) as usize;
        let name = the(&def(k).name);
        if needing.len() > uses {
            stuck.push(PuzzleProblem { id: def(k).key.clone(), what: "thing", message: format!("{} breaks after {uses} use{}, but the player needs it {} times to win: {}.", cap(&name), if uses == 1 { "" } else { "s" }, needing.len(), needing.iter().map(|&s| lower(&steps[s].text)).collect::<Vec<_>>().join("; ")) });
        } else if needing.len() == uses {
            stuck.push(PuzzleProblem { id: def(k).key.clone(), what: "thing", message: format!("{} breaks after {uses} use{}, exactly as many as the player needs: locking something with it, or unlocking anything else, leaves them stuck.", cap(&name), if uses == 1 { "" } else { "s" }) });
        }
    }

    // ------------------------------------------------- can't happen
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
        if e.kind == EntityKind::Object && keyish && !story.entities.iter().any(|o| o.unlocks_with == Some(EntityId(i as u32))) && !anywhere(i) {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("{} opens nothing: nothing has `unlocks_with: {}`.", cap(&the(&e.name)), e.key) });
        }
        // Locked, with no key and no script.
        if flag(i, "locked") && e.unlocks_with.is_none() && e.code.is_none() && !anywhere(i) {
            problems.push(PuzzleProblem { id: e.key.clone(), what: "thing", message: format!("{} is locked, and there's no key for it.", cap(&the(&e.name))) });
        }
    }
    // A way that waits on a variable nothing sets.
    for r in 0..n {
        for e in &def(r).exits {
            let (Some(w), Some(to)) = (&e.when, e.to) else { continue };
            for v in vars_read(&w.src) {
                let set_anywhere = code.iter().any(|s| sets_var(s.src, &v));
                let starts_true = matches!(story.vars.get(&v), Some(Value::Bool(true)));
                if !set_anywhere && !starts_true && story.vars.contains_key(&v) {
                    problems.push(PuzzleProblem {
                        id: def(r).key.clone(),
                        what: "room",
                        message: format!("The way {} from {} to {} waits on vars.{v}, but nothing ever sets it.", e.direction.key(), the_room(&name(r)), the_room(&name(to.index()))),
                    });
                }
            }
        }
    }

    // ------------------------------------------------- what it assumes
    let mut assumes: Vec<PuzzleProblem> = Vec::new();
    let mut said: BTreeSet<String> = BTreeSet::new();
    let on_the_way: BTreeSet<usize> = reach.iter().filter(|e| e.reachable).flat_map(|e| e.needs.iter().copied()).collect();
    for &si in &on_the_way {
        let Some(&ri) = rule_of.get(&si) else { continue };
        let r = &rules[ri];
        let what = match r.fact {
            Fact::Room(_) => "room",
            Fact::Thing(i) if def(i).kind == EntityKind::Character => "character",
            Fact::Code(_) => "choice",
            _ => "thing",
        };
        if let Some(a) = &r.assume
            && said.insert(a.clone())
        {
            assumes.push(PuzzleProblem { id: steps[si].id.clone(), what, message: format!("Assumes {a}.") });
        }
        if let Some(origin) = r.script.as_ref().filter(|_| r.kind != Some("choose"))
            && said.insert(origin.clone())
        {
            assumes.push(PuzzleProblem {
                id: steps[si].id.clone(),
                what,
                message: format!("Leans on a script ({origin}) for “{}”: the chain can't see inside scripts, so check it does that.", steps[si].text.trim_end_matches('.')),
            });
        }
    }

    Puzzles { steps, endings: reach, problems, stuck, assumes }
}

/// The entity a handler checks one of its arguments is: `args.tool ==
/// world.child` (either way round).
fn checked_arg(story: &Story, src: &str, arg: &str) -> Option<usize> {
    let a = format!("args.{arg}");
    for (i, _) in src.match_indices(&a) {
        let rest = src[i + a.len()..].trim_start();
        if let Some(r) = rest.strip_prefix("==").map(str::trim_start).and_then(|r| r.strip_prefix("world."))
            && let Some(e) = story.entity_by_key(&token(r))
        {
            return Some(e.index());
        }
    }
    let w = format!("== {a}");
    for (i, _) in src.match_indices(&w) {
        let before = src[..i].trim_end();
        let key: String = before.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>().chars().rev().collect();
        if before[..before.len() - key.len()].ends_with("world.")
            && let Some(e) = story.entity_by_key(&key)
        {
            return Some(e.index());
        }
    }
    None
}

/// What the player does to run a handler, in words, and the things it
/// checks for: `objects.bread.on.after_give` with `args.tool ==
/// world.child` is "Give the loaf of bread to the child", and needs the
/// child. None when it isn't a verb's handler.
fn handler_action(story: &Story, s: &Snippet) -> Option<(String, Vec<usize>)> {
    let parts: Vec<&str> = s.origin.split('.').collect();
    let [section, owner, "on", event, ..] = parts[..] else { return None };
    let o = story.entity_by_key(owner)?.index();
    let key = event.trim_start_matches("before_").trim_start_matches("after_");
    let verb = story.verbs.iter().find(|v| v.key == key)?;
    let tool = checked_arg(story, s.src, "tool");
    let target = if section == "rooms" { checked_arg(story, s.src, "target") } else { Some(o) };
    let slots = |p: &Pattern, slot: Slot| p.0.iter().any(|t| matches!(t, PatternToken::Slot(x) if *x == slot));
    let pattern = verb.patterns.iter().find(|p| {
        slots(p, Slot::Target) == target.is_some() && slots(p, Slot::Tool) == tool.is_some() && !slots(p, Slot::Text) && !slots(p, Slot::Direction)
    })?;
    let name = |i: usize| the(&story.entities[i].name);
    let words: Vec<String> = pattern
        .0
        .iter()
        .map(|t| match t {
            PatternToken::Word(w) => w.clone(),
            PatternToken::Slot(Slot::Target) => target.map(name).unwrap_or_default(),
            PatternToken::Slot(Slot::Tool) => tool.map(name).unwrap_or_default(),
            PatternToken::Slot(_) => String::new(),
        })
        .collect();
    let needs = tool.into_iter().chain(target.filter(|_| section == "rooms")).collect();
    Some((cap(&words.join(" ")), needs))
}

/// What a script does to bring something out, from where it's written:
/// `objects.tide_pool.on.search` is "Search the tide pool".
fn by_script(story: &Story, s: &Snippet) -> String {
    if let Some(l) = s.label.as_ref().filter(|l| !l.is_empty()) {
        return format!("Choose “{l}”");
    }
    if let Some((words, _)) = handler_action(story, s) {
        return words;
    }
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

/// Where a piece of code is, as an author would say it: "talking to the keeper", "entering the Cellar".
fn by_origin(story: &Story, s: &Snippet) -> String {
    let parts: Vec<&str> = s.origin.split('.').collect();
    if let [section, owner, "on", event, ..] = parts[..]
        && let Some(o) = story.entity_by_key(owner)
    {
        let owner = &story.entity(o).name;
        let event = event.trim_start_matches("before_").trim_start_matches("after_").replace('_', " ");
        return match (section, event.as_str()) {
            ("rooms", "enter") => format!("entering {owner}"),
            ("rooms", "leave") => format!("leaving {owner}"),
            ("rooms", _) => format!("a script in {owner}"),
            (_, "turn") => format!("{} in time", the(owner)),
            (_, verb) => format!("{} {}", verb.to_uppercase(), the(owner).to_uppercase()),
        };
    }
    if let ["choices", choice, ..] = parts[..] {
        return format!("the conversation {choice}");
    }
    format!("a script ({})", s.origin)
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
