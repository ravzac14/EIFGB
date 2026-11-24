//! Checking a game by playing it, for the editor and the tests.
//!
//! - **Walkthroughs:** one command a line, `#` comments and blank lines
//!   skipped. A line can say what the game should answer: `take key =>
//!   Taken.` (the reply must contain it, ignoring case and spacing). A
//!   walkthrough is played to a win, or to an ending named for it, and says
//!   what it never reached: rooms, things, conversation options.
//! - **The wanderer:** a test player that tries things at random, to find
//!   script errors, endings, and places where nothing new happens for a
//!   long time.

use crate::game::{Game, LineKind, Output};
use crate::rng::Rng;
use crate::script::ScriptHost;
use eifgb_ir::{EndingKind, EntityId, EntityKind, Segment, Text, PLAYER};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// One line of a walkthrough.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkStep {
    /// The line in the file, from 1.
    pub line: usize,
    pub command: String,
    /// What the reply should contain: `take key => Taken.`
    pub expect: Option<String>,
}

/// The commands in a walkthrough, with what each should answer.
pub fn walkthrough_steps(text: &str) -> Vec<WalkStep> {
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let (command, expect) = match l.split_once("=>") {
                Some((c, e)) => (c.trim(), Some(e.trim().to_string()).filter(|e| !e.is_empty())),
                None => (l, None),
            };
            Some(WalkStep { line: i + 1, command: command.to_string(), expect })
        })
        .collect()
}

/// What a walkthrough has to reach.
#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    /// Any win: `walkthrough.txt`.
    Win,
    /// This ending: `walkthrough-feast.txt`, or a `# ending: feast` line.
    Ending(String),
}

impl Goal {
    /// The goal a walkthrough file has: from a `# ending: key` line, else
    /// from its name (`walkthrough-feast.txt`), else a win.
    pub fn of(file: &str, text: &str) -> Goal {
        let said = text.lines().find_map(|l| l.trim().strip_prefix('#').and_then(|c| c.trim().strip_prefix("ending:")).map(|k| k.trim().to_string()));
        let named = file.strip_prefix("walkthrough-").and_then(|f| f.strip_suffix(".txt")).map(str::to_string);
        match said.or(named) {
            Some(k) if !k.is_empty() => Goal::Ending(k),
            _ => Goal::Win,
        }
    }
}

/// What a walkthrough never reached.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Coverage {
    /// Rooms never entered (their IDs).
    pub rooms: Vec<String>,
    /// Objects and characters never seen (their IDs).
    pub things: Vec<String>,
    /// Conversation options never chosen: `{ choice, node, option, text }`.
    pub options: Vec<UnchosenOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnchosenOption {
    pub choice: String,
    pub node: String,
    pub option: usize,
    pub text: String,
}

/// How a walkthrough played.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Played {
    /// `win`, `loss`, `neutral` (the ending it reached), `unfinished`
    /// (still going after the last line), `error` (a script error),
    /// `reply` (a reply that wasn't what the line expected).
    pub result: &'static str,
    /// The commands played.
    pub steps: usize,
    /// Where it went wrong, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// What went wrong, when it didn't reach its goal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// The ending it reached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ending: Option<String>,
    /// What it wants: `null` for any win, or an ending's key.
    pub goal: Option<String>,
    pub never: Coverage,
}

/// A text's words, without its `${…}` parts.
fn literal(t: &Text) -> String {
    t.0.iter().filter_map(|s| if let Segment::Lit(l) = s { Some(l.as_str()) } else { None }).collect::<String>().trim().to_string()
}

/// The first lines of a reply, for saying what the game said.
fn gist(out: &Output) -> String {
    let t = out.text();
    t.lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join(" ")
}

/// Whether `reply` says `expect`: ignoring case, and runs of spaces and line breaks.
fn says(reply: &str, expect: &str) -> bool {
    let norm = |s: &str| s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    norm(reply).contains(&norm(expect))
}

