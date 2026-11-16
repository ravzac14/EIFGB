//! Validates a parsed [`Source`] and lowers it to a [`Story`].
//!
//! Lowering keeps going after an error, so one run reports as many
//! problems as possible. The story is returned only when there are no
//! errors.

use crate::diag::Diagnostics;
use crate::files::{self, FileProvider};
use crate::util::{check_id, did_you_mean, is_identifier, literal_calls, RESERVED};
use crate::{pattern, source as src, template};
use eifgb_ir::*;
use eifgb_stdlib::Stdlib;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Names scripts already use on every entity: built-in fields and methods.
const RESERVED_FIELDS: &[&str] = &["key", "name", "kind", "location", "move_to", "is_in", "contents", "has"];

pub(crate) struct Lowerer<'a> {
    pub d: &'a mut Diagnostics,
    files: &'a dyn FileProvider,
    entry: &'a str,
    entity_ids: HashMap<String, EntityId>,
    entity_kinds: Vec<EntityKind>,
    choice_ids: HashMap<String, ChoiceId>,
    verb_keys: Vec<String>,
    categories: Vec<(String, Vec<String>)>,
}

impl<'a> Lowerer<'a> {
    pub fn new(d: &'a mut Diagnostics, files: &'a dyn FileProvider, entry: &'a str) -> Self {
        Lowerer {
            d,
            files,
            entry,
            entity_ids: HashMap::new(),
            entity_kinds: Vec::new(),
            choice_ids: HashMap::new(),
            verb_keys: Vec::new(),
            categories: Vec::new(),
        }
    }

    pub fn lower(&mut self, s: src::Source) -> Option<Story> {
        if !src::SUPPORTED_FORMATS.contains(&s.format.as_str()) {
            self.d.error(
                "format",
                format!(
                    "unsupported format '{}'; this compiler supports {}",
                    s.format,
                    src::SUPPORTED_FORMATS.join(", ")
                ),
            );
            return None;
        }

        let stdlib_version = s.game.stdlib.unwrap_or(eifgb_stdlib::LATEST);
        let stdlib = match eifgb_stdlib::load(stdlib_version, &s.game.language) {
            Ok(lib) => lib,
            Err(e) => {
                let path = if eifgb_stdlib::languages(stdlib_version).is_empty() {
                    "game.stdlib"
                } else {
                    "game.language"
                };
                self.d.error(path, e.to_string());
                return None;
            }
        };

        self.allocate_ids(&s);
        self.categories = stdlib.categories.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let verbs = self.lower_verbs(&s.verbs, &stdlib);

        let messages = self.lower_messages(&s.messages, &stdlib);
        let player_name = messages
            .get("player.name")
            .and_then(Text::as_literal)
            .unwrap_or_else(|| "yourself".into());

        // Entities, in the same order as `allocate_ids`.
        let start = self.room_ref(&s.game.start, "game.start");
        let mut entities = vec![Entity {
            key: PLAYER_KEY.into(),
            kind: EntityKind::Player,
            name: player_name,
            vocab: Vocab {
                nouns: stdlib.lexicon.self_words.clone(),
                adjectives: vec![],
            },
            description: self.text_opt(s.player.description.as_deref(), "player.description"),
            traits: vec![],
            state: BTreeMap::new(),
            location: start,
            exits: vec![],
            unlocks_with: None,
            talk: None,
            capacity: s.player.capacity,
            size: s.player.size.unwrap_or(1),
            revealed_by: vec![],
            handlers: vec![],
        }];
        for (key, room) in &s.rooms {
            entities.push(self.lower_room(key, room));
        }
        for (key, thing) in &s.objects {
            entities.push(self.lower_thing("objects", key, thing, EntityKind::Object));
        }
        for (key, thing) in &s.characters {
            entities.push(self.lower_thing("characters", key, thing, EntityKind::Character));
        }
        self.link_doors(&mut entities);
        self.check_world(&entities);

        let handlers = self.lower_handlers("on", &s.on, GLOBAL_EVENTS);
        let choices = self.lower_choices(&s.choices);
        let endings = self.lower_endings(&s.endings);
        let scripts = self.load_scripts(&s.scripts);

        let intro = self.text_opt(s.game.intro.as_deref(), "game.intro");
        let story = Story {
            ir_version: IR_VERSION,
            meta: Meta {
                id: s.game.id.clone(),
                title: s.game.title.clone(),
                author: s.game.author.clone(),
                version: s.game.version.clone(),
                language: s.game.language.clone(),
                stdlib: stdlib_version,
                start: start.unwrap_or(PLAYER),
                max_score: s.game.max_score,
                ai_fallback: s.game.ai_fallback,
                settings: Settings { describe_state: s.settings.describe_state },
            },
            intro,
            vars: s.vars.into_iter().collect(),
            entities,
            verbs,
            lexicon: stdlib.lexicon.clone(),
            messages,
            choices,
            handlers,
            endings,
            scripts,
            categories: stdlib.categories.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        };

        if let Err(e) = check_id(&story.meta.id) {
            self.d.error("game.id", e);
        }
        self.check_script_references(&story);

        if self.d.has_errors() { None } else { Some(story) }
    }

