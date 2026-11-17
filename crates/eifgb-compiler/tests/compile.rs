use eifgb_compiler::{compile, compile_str, DirFiles, Output};
use eifgb_ir::*;
use std::path::PathBuf;

fn example(name: &str) -> Output {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples").join(name);
    compile(&DirFiles { root }, "game.yaml")
}

fn errors(out: &Output) -> Vec<String> {
    out.errors().map(|d| d.to_string()).collect()
}

fn warnings(out: &Output) -> Vec<String> {
    out.warnings().map(|d| d.to_string()).collect()
}

/// A minimal valid game. Tests add sections to it.
const BASE: &str = r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
rooms:
  hall: { name: Hall }
"#;

fn with(extra: &str) -> Output {
    compile_str(&format!("{BASE}{extra}"))
}

fn assert_error(out: &Output, needle: &str) {
    let errs = errors(out);
    assert!(errs.iter().any(|e| e.contains(needle)), "expected an error containing {needle:?}, got {errs:#?}");
    assert!(out.story.is_none());
}

// ------------------------------------------------------------- the example

#[test]
fn lighthouse_compiles_without_errors_or_warnings() {
    let out = example("lighthouse");
    assert!(errors(&out).is_empty(), "{:#?}", errors(&out));
    assert!(warnings(&out).is_empty(), "{:#?}", warnings(&out));
    let story = out.story.unwrap();

    assert_eq!(story.ir_version, IR_VERSION);
    assert_eq!(story.meta.stdlib, 1);
    assert_eq!(story.entity(PLAYER).kind, EntityKind::Player);
    assert_eq!(story.entity(story.meta.start).key, "shore");
    assert_eq!(story.entity(PLAYER).location, Some(story.meta.start));

    let key = story.entity_by_key("brass_key").unwrap();
    let pool = story.entity_by_key("tide_pool").unwrap();
    assert_eq!(story.entity(key).location, Some(pool));
    assert_eq!(story.entity(key).state["hidden"], Value::Bool(true));
    assert!(story.entity(key).vocab.adjectives.contains(&"brass".to_string()));

    // The `open` trait is shorthand for state.
    assert!(!story.entity(pool).traits.iter().any(|t| t.key() == "open"));
    assert_eq!(story.entity(pool).state["open"], Value::Bool(true));

    let base = story.entity(story.entity_by_key("lighthouse_base").unwrap());
    let up = base.exits.iter().find(|e| e.direction == Direction::Up).unwrap();
    assert_eq!(up.through, story.entity_by_key("iron_gate"));

    let shore = story.entity(story.meta.start);
    let south = shore.exits.iter().find(|e| e.direction == Direction::South).unwrap();
    assert!(south.to.is_none() && south.blocked.is_some());

    // Author verbs come first, then the standard library's.
    assert_eq!(story.verbs[0].key, "fill");
    assert_eq!(story.verbs[0].origin, VerbOrigin::Author);
    assert!(story.verbs.iter().any(|v| v.key == "take" && v.origin == VerbOrigin::Stdlib));

    let lamp = story.entity(story.entity_by_key("great_lamp").unwrap());
    assert!(lamp.handlers.iter().any(|h| h.phase == Phase::Before && h.event == "light"));

    let keeper = story.entity(story.entity_by_key("keeper").unwrap());
    assert_eq!(story.choice(keeper.talk.unwrap()).key, "keeper_talk");

    assert_eq!(story.scripts.len(), 1);
    assert!(story.scripts[0].src.contains("trust_level"));
    assert!(story.messages.contains_key("take.done"));
}

#[test]
fn lighthouse_ir_round_trips_through_json() {
    let story = example("lighthouse").story.unwrap();
    let json = serde_json::to_string(&story).unwrap();
    let back: Story = serde_json::from_str(&json).unwrap();
    assert_eq!(story, back);
}

// -------------------------------------------------------------- validation

#[test]
fn minimal_game_compiles() {
    let out = with("");
    assert!(out.story.is_some(), "{:#?}", errors(&out));
}

#[test]
fn duplicate_keys_are_errors_not_silent_overwrites() {
    assert_error(&with("  hall2: { name: A, exits: { north: hall, north: hall2 } }\n"), "duplicate entry with key \"north\"");
    assert_error(&with("objects:\n  lamp: { name: lamp }\n  lamp: { name: other lamp }\n"), "duplicate entry with key \"lamp\"");
}

#[test]
fn yaml_typos_in_field_names_are_errors() {
    assert_error(&with("objects:\n  lamp: { name: lamp, in: hall, discription: x }\n"), "discription");
}

