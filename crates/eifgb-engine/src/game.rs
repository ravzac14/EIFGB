//! The turn loop: input in, [`Output`] out. The engine does no I/O of its
//! own. Hosts (the CLI, the web page) display the output, and handle
//! SAVE, RESTORE and QUIT when the output asks them to.
//!
//! How a command runs:
//! 1. `before_<verb>` handlers on the target, then the room, then global.
//!    Any of them can `return block(...)` to stop the command.
//! 2. The first `<verb>` handler found in that same order, if any, and
//!    otherwise the verb's built-in behaviour (or its `default` text).
//! 3. `after_<verb>` handlers, only if step 2 succeeded.
//! 4. The turn ends: the counter goes up, then `turn` handlers run
//!    (global, then the room's).
//! 5. Endings and newly opened choices take effect.

use crate::parser::{entity_token, Command, Failure, Grammar, Parsed};
use crate::script::{Ctx, Effect, ScriptError, ScriptHost, ScriptValue};
use crate::state::ChoiceCursor;
use crate::text::{reflow, render};
use crate::world::World;
use eifgb_ir::*;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    /// The game's title, at the start.
    Banner,
    /// A room name.
    Title,
    Text,
    /// Out-of-world remarks: score changes, "[Undone.]" and so on.
    Notice,
    /// A numbered option while a choice is open.
    Choice,
    /// An option not open yet, with why: shown greyed, not to be picked.
    Locked,
    Ending,
    /// A script error. Shown so authors can fix it; play carries on.
    Error,
    /// A picture: `picture` is the file or address, `text` what it shows.
    Picture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
    /// For a picture: its file (relative to the game's folder) or web address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub room: String,
    pub score: i64,
    pub max_score: Option<i64>,
    pub turns: u64,
    /// The time of day, like "7:30 am", when the game keeps time and shows it.
    pub time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Output {
    pub lines: Vec<Line>,
    pub status: Status,
    /// The open choice's options, in display order and numbered from 1.
    /// A web host can show these as buttons.
    pub options: Vec<String>,
    pub ended: Option<EndingKind>,
    /// Which ending was reached (its key in `endings:`), with `ended`.
    pub ending: Option<String>,
    /// Set after SAVE: the host should keep this string somewhere.
    pub save: Option<String>,
    /// Set after RESTORE: the host should fetch a save and pass it to [`Game::restore`].
    pub restore_requested: bool,
    pub quit_requested: bool,
    /// Set when the command wasn't understood (and no `parse_failed`
    /// handler answered it): why, for an editor to suggest a fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missed: Option<Missed>,
}

/// Why a command wasn't understood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missed {
    /// `unknown_verb`, `unknown_word` (a word nothing has), `scenery` (a word
    /// only in the room's description), `not_here`, `partial` (the verb, but
    /// not the rest), `no_pronoun` or `other`.
    pub kind: &'static str,
    /// The word that wasn't understood, where there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word: Option<String>,
    /// The verb, when that much was understood.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
}

impl Output {
    /// All lines as plain text, one per line. Handy for tests and logs.
    pub fn text(&self) -> String {
        self.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")
    }
}

pub(crate) enum Pending {
    /// We asked "which do you mean?"; the reply fills in the noun.
    Which { before: Vec<String>, after: Vec<String>, options: Vec<EntityId> },
    /// We asked "what do you want to take?"; the reply completes the command.
    Noun { prefix: Vec<String> },
    /// We asked "Did you mean TAKE?" about `word` in `toks`; YES, or one of
    /// the `options`, runs the command with it fixed.
    Guess { toks: Vec<String>, word: String, options: Vec<String> },
}

pub struct Game<H: ScriptHost> {
    pub(crate) world: World,
    pub(crate) host: H,
    pub(crate) grammar: Grammar,
    seed: u64,
    pub(crate) pending: Option<Pending>,
    pub(crate) last: Option<Vec<String>>,
    /// Set by an action that only asked a question, so the turn doesn't count.
    pub(crate) no_turn: bool,
    /// Walking through on the way somewhere (GO TO): rooms show only their name.
    pub(crate) passing: bool,
    /// Waiting several turns: WAIT says nothing each turn.
    pub(crate) quiet_wait: bool,
    /// What the player was holding when this input began.
    pub(crate) held_before: Vec<EntityId>,
    /// Every conversation option chosen in this game, as (choice, node,
    /// option): what a walkthrough reached. Not part of the saved state.
    chosen: std::collections::BTreeSet<(u32, u32, usize)>,
    /// Where the player was, and what they could see, when this input
    /// began: if it's gone by the end (eaten, burnt) and they're still
    /// there, they saw it go, and WHERE IS knows.
    pub(crate) seen_before: (Option<EntityId>, Vec<EntityId>),
    /// A meta command's free text (ERASE NOTE 2).
    pub(crate) last_text: Option<String>,
    /// The last command that wasn't understood, and the word that wasn't,
    /// for OOPS.
    pub(crate) oops: Option<(Vec<String>, String)>,
    /// What the player asked about (ASK KEEPER ABOUT THE GATE): the
    /// conversation opens at the option that mentions it.
    pub(crate) topic: Option<String>,
    /// Arriving somewhere already visited, in BRIEF mode: no description.
    pub(crate) briefly: bool,
    /// Whether the room was dark when it was last described: when that
    /// changes some other way (a script, a timer), it's described again.
    pub(crate) seen_dark: Option<bool>,
    /// Set while the player SNEAKs, so arriving doesn't wake anyone.
    pub(crate) sneaking: bool,
    /// Set while the player PUSHes something into the next room: it goes
    /// with them, if they get there.
    pub(crate) pushing: Option<EntityId>,
    /// Rooms the player is listening to this turn (EAVESDROP).
    pub(crate) listening: Vec<EntityId>,
    /// The turn the player last heard a fight next door, so it's said once per turn.
    pub(crate) heard_nearby: Option<u64>,
    pub(crate) out: Output,
    pub(crate) end_requested: Option<String>,
    /// True while a command's action runs: a timer started then counts
    /// from the next action, not this one.
    in_action: bool,
    pub(crate) choice_requested: Option<String>,
}

impl<H: ScriptHost> Game<H> {
    pub fn new(story: Story, mut host: H, seed: u64) -> Result<Self, ScriptError> {
        host.load(&story)?;
        let grammar = Grammar::new(&story);
        Ok(Game {
            world: World::new(story, seed),
            host,
            grammar,
            seed,
            pending: None,
            last: None,
            no_turn: false,
            passing: false,
            quiet_wait: false,
            held_before: Vec::new(),
            chosen: std::collections::BTreeSet::new(),
            seen_before: (None, Vec::new()),
            last_text: None,
            oops: None,
            topic: None,
            briefly: false,
            seen_dark: None,
            heard_nearby: None,
            sneaking: false,
            pushing: None,
            listening: Vec::new(),
            out: Output::default(),
            end_requested: None,
            in_action: false,
            choice_requested: None,
        })
    }