    // ---------------------------------------------------------------- ids

    fn allocate_ids(&mut self, s: &src::Source) {
        self.entity_ids.insert(PLAYER_KEY.into(), PLAYER);
        self.entity_kinds.push(EntityKind::Player);
        let sections: [(&str, Vec<&String>, EntityKind); 3] = [
            ("rooms", s.rooms.keys().collect(), EntityKind::Room),
            ("objects", s.objects.keys().collect(), EntityKind::Object),
            ("characters", s.characters.keys().collect(), EntityKind::Character),
        ];
        let mut first_seen: HashMap<String, &str> = HashMap::new();
        for (section, keys, kind) in sections {
            for key in keys {
                let path = format!("{section}.{key}");
                if let Err(e) = check_id(key) {
                    self.d.error(&path, e);
                }
                if RESERVED.contains(&key.as_str()) {
                    self.d.error(&path, format!("'{key}' is a reserved name"));
                }
                if let Some(prev) = first_seen.get(key.as_str()) {
                    self.d.error(&path, format!("ID '{key}' is already used in {prev}"));
                }
                first_seen.insert(key.clone(), section);
                let id = EntityId(self.entity_kinds.len() as u32);
                self.entity_ids.entry(key.clone()).or_insert(id);
                self.entity_kinds.push(kind);
            }
        }
        for (i, key) in s.choices.keys().enumerate() {
            if let Err(e) = check_id(key) {
                self.d.error(format!("choices.{key}"), e);
            }
            self.choice_ids.insert(key.clone(), ChoiceId(i as u32));
        }
    }

    fn entity_ref(&mut self, key: &str, path: &str) -> Option<EntityId> {
        match self.entity_ids.get(key) {
            Some(id) => Some(*id),
            None => {
                let hint = did_you_mean(key, self.entity_ids.keys().map(String::as_str));
                self.d.error(path, format!("unknown ID '{key}'{hint}"));
                None
            }
        }
    }

    fn kind(&self, id: EntityId) -> EntityKind {
        self.entity_kinds[id.index()]
    }

    fn room_ref(&mut self, key: &str, path: &str) -> Option<EntityId> {
        let id = self.entity_ref(key, path)?;
        if self.kind(id) != EntityKind::Room {
            self.d.error(path, format!("'{key}' is not a room"));
            return None;
        }
        Some(id)
    }

    fn choice_ref(&mut self, key: &str, path: &str) -> Option<ChoiceId> {
        match self.choice_ids.get(key) {
            Some(id) => Some(*id),
            None => {
                let hint = did_you_mean(key, self.choice_ids.keys().map(String::as_str));
                self.d.error(path, format!("unknown choice '{key}'{hint}"));
                None
            }
        }
    }

    // --------------------------------------------------------- text & code

    fn text(&mut self, s: &str, path: &str) -> Text {
        // A trailing newline from a YAML block scalar is never meaningful.
        match template::parse(s.trim_end(), path) {
            Ok(t) => t,
            Err(e) => {
                self.d.error(path, e);
                Text::default()
            }
        }
    }

    fn text_opt(&mut self, s: Option<&str>, path: &str) -> Option<Text> {
        s.map(|s| self.text(s, path))
    }

    fn code(&mut self, s: &str, path: &str) -> Code {
        let src = s.trim().to_string();
        if src.is_empty() {
            self.d.error(path, "script is empty");
        }
        Code { src, origin: path.to_string() }
    }

    // -------------------------------------------------------------- verbs

    fn lower_verbs(
        &mut self,
        author: &indexmap::IndexMap<String, src::Verb>,
        stdlib: &Stdlib,
    ) -> Vec<Verb> {
        let mut verbs = Vec::new();
        for (key, v) in author {
            let path = format!("verbs.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            if key.starts_with("before_") || key.starts_with("after_") {
                self.d.error(&path, "verb IDs can't start with 'before_' or 'after_'");
            }
            if GLOBAL_EVENTS.contains(&key.as_str()) || ROOM_EVENTS.contains(&key.as_str()) {
                self.d.error(
                    &path,
                    format!("'{key}' is the name of an event, so a verb can't use it; pick another ID (the patterns can still say \"{key} ...\")"),
                );
            }
            if v.patterns.is_empty() {
                self.d.error(&path, "a verb needs at least one pattern");
            }
            // An author verb with a built-in stdlib verb's name replaces its
            // grammar but keeps its behaviour. One with a hookable stdlib
            // verb's name (push, kick...) adds to it: there's nothing to
            // override, so there's nothing to warn about.
            let std = stdlib.verbs.get(key);
            let action = match std.and_then(|s| s.action) {
                Some(a) => {
                    self.d.warn(
                        &path,
                        format!("replaces the patterns of standard verb '{key}'; its built-in behaviour is kept"),
                    );
                    VerbAction::Builtin(a)
                }
                None => VerbAction::Script,
            };
            let mut patterns = self.patterns(&v.patterns, &format!("{path}.patterns"));
            let mut default = self.text_opt(v.default.as_deref(), &format!("{path}.default"));
            if let Some(std) = std
                && std.action.is_none()
            {
                for p in self.patterns(&std.patterns, &format!("stdlib.verbs.{key}")) {
                    if !patterns.contains(&p) {
                        patterns.push(p);
                    }
                }
                if default.is_none() {
                    default = self.text_opt(std.default.as_deref(), &format!("stdlib.verbs.{key}.default"));
                }
            }
            verbs.push(Verb { key: key.clone(), patterns, action, default, origin: VerbOrigin::Author });
        }

        // Later stdlib patterns that an author pattern shadows are still
        // kept, because author verbs are matched first. The warning lets
        // the author see what they've hidden.
        let author_patterns: HashMap<Pattern, String> = verbs
            .iter()
            .flat_map(|v| v.patterns.iter().map(|p| (p.clone(), v.key.clone())))
            .collect();

        for (key, std) in &stdlib.verbs {
            if author.contains_key(key) {
                continue;
            }
            let path = format!("stdlib.verbs.{key}");
            let patterns = self.patterns(&std.patterns, &path);
            for p in &patterns {
                if let Some(owner) = author_patterns.get(p) {
                    self.d.warn(
                        format!("verbs.{owner}"),
                        format!(
                            "pattern '{}' shadows standard verb '{key}'",
                            pattern::display(p)
                        ),
                    );
                }
            }
            let action = std.action.map_or(VerbAction::Script, VerbAction::Builtin);
            let default = self.text_opt(std.default.as_deref(), &format!("stdlib.verbs.{key}.default"));
            verbs.push(Verb { key: key.clone(), patterns, action, default, origin: VerbOrigin::Stdlib });
        }
        self.verb_keys = verbs.iter().map(|v| v.key.clone()).collect();
        verbs
    }