#[test]
fn unknown_references_suggest_fixes() {
    let out = with("objects:\n  lamp: { name: lamp, in: hal }\n");
    assert_error(&out, "did you mean 'hall'?");
}

#[test]
fn exit_to_unknown_room_is_an_error_but_prose_is_a_blocked_message() {
    assert_error(&with("  cellar: { name: Cellar, exits: { up: hal } }\n"), "unknown ID 'hal'");
    let out = with("  cellar: { name: Cellar, exits: { up: \"The hatch is stuck.\" } }\n");
    assert!(out.story.is_some(), "{:#?}", errors(&out));
}

#[test]
fn duplicate_ids_across_sections_are_errors() {
    assert_error(&with("objects:\n  hall: { name: hall }\n"), "already used in rooms");
}

#[test]
fn reserved_and_keyword_ids_are_errors() {
    assert_error(&with("objects:\n  world: { name: globe }\n"), "reserved");
    assert_error(&with("objects:\n  end: { name: end }\n"), "Lua keyword");
}

#[test]
fn unknown_trait_and_direction() {
    assert_error(&with("objects:\n  box: { name: box, traits: [contaner] }\n"), "did you mean 'container'?");
    assert_error(&with("  cellar: { name: Cellar, exits: { nrth: hall } }\n"), "did you mean 'north'?");
}

#[test]
fn handler_for_unknown_verb_suggests_one() {
    let out = with("objects:\n  box: { name: box, on: { serch: 'say(1)' } }\n");
    assert_error(&out, "did you mean 'search'?");
}

#[test]
fn before_and_after_work_on_verbs_but_not_events() {
    let ok = with("objects:\n  box: { name: box, on: { before_take: 'say(1)', after_take: 'say(2)' } }\n");
    assert!(ok.story.is_some(), "{:#?}", errors(&ok));
    assert_error(&with("on: { before_turn: 'say(1)' }\n"), "no before_/after_ form");
}

#[test]
fn bad_templates_report_their_path() {
    assert_error(&with("  cellar: { name: Cellar, description: '${if x}dark' }\n"), "rooms.cellar.description");
}

#[test]
fn things_inside_non_containers_are_errors() {
    let src = "objects:\n  rock: { name: rock, in: hall }\n  pebble: { name: pebble, in: rock }\n";
    assert_error(&with(src), "isn't a container");
}

#[test]
fn containment_loops_are_errors() {
    let src = "objects:\n  a: { name: a, in: b, traits: [container] }\n  b: { name: b, in: a, traits: [container] }\n";
    assert_error(&with(src), "loop");
}

fn exits_of<'a>(story: &'a Story, room: &str) -> Vec<&'a Exit> {
    story.entity(story.entity_by_key(room).unwrap()).exits.iter().collect()
}

#[test]
fn doors_work_both_ways_by_adding_the_return_exit() {
    let src = "  cellar: { name: Cellar }\nobjects:\n  hatch: { name: hatch, in: hall, traits: [door, openable] }\n";
    let src = src.replace("  cellar: { name: Cellar }", "  cellar: { name: Cellar }\n  attic:\n    name: Attic\n    exits: { down: { to: hall, through: hatch } }");
    let out = with(&src);
    let story = out.story.as_ref().unwrap_or_else(|| panic!("{:#?}", out.diagnostics));
    let back: Vec<_> = exits_of(story, "hall");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].direction, Direction::Up);
    assert_eq!(back[0].to, story.entity_by_key("attic"));
    assert_eq!(back[0].through, story.entity_by_key("hatch"));
}

#[test]
fn an_existing_way_back_gets_the_door() {
    let out = compile_str(
        "format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: a }\nrooms:\n  a: { name: A, exits: { north: { to: b, through: d } } }\n  b: { name: B, exits: { west: a } }\nobjects:\n  d: { name: door, in: a, traits: [door, openable] }\n",
    );
    let story = out.story.as_ref().unwrap();
    let back = exits_of(story, "b");
    assert_eq!(back.len(), 1, "the existing exit is reused, not duplicated");
    assert_eq!((back[0].direction, back[0].through), (Direction::West, story.entity_by_key("d")));
}

#[test]
fn a_door_that_cannot_be_linked_back_warns() {
    let out = compile_str(
        "format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: a }\nrooms:\n  a: { name: A, exits: { north: { to: b, through: d } } }\n  b: { name: B, exits: { south: c } }\n  c: { name: C }\nobjects:\n  d: { name: door, in: a, traits: [door, openable] }\n",
    );
    assert!(out.story.is_some());
    assert!(
        warnings(&out).iter().any(|w| w.contains("'d' only works one way") && w.contains("leads to 'c'")),
        "{:#?}",
        warnings(&out)
    );
}