/// What the game, as played so far, never reached.
pub fn coverage<H: ScriptHost>(g: &Game<H>) -> Coverage {
    let w = g.world();
    let story = w.story();
    let mut c = Coverage::default();
    for (i, e) in story.entities.iter().enumerate() {
        let id = EntityId(i as u32);
        match e.kind {
            EntityKind::Room if !w.state.visited.contains(&id) && id != w.here() => c.rooms.push(e.key.clone()),
            EntityKind::Object | EntityKind::Character if id != PLAYER && !w.state.located.contains_key(&id) && !w.state.got.contains_key(&id) => {
                c.things.push(e.key.clone())
            }
            _ => {}
        }
    }
    let chosen = g.chosen();
    for (ci, ch) in story.choices.iter().enumerate() {
        for (ni, node) in ch.nodes.iter().enumerate() {
            for (oi, o) in node.options.iter().enumerate() {
                if !chosen.contains(&(ci as u32, ni as u32, oi)) {
                    c.options.push(UnchosenOption { choice: ch.key.clone(), node: node.key.clone(), option: oi, text: literal(&o.text) });
                }
            }
        }
    }
    c
}

/// Plays a walkthrough on a started game, to its goal. `name` is how the
/// problem says it ("the walkthrough", "walkthrough-feast.txt").
pub fn play_walkthrough<H: ScriptHost>(g: &mut Game<H>, text: &str, goal: &Goal, name: &str) -> Played {
    let want = match goal {
        Goal::Win => None,
        Goal::Ending(k) => Some(k.clone()),
    };
    let done = |result: &'static str, steps: usize, line: Option<usize>, problem: Option<String>, ending: Option<String>, g: &Game<H>| Played {
        result,
        steps,
        line,
        problem,
        ending,
        goal: want.clone(),
        never: coverage(g),
    };
    let steps = walkthrough_steps(text);
    let mut n = 0;
    for s in &steps {
        n += 1;
        let out = g.input(&s.command);
        let (line, command) = (s.line, &s.command);
        if let Some(err) = out.lines.iter().find(|l| l.kind == LineKind::Error) {
            let p = format!("{name} hits a script error at line {line} ({command}): {}", err.text);
            return done("error", n, Some(line), Some(p), None, g);
        }
        if let Some(e) = &s.expect
            && !says(&out.text(), e)
        {
            let p = format!("{name} expected “{e}” at line {line} ({command}), but the game said: {}", gist(&out));
            return done("reply", n, Some(line), Some(p), None, g);
        }
        if let Some(kind) = out.ended {
            let result = match kind {
                EndingKind::Win => "win",
                EndingKind::Loss => "loss",
                EndingKind::Neutral => "neutral",
            };
            let ending = out.ending.clone();
            let problem = match &want {
                None if kind != EndingKind::Win => Some(format!("{name} no longer wins: at line {line} ({command}) the game ends in a {result}: {}", gist(&out))),
                Some(k) if ending.as_deref() != Some(k.as_str()) => Some(format!(
                    "{name} no longer reaches the {k} ending: at line {line} ({command}) the game ends with {}: {}",
                    ending.as_deref().map(|e| format!("the {e} ending")).unwrap_or_else(|| "another ending".into()),
                    gist(&out)
                )),
                _ => None,
            };
            return done(result, n, Some(line), problem, ending, g);
        }
    }
    let what = match &want {
        None => "no longer wins".to_string(),
        Some(k) => format!("no longer reaches the {k} ending"),
    };
    let problem = if n == 0 { format!("{name} is empty") } else { format!("{name} {what}: after its {n} commands the game is still going") };
    done("unfinished", n, None, Some(problem), None, g)
}

// ------------------------------------------------------------ the wanderer