    fn patterns(&mut self, raw: &[String], path: &str) -> Vec<Pattern> {
        raw.iter()
            .enumerate()
            .filter_map(|(i, p)| match pattern::parse(p) {
                Ok(p) => Some(p),
                Err(e) => {
                    self.d.error(format!("{path}[{i}]"), e);
                    None
                }
            })
            .collect()
    }

    fn lower_messages(
        &mut self,
        author: &indexmap::IndexMap<String, String>,
        stdlib: &Stdlib,
    ) -> BTreeMap<String, Text> {
        let mut out = BTreeMap::new();
        for (key, m) in &stdlib.messages {
            out.insert(key.clone(), self.text(m, &format!("stdlib.messages.{key}")));
        }
        for (key, m) in author {
            let path = format!("messages.{key}");
            if !stdlib.messages.contains_key(key) {
                let hint = did_you_mean(key, stdlib.messages.keys().map(String::as_str));
                self.d.warn(&path, format!("'{key}' is not a standard message, so nothing will use it{hint}"));
            }
            out.insert(key.clone(), self.text(m, &path));
        }
        out
    }

    // ------------------------------------------------------------ handlers

    fn lower_handlers(
        &mut self,
        path: &str,
        raw: &indexmap::IndexMap<String, String>,
        events: &[&str],
    ) -> Vec<Handler> {
        let mut out = Vec::new();
        for (key, body) in raw {
            let hpath = format!("{path}.{key}");
            let (phase, event) = if let Some(v) = key.strip_prefix("before_") {
                (Phase::Before, v)
            } else if let Some(v) = key.strip_prefix("after_") {
                (Phase::After, v)
            } else {
                (Phase::On, key.as_str())
            };
            let is_verb = self.verb_keys.iter().any(|v| v == event);
            let is_event = events.contains(&event);
            if !is_verb && !is_event {
                let candidates: Vec<String> = self
                    .verb_keys
                    .iter()
                    .cloned()
                    .chain(events.iter().map(|e| e.to_string()))
                    .collect();
                let hint = did_you_mean(event, candidates.iter().map(String::as_str));
                self.d.error(&hpath, format!("'{event}' is not a verb or an event here{hint}"));
                continue;
            }
            if is_event && !is_verb && phase != Phase::On {
                self.d.error(&hpath, format!("event '{event}' has no before_/after_ form"));
                continue;
            }
            let code = self.code(body, &hpath);
            out.push(Handler { phase, event: event.to_string(), code });
        }
        out
    }

    // --------------------------------------------------------------- rooms

    fn lower_room(&mut self, key: &str, r: &src::Room) -> Entity {
        let path = format!("rooms.{key}");
        let mut exits = Vec::new();
        for (dir, exit) in &r.exits {
            let epath = format!("{path}.exits.{dir}");
            let Some(direction) = Direction::from_key(dir) else {
                let hint = did_you_mean(dir, Direction::ALL.iter().map(|d| d.key()));
                self.d.error(&epath, format!("unknown direction '{dir}'{hint}"));
                continue;
            };
            let entries: Vec<(String, src::OneExit)> = match exit {
                src::Exit::Many(list) => {
                    if list.is_empty() {
                        self.d.error(&epath, "an exit list needs at least one entry");
                    }
                    list.iter().enumerate().map(|(i, e)| (format!("{epath}[{i}]"), e.clone())).collect()
                }
                src::Exit::Short(s) => vec![(epath.clone(), src::OneExit::Short(s.clone()))],
                src::Exit::Full(f) => vec![(epath.clone(), src::OneExit::Full(f.clone()))],
            };
            let first = exits.len();
            for (epath, entry) in &entries {
                self.lower_exit(direction, entry, epath, &mut exits);
            }
            self.check_same_direction(&exits[first..], &epath);
        }
        let handlers = self.lower_handlers(&format!("{path}.on"), &r.on, ROOM_EVENTS);
        Entity {
            key: key.into(),
            kind: EntityKind::Room,
            name: r.name.clone(),
            vocab: Vocab::default(),
            description: self.text_opt(r.description.as_deref(), &format!("{path}.description")),
            traits: vec![],
            state: BTreeMap::new(),
            location: None,
            exits,
            unlocks_with: None,
            talk: None,
            capacity: None,
            size: 1,
            revealed_by: vec![],
            handlers,
        }
    }