#[test]
fn blocked_must_name_a_side_of_the_door() {
    let base = "format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: a }\nrooms:\n  a: { name: A, exits: { north: { to: b, through: d } } }\n  b: { name: B }\n  far: { name: Far }\nobjects:\n  d: { name: door, in: a, traits: [door, openable], state: { blocked: BLOCKED } }\n";
    for ok in ["true", "false", "b", "[a, b]"] {
        let out = compile_str(&base.replace("BLOCKED", ok));
        assert!(out.story.is_some(), "blocked: {ok}: {:#?}", out.diagnostics);
    }
    assert_error(&compile_str(&base.replace("BLOCKED", "far")), "'far' isn't a room on either side of this door");
    assert_error(&compile_str(&base.replace("BLOCKED", "bb")), "did you mean 'b'?");
    assert_error(&compile_str(&base.replace("BLOCKED", "3")), "must be true, false, a room ID");
}

#[test]
fn revealed_by_rules_are_checked() {
    let thing = |rule: &str, hidden: bool| {
        with(&format!(
            "objects:\n  mat: {{ name: mat, in: hall }}\n  key: {{ name: key, in: hall, state: {{ hidden: {hidden} }}, revealed_by: {rule} }}\n"
        ))
    };
    let out = thing("search mat", true);
    let story = out.story.as_ref().unwrap_or_else(|| panic!("{:#?}", out.diagnostics));
    let key = story.entity(story.entity_by_key("key").unwrap());
    assert_eq!(
        key.revealed_by,
        vec![ActionRule { verbs: vec!["search".into()], category: None, target: story.entity_by_key("mat").unwrap() }]
    );
    assert!(out.diagnostics.is_empty());

    assert_error(&thing("serch mat", true), "'serch' is not a verb (did you mean 'search'?)");
    assert_error(&thing("search mta", true), "did you mean 'mat'?");
    assert_error(&thing("look under mat", true), "write it as '<verb> <object>'");
    assert_error(&thing("search key", true), "can't be revealed by acting on itself");
    let out = thing("search mat", false);
    assert!(warnings(&out).iter().any(|w| w.contains("doesn't start hidden")), "{:#?}", warnings(&out));
}

#[test]
fn verbs_cannot_be_named_after_events() {
    assert_error(&with("verbs:\n  turn: { patterns: ['crank {target}'] }\n"), "'turn' is the name of an event");
    assert_error(&with("verbs:\n  enter: { patterns: ['enter {target}'] }\n"), "'enter' is the name of an event");
    // The turn event still works, and doesn't fire on TURN DIAL.
    let out = with("objects:\n  dial: { name: dial, in: hall }\non: { turn: 'say(1)' }\n");
    assert!(out.story.is_some(), "{:#?}", out.diagnostics);
}

#[test]
fn categories_expand_to_their_verbs() {
    let thing = |rule: &str| {
        with(&format!("objects:\n  rug: {{ name: rug, in: hall }}\n  coin: {{ name: coin, in: hall, state: {{ hidden: true }}, revealed_by: '{rule}' }}\n"))
    };
    for spelling in ["(physical action) rug", "(PHYSICAL ACTION) rug", "(physical_action) rug", "( Physical  Action ) rug"] {
        let out = thing(spelling);
        let story = out.story.as_ref().unwrap_or_else(|| panic!("{spelling}: {:#?}", out.diagnostics));
        let r = &story.entity(story.entity_by_key("coin").unwrap()).revealed_by[0];
        assert_eq!(r.category.as_deref(), Some("physical_action"));
        assert!(r.verbs.contains(&"kick".to_string()) && r.verbs.contains(&"move".to_string()), "{:?}", r.verbs);
    }
    assert_error(&thing("(physical acton) rug"), "did you mean 'physical action'?");
    assert_error(&thing("(dancing) rug"), "the categories are (physical action), (inspect), (sense)");
    assert_error(&thing("(physical action rug"), "write it as");
}

#[test]
fn author_verbs_add_to_hookable_stdlib_verbs_without_warnings() {
    let out = with("verbs:\n  kick: { patterns: ['boot {target}'] }\n");
    let story = out.story.as_ref().unwrap();
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    let kick: Vec<_> = story.verbs.iter().filter(|v| v.key == "kick").collect();
    assert_eq!(kick.len(), 1);
    assert_eq!(kick[0].patterns.len(), 2, "the author's 'boot' plus the stdlib's 'kick'");
    assert!(kick[0].default.is_some(), "keeps the stdlib's default reply");
}

