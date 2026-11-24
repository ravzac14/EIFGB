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
const RESERVED_FIELDS: &[&str] = &["key", "name", "kind", "location", "capacity", "health", "money", "patience", "disposition", "visits", "seen", "feeling", "move_to", "is_in", "contents", "has", "can_hold", "wander", "step_toward", "go", "attack", "behave", "distance_to", "is_with", "eat", "is_hostile_to", "make_noise", "proper", "rename"];

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
        let lexicon = author_first_lexicon(&stdlib.lexicon, &verbs);

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
            state: s
                .player
                .health
                .map(|h| ("health".to_string(), Value::Int(i64::from(h))))
                .into_iter()
                .chain(s.player.money.map(|m| ("money".to_string(), Value::Int(i64::from(m)))))
                .chain(s.player.sees_in_dark.then(|| ("sees_in_dark".to_string(), Value::Bool(true))))
                .collect(),
            location: start,
            exits: vec![],
            unlocks_with: None,
            talk: None,
            capacity: s.player.capacity.and_then(|c| c.limit()),
            size: s.player.size.unwrap_or(1),
            picture: None,
            revealed_by: vec![],
            combines: vec![],
            marks: false,
            decoy_for: None,
            handlers: vec![],
            npc: None,
            health: s.player.health,
            damage: Some(s.player.damage.unwrap_or(1)),
            price: None,
            faction: None,
            disguise: None,
            eaten_by: vec![],
            kills: None,
            nameable: false,
            high: false,
            findable_in_dark: false,
            needs_flame: false,
            fuel: None,
            burns_away: false,
            spent: None,
            liquid_source: None,
            holds_liquid: false,
            uses: None,
            breaks: None,
            code: None,
            writing: None,
            sound: None,
            smell: None,
            outdoors: false,
            prints: None,
            leaves_tracks: true,
            diggable: false,
            dark: false,
            list_exits: None,
            night_vision: false,
            calls: None,
            trap: None,
            douses: false,
        }];
        if s.player.health == Some(0) {
            self.d.error("player.health", "health must be at least 1");
        }
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
        let mut endings = self.lower_endings(&s.endings);
        // A player who can be hurt can die. Unless the author wrote a `died`
        // ending, they get a plain one.
        if s.player.health.is_some() && !endings.iter().any(|e| e.key == "died") {
            let text = stdlib.messages.get("ending.died").map(String::as_str).unwrap_or("You have died.");
            endings.push(Ending { key: "died".into(), kind: EndingKind::Loss, text: self.text(text, "stdlib.messages.ending.died") });
        }
        let factions = self.lower_factions(&s.factions);
        for e in &entities {
            let section = if e.kind == EntityKind::Character { "characters" } else { "objects" };
            let mut faction_ref = |field: &str, f: &str| {
                if !factions.iter().any(|x| x.key == f) {
                    let hint = did_you_mean(f, factions.iter().map(|x| x.key.as_str()));
                    self.d.error(
                        format!("{section}.{}.{field}", e.key),
                        format!("unknown faction '{f}'{hint}; declare it under 'factions:'"),
                    );
                }
            };
            if let Some(f) = &e.disguise {
                faction_ref("disguise", f);
            }
            if let Some(f) = e.calls.as_ref().and_then(|c| c.faction.as_ref()) {
                faction_ref("calls.faction", f);
            }
            for f in &e.eaten_by {
                faction_ref("eaten_by", f);
            }
        }
        // Fears name objects, or `fire`; morale is a percentage.
        let check_fears = |d: &mut Diagnostics, path: String, fears: &[String], morale: Option<u32>| {
            for (i, f) in fears.iter().enumerate() {
                if f != "fire" && !entities.iter().any(|e| &e.key == f) {
                    let hint = did_you_mean(f, entities.iter().map(|e| e.key.as_str()).chain(["fire"]));
                    d.error(format!("{path}.fears[{i}]"), format!("unknown object '{f}'{hint}; fears name objects, or 'fire'"));
                }
            }
            if morale.is_some_and(|m| m > 100) {
                d.error(format!("{path}.morale"), "'morale' is a percentage, so it can't be more than 100");
            }
        };
        for e in &entities {
            if let Some(n) = &e.npc {
                check_fears(self.d, format!("characters.{}", e.key), &n.fears, n.morale);
            }
        }
        for f in &factions {
            check_fears(self.d, format!("factions.{}", f.key), &f.fears, f.morale);
        }
        for e in &entities {
            let path = format!("characters.{}.faction", e.key);
            if let Some(f) = &e.faction
                && !factions.iter().any(|x| &x.key == f)
            {
                let hint = did_you_mean(f, factions.iter().map(|x| x.key.as_str()));
                self.d.error(&path, format!("unknown faction '{f}'{hint}; declare it under 'factions:'"));
            }
        }
        for e in &entities {
            // With a faction, its `hostile_to` says whether it goes for the player.
            let attacks_player = match &e.faction {
                Some(f) => factions.iter().any(|x| &x.key == f && x.hostile_to.iter().any(|h| h == PLAYER_FACTION)),
                None => true,
            };
            if e.npc.as_ref().is_some_and(|n| n.behavior.hostile()) && attacks_player && s.player.health.is_none() {
                self.d.warn(
                    format!("characters.{}.behavior", e.key),
                    "attacks the player, but the player has no 'health', so its attacks do nothing; add 'player: { health: 10 }'",
                );
            }
        }
        let clock = s.clock.as_ref().and_then(|c| self.lower_clock(c));
        let weather = s.weather.as_ref().and_then(|w| self.lower_weather(w));
        let timers = self.lower_timers(&s.timers, &endings, clock.as_ref());
        for t in &timers {
            if let Some(f) = t.noise.as_ref().and_then(|n| n.faction.as_ref())
                && !factions.iter().any(|x| &x.key == f)
            {
                let hint = did_you_mean(f, factions.iter().map(|x| x.key.as_str()));
                self.d.error(format!("timers.{}.noise.faction", t.key), format!("unknown faction '{f}'{hint}"));
            }
        }
        let scripts = self.load_scripts(&s.scripts);
        let hints = self.lower_hints(&s.hints);
        let goals = self.lower_goals(&s.goals);
        let scenes = self.lower_scenes(&s.scenes);

        let presentation = s.presentation.as_ref().map(|p| self.presentation(p));
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
                settings: Settings {
                    describe_state: s.settings.describe_state,
                    voice: s.player.voice.unwrap_or(2),
                    track_life: s.settings.track_life.unwrap_or(30),
                    list_exits: s.settings.list_exits,
                },
                presentation,
            },
            intro,
            vars: s.vars.into_iter().collect(),
            lexicon: self.lower_aliases(&s.aliases, lexicon, &verbs, &entities),
            entities,
            verbs,
            messages,
            choices,
            handlers,
            endings,
            scripts,
            categories: stdlib.categories.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            timers,
            factions,
            hints,
            goals,
            scenes,
            clock,
            weather,
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

    /// Text that may vary: a string, a list (each in turn, then the last),
    /// `{ first, later }`, `{ cycle: [...] }` or `{ random: [...] }`.
    fn prose(&mut self, p: &src::Prose, path: &str) -> Text {
        let (mode, texts): (VaryMode, Vec<&str>) = match p {
            src::Prose::One(s) => return self.text(s, path),
            src::Prose::List(l) => (VaryMode::Stop, l.iter().map(String::as_str).collect()),
            src::Prose::Ways(w) => match (&w.first, &w.later, &w.cycle, &w.random) {
                (Some(f), later, None, None) => (VaryMode::Stop, [Some(f.as_str()), later.as_deref()].into_iter().flatten().collect()),
                (None, None, Some(c), None) => (VaryMode::Cycle, c.iter().map(String::as_str).collect()),
                (None, None, None, Some(r)) => (VaryMode::Random, r.iter().map(String::as_str).collect()),
                (None, Some(_), None, None) => {
                    self.d.error(path, "'later' needs a 'first': what's said the first time");
                    return Text::default();
                }
                _ => {
                    self.d.error(path, "varied text is one of: a list, 'first' (and 'later'), 'cycle' or 'random'");
                    return Text::default();
                }
            },
        };
        if texts.is_empty() {
            self.d.error(path, "varied text needs at least one line");
            return Text::default();
        }
        let options = texts.iter().enumerate().map(|(i, t)| self.text(t, &format!("{path}[{i}]"))).collect();
        Text(vec![Segment::Vary { key: path.to_string(), mode, options }])
    }

    fn prose_opt(&mut self, p: Option<&src::Prose>, path: &str) -> Option<Text> {
        p.map(|p| self.prose(p, path))
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
            if GLOBAL_EVENTS.contains(&key.as_str()) || ROOM_EVENTS.contains(&key.as_str()) || CHARACTER_EVENTS.contains(&key.as_str()) {
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
            let mut default = self.prose_opt(v.default.as_ref(), &format!("{path}.default"));
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
            let std_reach = std.map(|s| s.reach.unwrap_or_else(|| s.action.is_none_or(|a| a.needs_reach())));
            let reach = v.reach.or(std_reach).unwrap_or(true);
            verbs.push(Verb { key: key.clone(), patterns, action, default, origin: VerbOrigin::Author, reach });
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
            let reach = std.reach.unwrap_or_else(|| std.action.is_none_or(|a| a.needs_reach()));
            verbs.push(Verb { key: key.clone(), patterns, action, default, origin: VerbOrigin::Stdlib, reach });
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
        author: &indexmap::IndexMap<String, src::Prose>,
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
            out.insert(key.clone(), self.prose(m, &path));
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
        if r.name.trim().is_empty() {
            self.d.error(format!("{path}.name"), "a room needs a name: it's shown as the heading when the player is there");
        }
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
            description: self.prose_opt(r.description.as_ref(), &format!("{path}.description")),
            traits: vec![],
            state: BTreeMap::new(),
            location: None,
            exits,
            unlocks_with: None,
            talk: None,
            capacity: None,
            size: 1,
            picture: self.picture(r.picture.as_ref(), &format!("{path}.picture")),
            revealed_by: vec![],
            combines: vec![],
            marks: false,
            decoy_for: None,
            handlers,
            npc: None,
            health: None,
            damage: None,
            price: None,
            faction: None,
            disguise: None,
            eaten_by: vec![],
            kills: None,
            nameable: false,
            high: false,
            findable_in_dark: false,
            needs_flame: false,
            fuel: None,
            burns_away: false,
            spent: None,
            liquid_source: None,
            holds_liquid: false,
            uses: None,
            breaks: None,
            code: None,
            writing: None,
            sound: self.text_opt(r.sound.as_deref(), &format!("{path}.sound")),
            smell: self.text_opt(r.smell.as_deref(), &format!("{path}.smell")),
            outdoors: r.outdoors,
            prints: None,
            leaves_tracks: !r.trackless,
            diggable: r.diggable,
            dark: r.dark,
            list_exits: r.list_exits,
            night_vision: false,
            calls: None,
            trap: None,
            douses: false,
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
                        exits.push(Exit { direction, to: Some(to), through: None, when: None, blocked: None, via: Vec::new() });
                    }
                }
                src::OneExit::Short(msg) => {
                    let blocked = Some(self.text(msg, &epath));
                    exits.push(Exit { direction, to: None, through: None, when: None, blocked, via: Vec::new() });
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
                    let via: Vec<EntityId> = f
                        .via
                        .as_ref()
                        .map(|v| v.items().into_iter().filter_map(|t| self.entity_ref(t, &format!("{epath}.via"))).collect())
                        .unwrap_or_default();
                    if f.to.is_none() && !via.is_empty() {
                        self.d.warn(&epath, "'via' does nothing without 'to'");
                    }
                    exits.push(Exit { direction, to, through, when, blocked, via });
                }
            }
        }
    }

    // ------------------------------------------------- objects & characters

    fn lower_thing(&mut self, section: &str, key: &str, t: &src::Thing, kind: EntityKind) -> Entity {
        let path = format!("{section}.{key}");
        if t.name.trim().is_empty() {
            self.d.error(format!("{path}.name"), "needs a name: it's what the game calls it (\"You can see a lamp here\"), and what players type");
        }
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
        if has(Trait::Lockable) && t.unlocks_with.is_none() && t.code.is_none() && state.get("locked") == Some(&Value::Bool(true)) {
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
                let Some(r) = self.action_rule(rule, &rpath) else { continue };
                if r.target == self.entity_ids[key] {
                    self.d.error(&rpath, "a hidden thing can't be revealed by acting on itself, since it can't be seen");
                    continue;
                }
                revealed_by.push(r);
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

        let events: &[&str] = if kind == EntityKind::Character { CHARACTER_EVENTS } else { &[] };
        let handlers = self.lower_handlers(&format!("{path}.on"), &t.on, events);
        let npc = self.lower_npc(&path, t, kind, talk.is_some());
        let only = |d: &mut Diagnostics, field: &str, set: bool, ok: bool, who: &str| {
            if set && !ok {
                d.error(format!("{path}.{field}"), format!("only {who} can have '{field}'"));
            }
        };
        let is_char = kind == EntityKind::Character;
        only(self.d, "health", t.health.is_some(), is_char, "characters (and the player)");
        only(self.d, "price", t.price.is_some(), kind == EntityKind::Object, "objects");
        only(self.d, "disguise", t.disguise.is_some(), kind == EntityKind::Object, "objects");
        only(self.d, "eaten_by", !t.eaten_by.is_empty(), kind == EntityKind::Object, "objects");
        only(self.d, "prints", t.prints.is_some(), is_char, "characters");
        only(self.d, "trap", t.trap.is_some(), kind == EntityKind::Object, "objects");
        if t.trap.as_ref().is_some_and(|tr| tr.holds == 0) {
            self.d.error(format!("{path}.trap.holds"), "a trap that holds nothing for no turns does nothing; use 1 or more");
        }
        if kind == EntityKind::Object && t.sees_in_dark == Some(true) && !has(Trait::Wearable) {
            self.d.warn(format!("{path}.sees_in_dark"), "on an object, it only works when worn; add the 'wearable' trait");
        }
        only(self.d, "leaves_tracks", t.leaves_tracks.is_some(), is_char, "characters");
        if t.disguise.is_some() && !has(Trait::Wearable) {
            self.d.warn(format!("{path}.disguise"), "only works when worn; add the 'wearable' trait");
        }
        only(self.d, "money", t.money.is_some(), is_char, "characters (and the player)");
        if let Some(h) = t.health {
            if h == 0 {
                self.d.error(format!("{path}.health"), "health must be at least 1");
            }
            state.insert("health".into(), Value::Int(i64::from(h)));
        }
        if let Some(m) = t.money {
            state.insert("money".into(), Value::Int(i64::from(m)));
        }
        if let Some(n) = &npc
            && n.fights_back
        {
            state.insert("patience".into(), Value::Int(i64::from(n.patience)));
        }
        // How it feels about the player: a number, or a word for one.
        if npc.is_some() {
            let d = match &t.disposition {
                None => 0,
                Some(Value::Int(n)) => (*n).clamp(-10, 10),
                Some(Value::Str(w)) => match eifgb_ir::disposition_of(w) {
                    Some(n) => n,
                    None => {
                        self.d.error(format!("{path}.disposition"), format!("'{w}' isn't a disposition: one of hostile, wary, neutral, friendly or devoted, or a number from -10 to 10"));
                        0
                    }
                },
                Some(_) => {
                    self.d.error(format!("{path}.disposition"), "a disposition is a number from -10 to 10, or a word: hostile, wary, neutral, friendly or devoted");
                    0
                }
            };
            state.insert("disposition".into(), Value::Int(d));
        } else if t.disposition.is_some() {
            self.d.warn(format!("{path}.disposition"), "only characters have a disposition");
        }
        let hostile = npc.as_ref().is_some_and(|n| n.behavior.hostile());
        let damage = t.damage.or(if hostile { Some(1) } else { None });
        // Light, fuel, liquids, wear, codes.
        let light = traits.contains(&Trait::LightSource);
        for (field, set) in [("needs_flame", t.needs_flame), ("fuel", t.fuel.is_some()), ("burns_away", t.burns_away), ("spent", t.spent.is_some())] {
            if set && !light {
                self.d.warn(format!("{path}.{field}"), "only matters for a light_source");
            }
        }
        if t.fuel == Some(0) {
            self.d.error(format!("{path}.fuel"), "fuel must be at least 1; to start it empty, set `state: { fuel: 0 }`");
        }
        if t.burns_away && t.fuel.is_none() {
            self.d.warn(format!("{path}.burns_away"), "does nothing without 'fuel': it says it's gone when its fuel runs out");
        }
        if t.full_of.is_some() && !t.holds_liquid {
            self.d.error(format!("{path}.full_of"), "only something that `holds_liquid` can start full");
        }
        if t.liquid.is_some() && t.holds_liquid {
            self.d.warn(format!("{path}.liquid"), "is a source of liquid and holds liquid; a source never runs dry, so it doesn't need to");
        }
        if t.uses == Some(0) {
            self.d.error(format!("{path}.uses"), "uses must be at least 1");
        }
        if t.code.is_some() && !traits.contains(&Trait::Lockable) {
            self.d.warn(format!("{path}.code"), "a code unlocks something lockable; add `lockable` (and `state: { locked: true }`)");
        }
        if let Some(src::CodeText::Text(c)) = &t.code
            && c.trim().is_empty()
        {
            self.d.error(format!("{path}.code"), "a code can't be empty");
        }
        if let Some(f) = t.fuel {
            state.entry("fuel".into()).or_insert(Value::Int(i64::from(f)));
        }
        if let Some(u) = t.uses {
            state.entry("uses".into()).or_insert(Value::Int(i64::from(u)));
        }
        if let Some(l) = &t.full_of {
            state.entry("liquid".into()).or_insert(Value::Str(l.trim().to_lowercase()));
        }
        // What it makes with something else.
        let mut combines = Vec::new();
        let rules: Vec<&src::CombineSrc> = match &t.combines {
            None => vec![],
            Some(src::CombinesSrc::One(c)) => vec![c],
            Some(src::CombinesSrc::Many(l)) => l.iter().collect(),
        };
        for (i, c) in rules.into_iter().enumerate() {
            let cpath = format!("{path}.combines[{i}]");
            let (Some(with), Some(makes)) = (self.entity_ref(&c.with, &format!("{cpath}.with")), self.entity_ref(&c.makes, &format!("{cpath}.makes"))) else { continue };
            let by = c.by.as_ref().map(|b| b.items().into_iter().map(|v| v.trim().to_lowercase()).collect()).unwrap_or_default();
            let keep = c.keep.as_ref().map(|k| k.items().into_iter().filter_map(|x| self.entity_ref(x, &format!("{cpath}.keep"))).collect()).unwrap_or_default();
            let say = self.text_opt(c.say.as_deref(), &format!("{cpath}.say"));
            combines.push(Combination { with, makes, by, say, keep });
        }
        let decoy_for = t.decoy_for.as_deref().and_then(|d| self.entity_ref(d, &format!("{path}.decoy_for")));
        Entity {
            key: key.into(),
            kind,
            name: t.name.clone(),
            vocab: vocab(&t.name, &t.synonyms, &t.adjectives),
            description: self.prose_opt(t.description.as_ref(), &format!("{path}.description")),
            traits,
            state,
            location,
            exits: vec![],
            unlocks_with,
            talk,
            capacity: t.capacity,
            size: t.size.unwrap_or(1),
            picture: self.picture(t.picture.as_ref(), &format!("{path}.picture")),
            revealed_by,
            combines,
            marks: t.marks,
            decoy_for,
            handlers,
            npc,
            health: t.health.filter(|_| is_char),
            damage,
            price: t.price,
            faction: t.faction.clone().filter(|_| is_char),
            disguise: t.disguise.clone(),
            eaten_by: t.eaten_by.clone(),
            kills: t.kills.clone(),
            nameable: t.nameable,
            high: t.high && kind == EntityKind::Object,
            findable_in_dark: t.findable_in_dark && kind == EntityKind::Object,
            needs_flame: t.needs_flame,
            fuel: t.fuel,
            burns_away: t.burns_away,
            spent: self.text_opt(t.spent.as_deref(), &format!("{path}.spent")),
            sound: self.text_opt(t.sound.as_deref(), &format!("{path}.sound")),
            smell: self.text_opt(t.smell.as_deref(), &format!("{path}.smell")),
            liquid_source: t.liquid.as_ref().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()),
            holds_liquid: t.holds_liquid,
            uses: t.uses,
            breaks: self.text_opt(t.breaks.as_deref(), &format!("{path}.breaks")),
            code: t.code.as_ref().map(|c| match c {
                src::CodeText::Number(n) => n.to_string(),
                src::CodeText::Text(s) => s.trim().to_string(),
            }),
            writing: self.text_opt(t.text.as_deref(), &format!("{path}.text")),
            outdoors: false,
            prints: t.prints.clone().filter(|_| is_char),
            leaves_tracks: t.leaves_tracks.unwrap_or(true),
            diggable: false,
            dark: false,
            list_exits: None,
            night_vision: kind == EntityKind::Object && t.sees_in_dark == Some(true),
            calls: t.calls.as_ref().map(|n| Noise { at: None, loudness: n.loudness, faction: n.faction.clone() }),
            trap: t.trap.as_ref().filter(|_| kind == EntityKind::Object).map(|tr| TrapSpec { holds: tr.holds }),
            douses: t.douses && kind == EntityKind::Object,
        }
    }

    /// A character's behaviour. Every character has one: `stationary`, or
    /// `conversationalist` if it has `talk`, unless the author picks another.
    fn lower_npc(&mut self, path: &str, t: &src::Thing, kind: EntityKind, talks: bool) -> Option<Npc> {
        if kind != EntityKind::Character {
            for (field, set) in [
                ("faction", t.faction.is_some()),
                ("corpse", t.corpse.is_some()),
                ("fights_back", t.fights_back.is_some()),
                ("patience", t.patience.is_some()),
                ("pacifist", t.pacifist.is_some()),
                ("invincible", t.invincible.is_some()),
                ("forgiveness", t.forgiveness.is_some()),
                ("haggle", t.haggle.is_some()),
                ("tame_with", !t.tame_with.is_empty()),
                ("likes", !t.likes.is_empty()),
                ("bribe", t.bribe.is_some()),
                ("fetches", t.fetches.is_some()),
                ("tracker", t.tracker.is_some()),
                ("chatter", !t.chatter.is_empty()),
                ("obeys", t.obeys.is_some()),
                ("morale", t.morale.is_some()),
                ("fears", !t.fears.is_empty()),
                ("behavior", t.behavior.is_some()),
                ("act", t.act.is_some()),
                ("wander", t.wander.is_some()),
                ("area", !t.area.is_empty()),
                ("range", t.range.is_some()),
                ("buys_at", t.buys_at.is_some()),
            ] {
                if set {
                    self.d.error(format!("{path}.{field}"), format!("only characters can have '{field}'; move this to 'characters'"));
                }
            }
            return None;
        }
        let behavior = match t.behavior.as_deref() {
            None if talks => Behavior::Conversationalist,
            None => Behavior::Stationary,
            Some(b) => Behavior::from_key(b).unwrap_or_else(|| {
                let hint = did_you_mean(b, Behavior::ALL.iter().map(|b| b.key()));
                self.d.error(format!("{path}.behavior"), format!("unknown behavior '{b}'{hint}"));
                Behavior::Stationary
            }),
        };
        if behavior == Behavior::Conversationalist && !talks {
            self.d.warn(format!("{path}.behavior"), "a conversationalist needs 'talk' to have anything to say");
        }
        for (field, val) in [("wander", t.wander), ("buys_at", t.buys_at), ("haggle", t.haggle)] {
            if let Some(v) = val
                && v > 100
            {
                self.d.error(format!("{path}.{field}"), format!("'{field}' is a percentage, so it can't be more than 100"));
            }
        }
        if t.pacifist == Some(true) && behavior.hostile() {
            self.d.warn(
                format!("{path}.pacifist"),
                format!("a {} attacks its faction's enemies by nature, so 'pacifist' has no effect", behavior.key()),
            );
        }
        let pacifist = t.pacifist.unwrap_or(behavior.default_pacifist()) && !behavior.hostile();
        if pacifist && (t.patience.is_some() || t.fights_back == Some(true)) {
            let field = if t.patience.is_some() { "patience" } else { "fights_back" };
            self.d.warn(format!("{path}.{field}"), "has no effect on a pacifist, which never fights back");
        }
        if t.patience.is_some() && t.fights_back == Some(false) {
            self.d.warn(format!("{path}.patience"), "has no effect with 'fights_back: false'");
        }
        for (field, set) in [("buys_at", t.buys_at.is_some()), ("haggle", t.haggle.is_some())] {
            if set && behavior != Behavior::Merchant {
                self.d.warn(format!("{path}.{field}"), "only matters for a merchant");
            }
        }
        let area = t
            .area
            .iter()
            .enumerate()
            .filter_map(|(i, r)| self.room_ref(r, &format!("{path}.area[{i}]")))
            .collect();
        let act = t.act.as_deref().map(|c| self.code(c, &format!("{path}.act")));
        let objects = |this: &mut Self, field: &str, keys: &[String]| -> Vec<EntityId> {
            keys.iter()
                .enumerate()
                .filter_map(|(i, k)| this.entity_ref(k, &format!("{path}.{field}[{i}]")))
                .collect()
        };
        // Where it should be by the clock, in time order.
        let mut routine: Vec<(u32, EntityId)> = Vec::new();
        for (time, room) in &t.routine {
            let rpath = format!("{path}.routine.{time}");
            let at = match eifgb_ir::parse_time(time) {
                Ok(m) => m,
                Err(e) => {
                    self.d.error(&rpath, e);
                    continue;
                }
            };
            if let Some(r) = self.room_ref(room, &rpath) {
                routine.push((at, r));
            }
        }
        routine.sort_by_key(|(t, _)| *t);
        let watches = t.watches.iter().enumerate().filter_map(|(i, w)| self.entity_ref(w, &format!("{path}.watches[{i}]"))).collect();
        Some(Npc {
            behavior,
            routine,
            watches,
            wander: t.wander.unwrap_or(behavior.default_wander()),
            area,
            range: t.range,
            buys_at: t.buys_at.unwrap_or(50),
            act,
            corpse: t.corpse.unwrap_or(true),
            fights_back: t.fights_back.unwrap_or(true),
            patience: t.patience.unwrap_or(behavior.default_patience()),
            pacifist,
            invincible: t.invincible.unwrap_or(false),
            forgiveness: t.forgiveness.unwrap_or(1),
            haggle: t.haggle.unwrap_or(if behavior == Behavior::Merchant { 25 } else { 0 }),
            tame_with: objects(self, "tame_with", &t.tame_with),
            likes: objects(self, "likes", &t.likes),
            bribe: t.bribe,
            fetches: t.fetches.unwrap_or(false),
            tracker: t.tracker.unwrap_or(false),
            sees_in_dark: t.sees_in_dark.unwrap_or(false),
            chatter: t
                .chatter
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let p = format!("{path}.chatter[{i}]");
                    match c {
                        src::ChatterLine::Plain(s) => Chatter { text: self.text(s, &p), loud: false, when: None, repeat: false },
                        src::ChatterLine::Full(f) => Chatter {
                            text: self.text(&f.say, &format!("{p}.say")),
                            loud: f.loud,
                            when: f.when.as_deref().map(|w| self.code(w, &format!("{p}.when"))),
                            repeat: f.repeat,
                        },
                    }
                })
                .collect(),
            obeys: t.obeys.unwrap_or(false),
            morale: t.morale,
            fears: t.fears.clone(),
        })
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
            back.push(Exit { direction: opposite, to: Some(a), through: Some(door), when: None, blocked: None, via: Vec::new() });
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
        // A thing's weight is its size plus everything in or on it.
        fn weight(entities: &[Entity], i: usize, depth: usize) -> u32 {
            let inside: u32 = if depth < 16 {
                (0..entities.len())
                    .filter(|&c| entities[c].location.is_some_and(|l| l.index() == i))
                    .map(|c| weight(entities, c, depth + 1))
                    .sum()
            } else {
                0
            };
            entities[i].size.saturating_add(inside)
        }
        for (h, holder) in entities.iter().enumerate() {
            let Some(cap) = holder.capacity else { continue };
            let load: u32 = (0..entities.len())
                .filter(|&c| entities[c].location.is_some_and(|l| l.index() == h))
                .map(|c| weight(entities, c, 0))
                .sum();
            if load > cap {
                self.d.warn(
                    format!("{}.capacity", path_of(holder)),
                    format!("starts holding things of total weight {load}, more than its capacity of {cap}"),
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

    /// Parses `"search mat"` or `"(physical action) rug"`, as used by
    /// `revealed_by`, `started_by` and `stopped_by`.
    fn action_rule(&mut self, rule: &str, rpath: &str) -> Option<ActionRule> {
        let shape = || {
            format!("'{rule}': write it as '<verb> <object>' or '(<category>) <object>', e.g. 'search mat' or '(physical action) rug'")
        };
        let (verbs, category, rest) = if let Some(inner) = rule.trim().strip_prefix('(') {
            let Some((name, rest)) = inner.split_once(')') else {
                self.d.error(rpath, shape());
                return None;
            };
            let name = name.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("_");
            match self.categories.iter().find(|(k, _)| *k == name) {
                Some((_, verbs)) => (verbs.clone(), Some(name), rest),
                None => {
                    let names: Vec<String> = self.categories.iter().map(|(k, _)| k.replace('_', " ")).collect();
                    let hint = did_you_mean(&name.replace('_', " "), names.iter().map(String::as_str));
                    self.d.error(
                        rpath,
                        format!(
                            "'({})' is not an action category{hint}; the categories are ({})",
                            name.replace('_', " "),
                            names.join("), (")
                        ),
                    );
                    return None;
                }
            }
        } else {
            let Some((verb, rest)) = rule.trim().split_once(char::is_whitespace) else {
                self.d.error(rpath, shape());
                return None;
            };
            if !self.verb_keys.iter().any(|v| v == verb) {
                let hint = did_you_mean(verb, self.verb_keys.iter().map(String::as_str));
                self.d.error(rpath, format!("'{verb}' is not a verb{hint}"));
                return None;
            }
            (vec![verb.to_string()], None, rest)
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let [target] = words[..] else {
            self.d.error(rpath, shape());
            return None;
        };
        let target = self.entity_ref(target, rpath)?;
        Some(ActionRule { verbs, category, target })
    }

    // --------------------------------------------------------------- hints

    fn lower_hints(&mut self, raw: &indexmap::IndexMap<String, src::Hint>) -> Vec<Hint> {
        let mut out = Vec::new();
        for (key, h) in raw {
            let path = format!("hints.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            if h.steps.is_empty() {
                self.d.error(format!("{path}.steps"), "a hint needs at least one step");
                continue;
            }
            let when = h.when.as_deref().map(|w| self.code(w, &format!("{path}.when")));
            let steps = h.steps.iter().enumerate().map(|(i, s)| self.text(s, &format!("{path}.steps[{i}]"))).collect();
            out.push(Hint { key: key.clone(), when, steps });
        }
        out
    }

    // --------------------------------------------------------------- goals

    fn lower_scenes(&mut self, raw: &indexmap::IndexMap<String, src::Scene>) -> Vec<Scene> {
        let mut out = Vec::new();
        for (key, sc) in raw {
            let path = format!("scenes.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            let title = sc.title.clone().unwrap_or_else(|| {
                let t = key.replace('_', " ");
                let mut c = t.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            });
            out.push(Scene {
                key: key.clone(),
                title: self.text(&title, &format!("{path}.title")),
                when: sc.when.as_deref().map(|w| self.code(w, &format!("{path}.when"))),
                done: sc.done.as_deref().map(|d| self.code(d, &format!("{path}.done"))),
                begins: self.text_opt(sc.begins.as_deref(), &format!("{path}.begins")),
                ends: self.text_opt(sc.ends.as_deref(), &format!("{path}.ends")),
            });
        }
        out
    }

    fn lower_goals(&mut self, raw: &indexmap::IndexMap<String, src::Goal>) -> Vec<Goal> {
        let mut out = Vec::new();
        for (key, g) in raw {
            let path = format!("goals.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            if g.text.trim().is_empty() {
                self.d.error(format!("{path}.text"), "a goal needs text for the journal, like 'Find a way out of the house.'");
            }
            let points = g.points.unwrap_or(0);
            if points < 0 {
                self.d.error(format!("{path}.points"), "points for a goal can't be negative");
            }
            out.push(Goal {
                key: key.clone(),
                text: self.text(&g.text, &format!("{path}.text")),
                when: g.when.as_deref().map(|w| self.code(w, &format!("{path}.when"))),
                done: g.done.as_deref().map(|d| self.code(d, &format!("{path}.done"))),
                points,
            });
        }
        out
    }

    // --------------------------------------------------------------- clock

    fn lower_clock(&mut self, c: &src::Clock) -> Option<Clock> {
        let time = |t: &src::TimeOfDay, path: &str, this: &mut Self| match parse_time(t) {
            Ok(m) => Some(m),
            Err(e) => {
                this.d.error(path, e);
                None
            }
        };
        let start = time(&c.start, "clock.start", self);
        let dawn = c.dawn.as_ref().map_or(Some(6 * 60), |t| time(t, "clock.dawn", self));
        let dusk = c.dusk.as_ref().map_or(Some(18 * 60), |t| time(t, "clock.dusk", self));
        let per_turn = c.minutes_per_turn.unwrap_or(1);
        if per_turn == 0 {
            self.d.error("clock.minutes_per_turn", "time has to move: use 1 or more minutes a turn");
        }
        let (start, dawn, dusk) = (start?, dawn?, dusk?);
        if dawn == dusk {
            self.d.error("clock.dusk", "dawn and dusk can't be at the same time");
        }
        Some(Clock { start, per_turn: per_turn.max(1), dawn, dusk, show: c.show.unwrap_or(true) })
    }

    // -------------------------------------------------------------- timers

    fn lower_timers(&mut self, raw: &indexmap::IndexMap<String, src::Timer>, endings: &[Ending], clock: Option<&Clock>) -> Vec<Timer> {
        let mut out = Vec::new();
        for (key, t) in raw {
            let path = format!("timers.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            let mut spread = 0;
            let mut actions = |a: &src::Actions, field: &str, this: &mut Self| -> Option<u32> {
                match actions_range(a) {
                    Ok((lo, hi)) => {
                        spread = hi - lo;
                        Some(lo)
                    }
                    Err(e) => {
                        this.d.error(format!("{path}.{field}"), e);
                        None
                    }
                }
            };
            let schedule = match (&t.after, &t.every, &t.at) {
                (Some(a), None, None) => match actions(a, "after", self) {
                    Some(n) => Schedule::After(n),
                    None => continue,
                },
                (None, Some(a), None) => match actions(a, "every", self) {
                    Some(n) => Schedule::Every(n),
                    None => continue,
                },
                (None, None, Some(at)) => {
                    let Some(clock) = clock else {
                        self.d.error(format!("{path}.at"), "'at' needs the game to keep time; add a 'clock:' section with the time it starts");
                        continue;
                    };
                    let parsed = match at {
                        src::TimeOfDay::Text(s) if s.trim().eq_ignore_ascii_case("dawn") => Ok(clock.dawn),
                        src::TimeOfDay::Text(s) if s.trim().eq_ignore_ascii_case("dusk") => Ok(clock.dusk),
                        other => parse_time(other),
                    };
                    match parsed {
                        Ok(m) => Schedule::At(m),
                        Err(e) => {
                            self.d.error(format!("{path}.at"), format!("{e}, or 'dawn' or 'dusk'"));
                            continue;
                        }
                    }
                }
                (None, None, None) => {
                    self.d.error(&path, "a timer needs 'after: <actions>', 'every: <actions>' or 'at: <time of day>'");
                    continue;
                }
                _ => {
                    self.d.error(&path, "use only one of 'after' (once), 'every' (repeating) and 'at' (a time of day)");
                    continue;
                }
            };
            if matches!(schedule, Schedule::After(0) | Schedule::Every(0)) {
                self.d.error(&path, "the number of actions must be at least 1");
            }
            let rules = |field: &str, list: &Option<src::OneOrMany>, this: &mut Self| -> Vec<ActionRule> {
                list.iter()
                    .flat_map(|l| l.items())
                    .filter_map(|r| this.action_rule(r, &format!("{path}.{field}")))
                    .collect()
            };
            let started_by = rules("started_by", &t.started_by, self);
            let stopped_by = rules("stopped_by", &t.stopped_by, self);
            let end = t.end.as_deref().and_then(|e| {
                let found = endings.iter().position(|x| x.key == e).map(|i| EndingId(i as u32));
                if found.is_none() {
                    let hint = did_you_mean(e, endings.iter().map(|x| x.key.as_str()));
                    self.d.error(format!("{path}.end"), format!("no ending called '{e}'{hint}"));
                }
                found
            });
            let noise = t.noise.as_ref().map(|n| {
                if n.loudness == 0 {
                    self.d.error(format!("{path}.noise.loudness"), "a noise nobody can hear does nothing; use 1 or more");
                }
                let at = n.at.as_deref().and_then(|a| self.entity_ref(a, &format!("{path}.noise.at")));
                Noise { at, loudness: n.loudness, faction: n.faction.clone() }
            });
            if t.say.is_none() && t.run.is_none() && t.end.is_none() && noise.is_none() {
                self.d.warn(&path, "this timer does nothing; give it 'say', 'do', 'end' or 'noise'");
            }
            out.push(Timer {
                key: key.clone(),
                schedule,
                spread,
                running: t.running.unwrap_or(started_by.is_empty()),
                started_by,
                stopped_by,
                when: t.when.as_deref().map(|w| self.code(w, &format!("{path}.when"))),
                say: self.prose_opt(t.say.as_ref(), &format!("{path}.say")),
                say_again: self.text_opt(t.say_again.as_deref(), &format!("{path}.say_again")),
                chance: t.chance.as_ref().and_then(|c| {
                    let n = match c {
                        src::Chance::Number(n) => Some(*n),
                        src::Chance::Text(s) => s.trim().trim_end_matches('%').trim().parse::<f64>().ok(),
                    };
                    match n {
                        Some(n) if (0.0..=100.0).contains(&n) && n.fract() == 0.0 => Some(n as u32),
                        _ => {
                            self.d.error(format!("{path}.chance"), "a chance is a whole percentage from 0 to 100: 25%, or 25");
                            None
                        }
                    }
                }),
                heard_in: {
                    let heard: Vec<EntityId> = t
                        .heard_in
                        .iter()
                        .flat_map(|h| h.items())
                        .filter_map(|r| self.entity_ref(r, &format!("{path}.heard_in")))
                        .collect();
                    if !heard.is_empty() && t.say.is_none() && t.say_again.is_none() {
                        self.d.warn(format!("{path}.heard_in"), "does nothing without 'say': it says where the timer's message is heard");
                    }
                    heard
                },
                run: t.run.as_deref().map(|r| self.code(r, &format!("{path}.do"))),
                end,
                noise,
            });
        }
        out
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
                let text = self.prose_opt(n.text.as_ref(), &format!("{npath}.text"));
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
                        say: self.prose_opt(o.say.as_ref(), &format!("{opath}.say")),
                        why_not: self.text_opt(o.why_not.as_deref(), &format!("{opath}.why_not")),
                        disposition: o.disposition.unwrap_or(0),
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

    // ------------------------------------------------------------- factions

    fn lower_factions(&mut self, raw: &indexmap::IndexMap<String, src::Faction>) -> Vec<Faction> {
        let mut out = Vec::new();
        for (key, f) in raw {
            let path = format!("factions.{key}");
            if let Err(e) = check_id(key) {
                self.d.error(&path, e);
            }
            if key == PLAYER_FACTION {
                self.d.error(&path, "'player' is the player's own side; pick another name");
            }
            for (i, h) in f.hostile_to.iter().enumerate() {
                if h != PLAYER_FACTION && !raw.contains_key(h) {
                    let candidates = raw.keys().map(String::as_str).chain([PLAYER_FACTION]);
                    let hint = did_you_mean(h, candidates);
                    self.d.error(format!("{path}.hostile_to[{i}]"), format!("unknown faction '{h}'{hint}"));
                }
            }
            out.push(Faction {
                key: key.clone(),
                hostile_to: f.hostile_to.clone(),
                eats_corpses: f.eats_corpses,
                kills: f.kills.clone(),
                prints: f.prints.clone(),
                sees_in_dark: f.sees_in_dark,
                morale: f.morale,
                fears: f.fears.clone(),
                calls: f.calls.iter().map(|c| c.to_lowercase()).collect(),
                patience: f.patience.unwrap_or(2),
            });
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

    /// The `presentation:` section: unknown themes and colours that aren't
    /// `#rgb` or `#rrggbb` are warned about and left out.
    fn presentation(&mut self, p: &src::Presentation) -> Presentation {
        let theme = p.theme.clone().filter(|t| {
            let ok = THEMES.contains(&t.as_str());
            if !ok {
                let hint = did_you_mean(t, THEMES.iter().copied());
                self.d.warn("presentation.theme", format!("unknown theme '{t}'{hint}; the themes are {}", THEMES.join(", ")));
            }
            ok
        });
        let accent = p.accent.clone().filter(|a| {
            let hex = a.strip_prefix('#').filter(|h| (h.len() == 3 || h.len() == 6) && h.chars().all(|c| c.is_ascii_hexdigit()));
            if hex.is_none() {
                self.d.warn("presentation.accent", format!("'{a}' isn't a colour like \"#c33\" or \"#cc3333\""));
            }
            hex.is_some()
        });
        Presentation { theme, accent }
    }

    /// `weather:` its kinds, the one it starts as, and how often it changes.
    fn lower_weather(&mut self, w: &src::Weather) -> Option<Weather> {
        if w.kinds.is_empty() {
            self.d.error("weather.kinds", "list the kinds of weather there can be: clear, rain, fog…");
            return None;
        }
        let kinds: Vec<WeatherKind> = w
            .kinds
            .iter()
            .map(|(k, v)| WeatherKind {
                key: k.clone(),
                describe: self.text_opt(v.describe.as_deref(), &format!("weather.kinds.{k}.describe")),
                begins: self.text_opt(v.begins.as_deref(), &format!("weather.kinds.{k}.begins")),
            })
            .collect();
        if !kinds.iter().any(|k| k.key == w.start) {
            let hint = did_you_mean(&w.start, kinds.iter().map(|k| k.key.as_str()));
            self.d.error("weather.start", format!("no kind of weather called '{}'{hint}", w.start));
            return None;
        }
        let (every, spread) = match &w.changes {
            None => (None, 0),
            Some(a) => match actions_range(a) {
                Ok((lo, hi)) => (Some(lo), hi - lo),
                Err(e) => {
                    self.d.error("weather.changes", e);
                    (None, 0)
                }
            },
        };
        if every.is_some() && kinds.len() < 2 {
            self.d.warn("weather.changes", "with only one kind of weather, it has nothing to change to");
        }
        Some(Weather { start: w.start.clone(), kinds, every, spread })
    }

    /// `picture: hall.jpg`, or `{ file: hall.jpg, alt: "..." }`. A file must
    /// be in the game's folder; a web address must be https.
    fn picture(&mut self, p: Option<&src::PictureSrc>, path: &str) -> Option<Picture> {
        let (file, alt) = match p? {
            src::PictureSrc::File(f) => (f.trim().to_string(), None),
            src::PictureSrc::Full { file, alt } => (file.trim().to_string(), alt.as_ref().map(|a| a.trim().to_string()).filter(|a| !a.is_empty())),
        };
        if file.is_empty() {
            self.d.error(path, "a picture needs a file");
            return None;
        }
        let web = file.starts_with("https://") || file.starts_with("http://");
        if !web {
            match files::normalize(&file) {
                Err(e) => {
                    self.d.error(path, e);
                    return None;
                }
                Ok(_) if !self.files.exists(&files::sibling(self.entry, &file)) => {
                    self.d.warn(path, format!("there's no picture file '{file}' beside the game"));
                }
                Ok(_) => {}
            }
            if !file.rsplit('.').next().is_some_and(|e| ["png", "jpg", "jpeg", "gif", "webp", "svg"].contains(&e.to_ascii_lowercase().as_str())) {
                self.d.warn(path, "a picture should be a .png, .jpg, .gif, .webp or .svg file");
            }
        } else if file.starts_with("http://") {
            self.d.warn(path, "web pages on https can't show pictures from http:// addresses; use https://");
        }
        if alt.is_none() {
            self.d.warn(path, "say what the picture shows, with 'alt', for players who can't see it");
        }
        Some(Picture { file, alt })
    }

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
        // A routine keeps to the clock: there has to be one.
        if story.clock.is_none() {
            for e in &story.entities {
                if e.npc.as_ref().is_some_and(|n| !n.routine.is_empty()) {
                    self.d.error(format!("characters.{}.routine", e.key), "a routine goes by the time of day; add a 'clock:' section with the time it starts");
                }
            }
        }
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
            for call in ["start_timer", "stop_timer", "timer_left"] {
                for name in literal_calls(src, call) {
                    if !story.timers.iter().any(|t| t.key == name) {
                        let hint = did_you_mean(name, story.timers.iter().map(|t| t.key.as_str()));
                        self.d.error(origin, format!("{call}(\"{name}\"): no such timer{hint}"));
                    }
                }
            }
            for call in ["start_scene", "end_scene"] {
                for name in literal_calls(src, call) {
                    if !story.scenes.iter().any(|s| s.key == name) {
                        let hint = did_you_mean(name, story.scenes.iter().map(|s| s.key.as_str()));
                        self.d.error(origin, format!("{call}(\"{name}\"): no such scene{hint}"));
                    }
                }
            }
            for call in ["start_goal", "complete_goal"] {
                for name in literal_calls(src, call) {
                    if !story.goals.iter().any(|g| g.key == name) {
                        let hint = did_you_mean(name, story.goals.iter().map(|g| g.key.as_str()));
                        self.d.error(origin, format!("{call}(\"{name}\"): no such goal{hint}"));
                    }
                }
            }
            if story.clock.is_none() {
                let used = |word: &str| {
                    src.match_indices(word).any(|(i, _)| !src[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.'))
                };
                for word in ["pass_time(", "set_clock(", "clock."] {
                    if used(word) {
                        let name = word.trim_end_matches(['(', '.']);
                        self.d.error(origin, format!("{name}: the game doesn't keep time; add a 'clock:' section with the time it starts"));
                    }
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

impl Lowerer<'_> {
    /// `aliases:`: the game's own words for words it knows. Each one is
    /// swapped in wherever it appears in a command.
    fn lower_aliases(&mut self, aliases: &indexmap::IndexMap<String, String>, mut lexicon: Lexicon, verbs: &[Verb], entities: &[Entity]) -> Lexicon {
        if aliases.is_empty() {
            return lexicon;
        }
        let mut known: HashSet<String> = HashSet::new();
        for v in verbs {
            for p in &v.patterns {
                for t in &p.0 {
                    if let PatternToken::Word(w) = t {
                        known.insert(w.clone());
                    }
                }
            }
        }
        for e in entities {
            known.extend(e.vocab.nouns.iter().chain(&e.vocab.adjectives).cloned());
            known.extend(e.name.to_lowercase().split_whitespace().map(str::to_string));
        }
        let lx = &lexicon;
        let lists = [&lx.articles, &lx.pronouns, &lx.all_words, &lx.self_words, &lx.corpse_words, &lx.and_words, &lx.except_words, &lx.room_words, &lx.my_words, &lx.one_words, &lx.other_words];
        known.extend(lists.into_iter().flatten().cloned());
        known.extend(lx.directions.keys().cloned());
        known.extend(lx.phrases.keys().flat_map(|k| k.split_whitespace().map(str::to_string)).collect::<Vec<_>>());
        known.extend(lx.numbers.keys().chain(lx.durations.keys()).chain(lx.times.keys()).chain(lx.ordinals.keys()).cloned());
        for (from, to) in aliases {
            let path = format!("aliases.{from}");
            let (from, to) = (from.trim().to_lowercase(), to.trim().to_lowercase());
            if from.is_empty() || to.is_empty() {
                self.d.error(&path, "an alias needs a word and the word it means");
                continue;
            }
            if from == to {
                self.d.warn(&path, "an alias for itself does nothing");
                continue;
            }
            if let Some(w) = to.split_whitespace().find(|w| !known.contains(*w) && w.parse::<u32>().is_err()) {
                self.d.warn(&path, format!("'{w}' isn't a word this game knows, so the alias won't help; aliases stand for words the game already understands"));
            }
            if known.contains(&from) {
                self.d.warn(&path, format!("'{from}' already means something; the alias replaces it everywhere"));
            }
            lexicon.aliases.insert(from, to);
        }
        lexicon
    }
}

/// The stdlib lexicon, minus synonym phrases that would hijack the
/// author's own verbs: with an author verb "read {target}", the stdlib's
/// "read" → "examine" is dropped so READ reaches the author's verb.
fn author_first_lexicon(lexicon: &Lexicon, verbs: &[Verb]) -> Lexicon {
    let author_words: HashSet<&str> = verbs
        .iter()
        .filter(|v| v.origin == VerbOrigin::Author)
        .flat_map(|v| &v.patterns)
        .filter_map(|p| match p.0.first() {
            Some(PatternToken::Word(w)) => Some(w.as_str()),
            _ => None,
        })
        .collect();
    let mut out = lexicon.clone();
    out.phrases.retain(|from, _| !from.split_whitespace().next().is_some_and(|w| author_words.contains(w)));
    out
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

/// `5`, or `5-10` (`5 to 10`, `5..10`): the fewest and most actions.
fn actions_range(a: &src::Actions) -> Result<(u32, u32), String> {
    let bad = || "a number of turns, like 5, or a range to pick from at random, like 5-10".to_string();
    let (lo, hi) = match a {
        src::Actions::Exactly(n) => (*n, *n),
        src::Actions::Text(s) => {
            let s = s.trim();
            let parts: Vec<&str> = if let Some((a, b)) = s.split_once("..") {
                vec![a, b]
            } else if let Some((a, b)) = s.split_once(" to ") {
                vec![a, b]
            } else {
                s.splitn(2, '-').collect()
            };
            let num = |p: &str| p.trim().trim_start_matches('=').parse::<i64>().map_err(|_| bad());
            match parts[..] {
                [one] => (num(one)?, num(one)?),
                [a, b] => (num(a)?, num(b)?),
                _ => return Err(bad()),
            }
        }
    };
    if lo < 1 {
        return Err("the number of actions must be at least 1".into());
    }
    if hi < lo {
        return Err(format!("the range goes from {lo} down to {hi}; put the smaller number first"));
    }
    Ok((lo as u32, hi.min(i64::from(u32::MAX)) as u32))
}

/// A time as written in the game file, in minutes after midnight.
pub(crate) fn parse_time(t: &src::TimeOfDay) -> Result<u32, String> {
    match t {
        src::TimeOfDay::Hour(h) => eifgb_ir::parse_time(&h.to_string()),
        src::TimeOfDay::Text(s) => eifgb_ir::parse_time(s),
    }
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
                Segment::Vary { options, .. } => options.iter().for_each(|o| text(o, f)),
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
    for t in &story.timers {
        if let Some(w) = &t.when {
            f(w);
        }
        opt_text(&t.say, f);
        opt_text(&t.say_again, f);
        if let Some(r) = &t.run {
            f(r);
        }
    }
    for g in &story.goals {
        text(&g.text, f);
        for c in [&g.when, &g.done].into_iter().flatten() {
            f(c);
        }
    }
    for h in &story.hints {
        if let Some(w) = &h.when {
            f(w);
        }
        for s in &h.steps {
            text(s, f);
        }
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