    /// Several exits the same way can only be told apart by their
    /// conditions or their doors.
    fn check_same_direction(&mut self, group: &[Exit], path: &str) {
        let routes: Vec<&Exit> = group.iter().filter(|e| e.to.is_some()).collect();
        let unconditional: Vec<&&Exit> = routes.iter().filter(|e| e.when.is_none()).collect();
        if unconditional.len() > 1 {
            let mut doors: Vec<EntityId> = unconditional.iter().filter_map(|e| e.through).collect();
            doors.sort();
            doors.dedup();
            if doors.len() != unconditional.len() {
                self.d.warn(
                    path,
                    "several exits lead this way with no 'when' to choose between them; give each its own door with 'through', or add 'when' conditions",
                );
            }
        }
        if group.iter().filter(|e| e.to.is_none()).count() > 1 {
            self.d.warn(path, "only the first 'blocked' entry for a direction is ever shown");
        }
    }

    fn lower_exit(&mut self, direction: Direction, exit: &src::OneExit, epath: &str, exits: &mut Vec<Exit>) {
        let epath = epath.to_string();
        {
            match exit {
                src::OneExit::Short(s) if is_identifier(s.trim()) => {
                    if let Some(to) = self.room_ref(s.trim(), &epath) {
                        exits.push(Exit { direction, to: Some(to), through: None, when: None, blocked: None });
                    }
                }
                src::OneExit::Short(msg) => {
                    let blocked = Some(self.text(msg, &epath));
                    exits.push(Exit { direction, to: None, through: None, when: None, blocked });
                }
                src::OneExit::Full(f) => {
                    let to = f.to.as_deref().and_then(|t| self.room_ref(t, &format!("{epath}.to")));
                    let through = f
                        .through
                        .as_deref()
                        .and_then(|t| self.entity_ref(t, &format!("{epath}.through")));
                    let when = f.when.as_deref().map(|w| self.code(w, &format!("{epath}.when")));
                    let blocked = self.text_opt(f.blocked.as_deref(), &format!("{epath}.blocked"));
                    if f.to.is_none() && blocked.is_none() {
                        self.d.error(&epath, "an exit needs 'to' or 'blocked'");
                    }
                    if f.to.is_none() && (f.through.is_some() || f.when.is_some()) {
                        self.d.warn(&epath, "'through' and 'when' do nothing without 'to'");
                    }
                    exits.push(Exit { direction, to, through, when, blocked });
                }
            }
        }
    }

    // ------------------------------------------------- objects & characters