/// What the wanderer found.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Wandered {
    pub moves: usize,
    /// Games played (a new one after each ending).
    pub games: usize,
    /// Script errors, each once, with the commands that led to it.
    pub errors: Vec<WanderError>,
    /// Each ending reached, and the move it was first reached at.
    pub endings: BTreeMap<String, usize>,
    /// Rooms entered and things seen, of all there are.
    pub rooms: (usize, usize),
    pub things: (usize, usize),
    /// The longest stretch with nothing new (no room, nothing seen or got):
    /// how long, from which move, and where it began.
    pub quiet: Option<Quiet>,
    /// Rooms it never got to (their IDs).
    pub never: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WanderError {
    pub at: usize,
    pub command: String,
    pub text: String,
    /// The commands of that game, up to and including the one that failed.
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quiet {
    pub from: usize,
    pub moves: usize,
    /// The room it was in then (its ID).
    pub room: String,
}

/// Verbs the wanderer tries on what's around (English, like the stdlib v1).
const ON_A_THING: &[&str] = &["examine", "take", "drop", "open", "close", "search", "push", "pull", "eat", "wear", "read", "talk to", "climb", "turn", "light", "enter", "move"];
const ON_TWO: &[&str] = &["unlock {t} with {k}", "put {k} in {t}", "give {k} to {t}", "show {k} to {t}"];
const ALONE: &[&str] = &["look", "wait", "inventory", "listen", "hint"];

fn pick<'a, T>(rng: &mut Rng, list: &'a [T]) -> Option<&'a T> {
    if list.is_empty() { None } else { Some(&list[(rng.next_u64() % list.len() as u64) as usize]) }
}

/// A word a thing answers to: its last noun, or its name's last word.
fn word_for<H: ScriptHost>(g: &Game<H>, t: EntityId) -> String {
    let d = g.world().def(t);
    d.vocab.nouns.last().cloned().unwrap_or_else(|| d.name.split_whitespace().last().unwrap_or("it").to_lowercase())
}

/// Something to try now.
fn next_command<H: ScriptHost>(g: &Game<H>, rng: &mut Rng, options: usize) -> String {
    let w = g.world();
    if w.state.choice.is_some() && options > 0 {
        return (1 + rng.next_u64() % options as u64).to_string();
    }
    let ways: Vec<String> = w.def(w.here()).exits.iter().filter(|e| e.to.is_some() && w.exit_visible(e)).map(|e| e.direction.key().to_string()).collect();
    let things: Vec<EntityId> = w.scope().into_iter().filter(|&t| t != PLAYER && w.def(t).kind != EntityKind::Room).collect();
    let held: Vec<EntityId> = w.inventory();
    let roll = rng.next_u64() % 100;
    if roll < 30
        && let Some(d) = pick(rng, &ways)
    {
        return d.clone();
    }
    if roll < 80
        && let Some(&t) = pick(rng, &things)
    {
        if roll.is_multiple_of(5)
            && let (Some(&k), Some(how)) = (pick(rng, &held), pick(rng, ON_TWO))
            && k != t
        {
            return how.replace("{t}", &word_for(g, t)).replace("{k}", &word_for(g, k));
        }
        let verb = pick(rng, ON_A_THING).copied().unwrap_or("examine");
        return format!("{verb} {}", word_for(g, t));
    }
    pick(rng, ALONE).copied().unwrap_or("look").to_string()
}

/// Plays `moves` random moves, from new games made by `make`, starting a
/// new one after each ending. None from `make`: the game can't be played.
pub fn wander<H: ScriptHost>(mut make: impl FnMut() -> Option<Game<H>>, seed: u64, moves: usize) -> Wandered {
    let mut report = Wandered::default();
    let Some(mut g) = make() else { return report };
    let mut rng = Rng::new(seed);
    let mut out = g.start();
    report.games = 1;
    let story = g.world().story().clone();
    let rooms_total = story.entities.iter().filter(|e| e.kind == EntityKind::Room).count();
    let things_total = story.entities.iter().enumerate().filter(|(i, e)| *i != PLAYER.index() && matches!(e.kind, EntityKind::Object | EntityKind::Character)).count();
    let (mut rooms, mut things): (BTreeSet<EntityId>, BTreeSet<EntityId>) = (BTreeSet::new(), BTreeSet::new());
    let mut history: Vec<String> = Vec::new();
    let mut last_new = 0usize;
    let mut seen_errors: BTreeSet<String> = BTreeSet::new();
    let note = |g: &Game<H>, rooms: &mut BTreeSet<EntityId>, things: &mut BTreeSet<EntityId>| -> bool {
        let w = g.world();
        let before = rooms.len() + things.len();
        rooms.insert(w.here());
        rooms.extend(w.state.visited.iter().copied());
        things.extend(w.state.located.keys().copied());
        things.extend(w.state.got.keys().copied());
        rooms.len() + things.len() > before
    };
    note(&g, &mut rooms, &mut things);
    let mut quiet_room = g.world().def(g.world().here()).key.clone();
    for m in 1..=moves {
        let command = next_command(&g, &mut rng, out.options.len());
        history.push(command.clone());
        out = g.input(&command);
        report.moves = m;
        if let Some(err) = out.lines.iter().find(|l| l.kind == LineKind::Error)
            && seen_errors.insert(err.text.clone())
            && report.errors.len() < 10
        {
            report.errors.push(WanderError { at: m, command: command.clone(), text: err.text.clone(), commands: history.clone() });
        }
        if note(&g, &mut rooms, &mut things) {
            last_new = m;
            quiet_room = g.world().def(g.world().here()).key.clone();
        } else if m - last_new > report.quiet.as_ref().map_or(0, |q| q.moves) {
            report.quiet = Some(Quiet { from: last_new + 1, moves: m - last_new, room: quiet_room.clone() });
        }
        if out.ended.is_some() || out.quit_requested {
            if let Some(e) = &out.ending {
                report.endings.entry(e.clone()).or_insert(m);
            }
            let Some(next) = make() else { break };
            g = next;
            out = g.start();
            report.games += 1;
            history.clear();
        }
    }
    report.rooms = (rooms.len(), rooms_total);
    report.things = (things.len(), things_total);
    report.never = story.entities.iter().enumerate().filter(|(i, e)| e.kind == EntityKind::Room && !rooms.contains(&EntityId(*i as u32))).map(|(_, e)| e.key.clone()).collect();
    report
}

