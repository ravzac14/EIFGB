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
    Ending,
    /// A script error. Shown so authors can fix it; play carries on.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub room: String,
    pub score: i64,
    pub max_score: Option<i64>,
    pub turns: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Output {
    pub lines: Vec<Line>,
    pub status: Status,
    /// The open choice's options, in display order and numbered from 1.
    /// A web host can show these as buttons.
    pub options: Vec<String>,
    pub ended: Option<EndingKind>,
    /// Set after SAVE: the host should keep this string somewhere.
    pub save: Option<String>,
    /// Set after RESTORE: the host should fetch a save and pass it to [`Game::restore`].
    pub restore_requested: bool,
    pub quit_requested: bool,
}

impl Output {
    /// All lines as plain text, one per line. Handy for tests and logs.
    pub fn text(&self) -> String {
        self.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")
    }
}

/// Built-in actions that need the player to touch the thing, not just see it.
fn needs_reach(a: BuiltinAction) -> bool {
    use BuiltinAction as A;
    matches!(
        a,
        A::Take | A::PutIn | A::PutOn | A::Open | A::Close | A::Lock | A::Unlock | A::Wear | A::TakeOff | A::Eat | A::Enter | A::Empty
    )
}

pub(crate) enum Pending {
    /// We asked "which do you mean?"; the reply fills in the noun.
    Which { before: Vec<String>, after: Vec<String>, options: Vec<EntityId> },
    /// We asked "what do you want to take?"; the reply completes the command.
    Noun { prefix: Vec<String> },
}

pub struct Game<H: ScriptHost> {
    pub(crate) world: World,
    pub(crate) host: H,
    pub(crate) grammar: Grammar,
    seed: u64,
    pub(crate) pending: Option<Pending>,
    last: Option<Vec<String>>,
    /// Set by an action that only asked a question, so the turn doesn't count.
    pub(crate) no_turn: bool,
    pub(crate) out: Output,
    pub(crate) end_requested: Option<String>,
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
            out: Output::default(),
            end_requested: None,
            choice_requested: None,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// The opening: title, intro, `start` handlers and the first room.
    pub fn start(&mut self) -> Output {
        self.out = Output::default();
        self.intro();
        self.finish()
    }

    pub fn input(&mut self, line: &str) -> Output {
        self.out = Output::default();
        if self.world.state.choice.is_some() {
            self.choice_input(line);
        } else {
            let commands = self.grammar.split_commands(line);
            if commands.is_empty() {
                self.msg("parser.empty", &Ctx::new());
            }
            for toks in commands {
                if !self.command(toks) {
                    break;
                }
            }
        }
        self.finish()
    }

    /// Loads a save the host fetched after `restore_requested`.
    pub fn restore(&mut self, json: &str) -> Output {
        self.out = Output::default();
        match self.world.restore(json) {
            Ok(()) => {
                self.pending = None;
                self.msg_kind(LineKind::Notice, "restore.done", &Ctx::new());
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
        let here = self.world.here();
        let mut out = std::mem::take(&mut self.out);
        out.status = Status {
            room: self.world.def(here).name.clone(),
            score: self.world.state.score,
            max_score: self.world.story().meta.max_score,
            turns: self.world.state.turn,
        };
        out.ended = self.world.state.ending.map(|e| self.world.story().endings[e.index()].kind);
        out
    }

    // ------------------------------------------------------------ output

    pub(crate) fn push(&mut self, kind: LineKind, text: impl Into<String>) {
        let text = text.into();
        if !text.trim().is_empty() {
            self.out.lines.push(Line { kind, text });
        }
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
            }
        }
    }

    // ---------------------------------------------------------- commands

    /// Runs one command. Returns false when the rest of a chained input
    /// should be skipped.
    fn command(&mut self, toks: Vec<String>) -> bool {
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
            Some(Pending::Noun { prefix }) => {
                let mut merged = prefix;
                merged.extend(toks.iter().cloned());
                match self.grammar.parse(&self.world, &merged) {
                    p @ Parsed::Command(_) => p,
                    _ => self.grammar.parse(&self.world, &toks),
                }
            }
            None => self.grammar.parse(&self.world, &toks),
        };