    fn lower_thing(&mut self, section: &str, key: &str, t: &src::Thing, kind: EntityKind) -> Entity {
        let path = format!("{section}.{key}");
        let mut state: BTreeMap<String, Value> = t.state.clone().into_iter().collect();

        let mut traits = Vec::new();
        for (i, name) in t.traits.iter().enumerate() {
            // `open` is shorthand for the state `open: true`.
            if name == "open" {
                state.insert("open".into(), Value::Bool(true));
                continue;
            }
            match Trait::from_key(name) {
                Some(tr) if traits.contains(&tr) => {
                    self.d.warn(format!("{path}.traits[{i}]"), format!("trait '{name}' is listed twice"));
                }
                Some(tr) => traits.push(tr),
                None => {
                    let candidates = Trait::ALL.iter().map(|t| t.key()).chain(["open"]);
                    let hint = did_you_mean(name, candidates);
                    self.d.error(format!("{path}.traits[{i}]"), format!("unknown trait '{name}'{hint}"));
                }
            }
        }

        for field in state.keys() {
            if RESERVED_FIELDS.contains(&field.as_str()) {
                self.d.error(
                    format!("{path}.state.{field}"),
                    format!("'{field}' is a built-in field or method name and can't be used as state"),
                );
            }
        }

        // Built-in state fields must be booleans and get sensible defaults.
        let is_door = t.traits.iter().any(|n| n == "door");
        for field in ["open", "locked", "hidden", "worn"] {
            // On doors, `hidden` may also name the sides it's hidden from.
            if field == "hidden" && is_door {
                continue;
            }
            if let Some(v) = state.get(field)
                && v.as_bool().is_none()
            {
                self.d.error(
                    format!("{path}.state.{field}"),
                    format!("'{field}' must be true or false, not a {}", v.type_name()),
                );
            }
        }
        let has = |tr| traits.contains(&tr);
        if has(Trait::Openable) {
            state.entry("open".into()).or_insert(Value::Bool(false));
        }
        if has(Trait::Lockable) {
            state.entry("locked".into()).or_insert(Value::Bool(false));
            if !has(Trait::Openable) {
                self.d.warn(&path, "'lockable' without 'openable' can be locked but never opened");
            }
        }
        if state.get("open") == Some(&Value::Bool(true)) && state.get("locked") == Some(&Value::Bool(true)) {
            self.d.error(format!("{path}.state"), "can't start both open and locked");
        }
        let blocked = match state.get("blocked") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Str(s)) => !s.is_empty(),
            Some(Value::List(l)) => !l.is_empty(),
            _ => false,
        };
        if has(Trait::Door) && blocked && state.get("open") == Some(&Value::Bool(true)) {
            self.d.error(format!("{path}.state"), "a blocked door is held shut, so it can't start open");
        }
        if has(Trait::Enterable) && !has(Trait::Container) && !has(Trait::Supporter) {
            self.d.error(format!("{path}.traits"), "'enterable' needs 'container' (to get in) or 'supporter' (to get on)");
        }
        if has(Trait::Transparent) && !has(Trait::Container) {
            self.d.warn(format!("{path}.traits"), "'transparent' only matters on a 'container'");
        }
        if has(Trait::Portable) && has(Trait::Scenery) {
            self.d.error(format!("{path}.traits"), "'portable' and 'scenery' contradict each other");
        }

        let location = t.location.as_deref().and_then(|l| self.entity_ref(l, &format!("{path}.in")));

        let unlocks_with = t.unlocks_with.as_deref().and_then(|k| {
            let id = self.entity_ref(k, &format!("{path}.unlocks_with"))?;
            if !has(Trait::Lockable) {
                self.d.warn(format!("{path}.unlocks_with"), "has no effect without the 'lockable' trait");
            }
            Some(id)
        });
        // Only when no key was named at all: a misspelled key already has its own error.
        if has(Trait::Lockable) && t.unlocks_with.is_none() && state.get("locked") == Some(&Value::Bool(true)) {
            self.d.warn(&path, "starts locked but has no 'unlocks_with', so only a script can unlock it");
        }

        let talk = match (&t.talk, kind) {
            (Some(c), EntityKind::Character) => self.choice_ref(c, &format!("{path}.talk")),
            (Some(_), _) => {
                self.d.error(format!("{path}.talk"), "only characters can have 'talk'; move this to 'characters'");
                None
            }
            (None, _) => None,
        };

        let mut revealed_by = Vec::new();
        if let Some(rules) = &t.revealed_by {
            let rpath = format!("{path}.revealed_by");
            for rule in rules.items() {
                let shape = || format!("'{rule}': write it as '<verb> <object>' or '(<category>) <object>', e.g. 'search mat' or '(physical action) rug'");
                // "(physical action) rug" names a category; "search mat" a verb.
                let (verbs, category, rest) = if let Some(inner) = rule.trim().strip_prefix('(') {
                    let Some((name, rest)) = inner.split_once(')') else {
                        self.d.error(&rpath, shape());
                        continue;
                    };
                    let name = name.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("_");
                    match self.categories.iter().find(|(k, _)| *k == name) {
                        Some((_, verbs)) => (verbs.clone(), Some(name), rest),
                        None => {
                            let names: Vec<String> = self.categories.iter().map(|(k, _)| k.replace('_', " ")).collect();
                            let hint = did_you_mean(&name.replace('_', " "), names.iter().map(String::as_str));
                            self.d.error(
                                &rpath,
                                format!("'({})' is not an action category{hint}; the categories are ({})", name.replace('_', " "), names.join("), (")),
                            );
                            continue;
                        }
                    }
                } else {
                    let Some((verb, rest)) = rule.trim().split_once(char::is_whitespace) else {
                        self.d.error(&rpath, shape());
                        continue;
                    };
                    if !self.verb_keys.iter().any(|v| v == verb) {
                        let hint = did_you_mean(verb, self.verb_keys.iter().map(String::as_str));
                        self.d.error(&rpath, format!("'{verb}' is not a verb{hint}"));
                        continue;
                    }
                    (vec![verb.to_string()], None, rest)
                };
                let words: Vec<&str> = rest.split_whitespace().collect();
                let [target] = words[..] else {
                    self.d.error(&rpath, shape());
                    continue;
                };
                let Some(target_id) = self.entity_ref(target, &rpath) else { continue };
                if target == key {
                    self.d.error(&rpath, "a hidden thing can't be revealed by acting on itself, since it can't be seen");
                    continue;
                }
                revealed_by.push(Reveal { verbs, category, target: target_id });
            }
            let starts_hidden = match state.get("hidden") {
                Some(Value::Bool(b)) => *b,
                Some(Value::Str(s)) => !s.is_empty(),
                Some(Value::List(l)) => !l.is_empty(),
                _ => false,
            };
            if !starts_hidden {
                self.d.warn(&rpath, "this has 'revealed_by' but doesn't start hidden; add 'state: { hidden: true }'");
            }
        }

        let handlers = self.lower_handlers(&format!("{path}.on"), &t.on, &[]);
        Entity {
            key: key.into(),
            kind,
            name: t.name.clone(),
            vocab: vocab(&t.name, &t.synonyms, &t.adjectives),
            description: self.text_opt(t.description.as_deref(), &format!("{path}.description")),
            traits,
            state,
            location,
            exits: vec![],
            unlocks_with,
            talk,
            capacity: t.capacity,
            size: t.size.unwrap_or(1),
            revealed_by,
            handlers,
        }
    }

    /// Makes doors work from both sides. For every exit from room A to
    /// room B through a door, B's way back to A uses the same door:
    /// * an exit back to A through the door already exists: nothing to do;
    /// * exactly one plain exit back to A exists: the door is attached to it;
    /// * there's no way back: one is added in the opposite direction;
    /// * that direction already leads elsewhere: a warning, since guessing
    ///   would change the author's map.
    fn link_doors(&mut self, entities: &mut [Entity]) {
        let mut links = Vec::new();
        for (a, room) in entities.iter().enumerate() {
            for e in &room.exits {
                if let (Some(b), Some(door)) = (e.to, e.through) {
                    links.push((EntityId(a as u32), e.direction, b, door));
                }
            }
        }
        for (a, dir, b, door) in links {
            if a == b {
                continue;
            }
            let (a_key, door_key) = (entities[a.index()].key.clone(), entities[door.index()].key.clone());
            let b_key = entities[b.index()].key.clone();
            let back = &mut entities[b.index()].exits;
            if back.iter().any(|e| e.to == Some(a) && e.through == Some(door)) {
                continue;
            }
            let plain: Vec<usize> =
                back.iter().enumerate().filter(|(_, e)| e.to == Some(a) && e.through.is_none()).map(|(i, _)| i).collect();
            match plain.len() {
                1 => {
                    back[plain[0]].through = Some(door);
                    continue;
                }
                n if n > 1 => {
                    self.d.warn(
                        format!("rooms.{b_key}.exits"),
                        format!("several exits lead back to '{a_key}', so '{door_key}' only works one way; add 'through: {door_key}' to the right one"),
                    );
                    continue;
                }
                _ => {}
            }
            let opposite = dir.opposite();
            if let Some(other) = back.iter().find(|e| e.direction == opposite) {
                let where_to = other.to.map_or("is blocked".to_string(), |t| format!("leads to '{}'", entities[t.index()].key));
                self.d.warn(
                    format!("rooms.{b_key}.exits.{}", opposite.key()),
                    format!("'{door_key}' only works one way: this exit already {where_to}; add an exit back to '{a_key}' through '{door_key}'"),
                );
                continue;
            }
            back.push(Exit { direction: opposite, to: Some(a), through: Some(door), when: None, blocked: None });
        }
    }

    /// A door's `blocked` and `hidden` states are true/false, a room ID,
    /// or a list of room IDs: the sides they apply to.
    fn check_sides(&mut self, door: &Entity, entities: &[Entity], field: &str) {
        let Some(v) = door.state.get(field) else { return };
        let path = format!("objects.{}.state.{field}", door.key);
        let rooms: Vec<&Value> = match v {
            Value::Bool(_) => return,
            Value::Str(_) => vec![v],
            Value::List(items) => items.iter().collect(),
            other => {
                self.d.error(&path, format!("must be true, false, a room ID or a list of room IDs, not a {}", other.type_name()));
                return;
            }
        };
        for r in rooms {
            let Value::Str(key) = r else {
                self.d.error(&path, "a list of sides must contain room IDs");
                continue;
            };
            let uses_door = entities.iter().any(|e| {
                e.kind == EntityKind::Room && &e.key == key && e.exits.iter().any(|x| x.through.is_some_and(|d| entities[d.index()].key == door.key))
            });
            if !uses_door {
                let sides: Vec<&str> = entities
                    .iter()
                    .filter(|e| e.kind == EntityKind::Room && e.exits.iter().any(|x| x.through.is_some_and(|d| entities[d.index()].key == door.key)))
                    .map(|e| e.key.as_str())
                    .collect();
                let hint = did_you_mean(key, sides.iter().copied());
                self.d.error(&path, format!("'{key}' isn't a room on either side of this door{hint}"));
            }
        }
    }

    /// Checks that need every entity: containment and doors.
    fn check_world(&mut self, entities: &[Entity]) {
        let path_of = |e: &Entity| match e.kind {
            EntityKind::Room => format!("rooms.{}", e.key),
            EntityKind::Object => format!("objects.{}", e.key),
            EntityKind::Character => format!("characters.{}", e.key),
            EntityKind::Player => "player".to_string(),
        };

        for e in entities.iter().filter(|e| e.kind != EntityKind::Player) {
            let Some(loc) = e.location else { continue };
            let holder = &entities[loc.index()];
            let path = format!("{}.in", path_of(e));
            if holder.key == e.key {
                self.d.error(&path, "an object can't be inside itself");
                continue;
            }
            if holder.kind == EntityKind::Object
                && !holder.has(Trait::Container)
                && !holder.has(Trait::Supporter)
            {
                self.d.error(
                    &path,
                    format!("'{}' isn't a container or supporter, so nothing can be in it", holder.key),
                );
            }
        }

        // Things that start out overfilling their holder.
        for holder in entities {
            let Some(cap) = holder.capacity else { continue };
            let load: u32 = entities.iter().filter(|e| e.location.is_some_and(|l| entities[l.index()].key == holder.key)).map(|e| e.size).sum();
            if load > cap {
                self.d.warn(
                    format!("{}.capacity", path_of(holder)),
                    format!("starts holding things of total size {load}, more than its capacity of {cap}"),
                );
            }
        }

        // Containment cycles (a in b, b in a).
        for start in 0..entities.len() {
            let mut seen = HashSet::new();
            let mut cur = Some(EntityId(start as u32));
            while let Some(id) = cur {
                if !seen.insert(id) {
                    if id.index() == start {
                        let e = &entities[start];
                        self.d.error(format!("{}.in", path_of(e)), "objects contain each other in a loop");
                    }
                    break;
                }
                cur = entities[id.index()].location;
            }
        }

        // Doors used by exits.
        let mut checked = HashSet::new();
        for room in entities.iter().filter(|e| e.kind == EntityKind::Room) {
            for exit in &room.exits {
                let Some(door_id) = exit.through else { continue };
                let door = &entities[door_id.index()];
                let path = format!("rooms.{}.exits.{}.through", room.key, exit.direction.key());
                if !door.has(Trait::Door) {
                    self.d.error(&path, format!("'{}' needs the 'door' trait", door.key));
                }
                if checked.insert(door_id) {
                    self.check_sides(door, entities, "blocked");
                    self.check_sides(door, entities, "hidden");
                }
            }
        }
    }

    // ------------------------------------------------------------- choices

    fn lower_choices(&mut self, raw: &indexmap::IndexMap<String, src::Choice>) -> Vec<ChoiceGraph> {
        let mut out = Vec::new();
        for (key, c) in raw {
            let path = format!("choices.{key}");
            let node_ids: HashMap<&str, u32> =
                c.nodes.keys().enumerate().map(|(i, k)| (k.as_str(), i as u32)).collect();
            let node_ref = |this: &mut Self, k: &str, p: &str| -> Option<u32> {
                let r = node_ids.get(k).copied();
                if r.is_none() {
                    let hint = did_you_mean(k, node_ids.keys().copied());
                    this.d.error(p, format!("unknown node '{k}'{hint}"));
                }
                r
            };
            if c.nodes.is_empty() {
                self.d.error(&path, "a choice needs at least one node");
            }
            let start = node_ref(self, &c.start, &format!("{path}.start")).unwrap_or(0);

            let mut nodes = Vec::new();
            for (nkey, n) in &c.nodes {
                let npath = format!("{path}.nodes.{nkey}");
                let text = self.text_opt(n.text.as_deref(), &format!("{npath}.text"));
                let run = n.run.as_deref().map(|r| self.code(r, &format!("{npath}.do")));
                let mut options = Vec::new();
                for (i, o) in n.options.iter().enumerate() {
                    let opath = format!("{npath}.options[{i}]");
                    let next = match (&o.goto, o.end) {
                        (Some(_), true) => {
                            self.d.error(&opath, "use either 'goto' or 'end', not both");
                            Next::End
                        }
                        (Some(g), false) => node_ref(self, g, &format!("{opath}.goto")).map_or(Next::End, Next::Goto),
                        (None, true) => Next::End,
                        (None, false) => Next::Stay,
                    };
                    options.push(ChoiceOption {
                        text: self.text(&o.text, &format!("{opath}.text")),
                        when: o.when.as_deref().map(|w| self.code(w, &format!("{opath}.when"))),
                        say: self.text_opt(o.say.as_deref(), &format!("{opath}.say")),
                        run: o.run.as_deref().map(|r| self.code(r, &format!("{opath}.do"))),
                        next,
                    });
                }
                let next = if options.is_empty() {
                    match (&n.goto, n.end) {
                        (Some(_), true) => {
                            self.d.error(&npath, "use either 'goto' or 'end', not both");
                            Some(Next::End)
                        }
                        (Some(g), false) => Some(node_ref(self, g, &format!("{npath}.goto")).map_or(Next::End, Next::Goto)),
                        (None, _) => Some(Next::End),
                    }
                } else {
                    if n.goto.is_some() || n.end {
                        self.d.warn(&npath, "a node with options ignores its own 'goto'/'end'; put them on options");
                    }
                    None
                };
                nodes.push(ChoiceNode { key: nkey.clone(), text, run, options, next });
            }
            out.push(ChoiceGraph { key: key.clone(), start, nodes });
        }
        out
    }

    // ------------------------------------------------------------- endings

    fn lower_endings(&mut self, raw: &indexmap::IndexMap<String, src::Ending>) -> Vec<Ending> {
        let mut out = Vec::new();
        for (key, e) in raw {
            let path = format!("endings.{key}");
            if let Err(err) = check_id(key) {
                self.d.error(&path, err);
            }
            let kind = match e.kind.as_str() {
                "win" => EndingKind::Win,
                "loss" => EndingKind::Loss,
                "neutral" => EndingKind::Neutral,
                other => {
                    self.d.error(format!("{path}.kind"), format!("unknown ending kind '{other}'; use win, loss or neutral"));
                    EndingKind::Neutral
                }
            };
            out.push(Ending { key: key.clone(), kind, text: self.text(&e.text, &format!("{path}.text")) });
        }
        out
    }

    // ------------------------------------------------------------- scripts

    fn load_scripts(&mut self, paths: &[String]) -> Vec<Script> {
        let mut out = Vec::new();
        for (i, p) in paths.iter().enumerate() {
            let path = format!("scripts[{i}]");
            let full = match files::normalize(p) {
                Ok(_) => files::sibling(self.entry, p),
                Err(e) => {
                    self.d.error(&path, e);
                    continue;
                }
            };
            match self.files.read(&full) {
                Ok(src) => out.push(Script { name: p.clone(), src }),
                Err(e) => self.d.error(&path, e),
            }
        }
        out
    }

    /// Checks `end_game("x")` and `open_choice("x")` calls whose
    /// argument is a string literal.
    fn check_script_references(&mut self, story: &Story) {
        let endings: Vec<&str> = story.endings.iter().map(|e| e.key.as_str()).collect();
        let choices: Vec<&str> = story.choices.iter().map(|c| c.key.as_str()).collect();
        let mut sources: Vec<(String, String)> = Vec::new();
        visit_code(story, &mut |c| sources.push((c.origin.clone(), c.src.clone())));
        for s in &story.scripts {
            sources.push((format!("scripts:{}", s.name), s.src.clone()));
        }
        for (origin, src) in &sources {
            for name in literal_calls(src, "end_game") {
                if !endings.contains(&name) {
                    let hint = did_you_mean(name, endings.iter().copied());
                    self.d.error(origin, format!("end_game(\"{name}\"): no such ending{hint}"));
                }
            }
            for name in literal_calls(src, "open_choice") {
                if !choices.contains(&name) {
                    let hint = did_you_mean(name, choices.iter().copied());
                    self.d.error(origin, format!("open_choice(\"{name}\"): no such choice{hint}"));
                }
            }
        }
    }
}