    /// "REX, SIT" is "TELL REX TO SIT", when the words before the comma name
    /// one character here.
    fn addressed(&self, line: &str) -> String {
        let Some((who, what)) = line.split_once(',') else { return line.to_string() };
        let words: Vec<String> = self.grammar.split_commands(who).concat();
        let named = self.grammar.name_matches(&self.world, &words);
        match named.as_slice() {
            [c] if self.world.def(*c).kind == eifgb_ir::EntityKind::Character && !what.trim().is_empty() => {
                format!("tell {} to {}", who.trim(), what.trim())
            }
            _ => line.to_string(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Every conversation option chosen in this game: (choice, node, option).
    pub fn chosen(&self) -> &std::collections::BTreeSet<(u32, u32, usize)> {
        &self.chosen
    }

    /// The opening: title, intro, `start` handlers and the first room.
    pub fn start(&mut self) -> Output {
        self.out = Output::default();
        self.intro();
        self.finish()
    }

    pub fn input(&mut self, line: &str) -> Output {
        self.out = Output::default();
        self.held_before = self.world.ids().filter(|&t| t != PLAYER && self.world.is_within(t, PLAYER)).collect();
        self.seen_before = (Some(self.world.here()), self.world.scope());
        if self.world.state.choice.is_some() {
            self.choice_input(line);
        } else if let Some(note) = self.note_in(line) {
            self.add_note(note);
        } else {
            let line = self.addressed(line);
            let commands = self.grammar.split_commands(&line);
            if commands.is_empty() {
                self.msg("parser.empty", &Ctx::new());
            }
            for toks in commands {
                let turn = self.world.state.turn;
                let typed = toks.join(" ");
                let go_on = self.command(toks);
                // For RECAP: the moves that took a turn, as typed.
                if self.world.state.turn > turn {
                    self.remember_move(typed);
                }
                if !go_on {
                    break;
                }
            }
        }
        self.finish()
    }

    /// Loads a save the host fetched after `restore_requested`.
    /// What this input began with (held, seen) isn't this game's any more:
    /// UNDO, RESTART or RESTORE put another in its place.
    pub(crate) fn forget_this_turn(&mut self) {
        self.held_before.clear();
        self.seen_before = (None, Vec::new());
    }

    pub fn restore(&mut self, json: &str) -> Output {
        self.out = Output::default();
        self.forget_this_turn();
        let (notes, brief) = (self.world.state.notes.clone(), self.world.state.brief);
        match self.world.restore(json) {
            Ok(()) => {
                // The notebook stays the player's: an older save doesn't take notes away.
                if !notes.is_empty() {
                    self.world.state.notes = notes;
                }
                self.world.state.brief = brief;
                self.pending = None;
                self.oops = None;
                // Coming back to a game: RECAP is there to say where things stand.
                let recap = self.world.state.turn > 0;
                self.msg_kind(LineKind::Notice, "restore.done", &Ctx::new().value("recap", Value::Bool(recap)));
                self.look();
                if self.world.state.choice.is_some() {
                    self.show_options();
                }
            }
            Err(e) => self.msg("restore.failed", &Ctx::new().text("reason", e.to_string())),
        }
        self.finish()
    }

    fn intro(&mut self) {
        // What the player starts with, they've had from the start.
        for t in self.world.inventory_all() {
            self.world.state.got.insert(t, None);
        }
        let title = self.world.story().meta.title.clone();
        self.push(LineKind::Banner, title);
        if let Some(intro) = self.world.story().intro.clone() {
            self.render_line(LineKind::Text, &intro, &Ctx::new());
        }
        for code in self.global_handlers(Phase::On, "start") {
            self.run(&code, &Ctx::new());
        }
        self.look();
        self.settle();
    }

    fn finish(&mut self) -> Output {
        // The light changed without the room being described (a timer, a
        // script, a lamp running dry): describe it, or the darkness.
        let dark = self.world.in_darkness();
        if self.seen_dark.is_some_and(|was| was != dark) && self.world.state.ending.is_none() && self.world.state.choice.is_none() {
            self.look();
        }
        // What the player can see, they'll know where to find in the dark;
        // and what they put down in the dark, too.
        self.world.note_seen();
        let here = self.world.here();
        let (was, seen) = std::mem::take(&mut self.seen_before);
        for t in seen {
            if was == Some(here) && self.world.location(t).is_none() {
                self.world.state.located.remove(&t);
            }
        }
        for t in std::mem::take(&mut self.held_before) {
            if !self.world.is_within(t, PLAYER) && self.world.touchable_here(t) {
                self.world.state.located.insert(t, here);
            }
        }
        // Where the player first came to have each thing they carry.
        for t in self.world.inventory_all() {
            self.world.state.got.entry(t).or_insert(Some(here));
        }
        let mut out = std::mem::take(&mut self.out);
        out.status = Status {
            room: self.world.def(here).name.clone(),
            score: self.world.state.score,
            max_score: self.world.story().meta.max_score,
            turns: self.world.state.turn,
            time: self.world.clock().filter(|_| self.world.story().clock.as_ref().is_some_and(|c| c.show)).map(|c| c.time),
        };
        out.ended = self.world.state.ending.map(|e| self.world.story().endings[e.index()].kind);
        out.ending = self.world.state.ending.map(|e| self.world.story().endings[e.index()].key.clone());
        out
    }

    // ------------------------------------------------------------ output

    pub(crate) fn push(&mut self, kind: LineKind, text: impl Into<String>) {
        let text = text.into();
        if !text.trim().is_empty() {
            self.out.lines.push(Line { kind, text, picture: None });
        }
    }

    /// Shows an entity's picture, if it has one. Returns whether it did.
    pub(crate) fn show_picture(&mut self, id: EntityId) -> bool {
        let Some(p) = self.world.def(id).picture.clone() else { return false };
        let text = p.alt.unwrap_or_else(|| self.world.def(id).name.clone());
        self.out.lines.push(Line { kind: LineKind::Picture, text, picture: Some(p.file) });
        true
    }

    pub(crate) fn msg(&mut self, key: &str, ctx: &Ctx) {
        self.msg_kind(LineKind::Text, key, ctx);
    }

    pub(crate) fn msg_kind(&mut self, kind: LineKind, key: &str, ctx: &Ctx) {
        match self.world.story().messages.get(key).cloned() {
            Some(t) => self.render_line(kind, &t, ctx),
            None => self.push(LineKind::Error, format!("[missing message '{key}']")),
        }
    }

    pub(crate) fn render_line(&mut self, kind: LineKind, text: &Text, ctx: &Ctx) {
        let s = self.render_str(text, ctx);
        self.push(kind, s);
    }

    /// Adds a sentence to the end of the last text line (the same
    /// paragraph) when `attach` is set and there is one, otherwise as a new line.
    pub(crate) fn push_sentence(&mut self, s: String, attach: bool) {
        match self.out.lines.last_mut() {
            Some(line) if attach && line.kind == LineKind::Text && !s.trim().is_empty() => {
                line.text.push(' ');
                line.text.push_str(&s);
            }
            _ => self.push(LineKind::Text, s),
        }
    }

    pub(crate) fn msg_str(&mut self, key: &str, ctx: &Ctx) -> String {
        match self.world.story().messages.get(key).cloned() {
            Some(t) => self.render_str(&t, ctx),
            None => format!("[missing message '{key}']"),
        }
    }

    pub(crate) fn render_str(&mut self, text: &Text, ctx: &Ctx) -> String {
        let result = render(&mut self.host, &mut self.world, text, ctx);
        self.world.enforce_door_rules();
        match result {
            Ok(r) => {
                self.apply(r.effects);
                reflow(&r.text)
            }
            Err(e) => {
                self.script_error(e);
                String::new()
            }
        }
    }

    fn script_error(&mut self, e: ScriptError) {
        self.push(LineKind::Error, format!("[{e}]"));
    }

    pub(crate) fn run(&mut self, code: &Code, ctx: &Ctx) -> ScriptValue {
        let result = self.host.call(&mut self.world, code, ctx);
        self.world.enforce_door_rules();
        match result {
            Ok(o) => {
                self.apply(o.effects);
                o.value
            }
            Err(e) => {
                self.script_error(e);
                ScriptValue::Nil
            }
        }
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Say(s) => self.push(LineKind::Text, reflow(&s)),
                Effect::Score(n) => {
                    self.msg_kind(LineKind::Notice, "score.gained", &Ctx::new().value("points", Value::Int(n)))
                }
                Effect::EndGame(k) => {
                    self.end_requested.get_or_insert(k);
                }
                Effect::OpenChoice(k) => self.choice_requested = Some(k),
                Effect::StartTimer(k) => self.start_timer(&k),
                Effect::StopTimer(k) => {
                    self.world.state.timers.remove(&k);
                }
                Effect::StartGoal(k) => self.start_goal(&k),
                Effect::CompleteGoal(k) => self.complete_goal(&k),
                Effect::StartScene(k) => self.begin_scene(&k),
                Effect::EndScene(k) => self.end_scene(&k),
                Effect::Weather(k) => self.set_weather(&k),
                Effect::Npc(event) => self.narrate(&event),
            }
        }
    }

    // ---------------------------------------------------------- commands

    /// Runs one command. Returns false when the rest of a chained input
    /// should be skipped.
    pub(crate) fn command(&mut self, toks: Vec<String>) -> bool {
        let toks = self.grammar.normalize(toks);
        let parsed = match self.pending.take() {
            Some(Pending::Which { before, after, options }) => {
                match self.grammar.pick(&self.world, &options, &toks) {
                    Some(id) => {
                        let mut merged = before;
                        merged.push(entity_token(id));
                        merged.extend(after);
                        self.grammar.parse(&self.world, &merged)
                    }
                    None => self.grammar.parse(&self.world, &toks),
                }
            }
            Some(Pending::Guess { toks: was, word, options }) => {
                if let Some(fixed) = self.guess_reply(&toks, &was, &word, &options) {
                    return fixed;
                }
                self.grammar.parse(&self.world, &toks)
            }
            Some(Pending::Noun { prefix }) => {
                let mut merged = prefix;
                merged.extend(toks.iter().cloned());
                match self.grammar.parse(&self.world, &merged) {
                    // "Where did I get what?" "Look.": free text takes any
                    // words, but a reply that's a command of its own is one.
                    Parsed::Command(c) if c.text.is_some() && toks.first().is_some_and(|w| self.grammar.is_verb_word(w)) => match self.grammar.parse(&self.world, &toks) {
                        alone @ Parsed::Command(_) => alone,
                        _ => Parsed::Command(c),
                    },
                    p @ Parsed::Command(_) => p,
                    _ => self.grammar.parse(&self.world, &toks),
                }
            }
            None => self.grammar.parse(&self.world, &toks),
        };

        match parsed {
            Parsed::Command(cmd) => self.execute(cmd),
            Parsed::Ambiguous { before, after, options } => {
                let names = self.which_names(&options);
                let list = (self.grammar.lang().list_or)(&names);
                self.msg("parser.ambiguous", &Ctx::new().text("options", list));
                self.pending = Some(Pending::Which { before, after, options });
                false
            }
            Parsed::NeedNoun { prefix, verb, target, word, way } => {
                match (target, word) {
                    (Some(t), Some(w)) => self.msg(
                        "parser.need_tool",
                        &Ctx::new().text("verb", verb).entity("target", t).text("word", w),
                    ),
                    // GO: which way?
                    _ if way => self.msg("parser.need_way", &Ctx::new().text("verb", verb)),
                    // WHERE DID I GET: "Where did I get what?"
                    _ if is_question(&verb) => self.msg("parser.need_what", &Ctx::new().text("verb", as_asked(&verb))),
                    _ => self.msg("parser.need_target", &Ctx::new().text("verb", verb)),
                }
                self.pending = Some(Pending::Noun { prefix });
                false
            }
            Parsed::Fail(f) => {
                self.parse_failed(&toks, f);
                false
            }
        }
    }

    /// How "Which do you mean…?" names each option. Doors that share a
    /// name say which way they lead: "the door (northwest)".
    pub(crate) fn which_names(&mut self, options: &[EntityId]) -> Vec<String> {
        let names: Vec<String> = options.iter().map(|id| self.the(*id)).collect();
        let here = self.world.here();
        let exits = self.world.def(here).exits.clone();
        let mut out = Vec::new();
        for (i, &id) in options.iter().enumerate() {
            let twin = names.iter().enumerate().any(|(j, n)| j != i && *n == names[i]);
            let way = exits.iter().find(|e| e.through == Some(id) && self.world.exit_visible(e)).map(|e| e.direction);
            out.push(match way {
                Some(d) if twin => {
                    let ctx = Ctx::new().text("name", names[i].clone()).text("way", self.grammar.direction_word(d));
                    self.msg_str("parser.door_way", &ctx)
                }
                _ => names[i].clone(),
            });
        }
        out
    }

    /// Steps 7 and 9 of the fallback chain: first the author's
    /// `parse_failed` hook, then a failure message that says what was
    /// understood. (Step 8, the AI fallback, will slot in between.)
    fn parse_failed(&mut self, toks: &[String], f: Failure) {
        let before = self.out.lines.len();
        let ctx = Ctx::new().text("text", toks.join(" "));
        for code in self.global_handlers(Phase::On, "parse_failed") {
            self.run(&code, &ctx);
        }
        if self.out.lines.len() > before {
            return;
        }
        let verb = toks.first().filter(|w| self.grammar.is_verb_word(w)).cloned();
        let scenery_word = toks.iter().skip(1).find(|w| self.grammar.describes_here(&self.world, w)).cloned();
        // For OOPS, and "Did you mean…?": a near miss is never swapped in unasked.
        match &f {
            Failure::UnknownVerb { word } | Failure::UnknownWord { word } => {
                self.oops = Some((toks.to_vec(), word.clone()));
                let mut guesses = match f {
                    Failure::UnknownVerb { .. } if toks.first() == Some(word) => self.grammar.verb_guesses(word),
                    _ => self.grammar.noun_guesses(&self.world, word),
                };
                // Only guesses that make a command: TAK CAN is TAKE CAN, not
                // TALK CAN. (Or that leave another slip to ask about next:
                // EXAMIN BRASS KYE.)
                guesses.retain(|g| {
                    let fixed: Vec<String> = toks.iter().map(|t| if t == word { g.clone() } else { t.clone() }).collect();
                    match self.grammar.parse(&self.world, &fixed) {
                        Parsed::Fail(Failure::UnknownWord { word: next }) => next != *g,
                        Parsed::Fail(_) => false,
                        _ => true,
                    }
                });
                // A short word on its own is too little to choose between: NOD
                // isn't NO or NOW. (TKAE is TAKE, though.)
                if toks.len() == 1 && word.chars().count() <= 4 && guesses.len() > 1 {
                    guesses.clear();
                }
                if !guesses.is_empty() {
                    self.out.missed = Some(Missed { kind: if matches!(f, Failure::UnknownVerb { .. }) { "unknown_verb" } else { "unknown_word" }, word: Some(word.clone()), verb: None });
                    let shown: Vec<String> = guesses.iter().map(|g| g.to_uppercase()).collect();
                    let list = (self.grammar.lang().list_or)(&shown);
                    self.msg("parser.guess", &ctx.text("word", word.clone()).text("options", list));
                    self.pending = Some(Pending::Guess { toks: toks.to_vec(), word: word.clone(), options: guesses });
                    return;
                }
            }
            _ => self.oops = None,
        }
        self.out.missed = Some(match &f {
            Failure::UnknownVerb { word } => Missed { kind: "unknown_verb", word: Some(word.clone()), verb: None },
            Failure::UnknownWord { word } => Missed { kind: "unknown_word", word: Some(word.clone()), verb },
            Failure::Scenery => Missed { kind: "scenery", word: scenery_word, verb },
            Failure::NotHere => Missed { kind: "not_here", word: None, verb },
            Failure::Partial { verb: v } => Missed { kind: "partial", word: None, verb: Some(v.clone()) },
            Failure::NoPronoun => Missed { kind: "no_pronoun", word: None, verb },
            _ => Missed { kind: "other", word: None, verb },
        });
        match f {
            Failure::Empty => self.msg("parser.empty", &ctx),
            Failure::UnknownVerb { word } => {
                let suggestions = (self.grammar.lang().list_or)(&self.grammar.suggestions(&self.world));
                self.msg("parser.unknown_verb", &ctx.text("word", word).text("suggestions", suggestions));
            }
            Failure::Partial { verb } if is_question(&verb) => self.msg("parser.partial_question", &ctx),
            Failure::Partial { verb } => self.msg("parser.partial", &ctx.text("verb", verb)),
            Failure::UnknownWord { word } => self.msg("parser.unknown_word", &ctx.text("word", word)),
            Failure::NotHere if self.world.in_darkness() => self.msg("parser.dark", &ctx),
            Failure::NotHere => self.msg("parser.not_here", &ctx),
            Failure::Scenery => self.msg("parser.scenery", &ctx),
            Failure::NoPronoun => self.msg("parser.no_pronoun", &ctx),
            Failure::NoAll => self.msg("parser.no_all", &ctx),
            Failure::AllNone => self.msg("parser.all_none", &ctx),
        }
    }

    /// Runs a command made up by the engine, as if it had been typed.
    pub(crate) fn execute_command(&mut self, cmd: Command) -> bool {
        self.execute(cmd)
    }

    fn execute(&mut self, cmd: Command) -> bool {
        let verb = self.world.story().verbs[cmd.verb].clone();
        if let VerbAction::Builtin(a) = verb.action
            && a.is_meta()
        {
            self.last_text = cmd.text.clone();
            return self.meta(a);
        }
        self.oops = None;
        if self.world.state.ending.is_some() {
            self.msg_kind(LineKind::Notice, "ending.options", &Ctx::new());
            return false;
        }
        // Several turns at once: each one an ordinary command.
        if let VerbAction::Builtin(a @ (BuiltinAction::GoTo | BuiltinAction::GoBack | BuiltinAction::WaitFor | BuiltinAction::WaitUntil)) =
            verb.action
        {
            return self.errand(a, &cmd);
        }

        let before = self.world.state.clone();
        // Lying still is over, with anything but waiting or looking about.
        if !matches!(
            verb.action,
            VerbAction::Builtin(BuiltinAction::Wait | BuiltinAction::PlayDead | BuiltinAction::Look | BuiltinAction::Examine | BuiltinAction::Listen | BuiltinAction::Smell)
        ) {
            self.stop_playing_dead();
        }
        self.in_action = true;
        self.perform(&verb, &cmd);
        self.in_action = false;
        if std::mem::take(&mut self.no_turn) {
            return false;
        }
        let mentioned: Vec<EntityId> = cmd.targets.iter().copied().chain(cmd.tool).collect();
        if !mentioned.is_empty() {
            self.world.state.mentioned = mentioned;
        }
        self.end_turn();
        self.keep_barricades();
        self.dead_act_seen();
        self.world.push_undo(before);
        self.last = Some(cmd.words);
        self.settle();
        self.world.state.ending.is_none() && self.world.state.choice.is_none()
    }

    fn meta(&mut self, action: BuiltinAction) -> bool {
        let ctx = Ctx::new();
        match action {
            BuiltinAction::Again => {
                let with = self.last_text.take();
                return match self.last.clone() {
                    // AGAIN WITH THE IRON KEY: the same, with something else.
                    Some(words) if with.is_some() => self.again_with(words, with.as_deref().unwrap_or("")),
                    Some(words) => self.command(words),
                    None => {
                        self.msg("parser.empty", &ctx);
                        false
                    }
                };
            }
            BuiltinAction::Undo => {
                self.pending = None;
                // UNDO 3: several moves at once.
                let wanted = self.last_text.take().and_then(|t| self.count_in(&t)).unwrap_or(1).max(1);
                let mut done = 0;
                self.keep_players(|g| {
                    while done < wanted && g.world.undo() {
                        done += 1;
                    }
                });
                if done == 0 {
                    self.msg("undo.none", &ctx);
                } else {
                    if wanted == 1 {
                        self.msg_kind(LineKind::Notice, "undo.done", &ctx);
                    } else {
                        let c = Ctx::new().value("n", Value::Int(done as i64)).value("short", Value::Bool(done < wanted));
                        self.msg_kind(LineKind::Notice, "undo.done_many", &c);
                    }
                    self.look();
                    if self.world.state.choice.is_some() {
                        self.show_options();
                    }
                }
            }
            BuiltinAction::Save => {
                self.out.save = Some(self.world.save());
                self.msg_kind(LineKind::Notice, "save.done", &ctx);
            }
            BuiltinAction::Restore => self.out.restore_requested = true,
            BuiltinAction::Restart => {
                let seed = self.seed;
                self.keep_players(|g| g.world.restart(seed));
                self.pending = None;
                self.oops = None;
                self.last = None;
                self.intro();
            }
            BuiltinAction::Score => self.msg("score.report", &ctx),
            BuiltinAction::Help => self.msg("help.text", &ctx),
            BuiltinAction::Hint => self.hint(),
            BuiltinAction::Journal => self.journal(),
            BuiltinAction::Time => self.tell_time(),
            BuiltinAction::Exits => self.exits_action(),
            BuiltinAction::Who => self.who(),
            BuiltinAction::Suggest => self.suggest(),
            BuiltinAction::About => self.about(),
            BuiltinAction::Notes => self.show_notes(true),
            BuiltinAction::EraseNote => {
                let which = self.last_text.take().unwrap_or_default();
                self.erase_note(&which);
            }
            BuiltinAction::WhereIs => {
                let what = self.last_text.take().unwrap_or_default();
                self.where_is(&what);
            }
            BuiltinAction::Oops => {
                let word = self.last_text.take().unwrap_or_default();
                return self.oops_action(&word);
            }
            BuiltinAction::Verbose | BuiltinAction::Brief => {
                self.world.state.brief = action == BuiltinAction::Brief;
                self.msg_kind(LineKind::Notice, if self.world.state.brief { "brief.on" } else { "verbose.on" }, &ctx);
            }
            BuiltinAction::Recap => self.recap(),
            BuiltinAction::LastTalk => {
                let who = self.last_text.take().unwrap_or_default();
                self.last_talk(&who);
            }
            BuiltinAction::Map => self.text_map(),
            BuiltinAction::InventoryByPlace => self.inventory_by_place(),
            BuiltinAction::Quit => self.out.quit_requested = true,
            other => unreachable!("{other:?} is not a meta action"),
        }
        true
    }

    /// HINT: the next step for the first puzzle whose `when` is true. How
    /// many steps have been shown is kept with the game (`_hints` on the
    /// player), so it's saved; once they're all shown, the last repeats.
    fn hint(&mut self) {
        let hints = self.world.story().hints.clone();
        if hints.is_empty() {
            return self.msg("hint.none_in_game", &Ctx::new());
        }
        let mut current = None;
        for h in &hints {
            if h.when.as_ref().is_none_or(|w| self.run(w, &Ctx::new()).truthy()) {
                current = Some(h);
                break;
            }
        }
        let Some(h) = current else {
            return self.msg("hint.none", &Ctx::new());
        };
        let mut shown = match self.world.get(PLAYER, "_hints") {
            Some(Value::Map(m)) => m.clone(),
            _ => Default::default(),
        };
        let n = match shown.get(&h.key) {
            Some(Value::Int(n)) => (*n).max(0) as usize,
            _ => 0,
        };
        let step = n.min(h.steps.len() - 1);
        let text = self.render_str(&h.steps[step], &Ctx::new());
        let ctx = Ctx::new()
            .text("hint", text)
            .value("n", Value::Int(step as i64 + 1))
            .value("of", Value::Int(h.steps.len() as i64));
        self.msg("hint.show", &ctx);
        shown.insert(h.key.clone(), Value::Int((step + 1) as i64));
        self.world.set(PLAYER, "_hints", Value::Map(shown));
    }

    fn command_ctx(&self, verb: &Verb, target: Option<EntityId>, cmd: &Command) -> Ctx {
        let mut ctx = Ctx::new().text("verb", verb.key.clone());
        if let Some(t) = target {
            ctx = ctx.entity("target", t);
        }
        if let Some(t) = cmd.tool {
            ctx = ctx.entity("tool", t);
        }
        if let Some(t) = &cmd.text {
            ctx = ctx.text("text", t.clone());
        }
        if let Some(d) = cmd.direction {
            ctx = ctx.text("direction", d.key());
        }
        ctx
    }

    fn perform(&mut self, verb: &Verb, cmd: &Command) {
        let targets: Vec<Option<EntityId>> =
            if cmd.targets.is_empty() { vec![None] } else { cmd.targets.iter().copied().map(Some).collect() };
        let multi = targets.len() > 1;
        for target in targets {
            let start = self.out.lines.len();
            self.perform_one(verb, target, cmd);
            if multi
                && let Some(t) = target
                && self.out.lines.len() > start
            {
                let name = self.world.display_name(t);
                let line = &mut self.out.lines[start];
                line.text = format!("{name}: {}", line.text);
            }
            if self.end_requested.is_some() {
                break;
            }
        }
    }

    pub(crate) fn perform_one(&mut self, verb: &Verb, target: Option<EntityId>, cmd: &Command) {
        let here = self.world.here();
        let ctx = self.command_ctx(verb, target, cmd);
        let with_owner = |owner: Option<EntityId>| Ctx { this: owner, ..ctx.clone() };

        // Things behind glass (in a closed transparent container) can be
        // seen but not touched. Each verb says whether it needs touch;
        // custom verbs do unless the author says `reach: false`.
        if verb.reach {
            for thing in target.into_iter().chain(cmd.tool) {
                if let Some(holder) = self.world.blocked_reach(thing) {
                    self.msg("reach.closed", &Ctx::new().entity("target", thing).entity("holder", holder));
                    return;
                }
                // Up high: out of reach from the floor.
                if self.world.out_of_reach_high(thing) {
                    self.msg("reach.high", &Ctx::new().entity("target", thing));
                    return;
                }
            }
        }

        for (owner, code) in self.handlers(target, here, Phase::Before, &verb.key) {
            if self.run(&code, &with_owner(owner)) == ScriptValue::Block || self.end_requested.is_some() {
                return;
            }
        }

        // `revealed_by` rules matching this action. Revealing happens after
        // the action, but for verbs with nothing else to say here (SEARCH,
        // or a custom verb with no handler) "You find ..." replaces their
        // default reply.
        let reveals = target.map(|t| self.world.reveals(&verb.key, t)).unwrap_or_default();
        // Hitting something that can't be hurt has nothing else to say either.
        let pointless_attack = verb.action == VerbAction::Builtin(BuiltinAction::Attack)
            && target.is_some_and(|t| self.world.health(t).is_none() && !self.world.is_invincible(t));
        let quiet = !reveals.is_empty()
            && (pointless_attack
                || matches!(verb.action, VerbAction::Script | VerbAction::Builtin(BuiltinAction::Search | BuiltinAction::Smell | BuiltinAction::Listen | BuiltinAction::Read)));

        let before_action = self.out.lines.len();
        let combination = self.combination(&verb.key, target, cmd.tool);
        let ok = match self.handlers(target, here, Phase::On, &verb.key).into_iter().next() {
            Some((owner, code)) => self.run(&code, &with_owner(owner)) != ScriptValue::Block,
            None if quiet => true,
            // Two things that make a third: TIE THE ROPE TO THE HOOK.
            None if combination.is_some() => {
                let (a, b, c) = combination.expect("a combination");
                self.combine_now(a, b, &c)
            }
            None => match verb.action {
                VerbAction::Builtin(a) => self.builtin(a, target, cmd, &ctx),
                VerbAction::Script => {
                    match &verb.default {
                        Some(t) => self.render_line(LineKind::Text, t, &ctx),
                        None => self.msg("action.nothing", &ctx),
                    }
                    false
                }
            },
        };

        // A tool that wears out: a use for each time it's used.
        if ok {
            let tool = match verb.action {
                VerbAction::Builtin(BuiltinAction::Unlock | BuiltinAction::Lock) => {
                    cmd.tool.or_else(|| target.and_then(|t| self.world.def(t).unlocks_with).filter(|k| self.world.is_within(*k, PLAYER)))
                }
                VerbAction::Builtin(
                    BuiltinAction::Light | BuiltinAction::Burn | BuiltinAction::Attack | BuiltinAction::Douse | BuiltinAction::Climb | BuiltinAction::Use,
                )
                => cmd.tool,
                // A game's own verb, done by its handler: what it was done with, or to.
                VerbAction::Script => cmd.tool.filter(|t| self.world.def(*t).uses.is_some()).or(target.filter(|t| self.world.def(*t).uses.is_some())),
                _ => None,
            };
            if let Some(t) = tool {
                self.wear_out(t);
            }
        }
        if let Some(t) = target {
            self.timer_rules(&verb.key, t);
        }
        let found: Vec<EntityId> = reveals.into_iter().filter(|id| self.world.is_hidden(*id)).collect();
        if !found.is_empty() {
            for id in &found {
                self.world.unset(*id, "hidden");
            }
            let list = self.list_a(&found);
            let sentence = self.msg_str("reveal.found", &Ctx::new().text("list", list));
            // Joins the action's own reply ("You open the desk. You find a note.").
            self.push_sentence(sentence, self.out.lines.len() > before_action);
        }

        if ok && self.end_requested.is_none() {
            for (owner, code) in self.handlers(target, here, Phase::After, &verb.key) {
                self.run(&code, &with_owner(owner));
                if self.end_requested.is_some() {
                    break;
                }
            }
        }
    }

    /// Handlers for an event: the target's first, then the room's, then the global ones.
    pub(crate) fn handlers(
        &self,
        target: Option<EntityId>,
        room: EntityId,
        phase: Phase,
        event: &str,
    ) -> Vec<(Option<EntityId>, Code)> {
        let story = self.world.story();
        let mut out = Vec::new();
        let mut owners = Vec::new();
        if let Some(t) = target {
            owners.push(t);
        }
        if Some(room) != target {
            owners.push(room);
        }
        for o in owners {
            for h in &story.entity(o).handlers {
                if h.phase == phase && h.event == event {
                    out.push((Some(o), h.code.clone()));
                }
            }
        }
        for code in self.global_handlers(phase, event) {
            out.push((None, code));
        }
        out
    }

    pub(crate) fn global_handlers(&self, phase: Phase, event: &str) -> Vec<Code> {
        self.world
            .story()
            .handlers
            .iter()
            .filter(|h| h.phase == phase && h.event == event)
            .map(|h| h.code.clone())
            .collect()
    }

    pub(crate) fn room_event(&mut self, room: EntityId, event: &str) {
        let codes: Vec<Code> = self
            .world
            .def(room)
            .handlers
            .iter()
            .filter(|h| h.phase == Phase::On && h.event == event)
            .map(|h| h.code.clone())
            .collect();
        for code in codes {
            self.run(&code, &Ctx::new().this(room));
        }
    }

    fn end_turn(&mut self) {
        self.world.state.turn += 1;
        self.tick_clock();
        // Someone the player just beat doesn't get to strike back.
        self.check_defeats();
        self.report_woken();
        // Working free of a trap, a turn at a time.
        if self.world.held_by(PLAYER).is_some() && self.world.struggle(PLAYER) {
            self.msg("trap.you_free", &Ctx::new());
        }
        if self.end_requested.is_some() {
            return;
        }
        self.npc_turns();
        self.report_traps();
        self.fires();
        self.burn_fuel();
        self.weather_turn();
        if self.end_requested.is_some() {
            return;
        }
        self.chatter();
        for code in self.global_handlers(Phase::On, "turn") {
            self.run(&code, &Ctx::new());
        }
        let here = self.world.here();
        self.room_event(here, "turn");
        self.fire_timers();
        self.check_defeats();
        self.report_woken();
    }

    // ------------------------------------------------------------ timers

    fn start_timer(&mut self, key: &str) {
        let Some(t) = self.world.story().timers.iter().find(|t| t.key == key).cloned() else { return };
        if self.world.state.timers.contains_key(key) {
            return;
        }
        let offset = u64::from(self.in_action);
        let wait = timer_wait(&t, &mut self.world.state.rng);
        let due = self.world.state.turn + offset + wait;
        self.world.state.timers.insert(key.to_string(), due);
    }

    /// `started_by` and `stopped_by` rules matching this action.
    fn timer_rules(&mut self, verb: &str, target: EntityId) {
        let hits = |rules: &[ActionRule]| rules.iter().any(|r| r.target == target && r.verbs.iter().any(|v| v == verb));
        let (start, stop): (Vec<String>, Vec<String>) = {
            let timers = &self.world.story().timers;
            (
                timers.iter().filter(|t| hits(&t.started_by)).map(|t| t.key.clone()).collect(),
                timers.iter().filter(|t| hits(&t.stopped_by)).map(|t| t.key.clone()).collect(),
            )
        };
        for k in stop {
            self.world.state.timers.remove(&k);
        }
        for k in start {
            self.start_timer(&k);
        }
    }

    /// Runs timers that are due this turn, in the order they're declared.
    fn fire_timers(&mut self) {
        let now = self.world.state.turn;
        let timers = self.world.story().timers.clone();
        for t in timers {
            if self.end_requested.is_some() {
                break;
            }
            let Some(&due) = self.world.state.timers.get(&t.key) else { continue };
            let due_now = match t.schedule {
                Schedule::At(at) => crate::journal::passes(self.world.state.clock_checked, self.world.state.clock, at),
                _ => due <= now,
            };
            if !due_now {
                continue;
            }
            match t.schedule {
                Schedule::At(_) => {}
                Schedule::After(_) => {
                    self.world.state.timers.remove(&t.key);
                }
                Schedule::Every(_) => {
                    let wait = timer_wait(&t, &mut self.world.state.rng);
                    self.world.state.timers.insert(t.key.clone(), now + wait);
                }
            }
            // A chance: it may not go off this time.
            if let Some(c) = t.chance
                && self.world.state.rng.range(1, 100) > i64::from(c)
            {
                continue;
            }
            if let Some(w) = &t.when
                && !self.run(w, &Ctx::new()).truthy()
            {
                continue;
            }
            // Heard only where it's heard (a room, or wherever a thing is).
            let here = self.world.here();
            let heard = t.heard_in.is_empty() || t.heard_in.iter().any(|&h| h == here || self.world.room_of(h) == Some(here));
            // The first time it's heard, `say`; after that, `say_again` if there is one.
            if heard && (t.say.is_some() || t.say_again.is_some()) {
                let again = !self.world.state.fired.insert(t.key.clone());
                if let Some(s) = t.say_again.as_ref().filter(|_| again).or(t.say.as_ref()) {
                    self.render_line(LineKind::Text, s, &Ctx::new());
                }
            }
            if let Some(r) = &t.run {
                self.run(r, &Ctx::new());
            }
            if let Some(n) = &t.noise {
                self.world.make_call(n.at.unwrap_or(PLAYER), n.loudness, n.faction.as_deref());
            }
            if let Some(e) = t.end {
                let key = self.world.story().endings[e.index()].key.clone();
                self.end_requested.get_or_insert(key);
            }
        }
        self.world.state.clock_checked = self.world.state.clock;
    }

    /// Carries out an ending or an opened choice that a script asked for.
    fn settle(&mut self) {
        self.update_goals();
        self.update_scenes();
        if let Some(key) = self.end_requested.take() {
            self.choice_requested = None;
            self.finish_game(&key);
        } else if let Some(key) = self.choice_requested.take() {
            self.open_choice(&key);
        }
    }

    fn finish_game(&mut self, key: &str) {
        let Some(id) = self.world.story().ending_by_key(key) else { return };
        self.world.state.ending = Some(id);
        self.world.state.choice = None;
        let ending = self.world.story().endings[id.index()].clone();
        self.render_line(LineKind::Ending, &ending.text, &Ctx::new());
        let banner = match ending.kind {
            EndingKind::Win => "ending.win",
            EndingKind::Loss => "ending.loss",
            EndingKind::Neutral => "ending.neutral",
        };
        self.msg_kind(LineKind::Ending, banner, &Ctx::new());
        self.msg_kind(LineKind::Notice, "ending.options", &Ctx::new());
    }

    // ----------------------------------------------------------- choices

    pub(crate) fn open_choice(&mut self, key: &str) {
        let Some(idx) = self.world.story().choices.iter().position(|c| c.key == key) else { return };
        let start = self.world.story().choices[idx].start;
        self.world.state.choice = Some(ChoiceCursor { choice: ChoiceId(idx as u32), node: start });
        let topic = self.topic.take();
        let shown = self.out.lines.len();
        // A new conversation with them: what's said is kept, for WHAT DID SHE SAY?
        self.world.state.talks.insert(key.to_string(), Vec::new());
        self.world.state.last_talk = Some(key.to_string());
        self.enter_node();
        self.keep_talk(key, shown);
        // ASK ABOUT THE GATE: straight to what was asked about, without
        // listing the options first.
        if let Some(i) = topic.and_then(|t| self.topic_option(&t)) {
            let mut n = 0;
            self.out.lines.retain(|l| {
                n += 1;
                n <= shown || l.kind != LineKind::Choice
            });
            self.out.options.clear();
            self.take_option(i, None);
        }
    }

    /// For a conversation's scripts and text: `self` is the one being
    /// talked to (whoever has it as their `talk` and is here).
    fn talk_ctx(&self) -> Ctx {
        let who = self.world.state.choice.and_then(|c| self.talker_now(&self.world.story().choice(c.choice).key));
        match who {
            Some(t) => Ctx::new().this(t),
            None => Ctx::new(),
        }
    }

    fn enter_node(&mut self) {
        // Nodes without options move on by themselves. The limit stops a
        // loop of such nodes from hanging the game.
        for _ in 0..100 {
            let Some(cur) = self.world.state.choice else { return };
            let node = self.world.story().choice(cur.choice).nodes[cur.node as usize].clone();
            if let Some(t) = &node.text {
                self.render_line(LineKind::Text, t, &self.talk_ctx());
            }
            if let Some(r) = &node.run {
                self.run(r, &self.talk_ctx());
            }
            if self.end_requested.is_some() {
                self.world.state.choice = None;
                self.settle();
                return;
            }
            if !node.options.is_empty() {
                self.show_options();
                return;
            }
            match node.next {
                Some(Next::Goto(n)) => self.world.state.choice = Some(ChoiceCursor { node: n, ..cur }),
                _ => {
                    self.world.state.choice = None;
                    self.settle();
                    return;
                }
            }
        }
        self.push(LineKind::Error, "[choice nodes without options loop forever; stopping]");
        self.world.state.choice = None;
    }

    /// The open node's options whose `when` is true, as (index, text).
    pub(crate) fn visible_options(&mut self) -> Vec<(usize, String)> {
        let Some(cur) = self.world.state.choice else { return Vec::new() };
        let node = self.world.story().choice(cur.choice).nodes[cur.node as usize].clone();
        let mut out = Vec::new();
        for (i, o) in node.options.iter().enumerate() {
            if let Some(w) = &o.when
                && !self.run(w, &self.talk_ctx()).truthy()
            {
                continue;
            }
            let text = self.render_str(&o.text, &self.talk_ctx());
            out.push((i, text));
        }
        out
    }

    fn show_options(&mut self) {
        let vis = self.visible_options();
        if vis.is_empty() {
            // Never trap the player in a choice with nothing to pick.
            self.world.state.choice = None;
            return;
        }
        for (n, (_, text)) in vis.iter().enumerate() {
            self.push(LineKind::Choice, format!("{}. {text}", n + 1));
        }
        // Not yet: shown greyed, with why, and no number to pick.
        if let Some(cur) = self.world.state.choice {
            let node = self.world.story().choice(cur.choice).nodes[cur.node as usize].clone();
            for (i, o) in node.options.iter().enumerate() {
                let (Some(why), Some(_)) = (&o.why_not, &o.when) else { continue };
                if vis.iter().any(|(j, _)| *j == i) {
                    continue;
                }
                let ctx = self.talk_ctx();
                let (text, why) = (self.render_str(&o.text, &ctx), self.render_str(why, &ctx));
                let line = self.msg_str("choice.locked", &Ctx::new().text("text", text).text("why", why));
                self.push(LineKind::Locked, line);
            }
        }
        self.out.options = vis.into_iter().map(|(_, t)| t).collect();
    }

    fn choice_input(&mut self, line: &str) {
        // UNDO, SAVE, RESTORE, RESTART and QUIT still work inside a choice,
        // and WHAT DID SHE SAY?
        let toks: Vec<String> = self.grammar.split_commands(line).concat();
        if let Parsed::Command(cmd) = self.grammar.parse(&self.world, &self.grammar.normalize(toks.clone()))
            && let VerbAction::Builtin(a) = self.world.story().verbs[cmd.verb].action
            && matches!(
                a,
                BuiltinAction::Undo | BuiltinAction::Save | BuiltinAction::Restore | BuiltinAction::Restart | BuiltinAction::Quit | BuiltinAction::LastTalk
            )
        {
            self.last_text = cmd.text.clone();
            self.meta(a);
            // Still choosing: the options again.
            if a == BuiltinAction::LastTalk && self.world.state.choice.is_some() {
                self.show_options();
            }
            return;
        }

        let vis = self.visible_options();
        let reply = line.trim().to_lowercase();
        let by_number = reply.parse::<usize>().ok().filter(|n| (1..=vis.len()).contains(n)).map(|n| n - 1);
        let by_text = || {
            let hits: Vec<usize> = vis
                .iter()
                .enumerate()
                .filter(|(_, (_, t))| !reply.is_empty() && t.to_lowercase().contains(&reply))
                .map(|(i, _)| i)
                .collect();
            (hits.len() == 1).then(|| hits[0])
        };
        let Some(pick) = by_number.or_else(by_text) else {
            self.msg("choice.invalid", &self.talk_ctx());
            self.show_options();
            return;
        };

        let before = self.world.state.clone();
        self.take_option(vis[pick].0, Some(before));
    }

    /// Carries out an option of the open node (by its index there). With
    /// `before`, it's a step of its own for UNDO.
    pub(crate) fn take_option(&mut self, index: usize, before: Option<crate::state::GameState>) {
        let cur = self.world.state.choice.expect("a choice is open");
        let opt = self.world.story().choice(cur.choice).nodes[cur.node as usize].options[index].clone();
        let key = self.world.story().choice(cur.choice).key.clone();
        self.chosen.insert((cur.choice.0, cur.node, index));
        let said = self.render_str(&opt.text, &self.talk_ctx());
        let you = self.msg_str("talk.recall_you", &self.talk_ctx().text("text", said));
        self.world.state.talks.entry(key.clone()).or_default().push(you);
        let shown = self.out.lines.len();
        if let Some(s) = &opt.say {
            self.render_line(LineKind::Text, s, &self.talk_ctx());
        }
        if let Some(r) = &opt.run {
            self.run(r, &self.talk_ctx());
        }
        // How the one talked to feels about that.
        if opt.disposition != 0
            && let Some(who) = self.talker_now(&key)
            && let Some((_, now)) = self.world.shift_disposition(who, opt.disposition)
        {
            let warmer = opt.disposition > 0;
            self.msg_kind(LineKind::Notice, if warmer { "disposition.warmer" } else { "disposition.cooler" }, &self.talk_ctx().entity("target", who).text("feeling", now));
        }
        if let Some(b) = before {
            self.world.push_undo(b);
        }

        if self.end_requested.is_some() {
            self.keep_talk(&key, shown);
            self.world.state.choice = None;
            self.settle();
            return;
        }
        // A script may have closed or replaced the choice.
        if self.world.state.choice == Some(cur) {
            match opt.next {
                Next::Goto(n) => {
                    self.world.state.choice = Some(ChoiceCursor { node: n, ..cur });
                    self.enter_node();
                }
                Next::End => self.world.state.choice = None,
                Next::Stay => self.show_options(),
            }
        }
        self.keep_talk(&key, shown);
        self.settle();
    }

    // ----------------------------------------------------------- helpers

    pub(crate) fn the(&self, id: EntityId) -> String {
        (self.grammar.lang().the)(&self.world.display_name(id), id == PLAYER || self.world.is_proper(id))
    }

    pub(crate) fn a(&self, id: EntityId) -> String {
        (self.grammar.lang().a)(&self.world.display_name(id), id == PLAYER || self.world.is_proper(id))
    }

    /// "an iron gate (locked) and an oil can", with state tags when
    /// `settings.describe_state` is on.
    pub(crate) fn list_a(&self, ids: &[EntityId]) -> String {
        let lang = self.grammar.lang();
        let names: Vec<String> = ids
            .iter()
            .map(|id| {
                let tags = self.state_tag_texts(*id);
                (lang.tagged)(&self.a(*id), &tags)
            })
            .collect();
        (self.grammar.lang().list_and)(&names)
    }
}

/// Whether words begin a question: WHERE DID I GET, WHAT DID.
fn is_question(verb: &str) -> bool {
    matches!(verb.split_whitespace().next(), Some("what" | "where" | "who" | "which" | "how" | "why" | "when"))
}

/// Typed words as the start of a question back: "where did i get" → "Where did I get".
fn as_asked(verb: &str) -> String {
    let words: Vec<String> = verb.split_whitespace().map(|w| if w == "i" { "I".to_string() } else { w.to_string() }).collect();
    let s = words.join(" ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// How many actions until a timer goes off: its `after` or `every`, plus,
/// for a range (`5-10`), a random part of its spread.
pub(crate) fn timer_wait(t: &eifgb_ir::Timer, rng: &mut crate::rng::Rng) -> u64 {
    let base = u64::from(t.schedule.actions());
    if t.spread == 0 { base } else { base + rng.range(0, i64::from(t.spread)) as u64 }
}
