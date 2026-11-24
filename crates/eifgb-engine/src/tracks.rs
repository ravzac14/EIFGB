//! Tracks: everyone who walks out of a room leaves prints there, showing
//! which way they went. Players can read them (EXAMINE TRACKS) and follow
//! them (FOLLOW TRACKS), and characters with `tracker` follow the player's.
//!
//! Tracks are kept in each room's private `_tracks` state, so they're saved
//! and undone with everything else, and fade after `settings.track_life`
//! actions. A room with `trackless` (a stream, bare rock) holds none.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use crate::world::World;
use eifgb_ir::{Direction, EntityId, Value, PLAYER};
use std::collections::BTreeMap;

/// How many sets of tracks a room keeps.
const KEPT: usize = 8;
/// Tracks at most this many actions old are fresh, and trackers can follow them.
pub const FRESH: u64 = 5;
/// Older than this, they're old and half gone.
const OLD: u64 = 15;

/// One set of tracks: who left them, which way they lead, and how many actions ago.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub who: EntityId,
    pub direction: Direction,
    pub age: u64,
}

impl World {
    /// `who` left `room` going `direction`: its prints are left there,
    /// replacing any older ones of its own.
    pub fn leave_tracks(&mut self, who: EntityId, room: EntityId, direction: Direction) {
        if self.story().meta.settings.track_life == 0 || !self.def(room).leaves_tracks || !self.def(who).leaves_tracks || self.is_crowded(room) {
            return;
        }
        let key = Value::Str(self.def(who).key.clone());
        let mut list: Vec<Value> = match self.get(room, "_tracks") {
            Some(Value::List(l)) => l.iter().filter(|t| !matches!(t, Value::Map(m) if m.get("who") == Some(&key))).cloned().collect(),
            _ => Vec::new(),
        };
        let mut t = BTreeMap::new();
        t.insert("who".to_string(), key);
        t.insert("dir".to_string(), Value::Str(direction.key().into()));
        t.insert("turn".to_string(), Value::Int(self.state.turn as i64));
        list.push(Value::Map(t));
        if list.len() > KEPT {
            list.drain(..list.len() - KEPT);
        }
        self.set(room, "_tracks", Value::List(list));
    }

    /// The tracks in `room` that haven't faded yet, freshest first.
    pub fn tracks_in(&self, room: EntityId) -> Vec<Track> {
        let life = u64::from(self.story().meta.settings.track_life);
        let Some(Value::List(list)) = self.get(room, "_tracks") else { return Vec::new() };
        let mut out: Vec<Track> = list
            .iter()
            .filter_map(|t| {
                let Value::Map(m) = t else { return None };
                let (Some(Value::Str(who)), Some(Value::Str(dir)), Some(Value::Int(turn))) =
                    (m.get("who"), m.get("dir"), m.get("turn"))
                else {
                    return None;
                };
                Some(Track {
                    who: self.id(who)?,
                    direction: Direction::from_key(dir)?,
                    age: self.state.turn.saturating_sub(*turn as u64),
                })
            })
            .filter(|t| t.age < life)
            .collect();
        out.sort_by_key(|t| t.age);
        out
    }

    /// What `who`'s tracks look like: its own `prints`, its faction's, or footprints.
    pub fn prints_of(&self, who: EntityId) -> String {
        self.def(who)
            .prints
            .clone()
            .or_else(|| self.faction(who).and_then(|f| f.prints.clone()))
            .unwrap_or_else(|| "footprints".into())
    }
}

impl<H: ScriptHost> Game<H> {
    /// EXAMINE TRACKS: "Fresh paw prints lead east. Your own footprints lead south."
    pub(crate) fn tracks_action(&mut self) -> bool {
        let here = self.world.here();
        if !self.world.def(here).leaves_tracks {
            self.msg("tracks.trackless", &Ctx::new());
            return true;
        }
        let tracks = self.world.tracks_in(here);
        if tracks.is_empty() {
            self.msg("tracks.none", &Ctx::new());
            return true;
        }
        let mut seen: Vec<(String, Direction, &str)> = Vec::new();
        let mut sentences = Vec::new();
        for t in tracks {
            let prints = self.world.prints_of(t.who);
            let key = if t.who == PLAYER {
                "tracks.own"
            } else if t.age <= FRESH {
                "tracks.fresh"
            } else if t.age <= OLD {
                "tracks.plain"
            } else {
                "tracks.old"
            };
            // Two wolves going the same way leave one set of paw prints.
            if seen.iter().any(|(p, d, k)| *p == prints && *d == t.direction && *k == key) {
                continue;
            }
            seen.push((prints.clone(), t.direction, key));
            let dir = self.grammar.direction_word(t.direction);
            let s = self.msg_str(key, &Ctx::new().text("prints", prints).text("direction", dir));
            sentences.push(capitalize(&s));
        }
        self.push(LineKind::Text, sentences.join(" "));
        true
    }

    /// FOLLOW TRACKS / FOLLOW PAW PRINTS / FOLLOW THE WOLF: walks the way the
    /// freshest matching tracks lead, not counting the player's own.
    pub(crate) fn follow_tracks_action(&mut self, text: Option<&str>, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let general = ["track", "tracks", "trail", "prints", "print", "footprints", "the", "a", "some", "them"];
        let wanted: Vec<String> = text
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|w| !general.contains(&w.as_str()))
            .collect();
        let pick = self.world.tracks_in(here).into_iter().filter(|t| t.who != PLAYER).find(|t| {
            if wanted.is_empty() {
                return true;
            }
            let prints = self.world.prints_of(t.who);
            let v = &self.world.def(t.who).vocab;
            wanted.iter().all(|w| {
                prints.split_whitespace().any(|p| p == w || p.trim_end_matches('s') == w.trim_end_matches('s'))
                    || v.nouns.iter().chain(&v.adjectives).any(|n| n == w)
            })
        });
        let Some(t) = pick else {
            self.msg("tracks.nothing_to_follow", ctx);
            return false;
        };
        let Some(exit) = self.world.def(here).exits.iter().find(|e| e.direction == t.direction && e.to.is_some()).cloned()
        else {
            self.msg("tracks.nothing_to_follow", ctx);
            return false;
        };
        let prints = self.world.prints_of(t.who);
        let dir = self.grammar.direction_word(t.direction);
        self.msg("tracks.follow", &ctx.clone().text("prints", prints).text("direction", dir));
        self.travel(here, &exit)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}