#[test]
fn door_hidden_sides_are_checked() {
    let base = "format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: a }\nrooms:\n  a: { name: A, exits: { north: { to: b, through: d } } }\n  b: { name: B }\nobjects:\n  d: { name: door, in: a, traits: [door, openable], state: { hidden: HIDDEN } }\n  box: { name: box, in: a, state: { hidden: b } }\n";
    let out = compile_str(&base.replace("HIDDEN", "b"));
    // The door can name a side; a non-door can't.
    assert!(errors(&out).iter().any(|e| e.contains("objects.box.state.hidden") && e.contains("true or false")), "{:#?}", errors(&out));
    assert!(!errors(&out).iter().any(|e| e.contains("objects.d.state.hidden")), "{:#?}", errors(&out));
    let fixed = base.replace("box: { name: box, in: a, state: { hidden: b } }", "box: { name: box, in: a }");
    assert!(compile_str(&fixed.replace("HIDDEN", "[a, b]")).story.is_some());
    assert_error(&compile_str(&fixed.replace("HIDDEN", "c")), "'c' isn't a room on either side of this door");
}

#[test]
fn author_verbs_win_over_stdlib_synonyms() {
    let story = with("").story.unwrap();
    assert_eq!(story.lexicon.phrases.get("read").map(String::as_str), Some("examine"));
    let story = with("verbs:\n  read: { patterns: ['read {target}'] }\n").story.unwrap();
    assert!(!story.lexicon.phrases.contains_key("read"), "the author's READ replaces the synonym");
    assert!(story.lexicon.phrases.contains_key("grab"), "other synonyms stay");
}

#[test]
fn verbs_need_reach_unless_they_say_otherwise() {
    let out = with("verbs:\n  polish: { patterns: ['polish {target}'] }\n  read: { patterns: ['read {target}'], reach: false }\n");
    let story = out.story.as_ref().unwrap();
    let reach = |k: &str| story.verbs.iter().find(|v| v.key == k).unwrap().reach;
    assert!(reach("polish"), "author verbs need reach by default");
    assert!(!reach("read"));
    assert!(reach("take") && reach("kick"), "touching verbs need reach");
    assert!(!reach("examine") && !reach("listen") && !reach("look"), "looking and listening don't");
}

#[test]
fn container_traits_are_checked() {
    assert_error(&with("objects:\n  rug: { name: rug, in: hall, traits: [enterable] }\n"), "'enterable' needs 'container'");
    let out = with("objects:\n  pane: { name: pane, in: hall, traits: [transparent] }\n");
    assert!(warnings(&out).iter().any(|w| w.contains("'transparent' only matters on a 'container'")), "{:#?}", warnings(&out));
    let out = with("objects:\n  cup: { name: cup, in: hall, traits: [container], capacity: 1 }\n  a: { name: a, in: cup }\n  b: { name: b, in: cup, size: 2 }\n");
    assert!(warnings(&out).iter().any(|w| w.contains("total weight 3, more than its capacity of 1")), "{:#?}", warnings(&out));
}

#[test]
fn timers_are_checked() {
    let t = |body: &str| with(&format!("objects:\n  fuse: {{ name: fuse, in: hall }}\nendings:\n  boom: {{ text: Boom }}\ntimers:\n  t: {{ {body} }}\n"));
    let ok = t("after: 3, started_by: kick fuse, end: boom");
    let story = ok.story.as_ref().unwrap_or_else(|| panic!("{:#?}", ok.diagnostics));
    let timer = &story.timers[0];
    assert_eq!(timer.schedule, Schedule::After(3));
    assert!(!timer.running, "a timer with started_by waits to be started");
    assert!(t("every: 2, say: drip").story.unwrap().timers[0].running);

    assert_error(&t("after: 3, every: 2, say: x"), "not both");
    assert_error(&t("say: x"), "needs 'after");
    assert_error(&t("after: 0, say: x"), "at least 1");
    assert_error(&t("after: 3, end: bom"), "did you mean 'boom'?");
    assert_error(&t("after: 3, started_by: kick fuze, say: x"), "did you mean 'fuse'?");
    let quiet = t("after: 3");
    assert!(warnings(&quiet).iter().any(|w| w.contains("this timer does nothing")), "{:#?}", warnings(&quiet));
    assert_error(&with("on: { start: 'start_timer(\"nope\")' }\n"), "start_timer(\"nope\"): no such timer");
}