        match parsed {
            Parsed::Command(cmd) => self.execute(cmd),
            Parsed::Ambiguous { before, after, options } => {
                let names: Vec<String> = options.iter().map(|id| self.the(*id)).collect();
                let list = (self.grammar.lang().list_or)(&names);
                self.msg("parser.ambiguous", &Ctx::new().text("options", list));
                self.pending = Some(Pending::Which { before, after, options });
                false
            }
            Parsed::NeedNoun { prefix, verb, target, word } => {
                match (target, word) {
                    (Some(t), Some(w)) => self.msg(
                        "parser.need_tool",
                        &Ctx::new().text("verb", verb).entity("target", t).text("word", w),
                    ),
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

    /// Steps 6 and 8 of the fallback chain: first the author's
    /// `parse_failed` hook, then a failure message that says what was
    /// understood. (Step 7, the AI fallback, will slot in between.)
    fn parse_failed(&mut self, toks: &[String], f: Failure) {
        let before = self.out.lines.len();
        let ctx = Ctx::new().text("text", toks.join(" "));
        for code in self.global_handlers(Phase::On, "parse_failed") {
            self.run(&code, &ctx);
        }
        if self.out.lines.len() > before {
            return;
        }
        match f {
            Failure::Empty => self.msg("parser.empty", &ctx),
            Failure::UnknownVerb { word } => {
                let suggestions = (self.grammar.lang().list_or)(&self.grammar.suggestions(&self.world));
                self.msg("parser.unknown_verb", &ctx.text("word", word).text("suggestions", suggestions));
            }
            Failure::Partial { verb } => self.msg("parser.partial", &ctx.text("verb", verb)),
            Failure::UnknownWord { word } => self.msg("parser.unknown_word", &ctx.text("word", word)),
            Failure::NotHere => self.msg("parser.not_here", &ctx),
            Failure::NoPronoun => self.msg("parser.no_pronoun", &ctx),
            Failure::NoAll => self.msg("parser.no_all", &ctx),
            Failure::AllNone => self.msg("parser.all_none", &ctx),
        }
    }

    fn execute(&mut self, cmd: Command) -> bool {
        let verb = self.world.story().verbs[cmd.verb].clone();
        if let VerbAction::Builtin(a) = verb.action
            && a.is_meta()
        {
            return self.meta(a);
        }
        if self.world.state.ending.is_some() {
            self.msg_kind(LineKind::Notice, "ending.options", &Ctx::new());
            return false;
        }

        let before = self.world.state.clone();
        self.perform(&verb, &cmd);
        if std::mem::take(&mut self.no_turn) {
            return false;
        }
        let mentioned: Vec<EntityId> = cmd.targets.iter().copied().chain(cmd.tool).collect();
        if !mentioned.is_empty() {
            self.world.state.mentioned = mentioned;
        }
        self.end_turn();
        self.world.push_undo(before);
        self.last = Some(cmd.words);
        self.settle();
        self.world.state.ending.is_none() && self.world.state.choice.is_none()
    }

    fn meta(&mut self, action: BuiltinAction) -> bool {
        let ctx = Ctx::new();
        match action {
            BuiltinAction::Again => {
                return match self.last.clone() {
                    Some(words) => self.command(words),
                    None => {
                        self.msg("parser.empty", &ctx);
                        false
                    }
                };
            }
            BuiltinAction::Undo => {
                self.pending = None;
                if self.world.undo() {
                    self.msg_kind(LineKind::Notice, "undo.done", &ctx);
                    self.look();
                    if self.world.state.choice.is_some() {
                        self.show_options();
                    }
                } else {
                    self.msg("undo.none", &ctx);
                }
            }
            BuiltinAction::Save => {
                self.out.save = Some(self.world.save());
                self.msg_kind(LineKind::Notice, "save.done", &ctx);
            }
            BuiltinAction::Restore => self.out.restore_requested = true,
            BuiltinAction::Restart => {
                self.world.restart(self.seed);
                self.pending = None;
                self.last = None;
                self.intro();
            }
            BuiltinAction::Score => self.msg("score.report", &ctx),
            BuiltinAction::Help => self.msg("help.text", &ctx),
            BuiltinAction::Quit => self.out.quit_requested = true,
            other => unreachable!("{other:?} is not a meta action"),
        }
        true
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
                let name = self.world.def(t).name.clone();
                let line = &mut self.out.lines[start];
                line.text = format!("{name}: {}", line.text);
            }
            if self.end_requested.is_some() {
                break;
            }
        }
    }

    fn perform_one(&mut self, verb: &Verb, target: Option<EntityId>, cmd: &Command) {
        let here = self.world.here();
        let ctx = self.command_ctx(verb, target, cmd);
        let with_owner = |owner: Option<EntityId>| Ctx { this: owner, ..ctx.clone() };

        // Things behind glass (in a closed transparent container) can be
        // seen but not touched.
        if let VerbAction::Builtin(a) = verb.action
            && needs_reach(a)
        {
            for thing in target.into_iter().chain(cmd.tool) {
                if let Some(holder) = self.world.blocked_reach(thing) {
                    self.msg("reach.closed", &Ctx::new().entity("target", thing).entity("holder", holder));
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
        let quiet = !reveals.is_empty()
            && matches!(verb.action, VerbAction::Script | VerbAction::Builtin(BuiltinAction::Search));

        let before_action = self.out.lines.len();
        let ok = match self.handlers(target, here, Phase::On, &verb.key).into_iter().next() {
            Some((owner, code)) => self.run(&code, &with_owner(owner)) != ScriptValue::Block,
            None if quiet => true,
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

    fn global_handlers(&self, phase: Phase, event: &str) -> Vec<Code> {
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
        if self.end_requested.is_some() {
            return;
        }
        for code in self.global_handlers(Phase::On, "turn") {
            self.run(&code, &Ctx::new());
        }
        let here = self.world.here();
        self.room_event(here, "turn");
    }

    /// Carries out an ending or an opened choice that a script asked for.
    fn settle(&mut self) {
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
        self.enter_node();
    }

    fn enter_node(&mut self) {
        // Nodes without options move on by themselves. The limit stops a
        // loop of such nodes from hanging the game.
        for _ in 0..100 {
            let Some(cur) = self.world.state.choice else { return };
            let node = self.world.story().choice(cur.choice).nodes[cur.node as usize].clone();
            if let Some(t) = &node.text {
                self.render_line(LineKind::Text, t, &Ctx::new());
            }
            if let Some(r) = &node.run {
                self.run(r, &Ctx::new());
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
    fn visible_options(&mut self) -> Vec<(usize, String)> {
        let Some(cur) = self.world.state.choice else { return Vec::new() };
        let node = self.world.story().choice(cur.choice).nodes[cur.node as usize].clone();
        let mut out = Vec::new();
        for (i, o) in node.options.iter().enumerate() {
            if let Some(w) = &o.when
                && !self.run(w, &Ctx::new()).truthy()
            {
                continue;
            }
            let text = self.render_str(&o.text, &Ctx::new());
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
        self.out.options = vis.into_iter().map(|(_, t)| t).collect();
    }

    fn choice_input(&mut self, line: &str) {
        // UNDO, SAVE, RESTORE, RESTART and QUIT still work inside a choice.
        let toks: Vec<String> = self.grammar.split_commands(line).concat();
        if let Parsed::Command(cmd) = self.grammar.parse(&self.world, &self.grammar.normalize(toks.clone()))
            && let VerbAction::Builtin(a) = self.world.story().verbs[cmd.verb].action
            && matches!(
                a,
                BuiltinAction::Undo | BuiltinAction::Save | BuiltinAction::Restore | BuiltinAction::Restart | BuiltinAction::Quit
            )
        {
            self.meta(a);
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
            self.msg("choice.invalid", &Ctx::new());
            self.show_options();
            return;
        };

        let before = self.world.state.clone();
        let cur = self.world.state.choice.expect("a choice is open");
        let opt = self.world.story().choice(cur.choice).nodes[cur.node as usize].options[vis[pick].0].clone();
        if let Some(s) = &opt.say {
            self.render_line(LineKind::Text, s, &Ctx::new());
        }
        if let Some(r) = &opt.run {
            self.run(r, &Ctx::new());
        }
        self.world.push_undo(before);

        if self.end_requested.is_some() {
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
        self.settle();
    }

    // ----------------------------------------------------------- helpers

    pub(crate) fn the(&self, id: EntityId) -> String {
        (self.grammar.lang().the)(&self.world.def(id).name, id == PLAYER)
    }

    pub(crate) fn a(&self, id: EntityId) -> String {
        (self.grammar.lang().a)(&self.world.def(id).name, id == PLAYER)
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