/// Builds parser vocabulary. The last word of the name and of each
/// synonym is a noun, and the words before it are adjectives.
fn vocab(name: &str, synonyms: &[String], adjectives: &[String]) -> Vocab {
    let mut v = Vocab::default();
    let push = |list: &mut Vec<String>, w: &str| {
        let w = w.to_lowercase();
        if !list.contains(&w) {
            list.push(w);
        }
    };
    for phrase in std::iter::once(name).chain(synonyms.iter().map(String::as_str)) {
        let words: Vec<&str> = phrase.split_whitespace().collect();
        if let Some((noun, adjs)) = words.split_last() {
            push(&mut v.nouns, noun);
            for a in adjs {
                push(&mut v.adjectives, a);
            }
        }
    }
    for a in adjectives.iter().flat_map(|a| a.split_whitespace()) {
        push(&mut v.adjectives, a);
    }
    v
}

/// Calls `f` on every Lua snippet in the story, including the ones inside templates.
pub fn visit_code(story: &Story, f: &mut dyn FnMut(&Code)) {
    fn text(t: &Text, f: &mut dyn FnMut(&Code)) {
        for seg in &t.0 {
            match seg {
                Segment::Lit(_) => {}
                Segment::Expr(c) => f(c),
                Segment::If { cond, then, otherwise } => {
                    f(cond);
                    text(then, f);
                    text(otherwise, f);
                }
            }
        }
    }
    let opt_text = |t: &Option<Text>, f: &mut dyn FnMut(&Code)| {
        if let Some(t) = t {
            text(t, f);
        }
    };
    opt_text(&story.intro, f);
    for e in &story.entities {
        opt_text(&e.description, f);
        for x in &e.exits {
            if let Some(w) = &x.when {
                f(w);
            }
            opt_text(&x.blocked, f);
        }
        for h in &e.handlers {
            f(&h.code);
        }
    }
    for v in &story.verbs {
        opt_text(&v.default, f);
    }
    for m in story.messages.values() {
        text(m, f);
    }
    for c in &story.choices {
        for n in &c.nodes {
            opt_text(&n.text, f);
            if let Some(r) = &n.run {
                f(r);
            }
            for o in &n.options {
                text(&o.text, f);
                if let Some(w) = &o.when {
                    f(w);
                }
                opt_text(&o.say, f);
                if let Some(r) = &o.run {
                    f(r);
                }
            }
        }
    }
    for h in &story.handlers {
        f(&h.code);
    }
    for e in &story.endings {
        text(&e.text, f);
    }
}

#[cfg(test)]
mod tests {
    use super::vocab;

    #[test]
    fn vocab_splits_nouns_and_adjectives() {
        let v = vocab("brass key", &["key".into(), "old thing".into()], &["Small".into()]);
        assert_eq!(v.nouns, vec!["key", "thing"]);
        assert_eq!(v.adjectives, vec!["brass", "old", "small"]);
    }
}