#[test]
fn through_requires_a_door() {
    let src = "  cellar:\n    name: Cellar\n    exits: { up: { to: hall, through: crate } }\nobjects:\n  crate: { name: crate, in: cellar }\n";
    assert_error(&with(src), "needs the 'door' trait");
}

#[test]
fn literal_script_references_are_checked() {
    let src = "on: { turn: 'end_game(\"won\")' }\nendings:\n  win: { kind: win, text: Yay }\n";
    assert_error(&with(src), "no such ending (did you mean 'win'?)");
    assert_error(&with("on: { turn: 'open_choice(\"nope\")' }\n"), "no such choice");
}

#[test]
fn talk_is_only_for_characters() {
    let src = "objects:\n  rock: { name: rock, talk: chat }\nchoices:\n  chat: { start: a, nodes: { a: { text: hi } } }\n";
    assert_error(&with(src), "only characters");
}

#[test]
fn choice_gotos_are_checked() {
    let src = "choices:\n  c:\n    start: a\n    nodes:\n      a: { text: hi, options: [ { text: go, goto: bb } ] }\n      b: { text: there }\n";
    assert_error(&with(src), "unknown node 'bb'");
}

#[test]
fn author_verb_replacing_stdlib_verb_keeps_builtin_action_and_warns() {
    let out = with("verbs:\n  take: { patterns: ['grab {target}'] }\n");
    let story = out.story.as_ref().expect("should compile");
    let take: Vec<_> = story.verbs.iter().filter(|v| v.key == "take").collect();
    assert_eq!(take.len(), 1);
    assert_eq!(take[0].action, VerbAction::Builtin(BuiltinAction::Take));
    assert!(warnings(&out).iter().any(|w| w.contains("replaces the patterns")));
}

#[test]
fn shadowing_a_stdlib_pattern_warns() {
    let out = with("verbs:\n  peek: { patterns: ['search {target}'] }\n");
    assert!(out.story.is_some());
    assert!(warnings(&out).iter().any(|w| w.contains("shadows standard verb 'search'")), "{:#?}", warnings(&out));
}

#[test]
fn unknown_stdlib_version_or_language() {
    let bad_version = compile_str("format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: hall, stdlib: 99 }\nrooms: { hall: { name: Hall } }\n");
    assert_error(&bad_version, "no standard library v99");
    let bad_lang = compile_str("format: eifgb/0.1\ngame: { id: t, title: T, version: '1', start: hall, language: fr }\nrooms: { hall: { name: Hall } }\n");
    assert_error(&bad_lang, "game.language");
}

#[test]
fn a_misspelled_key_gives_one_error_and_no_follow_on_warning() {
    let src = "objects:\n  box: { name: box, traits: [container, openable, lockable], state: { locked: true }, unlocks_with: kye }\n  key: { name: key }\n";
    let out = with(src);
    assert_error(&out, "did you mean 'key'?");
    assert!(warnings(&out).is_empty(), "{:#?}", warnings(&out));
}

#[test]
fn state_fields_cannot_shadow_builtin_fields_or_methods() {
    assert_error(&with("objects:\n  box: { name: box, state: { contents: 3 } }\n"), "built-in field or method");
    assert_error(&with("objects:\n  box: { name: box, state: { location: hall } }\n"), "built-in field or method");
    assert_error(&with("objects:\n  box: { name: box, state: { capacity: 3 } }\n"), "built-in field or method");
}

#[test]
fn script_paths_cannot_escape_the_project() {
    assert_error(&with("scripts: ['../../etc/passwd']\n"), "must not contain '..'");
}

#[test]
fn player_capacity_is_a_number_or_none() {
    for ok in ["0", "5", "none", "~"] {
        let out = with(&format!("player: {{ capacity: {ok} }}\n"));
        assert!(out.story.is_some(), "capacity {ok}: {:#?}", out.diagnostics);
    }
    let out = with("player: { capacity: lots }\n");
    assert!(out.story.is_none());
    assert!(format!("{:?}", out.diagnostics).contains("a whole number, or `none` for no limit"), "{:#?}", out.diagnostics);
    let out = with("player: { capacity: -1 }\n");
    assert!(format!("{:?}", out.diagnostics).contains("can't be negative"), "{:#?}", out.diagnostics);
    // Starting with things in hand that a capacity of 0 can't hold is worth a warning.
    let out = with("player: { capacity: 0 }\nobjects:\n  coin: { name: coin, in: player, traits: [portable] }\n");
    assert!(warnings(&out).iter().any(|w| w.contains("more than its capacity of 0")), "{:#?}", warnings(&out));
}