// ---------------------------------------------------------- dead ends

/// A way to make the game unwinnable: after `after` steps of the
/// walkthrough, `command` loses something it needs later, for good.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeadEnd {
    /// Walkthrough steps played first.
    pub after: usize,
    /// What the player does then.
    pub command: String,
    /// What's gone for good: names.
    pub lost: Vec<String>,
    /// The later walkthrough command that needed it.
    pub needed_by: String,
    /// Every command, from the start: the steps, then `command`.
    pub commands: Vec<String>,
}

/// The most candidate moves tried, so a long game stays quick.
const DEAD_END_TRIES: usize = 600;

/// Looks for dead ends along a walkthrough: at each step, things the rest
/// of it needs are eaten, drunk, burnt, smashed, given away or thrown at
/// someone; if one's gone for good and the rest of the walkthrough no
/// longer wins, that's a dead end. Only the first way of losing each thing
/// by each verb is said.
pub fn dead_ends<H: ScriptHost>(mut make: impl FnMut() -> Option<Game<H>>, walkthrough: &str) -> Vec<DeadEnd> {
    let Some(mut g) = make() else { return Vec::new() };
    g.start();
    let steps = walkthrough_steps(walkthrough);
    // Played once: the game before each step, and what each step named.
    let mut saves = Vec::new();
    let mut named: Vec<Vec<EntityId>> = Vec::new();
    for s in &steps {
        saves.push(g.world().save());
        let out = g.input(&s.command);
        // What it named (the game keeps the last things named, even
        // through a command that names nothing, so only those whose name is in it).
        let words: Vec<String> = s.command.to_lowercase().split_whitespace().map(str::to_string).collect();
        let w = g.world();
        let pronoun = words.iter().any(|c| matches!(c.as_str(), "it" | "them" | "him" | "her"));
        named.push(
            w.state
                .mentioned
                .iter()
                .copied()
                .filter(|&e| pronoun || w.display_name(e).to_lowercase().split_whitespace().map(str::to_string).chain(w.def(e).vocab.nouns.iter().cloned()).any(|n| words.contains(&n)))
                .collect(),
        );
        if out.ended.is_some() {
            break;
        }
    }
    let n = saves.len();
    let Some(mut w) = make() else { return Vec::new() };
    w.start();
    let mut found = Vec::new();
    let mut said: BTreeSet<(String, EntityId)> = BTreeSet::new();
    let mut tries = 0;
    for i in 0..n {
        // Needed later: what the rest of the walkthrough names, and the last
        // step that does (where it's really wanted: GIVE CAKE TO TROLL).
        let later: BTreeMap<EntityId, usize> = (i..n).flat_map(|j| named[j].iter().map(move |&e| (e, j))).collect();
        w.restore(&saves[i]);
        let world = w.world();
        let scope = world.scope();
        let here: Vec<EntityId> = scope.iter().copied().filter(|&c| c != PLAYER && world.def(c).kind == EntityKind::Character && world.is_active(c)).collect();
        let mut tried: Vec<(String, String)> = Vec::new();
        for (&x, _) in later.iter().filter(|(x, _)| scope.contains(x) && world.def(**x).kind == EntityKind::Object) {
            let name = world.display_name(x);
            for verb in ["eat", "drink", "burn", "smash"] {
                tried.push((verb.into(), format!("{verb} {name}")));
            }
            for &c in &here {
                let who = world.display_name(c);
                tried.push(("give".into(), format!("give {name} to {who}")));
                tried.push(("throw".into(), format!("throw {name} at {who}")));
            }
        }
        for (verb, command) in tried {
            if tries >= DEAD_END_TRIES {
                return found;
            }
            tries += 1;
            w.restore(&saves[i]);
            let before: Vec<(EntityId, bool)> = later.keys().map(|&e| (e, w.world().location(e).is_some() && !w.world().is_dead(e))).collect();
            let out = w.input(&command);
            if out.ended.is_some() {
                continue;
            }
            let lost: Vec<EntityId> = before.iter().filter(|(e, was)| *was && (w.world().location(*e).is_none() || w.world().is_dead(*e))).map(|(e, _)| *e).collect();
            let Some(&first) = lost.first() else { continue };
            if !said.insert((verb.clone(), first)) {
                continue;
            }
            // Does the rest still win, with that gone?
            let mut wins = false;
            for s in &steps[i..] {
                let out = w.input(&s.command);
                if let Some(k) = out.ended {
                    wins = k == EndingKind::Win;
                    break;
                }
            }
            if wins {
                continue;
            }
            let needed_by = steps[later[&first]].command.clone();
            let mut commands: Vec<String> = steps[..i].iter().map(|s| s.command.clone()).collect();
            commands.push(command.clone());
            let names = lost.iter().map(|&e| w.world().display_name(e)).collect();
            found.push(DeadEnd { after: i, command, lost: names, needed_by, commands });
        }
    }
    found
}

/// One step of the walkthrough, for the pacing chart: what it brought.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Beat {
    pub command: String,
    /// The room the player ends the step in.
    pub room: String,
    /// Rooms seen for the first time.
    pub rooms: Vec<String>,
    /// Things picked up for the first time.
    pub things: Vec<String>,
    /// Points scored.
    pub score: i64,
    /// Facts learned, and goals done.
    pub learned: Vec<String>,
    pub goals: Vec<String>,
    /// The ending reached, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ending: Option<String>,
}

/// Plays the walkthrough, saying what each step brought: new rooms, new
/// things, points, facts, goals. Where nothing comes for a long stretch,
/// a player may be bored, or lost.
pub fn pacing<H: ScriptHost>(make: impl FnOnce() -> Option<Game<H>>, walkthrough: &str) -> Vec<Beat> {
    let Some(mut g) = make() else { return Vec::new() };
    g.start();
    let mut out = Vec::new();
    for s in walkthrough_steps(walkthrough) {
        let (visited, got, score, learned, goals) = {
            let st = &g.world().state;
            (st.visited.clone(), st.got.keys().copied().collect::<BTreeSet<EntityId>>(), st.score, st.learned.len(), st.goals.iter().filter(|(_, d)| **d).map(|(k, _)| k.clone()).collect::<BTreeSet<String>>())
        };
        let o = g.input(&s.command);
        let w = g.world();
        let st = &w.state;
        let name = |e: &EntityId| w.def(*e).name.clone();
        out.push(Beat {
            command: s.command.clone(),
            room: w.def(w.here()).name.clone(),
            rooms: st.visited.difference(&visited).map(name).collect(),
            things: st.got.keys().filter(|k| !got.contains(k)).map(name).collect(),
            score: st.score - score,
            learned: st.learned[learned.min(st.learned.len())..].iter().map(|l| l.key.replace('_', " ")).collect(),
            goals: st.goals.iter().filter(|(k, d)| **d && !goals.contains(*k)).map(|(k, _)| k.replace('_', " ")).collect(),
            ending: o.ending.clone(),
        });
        if o.ended.is_some() {
            break;
        }
    }
    out
}
