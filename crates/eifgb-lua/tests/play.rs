//! End-to-end tests: compiled story + Lua host + turn loop, driven by text.

use eifgb_compiler::{compile, compile_str, DirFiles};
use eifgb_engine::ir::{EndingKind, Story};
use eifgb_engine::{Game, LineKind, Output};
use eifgb_lua::LuaHost;
use std::path::PathBuf;

fn lighthouse_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/lighthouse")
}

fn lighthouse() -> Story {
    let out = compile(&DirFiles { root: lighthouse_dir() }, "game.yaml");
    out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics))
}

fn game(story: Story) -> Game<LuaHost> {
    let mut g = Game::new(story, LuaHost::new().unwrap(), 1).unwrap();
    g.start();
    g
}

fn small(extra: &str) -> Game<LuaHost> {
    let src = format!(
        "format: eifgb/0.1\ngame: {{ id: t, title: T, version: '1', start: hall }}\nrooms:\n  hall: {{ name: Hall, exits: {{ north: yard }} }}\n  yard: {{ name: Yard, exits: {{ south: hall }} }}\n{extra}"
    );
    let out = compile_str(&src);
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

/// Sends one input and returns the output text.
fn say(g: &mut Game<LuaHost>, input: &str) -> String {
    g.input(input).text()
}

fn assert_says(g: &mut Game<LuaHost>, input: &str, needle: &str) {
    let out = say(g, input);
    assert!(out.contains(needle), "> {input}\n{out}\n(expected {needle:?})");
}

// --------------------------------------------------------- the lighthouse

/// Every example game with a walkthrough.txt must play through to a win
/// without script errors. This keeps the README tutorial honest.
#[test]
fn every_example_walkthrough_wins() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples");
    let mut played = 0;
    for entry in std::fs::read_dir(&examples).unwrap() {
        let dir = entry.unwrap().path();
        let Ok(walkthrough) = std::fs::read_to_string(dir.join("walkthrough.txt")) else { continue };
        let out = compile(&DirFiles { root: dir.clone() }, "game.yaml");
        assert!(out.diagnostics.is_empty(), "{}: {:#?}", dir.display(), out.diagnostics);
        let mut g = game(out.story.unwrap());
        let mut last = Output::default();
        for line in walkthrough.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            last = g.input(line);
            assert!(
                !last.lines.iter().any(|l| l.kind == LineKind::Error),
                "{}\n> {line}\n{}",
                dir.display(),
                last.text()
            );
        }
        assert_eq!(last.ended, Some(EndingKind::Win), "{}\n{}", dir.display(), last.text());
        played += 1;
    }
    assert!(played >= 4, "expected the lighthouse, midnight_snack, campfire and blackfang walkthroughs");
}

#[test]
fn lighthouse_walkthrough_scores_15() {
    let mut g = game(lighthouse());
    let walkthrough = std::fs::read_to_string(lighthouse_dir().join("walkthrough.txt")).unwrap();
    let mut last = Output::default();
    for line in walkthrough.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        last = g.input(line);
    }
    assert_eq!(last.status.score, 15);
    assert!(last.text().contains("We see the light!"));
}

#[test]
fn waiting_too_long_loses() {
    let mut g = game(lighthouse());
    let mut out = Output::default();
    for _ in 0..60 {
        out = g.input("wait");
    }
    assert_eq!(out.ended, Some(EndingKind::Loss));
    assert_says(&mut g, "look", "UNDO");
    // UNDO goes back to before the ending.
    let out = g.input("undo");
    assert_eq!(out.ended, None);
    assert_eq!(out.status.turns, 59);
}

#[test]
fn opening_output_and_status() {
    let mut g = Game::new(lighthouse(), LuaHost::new().unwrap(), 1).unwrap();
    let out = g.start();
    assert_eq!(out.lines[0].kind, LineKind::Banner);
    assert!(out.lines.iter().any(|l| l.kind == LineKind::Title && l.text == "Rocky Shore"));
    assert_eq!(out.status.room, "Rocky Shore");
    assert_eq!(out.status.max_score, Some(30));
}

// ------------------------------------------------------------ the parser

#[test]
fn typos_synonyms_and_inflections() {
    let mut g = game(lighthouse());
    assert_says(&mut g, "serch the pool", "pull out a small brass key");
    assert_says(&mut g, "examin brass kye", "green with age");
    assert_says(&mut g, "nrth", "Base of the Lighthouse");
    assert_says(&mut g, "grabbed the can", "Taken.");
    assert_says(&mut g, "peer at keys", "green with age");
    assert_says(&mut g, "please drop the can", "Dropped.");
    assert_says(&mut g, "i'd like to snatch the kerosene", "Taken.");
}

#[test]
fn bare_nouns_are_examined() {
    let mut g = game(lighthouse());
    assert_says(&mut g, "pool", "Cold water");
}

#[test]
fn missing_objects_are_asked_for_and_the_reply_completes_the_command() {
    let mut g = game(lighthouse());
    assert_says(&mut g, "x", "What do you want to examine?");
    assert_says(&mut g, "pool", "Cold water");
    say(&mut g, "search pool");
    say(&mut g, "n");
    say(&mut g, "take can");
    say(&mut g, "unlock gate");
    say(&mut g, "u");
    assert_says(&mut g, "fill lamp", "What do you want to fill the great lamp with?");
    assert_says(&mut g, "oil", "until it's full");
    // A reply that is a new command is treated as one.
    assert_says(&mut g, "take", "What do you want to take?");
    assert_says(&mut g, "look", "Lamp Room");
}

#[test]
fn pronouns_refer_to_the_last_object() {
    let mut g = game(lighthouse());
    assert_says(&mut g, "take it", "not sure what you're referring to");
    say(&mut g, "search pool");
    say(&mut g, "x key");
    assert_says(&mut g, "drop it", "Dropped.");
}

#[test]
fn unknown_verbs_suggest_what_works_here() {
    let mut g = game(lighthouse());
    let out = say(&mut g, "dance");
    assert!(out.contains("I don't know the verb \"dance\""), "{out}");
    assert!(out.contains("SEARCH"), "the tide pool handles SEARCH, so it should be suggested: {out}");
    assert_says(&mut g, "take zorkmid", "I don't know the word \"zorkmid\"");
    assert_says(&mut g, "take radio", "You can't see any such thing.");
    assert_says(&mut g, "take key from", "only understood you as far as wanting to take");
}

#[test]
fn author_parse_failed_hook_runs_first() {
    let mut g = game(lighthouse());
    assert_says(&mut g, "swim", "The waves would smash you");
}

#[test]
fn chained_commands_stop_at_the_first_failure() {
    let mut g = game(lighthouse());
    let out = say(&mut g, "search pool. n then take can");
    assert!(out.contains("brass key") && out.contains("Base of the Lighthouse") && out.contains("Taken."), "{out}");
    let out = say(&mut g, "dance. s");
    assert!(!out.contains("Rocky Shore"), "{out}");
}

#[test]
fn ambiguity_is_resolved_by_asking() {
    let mut g = small(
        "objects:\n  brass_key: { name: brass key, in: hall, traits: [portable] }\n  iron_key: { name: iron key, in: hall, traits: [portable] }\n",
    );
    assert_says(&mut g, "take key", "Which do you mean, the brass key or the iron key?");
    assert_says(&mut g, "iron", "Taken.");
    // Holding the iron key, DROP KEY can only mean that one.
    assert_says(&mut g, "drop key", "Dropped.");
    assert_says(&mut g, "take brass key", "Taken.");
    // TAKE prefers things you aren't holding.
    assert_says(&mut g, "take key", "Taken.");
    assert_says(&mut g, "i", "a brass key and an iron key");
}

#[test]
fn take_all_and_drop_all() {
    let mut g = small(
        "objects:\n  cup: { name: cup, in: hall, traits: [portable] }\n  plate: { name: plate, in: hall, traits: [portable] }\n  statue: { name: statue, in: hall }\n",
    );
    let out = say(&mut g, "take all");
    assert!(out.contains("cup: Taken.") && out.contains("plate: Taken."), "{out}");
    assert!(!out.contains("statue"), "{out}");
    assert_says(&mut g, "drop everything", "plate: Dropped.");
    assert_says(&mut g, "open all", "You can't use \"all\" with that.");
}

// ------------------------------------------------- doors, containers, keys

#[test]
fn doors_keys_and_containers() {
    let mut g = small(
        "objects:\n  box: { name: wooden box, in: hall, traits: [container, openable, lockable], state: { locked: true }, unlocks_with: key }\n  key: { name: key, in: hall, traits: [portable] }\n  coin: { name: coin, in: box, traits: [portable] }\n",
    );
    assert_says(&mut g, "take coin", "You can't see any such thing.");
    assert_says(&mut g, "open box", "is locked");
    assert_says(&mut g, "unlock box", "You don't have anything that unlocks it.");
    say(&mut g, "take key");
    assert_says(&mut g, "unlock box", "You unlock the wooden box with the key.");
    let out = say(&mut g, "open box");
    assert!(out.contains("You open the wooden box.") && out.contains("you can see a coin"), "{out}");
    assert_says(&mut g, "take coin", "Taken.");
    assert_says(&mut g, "put coin in box", "You put the coin in the wooden box.");
    assert_says(&mut g, "put box in box", "can't put something inside itself");
    assert_says(&mut g, "lock box", "close the wooden box first");
}

#[test]
fn blocked_and_conditional_exits() {
    let g = small(
        "  cellar:\n    name: Cellar\n    exits:\n      up: hall\n      down: { to: hall, when: vars.lit, blocked: \"It's too dark to go further.\" }\n      east: \"A wall of earth.\"\nvars: { lit: false }\n",
    );
    drop(g);
    // The cellar isn't reachable from the hall, so a custom verb moves the player there.
    let mut g = small(
        "  cellar:\n    name: Cellar\n    exits:\n      up: hall\n      down: { to: hall, when: vars.lit, blocked: \"It's too dark to go further.\" }\n      east: \"A wall of earth.\"\nvars: { lit: false }\nverbs:\n  descend: { patterns: [descend] }\non:\n  descend: 'player:move_to(world.cellar)'\n",
    );
    say(&mut g, "descend");
    assert_says(&mut g, "e", "A wall of earth.");
    assert_says(&mut g, "d", "too dark");
    assert_says(&mut g, "w", "You can't go that way.");
    assert_says(&mut g, "u", "Hall");
}

// -------------------------------------------------------------- choices

#[test]
fn dialogue_options_appear_as_conditions_change() {
    let mut g = game(lighthouse());
    say(&mut g, "search pool");
    say(&mut g, "n");
    let out = g.input("talk to keeper");
    assert_eq!(out.options, vec!["Ask about the gate", "Say goodbye"]);
    assert_says(&mut g, "5", "Please choose one of the numbered options.");
    assert_says(&mut g, "goodbye", "");
    say(&mut g, "unlock gate");
    say(&mut g, "open gate");
    let out = g.input("talk to keeper");
    assert_eq!(out.options, vec!["Ask about the gate", "Ask about the lamp", "Say goodbye"]);
    assert_says(&mut g, "lamp", "a can by the stairs");
    assert_eq!(g.world().state.vars["keeper_trust"], eifgb_engine::ir::Value::Int(1));
}

#[test]
fn undo_works_inside_a_choice() {
    let mut g = game(lighthouse());
    say(&mut g, "search pool");
    say(&mut g, "n");
    say(&mut g, "talk to keeper");
    say(&mut g, "1");
    assert_eq!(g.world().state.vars["keeper_trust"], eifgb_engine::ir::Value::Int(1));
    let out = g.input("undo");
    assert_eq!(g.world().state.vars["keeper_trust"], eifgb_engine::ir::Value::Int(0));
    assert!(!out.options.is_empty(), "the choice is still open after undo: {}", out.text());
}

// --------------------------------------------------------- meta commands

#[test]
fn save_and_restore_through_the_host() {
    let mut g = game(lighthouse());
    say(&mut g, "search pool");
    let out = g.input("save");
    let data = out.save.expect("SAVE hands the host a save");
    say(&mut g, "drop key");
    say(&mut g, "n");
    let out = g.input("restore");
    assert!(out.restore_requested);
    let out = g.restore(&data);
    assert!(out.text().contains("Rocky Shore"), "{}", out.text());
    assert_says(&mut g, "i", "brass key");
    assert_says(&mut g, "z", "Time passes.");

    let out = g.restore("{}");
    assert!(out.text().contains("couldn't be restored"), "{}", out.text());
}

#[test]
fn again_repeats_and_meta_commands_take_no_time() {
    let mut g = game(lighthouse());
    say(&mut g, "wait");
    say(&mut g, "again");
    assert_eq!(g.world().state.turn, 2);
    say(&mut g, "score");
    say(&mut g, "help");
    assert_eq!(g.world().state.turn, 2);
}

#[test]
fn restart_begins_again() {
    let mut g = game(lighthouse());
    say(&mut g, "search pool");
    let out = g.input("restart");
    assert_eq!(out.lines[0].kind, LineKind::Banner);
    assert_eq!(g.world().state.score, 0);
}

// ------------------------------------------------------- script failures

#[test]
fn script_errors_are_shown_and_play_continues() {
    let mut g = small("objects:\n  bell: { name: bell, in: hall, on: { examine: 'nothing_here()' } }\n");
    let out = g.input("x bell");
    assert!(out.lines.iter().any(|l| l.kind == LineKind::Error && l.text.contains("objects.bell.on.examine")), "{}", out.text());
    assert_says(&mut g, "n", "Yard");
}

#[test]
fn before_handlers_can_block_and_after_handlers_follow_success() {
    let mut g = small(
        "objects:\n  cake: { name: cake, in: hall, traits: [portable, edible], on: { before_eat: 'if not vars.hungry then return block(\"You are not hungry.\") end', after_eat: 'say(\"Delicious.\")' } }\nvars: { hungry: false }\n",
    );
    assert_says(&mut g, "eat cake", "You are not hungry.");
    let mut g2 = small(
        "objects:\n  cake: { name: cake, in: hall, traits: [portable, edible], on: { after_eat: 'say(\"Delicious.\")' } }\n",
    );
    let out = say(&mut g2, "eat cake");
    assert!(out.contains("You eat the cake.") && out.contains("Delicious."), "{out}");
    assert_says(&mut g2, "x cake", "You can't see any such thing.");
}

// ------------------------------------------------- automatic state descriptions

const BOX_AND_SCARF: &str = "objects:\n  box: { name: wooden box, in: hall, traits: [container, openable, lockable], state: { locked: true }, unlocks_with: key }\n  key: { name: key, in: hall, traits: [portable] }\n  scarf: { name: scarf, in: hall, traits: [portable, wearable] }\n  hatch: { name: hatch, in: hall, traits: [openable, scenery] }\n";

#[test]
fn states_are_described_when_examining_listing_and_looking() {
    let mut g = small(BOX_AND_SCARF);
    let out = say(&mut g, "look");
    // Nothing's been looked at yet, so nothing is tagged.
    assert!(out.contains("You can see a wooden box, a key and a scarf here."), "{out}");
    // The hatch is scenery, so it isn't listed. Its state gets a sentence
    // instead, but only once the player knows it.
    assert!(!out.contains("hatch"), "{out}");
    say(&mut g, "x hatch");
    assert_says(&mut g, "look", "The hatch is closed.");

    assert_says(&mut g, "x box", "It's locked.");
    assert_says(&mut g, "look", "You can see a wooden box (locked), a key and a scarf here.");
    say(&mut g, "take key");
    say(&mut g, "unlock box");
    assert_says(&mut g, "x box", "It's closed.");
    say(&mut g, "open box");
    assert_says(&mut g, "x box", "It's open.");
    assert_says(&mut g, "look", "a wooden box (open, empty)");

    say(&mut g, "take scarf");
    say(&mut g, "wear scarf");
    assert_says(&mut g, "i", "a scarf (worn)");
    assert_says(&mut g, "x scarf", "You're wearing it.");
}

#[test]
fn objects_without_a_description_show_just_their_state() {
    let mut g = small(BOX_AND_SCARF);
    assert_eq!(say(&mut g, "x hatch"), "It's closed.");
    assert_says(&mut g, "x key", "You see nothing special about the key.");
}

#[test]
fn describe_state_can_be_switched_off() {
    let mut g = small(&format!("settings: {{ describe_state: false }}\n{BOX_AND_SCARF}"));
    let out = say(&mut g, "look");
    assert!(out.contains("You can see a wooden box, a key and a scarf here."), "{out}");
    assert!(!out.contains("hatch"), "{out}");
    let out = say(&mut g, "x box");
    assert!(!out.contains("locked"), "{out}");
}

#[test]
fn hand_written_state_text_is_not_repeated() {
    let mut g = small(
        "objects:\n  door: { name: door, in: hall, traits: [openable, scenery], description: 'A door, ${if self.open}wide open${else}shut tight${end}.' }\n",
    );
    assert_eq!(say(&mut g, "x door"), "A door, shut tight.");
    // The room description doesn't mention the door, so LOOK still does.
    assert_says(&mut g, "look", "The door is closed.");
}

// ------------------------------------------------------- same-direction exits

#[test]
fn conditional_routes_take_the_first_open_one() {
    let src = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
vars: { bridge_down: false }
rooms:
  hall:
    name: Hall
    exits:
      north:
        - { to: bridge, when: vars.bridge_down }
        - { blocked: \"The bridge is raised.\" }
  bridge: { name: Bridge, exits: { south: hall } }
verbs:
  lower: { patterns: ['lower bridge'] }
on:
  lower: 'vars.bridge_down = true; say(\"The bridge thuds down.\")'
";
    let out = compile_str(src);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    let mut g = game(out.story.unwrap());
    assert_says(&mut g, "n", "The bridge is raised.");
    say(&mut g, "lower bridge");
    assert_says(&mut g, "n", "Bridge");
}

const TWO_DOORS: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
rooms:
  hall:
    name: Hall
    exits:
      north:
        - { to: library, through: red_door }
        - { to: study, through: blue_door }
  library: { name: Library, exits: { south: hall } }
  study: { name: Study, exits: { south: hall } }
objects:
  red_door: { name: red door, in: hall, traits: [door, openable, scenery] }
  blue_door: { name: blue door, in: hall, traits: [door, openable, scenery] }
  statue: { name: statue, in: hall }
";

#[test]
fn separate_passages_ask_which_door() {
    let out = compile_str(TWO_DOORS);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    let mut g = game(out.story.unwrap());
    assert_says(&mut g, "n", "Which do you mean, the red door or the blue door?");
    assert_eq!(g.world().state.turn, 0, "asking which door doesn't take a turn");
    let out = say(&mut g, "blue");
    assert!(out.contains("(first opening the blue door)") && out.contains("Study"), "{out}");
    say(&mut g, "s");
    assert_says(&mut g, "go through the red door", "Library");
    say(&mut g, "s");
    assert_says(&mut g, "enter red door", "Library");
    say(&mut g, "s");
    assert_says(&mut g, "go through statue", "That's not something you can get into or go through.");
}

#[test]
fn only_doors_or_conditions_can_tell_same_direction_exits_apart() {
    let src = TWO_DOORS.replace(", through: red_door", "").replace(", through: blue_door", "");
    let out = compile_str(&src);
    assert!(out.story.is_some());
    assert!(
        out.diagnostics.iter().any(|d| d.message.contains("several exits lead this way")),
        "{:#?}",
        out.diagnostics
    );
}

// ------------------------------------------------- author-defined states

const GADGETS: &str = "objects:\n  lamp: { name: lamp, in: hall, traits: [portable], description: A brass lamp., state: { is_lit: false, fuel: 3, _uses: 7 } }\n  radio: { name: radio, in: hall, traits: [scenery], state: { broken: true, channel: \"sixteen\" } }\nverbs:\n  light: { patterns: ['light {target}'] }\non:\n  light: 'args.target.is_lit = true; args.target.fuel = args.target.fuel - 1'\n";

#[test]
fn author_states_are_described_from_their_names() {
    let mut g = small(GADGETS);
    // Off flags and values are described when examining, private (`_`) fields never.
    assert_eq!(say(&mut g, "x lamp"), "A brass lamp. Its fuel is 3. It isn't lit.");
    // Lists only tag flags that are on, so the lamp has no tag yet. The radio is
    // scenery, so the room description mentions its "on" flag instead.
    let out = say(&mut g, "look");
    assert!(!out.contains("radio"), "{out}");
    assert!(out.contains("You can see a lamp here."), "{out}");

    say(&mut g, "light lamp");
    assert_eq!(say(&mut g, "x lamp"), "A brass lamp. Its fuel is 2. It's lit.");
    assert_says(&mut g, "look", "You can see a lamp (lit) here.");
    assert_eq!(say(&mut g, "x radio"), "It's broken. Its channel is sixteen.");
    assert_says(&mut g, "look", "The radio is broken.");
}

#[test]
fn fields_added_by_scripts_are_described_too() {
    let mut g = small(
        "objects:\n  box: { name: box, in: hall }\nverbs:\n  kick: { patterns: ['kick {target}'] }\non:\n  kick: 'args.target.dented = true'\n",
    );
    assert_eq!(say(&mut g, "x box"), "You see nothing special about the box.");
    say(&mut g, "kick box");
    assert_eq!(say(&mut g, "x box"), "It's dented.");
    assert_says(&mut g, "look", "a box (dented)");
}

#[test]
fn author_states_written_into_the_description_are_not_repeated() {
    let mut g = small(
        "objects:\n  jar: { name: jar, in: hall, state: { sealed: true, opened: false }, description: 'A jar, ${if self.sealed}sealed tight${else}unsealed${end}.' }\n",
    );
    // `sealed` is in the description. `opened` isn't, and `self.sealed` doesn't count
    // as mentioning a field called `seal`, or `open` as mentioning `opened`.
    assert_eq!(say(&mut g, "x jar"), "A jar, sealed tight. It isn't opened.");
}

#[test]
fn describe_state_off_also_silences_author_states() {
    let mut g = small(&format!("settings: {{ describe_state: false }}\n{GADGETS}"));
    assert_eq!(say(&mut g, "x lamp"), "A brass lamp.");
    let out = say(&mut g, "look");
    assert!(!out.contains("broken"), "{out}");
}

// ------------------------------------------------------- two-way doors

fn midnight_snack() -> Game<LuaHost> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/midnight_snack");
    let out = compile(&DirFiles { root: dir }, "game.yaml");
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

#[test]
fn doors_work_from_the_far_side() {
    let mut g = midnight_snack();
    say(&mut g, "look under mat");
    say(&mut g, "take key");
    say(&mut g, "unlock door");
    say(&mut g, "n");
    // The door is reachable from the pantry, even though its `in:` is the kitchen.
    assert_says(&mut g, "close door", "You close the pantry door.");
    assert_says(&mut g, "lock door", "You lock the pantry door with the small key.");
    assert_says(&mut g, "s", "The pantry door is locked.");
    assert_says(&mut g, "unlock door", "You unlock");
    let out = say(&mut g, "s");
    assert!(out.contains("(first opening the pantry door)") && out.contains("Kitchen"), "{out}");
}

#[test]
fn the_pie_is_too_hot_until_its_timer_runs_out() {
    let mut g = midnight_snack();
    assert_says(&mut g, "take pie", "far too hot");
    assert_says(&mut g, "eat pie", "burn your tongue");
    for _ in 0..3 {
        say(&mut g, "wait");
    }
    // Six actions in: the timer fires at the end of this one.
    assert_says(&mut g, "x pie", "must have cooled");
    assert_says(&mut g, "x pie", "It isn't steaming.");
    assert_says(&mut g, "take pie", "Taken.");
    assert_says(&mut g, "eat pie", "Nothing could be better.");
}

const TRAPDOOR: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
rooms:
  hall: { name: Hall, exits: { down: { to: cellar, through: trapdoor } } }
  cellar: { name: Cellar }
objects:
  trapdoor: { name: trapdoor, in: hall, traits: [door, openable], state: { blocked: BLOCKED } }
  bolt: { name: bolt, in: hall, traits: [scenery], on: { push: 'world.trapdoor.blocked = \"cellar\"; say(\"The bolt slides home.\")' } }
  lever: { name: lever, in: cellar, traits: [scenery], on: { pull: 'world.trapdoor.blocked = false; say(\"Clunk.\")' } }
verbs:
  pull: { patterns: ['pull {target}'] }
  push: { patterns: ['push {target}'] }
";

fn trapdoor(blocked: &str) -> Game<LuaHost> {
    let out = compile_str(&TRAPDOOR.replace("BLOCKED", blocked));
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

#[test]
fn opening_from_the_unblocked_side_lifts_the_block() {
    let mut g = trapdoor("cellar");
    assert_says(&mut g, "open trapdoor", "You open the trapdoor.");
    say(&mut g, "d");
    assert_says(&mut g, "u", "Hall");
}

#[test]
fn blocked_both_ways_wont_open_from_either_side() {
    let mut g = trapdoor("true");
    assert_says(&mut g, "open trapdoor", "The trapdoor won't open.");
    assert_says(&mut g, "d", "The trapdoor won't open.");
    assert_says(&mut g, "x trapdoor", "It's closed. It's blocked.");
}

#[test]
fn bolting_an_open_door_closes_it_and_the_bolted_side_can_still_open_it() {
    let mut g = trapdoor("false");
    say(&mut g, "d");
    let id = g.world().id("trapdoor").unwrap();
    // A script bolts it from the hall side while it's open below.
    g.input("u");
    say(&mut g, "push bolt");
    assert!(!g.world().flag(id, "open"), "blocking an open door closes it");
    // Walking through from the hall lifts the bolt again.
    let out = say(&mut g, "d");
    assert!(out.contains("(first opening the trapdoor)") && out.contains("Cellar"), "{out}");
    assert!(!g.world().blocked_anywhere(id));
}

#[test]
fn from_below_a_bolted_trapdoor_says_so() {
    let src = TRAPDOOR.replace("BLOCKED", "cellar").replace("start: hall", "start: cellar");
    let out = compile_str(&src);
    let mut g = game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)));
    assert_says(&mut g, "u", "The trapdoor won't open from this side.");
    assert_says(&mut g, "open trapdoor", "The trapdoor won't open from this side.");
    assert_says(&mut g, "x trapdoor", "It's closed. It's blocked from this side.");
    assert_says(&mut g, "look", "a trapdoor (closed, blocked from this side)");
    assert_says(&mut g, "pull lever", "Clunk.");
    let out = say(&mut g, "u");
    assert!(out.contains("(first opening the trapdoor)") && out.contains("Hall"), "{out}");
}

#[test]
fn a_door_cannot_start_open_and_blocked() {
    let out = compile_str(&TRAPDOOR.replace("blocked: BLOCKED", "open: true, blocked: true"));
    assert!(out.story.is_none());
    assert!(out.diagnostics.iter().any(|d| d.message.contains("held shut")), "{:#?}", out.diagnostics);
}

#[test]
fn blocked_is_saved_and_undone_like_any_state() {
    let src = TRAPDOOR.replace("BLOCKED", "cellar").replace("start: hall", "start: cellar");
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "pull lever");
    say(&mut g, "undo");
    assert_says(&mut g, "u", "won't open from this side");
}

// ------------------------------------------------------- revealed_by

const SECRETS: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: study }
rooms:
  study:
    name: Study
    exits:
      east: { to: vault, through: panel }
  vault: { name: Vault }
objects:
  desk: { name: desk, in: study, traits: [scenery, container, openable] }
  painting: { name: painting, in: study, traits: [scenery], description: A gloomy oil painting. }
  rug: { name: rug, in: study, traits: [scenery] }
  note: { name: note, in: desk, traits: [portable], state: { hidden: true }, revealed_by: open desk }
  coin: { name: coin, in: study, traits: [portable], state: { hidden: true }, revealed_by: [search rug, examine rug] }
  panel: { name: sliding panel, synonyms: [panel], in: study, traits: [door, openable], state: { hidden: true }, revealed_by: examine painting }
  hatch: { name: hatch, in: study, traits: [portable], state: { hidden: true }, revealed_by: kick rug }
verbs:
  kick: { patterns: ['kick {target}'] }
";

fn secrets() -> Game<LuaHost> {
    let out = compile_str(SECRETS);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    game(out.story.unwrap())
}

#[test]
fn searching_reveals_instead_of_finding_nothing() {
    let mut g = secrets();
    assert_says(&mut g, "take coin", "You can't see any such thing.");
    assert_eq!(say(&mut g, "look under the rug"), "You find a coin.");
    assert_says(&mut g, "take coin", "Taken.");
    // Once found, the rule is spent and SEARCH behaves normally again.
    assert_eq!(say(&mut g, "search rug"), "You find nothing of interest.");
}

#[test]
fn other_actions_reveal_after_their_usual_reply() {
    let mut g = secrets();
    let out = say(&mut g, "open desk");
    assert_eq!(out, "You open the desk. You find a note.");
    let out = say(&mut g, "x painting");
    assert_eq!(out, "A gloomy oil painting. You find a sliding panel.");
}

#[test]
fn custom_verbs_without_handlers_reveal_instead_of_nothing_happening() {
    let mut g = secrets();
    assert_eq!(say(&mut g, "kick rug"), "You find a hatch.");
    // Once found, KICK goes back to its usual reply.
    assert_eq!(say(&mut g, "kick rug"), "Kicking that won't help.");
}

#[test]
fn any_rule_in_a_list_reveals() {
    let mut g = secrets();
    assert_says(&mut g, "x rug", "You find a coin.");
}

#[test]
fn secret_doors_and_their_exits_appear_when_found() {
    let mut g = secrets();
    let out = say(&mut g, "look");
    assert!(out.contains("There's no obvious way out.") && !out.contains("panel"), "{out}");
    assert_says(&mut g, "e", "You can't go that way.");
    assert_says(&mut g, "open panel", "You can't see any such thing.");
    say(&mut g, "x painting");
    assert_says(&mut g, "look", "Exits: east.");
    let out = say(&mut g, "e");
    assert!(out.contains("(first opening the sliding panel)") && out.contains("Vault"), "{out}");
    // The return exit is linked through the same panel.
    assert_says(&mut g, "look", "Exits: west.");
}

#[test]
fn undo_hides_it_again() {
    let mut g = secrets();
    say(&mut g, "search rug");
    say(&mut g, "undo");
    assert_says(&mut g, "take coin", "You can't see any such thing.");
}

// -------------------------------------------- categories and one-sided secrets

const CATEGORY_SECRETS: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: study }
rooms:
  study: { name: Study }
objects:
  rug: { name: rug, in: study, traits: [scenery] }
  candle: { name: candle, in: study, traits: [scenery] }
  coin: { name: coin, in: study, traits: [portable], state: { hidden: true }, revealed_by: (physical action) rug }
  scroll: { name: scroll, in: study, traits: [portable], state: { hidden: true }, revealed_by: (sense) candle }
";

fn category_game() -> Game<LuaHost> {
    let out = compile_str(CATEGORY_SECRETS);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    game(out.story.unwrap())
}

#[test]
fn any_verb_in_a_category_reveals() {
    for attempt in ["kick rug", "drag the rug", "punch rug", "pull rug", "lift rug", "shove rug", "rub rug"] {
        let mut g = category_game();
        assert_eq!(say(&mut g, attempt), "You find a coin.", "{attempt}");
    }
    // Verbs outside the category don't.
    let mut g = category_game();
    assert_says(&mut g, "x rug", "You see nothing special about the rug.");
    assert_says(&mut g, "take coin", "You can't see any such thing.");
    assert_eq!(say(&mut g, "sniff candle"), "You find a scroll.");
}

#[test]
fn the_new_verbs_have_sensible_defaults() {
    let mut g = category_game();
    assert_eq!(say(&mut g, "kick candle"), "Kicking that won't help.");
    assert_eq!(say(&mut g, "push candle"), "Nothing obvious happens.");
    assert_eq!(say(&mut g, "listen"), "You hear nothing unusual.");
    assert_eq!(say(&mut g, "taste rug"), "You'd rather not.");
}

const BOOKCASE: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: library }
rooms:
  library: { name: Library, exits: { north: { to: den, through: bookcase } } }
  den: { name: Secret Den }
objects:
  bookcase: { name: bookcase door, synonyms: [bookcase], in: den, traits: [door, openable], state: { hidden: library }, revealed_by: (physical action) books }
  books: { name: books, in: library, traits: [scenery] }
";

fn bookcase(start: &str) -> Game<LuaHost> {
    let out = compile_str(&BOOKCASE.replace("start: library", &format!("start: {start}")));
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    game(out.story.unwrap())
}

#[test]
fn a_door_hidden_from_one_side_is_plain_from_the_other() {
    let mut g = bookcase("library");
    let out = say(&mut g, "look");
    assert!(out.contains("There's no obvious way out.") && !out.contains("bookcase"), "{out}");
    assert_says(&mut g, "n", "You can't go that way.");

    let mut g = bookcase("den");
    let out = say(&mut g, "look");
    assert!(out.contains("You can see a bookcase door here.") && out.contains("Exits: south."), "{out}");
    // Coming through from the plain side reveals it on the hidden side too.
    let out = say(&mut g, "s");
    assert!(out.contains("Library") && out.contains("Exits: north."), "{out}");
    assert_says(&mut g, "x bookcase", "It's open.");
}

#[test]
fn revealing_a_one_sided_secret_door() {
    let mut g = bookcase("library");
    assert_eq!(say(&mut g, "pull books"), "You find a bookcase door.");
    assert_says(&mut g, "n", "Secret Den");
}

#[test]
fn turning_things_and_entering_doors_dont_fire_the_same_named_events() {
    let src = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
vars: { ticks: 0, arrivals: 0 }
rooms:
  hall: { name: Hall, exits: { north: { to: den, through: door } } }
  den: { name: Den, on: { enter: 'vars.arrivals = vars.arrivals + 1' } }
objects:
  dial: { name: dial, in: hall, traits: [scenery] }
  door: { name: door, in: hall, traits: [door, openable] }
on:
  turn: 'vars.ticks = vars.ticks + 1'
";
    let out = compile_str(src);
    let mut g = game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)));
    assert_says(&mut g, "turn dial", "Nothing obvious happens.");
    assert_eq!(g.world().state.vars["ticks"], eifgb_engine::ir::Value::Int(1), "one turn passed, so the event ran once");
    say(&mut g, "enter door");
    assert_eq!(g.world().state.vars["arrivals"], eifgb_engine::ir::Value::Int(1), "the den's enter event ran once");
}

// ------------------------------------------------------------ containers

const CONTAINERS: &str = "\
format: eifgb/0.1
game: { id: c, title: C, version: '1', start: hall }
rooms:
  hall: { name: Hall, exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
objects:
  table: { name: table, in: hall, traits: [supporter, scenery] }
  bag: { name: cloth bag, synonyms: [bag], in: table, traits: [portable, container, openable], capacity: 2 }
  box: { name: small box, synonyms: [box], in: table, traits: [portable, container] }
  ring: { name: ring, in: box, traits: [portable] }
  jar: { name: glass jar, synonyms: [jar], in: hall, traits: [portable, container] }
  marble: { name: marble, in: jar, traits: [portable] }
  coin: { name: coin, in: bag, traits: [portable] }
  pebble: { name: pebble, in: hall, traits: [portable] }
  piano: { name: piano, in: hall, traits: [portable], size: 10 }
  case: { name: glass case, synonyms: [case], in: hall, traits: [container, openable, transparent, scenery] }
  jewel: { name: jewel, in: case, traits: [portable] }
  wardrobe: { name: wardrobe, in: hall, traits: [container, openable, enterable, scenery], size: 5 }
  bed: { name: bed, in: hall, traits: [supporter, enterable, scenery] }
";

fn containers() -> Game<LuaHost> {
    let out = compile_str(CONTAINERS);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    game(out.story.unwrap())
}

#[test]
fn take_from_checks_where_the_thing_is() {
    let mut g = containers();
    say(&mut g, "open bag");
    assert_says(&mut g, "take coin from jar", "The coin isn't in the glass jar.");
    assert_says(&mut g, "take bag from table", "Taken.");
    // Things in a box on the table count as being on the table.
    assert_says(&mut g, "take marble from table", "The marble isn't on the table.");
    assert_says(&mut g, "take ring from table", "Taken.");
    assert_says(&mut g, "take coin from bag", "Taken.");
}

#[test]
fn inventory_shows_what_is_in_carried_containers() {
    let mut g = containers();
    say(&mut g, "open bag");
    say(&mut g, "take bag");
    say(&mut g, "take box");
    assert_eq!(
        say(&mut g, "i"),
        "You're carrying a cloth bag (open) and a small box.\nIn the cloth bag is a coin.\nIn the small box is a ring."
    );
    say(&mut g, "close bag");
    assert_eq!(say(&mut g, "i"), "You're carrying a cloth bag (closed) and a small box.\nIn the small box is a ring.");
    say(&mut g, "take ring");
    say(&mut g, "x box");
    assert_says(&mut g, "i", "You're carrying a cloth bag (closed), a small box (empty) and a ring.");
}

const DINING: &str = "\
format: eifgb/0.1
game: { id: d, title: D, version: '1', start: dining }
rooms:
  dining: { name: Dining Room, description: A simple dining room in a log cabin. }
objects:
  table: { name: simple wooden table, synonyms: [table], in: dining, traits: [supporter, scenery] }
  box: { name: small box, synonyms: [box], in: table, traits: [portable, container, openable], state: { open: true } }
  key: { name: key, in: box, traits: [portable], state: { hidden: true } }
";

#[test]
fn a_container_is_tagged_empty_once_examined() {
    let mut g = game(compile_str(DINING).story.unwrap());
    let out = say(&mut g, "look");
    assert!(out.contains("On the simple wooden table you can see a small box."), "{out}");
    // Hidden things don't count: as far as the player can tell, it's empty.
    assert_eq!(say(&mut g, "x box"), "It's open. It's empty.");
    assert_says(&mut g, "look", "On the simple wooden table you can see a small box (open, empty).");
    // Tags follow the state as it is now.
    say(&mut g, "close box");
    assert_says(&mut g, "look", "a small box (closed).");
}

const CRATE: &str = "\
player: { capacity: 4 }
objects:
  crate: { name: crate, in: hall, traits: [portable, container, open], size: 2 }
  brick: { name: brick, in: crate, traits: [portable], size: 2 }
  feather: { name: feather, in: crate, traits: [portable] }
  sack: { name: sack, in: hall, traits: [portable, container, open] }
  apple: { name: apple, in: sack, traits: [portable] }
";

#[test]
fn contents_add_weight() {
    let mut g = small(CRATE);
    // 2 + 2 + 1 = 5, over the capacity of 4, though the crate alone is size 2.
    assert_says(&mut g, "take crate", "The crate is too heavy to lift with everything in it.");
    assert_says(&mut g, "look", "You can see a crate and a sack here.");
    assert_says(&mut g, "x crate", "It's too heavy to lift with everything in it.");
    assert_says(&mut g, "look", "a crate (heavy)");
    say(&mut g, "take feather");
    say(&mut g, "take sack");
    // Carried: feather 1 + sack with apple 2 = 3; the crate with its brick weighs 4.
    assert_says(&mut g, "take crate", "You're carrying too much already.");
    say(&mut g, "drop feather");
    say(&mut g, "drop sack");
    assert_says(&mut g, "take crate", "Taken.");
    assert_says(&mut g, "i", "In the crate is a brick.");
}

#[test]
fn look_describes_nested_contents() {
    let mut g = containers();
    let out = say(&mut g, "look");
    assert!(out.contains("On the table you can see a cloth bag and a small box."), "{out}");
    assert!(out.contains("In the small box you can see a ring."), "{out}");
}

#[test]
fn empty_and_all_with_a_second_object() {
    let mut g = containers();
    assert_says(&mut g, "take all from box", "Taken.");
    assert_says(&mut g, "take pebble", "Taken.");
    say(&mut g, "open bag");
    say(&mut g, "take bag");
    // The bag holds 2 and already has the coin, so only one more fits.
    let out = say(&mut g, "put all in bag");
    assert!(out.contains("ring: You put the ring in the cloth bag."), "{out}");
    assert!(out.contains("pebble: There's no more room in the cloth bag."), "{out}");
    assert_says(&mut g, "empty bag", "You tip a ring and a coin out of the cloth bag.");
    assert_says(&mut g, "empty bag", "The cloth bag is already empty.");
    assert_says(&mut g, "empty jar into box", "You tip a marble from the glass jar into the small box.");
    assert_says(&mut g, "empty box onto table", "You tip a marble from the small box onto the table.");
    assert_says(&mut g, "empty pebble", "That's not something you can empty.");
}

#[test]
fn transparent_containers_show_but_dont_let_you_reach() {
    let mut g = containers();
    assert_says(&mut g, "look", "In the glass case you can see a jewel.");
    assert_says(&mut g, "x jewel", "You see nothing special about the jewel.");
    assert_says(&mut g, "take jewel", "You can't reach the jewel: the glass case is closed.");
    say(&mut g, "open case");
    assert_says(&mut g, "take jewel", "Taken.");
}

#[test]
fn getting_in_and_on_things() {
    let mut g = containers();
    let out = say(&mut g, "get in wardrobe");
    assert!(out.contains("(first opening the wardrobe)") && out.contains("You get into the wardrobe."), "{out}");
    assert_says(&mut g, "get in wardrobe", "You're already in the wardrobe.");
    assert_says(&mut g, "look", "Hall (in the wardrobe)");
    say(&mut g, "close wardrobe");
    let out = say(&mut g, "look");
    assert!(out.contains("You're shut inside the wardrobe.") && !out.contains("piano"), "you can't see out: {out}");
    let out = say(&mut g, "get out");
    assert!(out.contains("(first opening the wardrobe)") && out.contains("You get out of the wardrobe."), "{out}");
    assert_says(&mut g, "get out", "You're not in or on anything.");

    assert_says(&mut g, "sit on bed", "You get onto the bed.");
    assert_says(&mut g, "look", "Hall (on the bed)");
    // Moving away gets off first.
    let out = say(&mut g, "n");
    assert!(out.contains("(first getting off the bed)") && out.contains("Yard"), "{out}");
}

#[test]
fn out_gets_you_out_and_climbing_between_things_works() {
    let mut g = containers();
    say(&mut g, "sit on bed");
    assert_says(&mut g, "out", "You get off the bed.");
    say(&mut g, "sit on bed");
    let out = say(&mut g, "get in wardrobe");
    assert!(out.contains("(first getting off the bed)") && out.contains("You get into the wardrobe."), "{out}");
}

#[test]
fn sizes_decide_what_fits() {
    let mut g = containers();
    // No capacity on the jar, so the rule is: nothing bigger than the jar itself.
    assert_says(&mut g, "take piano", "Taken.");
    assert_says(&mut g, "put piano in jar", "The piano is too big for the glass jar.");
    let mut g = small(
        "player: { capacity: 3 }\nobjects:\n  crate: { name: crate, in: hall, traits: [portable], size: 2 }\n  sack: { name: sack, in: hall, traits: [portable], size: 2 }\n  boulder: { name: boulder, in: hall, traits: [portable], size: 9 }\n",
    );
    assert_says(&mut g, "take boulder", "The boulder is too big to carry.");
    assert_says(&mut g, "take crate", "Taken.");
    assert_says(&mut g, "take sack", "You're carrying too much already.");
}

#[test]
fn a_zero_capacity_carries_nothing_and_none_carries_anything() {
    let things = "objects:\n  pebble: { name: pebble, in: hall, traits: [portable] }\n  boulder: { name: boulder, in: hall, traits: [portable], size: 9 }\n";
    let mut g = small(&format!("player: {{ capacity: 0 }}\n{things}"));
    assert_says(&mut g, "take pebble", "You can't carry anything.");
    assert_says(&mut g, "take all", "You can't carry anything.");
    for word in ["none", "None", "~"] {
        let mut g = small(&format!("player: {{ capacity: {word} }}\n{things}"));
        assert_says(&mut g, "take boulder", "Taken.");
        assert_says(&mut g, "take pebble", "Taken.");
    }
}

const BACKPACK: &str = "\
player: { capacity: 1 }
objects:
  pack: { name: backpack, synonyms: [pack], in: hall, traits: [portable, wearable], on: { after_wear: 'player.capacity = 3; say(\"Now you can carry more.\")', after_take_off: 'player.capacity = 1' } }
  rock: { name: rock, in: hall, traits: [portable] }
  stick: { name: stick, in: hall, traits: [portable] }
  jar: { name: jar, in: hall, traits: [container], on: { push: 'say(tostring(self.capacity)); self.capacity = 0', pull: 'self.capacity = nil' } }
  oops: { name: oops, in: hall, traits: [scenery], on: { push: 'player.capacity = -2', pull: 'player.capacity = \"lots\"', touch: 'player.capacity = \"none\"; say(tostring(player.capacity))' } }
verbs:
  push: { patterns: ['push {target}'] }
  pull: { patterns: ['pull {target}'] }
  touch: { patterns: ['touch {target}'] }
";

#[test]
fn scripts_can_change_capacity() {
    let mut g = small(BACKPACK);
    assert_says(&mut g, "take pack", "Taken.");
    assert_says(&mut g, "take rock", "You're carrying too much already.");
    assert_says(&mut g, "wear pack", "Now you can carry more.");
    assert_says(&mut g, "take rock", "Taken.");
    assert_says(&mut g, "take stick", "Taken.");
    // Undo puts the old capacity back along with everything else.
    say(&mut g, "undo");
    say(&mut g, "drop rock");
    assert_says(&mut g, "take off pack", "You take off");
    assert_says(&mut g, "take stick", "You're carrying too much already.");

    // Saved and restored.
    say(&mut g, "wear pack");
    let data = g.input("save").save.expect("a save");
    say(&mut g, "take off pack");
    g.restore(&data);
    assert_says(&mut g, "take stick", "Taken.");

    // Containers too; nil goes back to the "no bigger than the holder" rule.
    let mut g = small(BACKPACK);
    assert_says(&mut g, "push jar", "nil");
    assert_says(&mut g, "take rock", "Taken.");
    assert_says(&mut g, "put rock in jar", "The rock is too big for the jar.");
    say(&mut g, "pull jar");
    assert_says(&mut g, "put rock in jar", "You put the rock in the jar.");

    // "none" means no limit; bad values are script errors.
    assert_says(&mut g, "touch oops", "nil");
    assert_says(&mut g, "push oops", "capacity must be a whole number");
    assert_says(&mut g, "pull oops", "capacity must be a whole number");
}

#[test]
fn can_hold_checks_size_and_capacity() {
    let src = "\
player: { capacity: 2 }
objects:
  rock: { name: rock, in: hall, traits: [portable] }
  log: { name: log, in: hall, traits: [portable], size: 2 }
  jar: { name: jar, in: hall, traits: [container, open] }
  probe: { name: probe, in: hall, traits: [scenery], on: { push: 'say(tostring(player:can_hold(world.rock)), \" \", tostring(player:can_hold(world.log)), \" \", tostring(world.jar:can_hold(world.log)))' } }
verbs:
  push: { patterns: ['push {target}'] }
";
    let mut g = small(src);
    assert_says(&mut g, "push probe", "true true false");
    say(&mut g, "take rock");
    // Already carried counts as "can hold"; the log no longer fits.
    assert_says(&mut g, "push probe", "true false false");
}

#[test]
fn custom_verbs_check_reach_too() {
    let src = format!(
        "{CONTAINERS}  label: {{ name: label, in: case, traits: [scenery] }}\nverbs:\n  polish: {{ patterns: ['polish {{target}}'], default: \"It gleams.\" }}\n  read: {{ patterns: ['read {{target}}'], reach: false }}\n"
    )
    .replace("objects:\n", "objects:\n");
    // `verbs:` must come after `objects:` entries; CONTAINERS ends with objects, so append works.
    let out = compile_str(&src);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    let mut g = game(out.story.unwrap());
    // Try-it verbs and the author's own verbs need touch by default.
    assert_says(&mut g, "kick jewel", "You can't reach the jewel: the glass case is closed.");
    assert_says(&mut g, "polish jewel", "You can't reach the jewel: the glass case is closed.");
    // Verbs marked `reach: false` work through glass, as does LISTEN.
    assert_says(&mut g, "read label", "Nothing happens.");
    assert_says(&mut g, "listen to jewel", "You hear nothing unusual.");
    say(&mut g, "open case");
    assert_says(&mut g, "polish jewel", "It gleams.");
}

#[test]
fn handlers_behind_glass_do_not_run_for_verbs_that_need_reach() {
    let src = CONTAINERS.replace(
        "  jewel: { name: jewel, in: case, traits: [portable] }",
        "  jewel: { name: jewel, in: case, traits: [portable], on: { kick: 'say(\"Ow.\")' } }",
    );
    let out = compile_str(&src);
    let mut g = game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)));
    let out = say(&mut g, "kick jewel");
    assert_eq!(out, "You can't reach the jewel: the glass case is closed.");
}

// ---------------------------------------------------------------- timers

#[test]
fn lighthouse_timers_warn_then_end_the_game() {
    let mut g = game(lighthouse());
    let mut warned = false;
    for i in 1..=60 {
        let out = g.input("wait");
        if i == 40 {
            warned = out.text().contains("dangerously close");
        }
        assert_eq!(out.ended.is_some(), i == 60, "turn {i}");
    }
    assert!(warned);
}

const FUSE: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
rooms:
  hall: { name: Hall }
objects:
  fuse:
    name: fuse
    in: hall
    traits: [scenery]
    on: { measure: 'say(timer_left(\"fuse\") or \"It is not lit.\")' }
  tap: { name: tap, in: hall, traits: [scenery] }
verbs:
  light: { patterns: ['light {target}'], default: 'The fuse sputters into life.' }
  measure: { patterns: ['measure {target}'], reach: false }
endings:
  boom: { kind: loss, text: BOOM. }
on:
  start: 'start_timer(\"bell\")'
timers:
  fuse:
    after: 3
    started_by: light fuse
    stopped_by: (physical action) fuse
    say: The fuse burns down...
    end: boom
  drip:
    every: 2
    started_by: twist tap
    stopped_by: push tap
    say: Drip.
  bell:
    after: 2
    running: false
    say: A bell rings.
";

fn fuse() -> Game<LuaHost> {
    let out = compile_str(FUSE);
    assert!(out.diagnostics.is_empty(), "{:#?}", out.diagnostics);
    game(out.story.unwrap())
}

#[test]
fn a_started_timer_counts_the_actions_after_the_one_that_started_it() {
    let mut g = fuse();
    say(&mut g, "light fuse");
    assert!(!say(&mut g, "wait").contains("BOOM"));
    assert!(!say(&mut g, "wait").contains("BOOM"));
    let out = g.input("wait");
    assert!(out.text().contains("The fuse burns down...") && out.text().contains("BOOM."), "{}", out.text());
    assert_eq!(out.ended, Some(EndingKind::Loss));
}

#[test]
fn stopped_by_cancels_and_categories_work() {
    let mut g = fuse();
    say(&mut g, "light fuse");
    say(&mut g, "kick fuse");
    for _ in 0..5 {
        assert!(!say(&mut g, "wait").contains("BOOM"));
    }
}

#[test]
fn repeating_timers_and_scripted_starts() {
    let mut g = fuse();
    // The bell was started by the `start` handler: it rings after 2 actions.
    assert!(!say(&mut g, "wait").contains("bell"));
    assert_says(&mut g, "wait", "A bell rings.");
    say(&mut g, "turn tap");
    let outs: Vec<String> = (0..4).map(|_| say(&mut g, "wait")).collect();
    let drips = outs.iter().filter(|o| o.contains("Drip.")).count();
    assert_eq!(drips, 2, "every 2 actions: {outs:?}");
    say(&mut g, "push tap");
    for _ in 0..4 {
        assert!(!say(&mut g, "wait").contains("Drip."));
    }
}

#[test]
fn timer_left_counts_down_and_undo_rewinds_timers() {
    let mut g = fuse();
    assert_eq!(say(&mut g, "measure fuse"), "It is not lit.");
    say(&mut g, "light fuse");
    // Three actions after lighting: this MEASURE is the first, so three are
    // left counting it. It fires at the end of the second WAIT after this.
    assert_eq!(say(&mut g, "measure fuse"), "3");
    say(&mut g, "wait");
    assert_says(&mut g, "wait", "BOOM.");
    // Undo the two waits, the measure and the lighting.
    for _ in 0..4 {
        say(&mut g, "undo");
    }
    // (This is the game's second action, so the bell rings as well.)
    assert_says(&mut g, "measure fuse", "It is not lit.");
}

#[test]
fn several_room_states_of_one_thing_make_one_sentence() {
    let mut g = small("objects:\n  cupboard: { name: cupboard, in: hall, traits: [container, openable, scenery] }\n");
    assert!(!say(&mut g, "look").contains("cupboard"));
    say(&mut g, "open cupboard");
    let out = say(&mut g, "look");
    assert!(out.contains("The cupboard is open and empty."), "{out}");
}

// ------------------------------------------------- words from descriptions

const SHORE: &str = "\
format: eifgb/0.1
game: { id: s, title: S, version: '1', start: shore }
rooms:
  shore: { name: Shore, description: Waves break against black rocks. A narrow path leads north., exits: { north: cliff } }
  cliff: { name: Cliff, description: A windswept cliff top with a lonely bench., exits: { south: shore } }
objects:
  pool: { name: tide pool, synonyms: [pool], in: shore, traits: [scenery, container, open], description: 'Cold water.${if world.key.hidden} Something glints at the bottom.${end}' }
  key: { name: brass key, synonyms: [key], in: pool, traits: [portable], state: { hidden: true }, revealed_by: search pool }
  shell: { name: shell, in: shore, traits: [portable], description: A spiral shell with something rattling inside. }
  crab: { name: crab, in: shore, traits: [scenery], description: A crab guards the narrow path. }
";

#[test]
fn description_words_refer_to_what_they_describe() {
    let mut g = game(compile_str(SHORE).story.unwrap());
    // Another form of a description word finds the thing it describes.
    assert_says(&mut g, "x glinting", "Something glints at the bottom.");
    assert_says(&mut g, "search the glint", "You find a brass key.");
    // Once the key is found and taken, the pool really is empty.
    say(&mut g, "take key");
    assert_says(&mut g, "x pool", "It's empty.");
    // A word in two descriptions asks which.
    assert_says(&mut g, "x something", "Which do you mean");
    say(&mut g, "shell");
    // Words only in the room description.
    assert_says(&mut g, "x waves", "That's just part of the scenery.");
    assert_says(&mut g, "take black rocks", "That's just part of the scenery.");
    // "path" is in the crab's description too, so the crab wins over the room.
    assert_says(&mut g, "x path", "A crab guards");
    // A description word from another room: known, but not here.
    assert_says(&mut g, "x bench", "You can't see any such thing.");
    assert_says(&mut g, "x zorkmid", "I don't know the word \"zorkmid\".");
    // Stopwords and directions never match.
    assert_says(&mut g, "x against", "I don't know the word");
    assert_says(&mut g, "x north", "I don't know the word");
}

#[test]
fn a_hint_at_something_hidden_suppresses_empty() {
    let mut g = game(compile_str(SHORE).story.unwrap());
    assert_eq!(say(&mut g, "x pool"), "Cold water. Something glints at the bottom.");
}

#[test]
fn real_names_beat_description_words() {
    let src = SHORE.replace("  crab:", "  rocks: { name: black rocks, synonyms: [rocks], in: shore, traits: [scenery], description: Slick with spray. }\n  crab:");
    let mut g = game(compile_str(&src).story.unwrap());
    assert_says(&mut g, "x rocks", "Slick with spray.");
}

// ------------------------------------------------------- characters acting

const PEOPLE: &str = "\
format: eifgb/0.1
game: { id: p, title: P, version: '1', start: hall }
player: { health: 30, money: 20 }
rooms:
  hall: { name: Hall, exits: { north: yard, east: shop } }
  yard: { name: Yard, exits: { south: hall, north: woods } }
  woods: { name: Woods, exits: { south: yard } }
  shop: { name: Shop, exits: { west: hall } }
objects:
  sword: { name: sword, in: hall, traits: [portable], damage: 4 }
  lamp: { name: brass lamp, synonyms: [lamp], in: peddler, traits: [portable], price: 8, description: A shiny lamp. }
  rope: { name: rope, in: peddler, traits: [portable], price: 30 }
characters:
  wolf: { name: grey wolf, synonyms: [wolf], in: woods, behavior: hunter, health: 8, damage: 3, wander: 0 }
  peddler: { name: peddler, in: shop, behavior: merchant, wander: 0, money: 5 }
";

fn people(extra: &str) -> Game<LuaHost> {
    let out = compile_str(&format!("{PEOPLE}{extra}"));
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

#[test]
fn a_hunter_seeks_out_the_player_and_attacks() {
    let mut g = people("");
    // Two rooms away: one step per turn, out of sight at first.
    assert!(!say(&mut g, "take sword").contains("wolf"));
    assert_says(&mut g, "z", "The grey wolf arrives.");
    assert_says(&mut g, "z", "The grey wolf attacks you.");
    // HEALTH is free, so the wolf doesn't get another bite.
    assert_eq!(say(&mut g, "health"), "You're hurt: 27 out of 30.");
    assert_eq!(say(&mut g, "health"), "You're hurt: 27 out of 30.");
}

#[test]
fn fighting_wounds_and_defeats() {
    let mut g = people("");
    say(&mut g, "take sword");
    say(&mut g, "z");
    let out = say(&mut g, "attack wolf with sword");
    assert!(out.contains("You hit the grey wolf with the sword.") && out.contains("attacks you"), "{out}");
    assert_says(&mut g, "x wolf", "The grey wolf is badly wounded.");
    assert_says(&mut g, "look", "a grey wolf (badly wounded)");
    let out = say(&mut g, "kill wolf with sword");
    // Defeated by that blow, so it doesn't strike back.
    assert_eq!(out, "You hit the grey wolf with the sword.\nThe grey wolf is defeated.");
    // It leaves a body, which "corpse" and "body" refer to.
    assert_says(&mut g, "look", "You can see a grey wolf (dead) here.");
    assert_eq!(say(&mut g, "x corpse"), "The grey wolf is dead. You killed the grey wolf.");
    assert_says(&mut g, "hit the body", "The grey wolf is already dead.");
    assert!(!say(&mut g, "z").contains("attacks"));
    // Things that can't be hurt.
    assert_says(&mut g, "hit sword", "Violence isn't the answer here.");
}

fn weak_player(extra: &str) -> Game<LuaHost> {
    let src = format!("{PEOPLE}{extra}").replace("player: { health: 30, money: 20 }", "player: { health: 3, money: 20 }");
    game(compile_str(&src).story.unwrap())
}

#[test]
fn the_player_can_die() {
    let mut g = weak_player("");
    say(&mut g, "z");
    say(&mut g, "z");
    let out = g.input("z");
    assert!(out.text().contains("You have died."), "{}", out.text());
    assert_eq!(out.ended, Some(EndingKind::Loss));
}

#[test]
fn a_died_handler_can_save_the_player() {
    let mut g = weak_player("on:\n  died: 'player.health = 5; say(\"A guardian spirit pulls you back.\")'\n");
    say(&mut g, "z");
    say(&mut g, "z");
    let out = g.input("z");
    assert!(out.text().contains("A guardian spirit") && out.ended.is_none(), "{}", out.text());
}

#[test]
fn a_defeated_handler_can_replace_the_default() {
    let src = PEOPLE.replace("objects:\n", "objects:\n  pelt: { name: pelt, in: wolf, traits: [portable] }\n");
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "take sword");
    say(&mut g, "z");
    say(&mut g, "hit wolf with sword");
    // Without a handler, it drops what it carried.
    assert_says(&mut g, "hit wolf with sword", "The grey wolf drops a pelt.");

    let src = PEOPLE.replace(
        "damage: 3, wander: 0 }",
        "damage: 3, wander: 0, on: { defeated: 'self.health = 1; self:move_to(world.woods); return block(\"The wolf limps away.\")' } }",
    );
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "take sword");
    say(&mut g, "z");
    say(&mut g, "hit wolf with sword");
    let out = say(&mut g, "hit wolf with sword");
    // Back in the woods with 1 health; being a hunter, it starts back toward the player.
    assert!(out.contains("The wolf limps away.") && !out.contains("defeated"), "{out}");
    assert!(!say(&mut g, "look").contains("You can see a grey wolf"));
}

#[test]
fn merchants_buy_and_sell() {
    let src = PEOPLE.replace("behavior: hunter", "behavior: stationary");
    let mut g = game(compile_str(&src).story.unwrap());
    // The peddler's goods can't be seen from here.
    assert_says(&mut g, "buy lamp", "You can't see any such thing.");
    assert_says(&mut g, "wares", "There's nobody here to buy from.");
    say(&mut g, "e");
    assert_eq!(say(&mut g, "wares"), "The peddler has for sale: a brass lamp (8 coins) and a rope (30 coins).");
    assert_eq!(say(&mut g, "x lamp"), "A shiny lamp.");
    assert_eq!(say(&mut g, "buy lamp"), "You buy the brass lamp from the peddler for 8 coins.");
    assert_eq!(say(&mut g, "i"), "You're carrying a brass lamp.\nYou have 12 coins.");
    assert_eq!(say(&mut g, "buy rope"), "The rope costs 30 coins, and you have only 12 coins.");
    // At half price, and the peddler had 5 + 8. A merchant who haggles
    // names its price first; selling again takes it.
    assert_says(&mut g, "sell lamp", "The peddler offers 4 coins for the brass lamp.");
    assert_eq!(say(&mut g, "sell lamp"), "You sell the brass lamp to the peddler for 4 coins.");
    assert_says(&mut g, "i", "You have 16 coins.");
    say(&mut g, "w");
    say(&mut g, "take sword");
    say(&mut g, "e");
    assert_says(&mut g, "sell sword", "The peddler isn't interested in the sword.");
}

#[test]
fn wanderers_followers_and_guards() {
    let src = "\
format: eifgb/0.1
game: { id: w, title: W, version: '1', start: hall }
player: { health: 10 }
rooms:
  hall: { name: Hall, exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
characters:
  cat: { name: cat, in: hall, behavior: wanderer, wander: 100 }
  dog: { name: dog, in: hall, behavior: follower }
  troll: { name: troll, in: yard, behavior: guard, damage: 2 }
";
    let mut g = game(compile_str(src).story.unwrap());
    // The cat always wanders, and the hall has one way out.
    assert_says(&mut g, "z", "The cat goes north.");
    let out = say(&mut g, "n");
    // The dog follows; the troll attacks, but only here, and doesn't follow.
    assert!(out.contains("The dog arrives.") && out.contains("The troll attacks you."), "{out}");
    let out = say(&mut g, "s");
    assert!(out.contains("The dog arrives.") && !out.contains("troll"), "{out}");
    assert_eq!(say(&mut g, "health"), "You're hurt: 8 out of 10.");
}

#[test]
fn custom_algorithms_in_lua() {
    let src = "\
format: eifgb/0.1
game: { id: a, title: A, version: '1', start: hall }
player: { health: 10 }
rooms:
  hall: { name: Hall, exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
characters:
  parrot:
    name: parrot
    in: yard
    act: |
      if self:is_with(player) then
        say(\"The parrot squawks.\")
      else
        self:step_toward(player)
      end
  ghost:
    name: ghost
    in: yard
    behavior: hunter
    damage: 1
    act: |
      if game.turn % 2 == 0 then self:behave() end
";
    let mut g = game(compile_str(src).story.unwrap());
    let out = say(&mut g, "z");
    assert!(out.contains("The parrot arrives.") && !out.contains("ghost"), "{out}");
    let out = say(&mut g, "z");
    assert!(out.contains("The parrot squawks.") && out.contains("The ghost arrives."), "{out}");
    // The ghost only acts on even turns.
    assert!(!say(&mut g, "z").contains("ghost attacks"));
    assert_says(&mut g, "z", "The ghost attacks you.");
}

#[test]
fn characters_respect_doors_and_areas() {
    let src = "\
format: eifgb/0.1
game: { id: d, title: D, version: '1', start: hall }
rooms:
  hall: { name: Hall, exits: { north: { to: vault, through: gate }, east: garden } }
  vault: { name: Vault }
  garden: { name: Garden }
objects:
  gate: { name: gate, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: key }
  key: { name: key, in: hall, traits: [portable] }
characters:
  mouse: { name: mouse, in: hall, behavior: wanderer, wander: 100, area: [hall, vault] }
";
    let mut g = game(compile_str(src).story.unwrap());
    // Locked gate, and the garden is outside its area: it can't go anywhere.
    assert!(!say(&mut g, "z").contains("mouse"));
    say(&mut g, "take key");
    // Unlocked but closed: characters slip through without opening it.
    assert_says(&mut g, "unlock gate", "The mouse goes north.");
    assert_says(&mut g, "x gate", "It's closed.");
}

// ---------------------------------------------------------------- factions

const FACTIONS: &str = "\
format: eifgb/0.1
game: { id: f, title: F, version: '1', start: south_room }
player: { health: 50 }
factions:
  cats: { hostile_to: [rats, player], eats_corpses: true }
  rats: { hostile_to: [cats] }
  ghosts: { hostile_to: [player] }
rooms:
  south_room: { name: South Room, exits: { north: hall } }
  hall: { name: Hall, exits: { south: south_room } }
characters:
  cat: { name: cat, in: hall, faction: cats, behavior: hunter, wander: 0, health: 9, damage: 3 }
  rat: { name: rat, in: hall, faction: rats, behavior: guard, health: 3, damage: 1 }
  rat2: { name: brown rat, synonyms: [rat], in: hall, faction: rats, behavior: guard, health: 3, damage: 1 }
  ghost: { name: ghost, in: hall, faction: ghosts, behavior: guard, health: 3, corpse: false }
";

#[test]
fn factions_fight_each_other_and_the_hungry_eat_the_fallen() {
    let mut g = game(compile_str(FACTIONS).story.unwrap());
    // Out of sight but next door: the fight can be heard.
    assert_says(&mut g, "z", "You hear snarls and the clash of a fight somewhere north.");
    say(&mut g, "z");
    // Both rats dead; the cat eats one body a turn.
    assert_says(&mut g, "z", "You hear tearing and crunching somewhere north.");
    let out = say(&mut g, "n");
    // One body left, plus the cat and the ghost, who both go for the player.
    // The dead are plain to see; the cat's wounds aren't tagged until the player looks.
    assert!(out.contains("You can see a cat, a brown rat (dead) and a ghost here."), "{out}");
    assert!(out.contains("The cat attacks you.") && out.contains("The ghost attacks you."), "{out}");
}

#[test]
fn the_player_comes_first_when_everyone_is_in_the_room() {
    let src = FACTIONS.replace("start: south_room", "start: hall");
    let mut g = game(compile_str(&src).story.unwrap());
    let out = say(&mut g, "z");
    // The cat could fight the rats, but goes for the player. The rats don't hate the player.
    assert!(out.contains("The cat attacks you.") && out.contains("The ghost attacks you."), "{out}");
    assert!(out.contains("The rat attacks the cat.") && out.contains("The brown rat attacks the cat."), "{out}");
}

#[test]
fn bodies_can_vanish_and_handlers_can_reword_defeat() {
    let src = FACTIONS
        .replace("start: south_room", "start: hall")
        .replace("corpse: false }", "corpse: false, on: { defeated: 'return block(\"The ghost fades away.\")' } }");
    let mut g = game(compile_str(&src).story.unwrap());
    let out = say(&mut g, "hit ghost");
    let out2 = say(&mut g, "hit ghost");
    let out3 = say(&mut g, "hit ghost");
    let all = format!("{out}\n{out2}\n{out3}");
    assert!(all.contains("The ghost fades away.") && !all.contains("ghost is defeated"), "{all}");
    // corpse: false, so nothing is left, and the handler doesn't run again.
    let out = say(&mut g, "look");
    assert!(!out.contains("ghost"), "{out}");
    assert!(!say(&mut g, "z").contains("fades"));
}

#[test]
fn scripts_can_ask_about_factions_and_eat() {
    let src = FACTIONS.replace(
        "  ghost: { name: ghost",
        "  probe: { name: probe, in: south_room, act: 'if game.turn == 1 then say(tostring(world.cat:is_hostile_to(world.rat2)), \" \", tostring(world.rat:is_hostile_to(player)), \" \", tostring(world.cat:is_hostile_to(player))) end' }\n  ghost: { name: ghost",
    );
    let mut g = game(compile_str(&src).story.unwrap());
    assert_says(&mut g, "z", "true false true");
}

// ------------------------------------------------------------ self-defense

const DEFENDERS: &str = "\
format: eifgb/0.1
game: { id: d, title: D, version: '1', start: hall }
player: { health: 20 }
factions:
  wolves: { hostile_to: [deer] }
  deer: {}
rooms:
  hall: { name: Hall, exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
characters:
  bear: { name: bear, in: hall, behavior: defender, health: 10, damage: 3 }
  trader: { name: trader, in: hall, behavior: merchant, wander: 0, pacifist: false, patience: 0, health: 6, damage: 1 }
  saint: { name: saint, in: hall, health: 9, fights_back: false }
  monk: { name: monk, in: hall, health: 4, on: { pray: 'self.foes = nil; say(\"The monk forgives you.\")' } }
verbs:
  pray: { patterns: ['pray with {target}'], reach: false }
";

#[test]
fn defenders_fight_back_at_once() {
    let mut g = game(compile_str(DEFENDERS).story.unwrap());
    // Nobody starts a fight.
    assert_eq!(say(&mut g, "z"), "Time passes.");
    assert_eq!(say(&mut g, "hit bear"), "You hit the bear.\nThe bear attacks you.");
    assert_says(&mut g, "z", "The bear attacks you.");
    // It doesn't chase, but it remembers.
    say(&mut g, "n");
    assert_eq!(say(&mut g, "z"), "Time passes.");
    assert_says(&mut g, "s", "The bear attacks you.");
    // patience: 0 makes anyone fight back at once, even a merchant who
    // isn't a pacifist.
    assert_says(&mut g, "hit trader", "The trader attacks you.");
}

#[test]
fn everyone_else_fights_back_once_their_patience_runs_out() {
    let mut g = game(compile_str(DEFENDERS).story.unwrap());
    // A plain character puts up with two attacks, and says so.
    assert_eq!(say(&mut g, "hit monk"), "You hit the monk.\nThe monk is losing patience with you.");
    assert_eq!(say(&mut g, "hit monk"), "You hit the monk.\nThe monk is losing patience with you.");
    assert_eq!(say(&mut g, "hit monk"), "You hit the monk.\nThe monk attacks you.");
    // fights_back: false never does.
    let mut g = game(compile_str(DEFENDERS).story.unwrap());
    for _ in 0..4 {
        assert_eq!(say(&mut g, "hit saint"), "You hit the saint.");
    }
}

#[test]
fn foes_are_remembered_by_key_and_can_be_forgiven() {
    let src = DEFENDERS.replace("monk: { name: monk, in: hall, health: 4,", "monk: { name: monk, in: hall, health: 4, patience: 0,");
    let mut g = game(compile_str(&src).story.unwrap());
    assert_says(&mut g, "hit monk", "The monk attacks you.");
    assert_says(&mut g, "z", "The monk attacks you.");
    // Clearing `foes` ends the grudge.
    assert_eq!(say(&mut g, "pray with monk"), "The monk forgives you.");
    assert_eq!(say(&mut g, "z"), "Time passes.");
}

#[test]
fn characters_defend_themselves_from_each_other() {
    let src = DEFENDERS.replace(
        "characters:\n",
        "characters:\n  wolf: { name: wolf, in: yard, faction: wolves, behavior: hunter, wander: 0, health: 30, damage: 1 }\n  deer: { name: deer, in: yard, faction: deer, behavior: defender, health: 30, damage: 1 }\n",
    );
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "n");
    let out = say(&mut g, "z");
    assert!(out.contains("The wolf attacks the deer.") && out.contains("The deer attacks the wolf."), "{out}");
}

// ------------------------------------------------- creative solutions

fn blackfang() -> Game<LuaHost> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/blackfang");
    let out = compile(&DirFiles { root: dir }, "game.yaml");
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

#[test]
fn a_disguise_fools_its_faction_but_not_their_enemies() {
    let mut g = blackfang();
    say(&mut g, "take helmet");
    say(&mut g, "wear helmet");
    say(&mut g, "n");
    let out = say(&mut g, "z");
    assert!(out.contains("The alpha wolf attacks you.") && out.contains("The goblin chief attacks the alpha wolf."), "{out}");
    assert!(!out.contains("goblin chief attacks you") && !out.contains("goblin grunt attacks you"), "{out}");
    // Take it off and you're fair game again.
    let out = say(&mut g, "take off helmet");
    assert!(out.contains("goblin") && out.contains("attacks you"), "{out}");
}

#[test]
fn bait_beats_a_fight_and_lasts_by_size() {
    let mut g = blackfang();
    say(&mut g, "take haunch");
    say(&mut g, "n");
    let out = say(&mut g, "drop haunch");
    assert!(out.contains("The alpha wolf tears into the haunch of venison."), "{out}");
    assert!(!out.contains("wolf attacks you"), "{out}");
    // Size 6: two wolves take three turns to finish it.
    assert!(!say(&mut g, "z").contains("finishes off"));
    assert_says(&mut g, "z", "The young wolf finishes off the haunch of venison.");
}

#[test]
fn hunters_lose_a_hidden_player_and_search_where_they_last_saw_them() {
    let mut g = blackfang();
    say(&mut g, "n");
    say(&mut g, "get in barrel");
    let out = say(&mut g, "close barrel");
    // Hidden: they fight each other instead, in plain view.
    assert!(out.contains("The alpha wolf attacks the goblin chief.") && !out.contains("attacks you"), "{out}");
    let mut seen = String::new();
    for _ in 0..6 {
        seen.push_str(&say(&mut g, "z"));
    }
    assert!(seen.contains("searches all around, but finds nothing."), "{seen}");
    assert!(!seen.contains("attacks you"), "{seen}");
    // Out of the barrel, it's noticed again.
    assert_says(&mut g, "open barrel", "attacks you");
}

#[test]
fn corpses_tell_how_they_died() {
    let mut g = blackfang();
    // Set up in the story: killed by the alpha, whose faction says how.
    assert_eq!(say(&mut g, "x corpse"), "A skinny goblin, not much bigger than a child. The goblin scout is dead. The goblin scout has been torn apart by teeth.");
    say(&mut g, "take spear");
    say(&mut g, "n");
    say(&mut g, "s");
    for _ in 0..5 {
        say(&mut g, "z");
    }
    say(&mut g, "n");
    say(&mut g, "kill young wolf with spear");
    say(&mut g, "kill young wolf with spear");
    // The weapon's words win over the killer's faction.
    assert_says(&mut g, "x young wolf", "The young wolf has been run through.");
}

#[test]
fn plain_kills_name_the_killer() {
    let mut g = people("");
    say(&mut g, "z");
    say(&mut g, "z");
    for _ in 0..8 {
        say(&mut g, "hit wolf");
    }
    assert_says(&mut g, "x wolf", "You killed the grey wolf.");
}

#[test]
fn blackfang_has_more_than_one_way_through() {
    let mut g = blackfang();
    for cmd in [
        "take spear", "take helmet", "wear helmet", "take haunch", "n", "drop haunch", "w", "take key", "e", "n",
        "kill skeleton with spear", "kill skeleton with spear", "unlock gate", "n",
    ] {
        say(&mut g, cmd);
    }
    let out = g.input("take sunstone");
    assert_eq!(out.ended, Some(EndingKind::Win), "{}", out.text());
}

// -------------------------------------------------------- faction attitude

#[test]
fn provoking_a_faction_in_disguise_blows_it() {
    let mut g = blackfang();
    for cmd in ["take spear", "take helmet", "wear helmet", "n"] {
        say(&mut g, cmd);
    }
    // Hunters have no patience, so each goblin hit is a new foe, and the
    // goblins' patience with "one of their own" (2) drops by one.
    let out = say(&mut g, "hit grunt with spear");
    assert!(out.contains("The goblins are growing wary of you.") && out.contains("The goblin chief attacks the alpha wolf."), "{out}");
    let out = say(&mut g, "hit chief with spear");
    assert!(out.contains("The goblins see through your disguise!"), "{out}");
    assert!(out.contains("The goblin chief attacks you"), "{out}");
}

#[test]
fn a_blown_disguise_stays_blown() {
    let src = "\
format: eifgb/0.1
game: { id: g, title: G, version: '1', start: hall }
player: { health: 30 }
factions:
  goblins: { hostile_to: [player], patience: 1 }
rooms:
  hall: { name: Hall }
objects:
  hat: { name: goblin hat, synonyms: [hat], in: player, traits: [wearable], disguise: goblins }
characters:
  goblin: { name: goblin, in: hall, faction: goblins, behavior: guard, health: 50 }
";
    let mut g = game(compile_str(src).story.unwrap());
    assert_says(&mut g, "z", "The goblin attacks you.");
    assert_eq!(say(&mut g, "wear hat"), "You put on the goblin hat.");
    assert_eq!(say(&mut g, "z"), "Time passes.");
    assert_says(&mut g, "hit goblin", "The goblins see through your disguise!");
    say(&mut g, "take off hat");
    assert_says(&mut g, "wear hat", "The goblin attacks you.");
}

const VILLAGE: &str = "\
format: eifgb/0.1
game: { id: v, title: V, version: '1', start: green }
player: { health: 30 }
factions:
  villagers: { patience: 2 }
rooms:
  green: { name: Green }
characters:
  baker: { name: baker, in: green, faction: villagers, health: 9, patience: 0 }
  smith: { name: smith, in: green, faction: villagers, health: 9, patience: 0 }
  miller: { name: miller, in: green, faction: villagers, health: 9 }
";

#[test]
fn a_neutral_faction_turns_on_a_player_who_provokes_enough_of_it() {
    let mut g = game(compile_str(VILLAGE).story.unwrap());
    let out = say(&mut g, "hit baker");
    assert!(out.contains("The villagers are growing wary of you.") && out.contains("The baker attacks you."), "{out}");
    assert!(!out.contains("miller attacks"), "{out}");
    let out = say(&mut g, "hit smith");
    assert!(out.contains("The villagers turn against you."), "{out}");
    // The miller was never touched, but now attacks too.
    assert!(out.contains("The miller attacks you."), "{out}");
}

#[test]
fn pacifists_never_fight_but_their_faction_remembers() {
    let src = VILLAGE.replace(
        "  miller:",
        "  trader: { name: trader, in: green, faction: villagers, behavior: merchant, wander: 0, health: 30 }\n  elder: { name: elder, in: green, faction: villagers, health: 9, pacifist: true }\n  miller:",
    );
    let mut g = game(compile_str(&src).story.unwrap());
    // Hurting a pacifist angers its faction, though it won't fight itself.
    let out = say(&mut g, "hit trader");
    assert!(out.contains("The villagers are growing wary of you.") && !out.contains("trader attacks"), "{out}");
    let out = say(&mut g, "hit baker");
    assert!(out.contains("The villagers turn against you.") && out.contains("The miller attacks you."), "{out}");
    // A merchant, and anyone marked pacifist, keeps out of it, however often hit.
    for _ in 0..4 {
        let out = say(&mut g, "hit trader");
        assert!(!out.contains("trader attacks") && !out.contains("elder attacks"), "{out}");
    }
}

#[test]
fn invincible_characters_shrug_off_any_blow() {
    let src = "\
format: eifgb/0.1
game: { id: i, title: I, version: '1', start: hall }
player: { health: 30 }
rooms:
  hall: { name: Hall }
objects:
  sword: { name: sword, in: player, traits: [portable], damage: 50 }
characters:
  oracle: { name: oracle, in: hall, invincible: true, pacifist: true }
  knight: { name: knight, in: hall, invincible: true, health: 10, patience: 0 }
";
    let mut g = game(compile_str(src).story.unwrap());
    // Even without health, it can be struck, and isn't hurt.
    assert_eq!(say(&mut g, "hit oracle with sword"), "You hit the oracle with the sword.\nThe oracle shrugs off the blow.");
    // With health, it doesn't go down, and can still lose patience.
    let out = say(&mut g, "hit knight with sword");
    assert!(out.contains("The knight shrugs off the blow.") && out.contains("The knight attacks you."), "{out}");
    say(&mut g, "hit knight with sword");
    assert!(!say(&mut g, "x knight").contains("wounded"));
}

// ------------------------------------------------------------------ noise

const TOWN: &str = "\
format: eifgb/0.1
game: { id: t, title: T, version: '1', start: START }
player: { health: 30 }
rooms:
  lair: { name: Lair, exits: { east: tunnel } }
  tunnel: { name: Tunnel, exits: { west: lair, east: square } }
  square: { name: Square, exits: { west: tunnel, east: belfry, south: shop } }
  belfry: { name: Belfry, exits: { west: square } }
  shop: { name: Shop, exits: { north: square } }
objects:
  pot: { name: pot, in: shop, traits: [portable], on: { bang: 'self:make_noise(3); say(\"BONG.\")' } }
characters:
  ghoul: { name: ghoul, in: lair, behavior: hunter, range: 0, wander: 0, health: 30 }
verbs:
  bang: { patterns: ['bang {target}'] }
";

fn town(start: &str, extra: &str) -> Game<LuaHost> {
    let out = compile_str(&format!("{}{extra}", TOWN.replace("START", start)));
    game(out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics)))
}

#[test]
fn shouting_draws_hunters_within_earshot() {
    // The square is two rooms from the lair: in earshot of a shout (voice 2).
    let mut g = town("square", "");
    assert_says(&mut g, "shout", "You shout at the top of your lungs.");
    assert_says(&mut g, "z", "The ghoul arrives.");
    assert_says(&mut g, "z", "The ghoul attacks you.");
    // The belfry is three rooms away: out of earshot.
    let mut g = town("belfry", "");
    say(&mut g, "yell");
    for _ in 0..4 {
        assert!(!say(&mut g, "z").contains("ghoul"));
    }
}

#[test]
fn a_louder_voice_carries_further() {
    let src = TOWN.replace("START", "belfry").replace("player: { health: 30 }", "player: { health: 30, voice: 3 }");
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "shout");
    say(&mut g, "z");
    assert_says(&mut g, "z", "The ghoul arrives.");
}

#[test]
fn a_timer_can_make_a_noise_somewhere() {
    // The bell rings in the belfry while the player waits in the shop.
    let mut g = town("shop", "timers:\n  bell: { after: 1, say: A bell tolls in the belfry., noise: { at: belfry, loudness: 4 } }\n");
    assert_says(&mut g, "z", "A bell tolls in the belfry.");
    // It goes to the belfry, searches, and gives up. The player then walks in.
    for _ in 0..4 {
        say(&mut g, "z");
    }
    let out = say(&mut g, "n");
    assert!(out.contains("Square"), "{out}");
    let out = say(&mut g, "e");
    assert!(out.contains("You can see a ghoul here."), "{out}");
}

#[test]
fn a_noise_follows_the_thing_that_makes_it() {
    // An alarm clock in the player's pocket: the noise is wherever they are.
    let src = format!(
        "{}timers:\n  alarm: {{ after: 1, say: Your alarm clock goes off!, noise: {{ at: clock, loudness: 2 }} }}\n",
        TOWN.replace("START", "square").replace("  pot:", "  clock: { name: alarm clock, in: player, traits: [portable] }\n  pot:")
    );
    let mut g = game(compile_str(&src).story.unwrap());
    // Timers go off at the end of a turn, after characters have moved, so
    // it sets off one turn later than a shout would.
    assert_says(&mut g, "z", "Your alarm clock goes off!");
    say(&mut g, "z");
    assert_says(&mut g, "z", "The ghoul arrives.");
}

#[test]
fn scripts_can_make_noise() {
    let mut g = town("shop", "");
    assert_says(&mut g, "bang pot", "BONG.");
    // Three rooms to the shop, one a turn starting with this one: it comes
    // all the way, and finds the player.
    say(&mut g, "z");
    assert_says(&mut g, "z", "The ghoul arrives.");
    assert_says(&mut g, "z", "The ghoul attacks you.");
}

// -------------------------------------------------------------- apologies

#[test]
fn apologies_win_back_a_character_and_its_faction_but_only_so_often() {
    let mut g = game(compile_str(VILLAGE).story.unwrap());
    assert_eq!(say(&mut g, "apologize to miller"), "The miller has nothing against you.");
    say(&mut g, "hit baker");
    assert_eq!(
        say(&mut g, "say sorry to the baker"),
        "The baker accepts your apology.\nThe villagers seem a little less wary of you."
    );
    assert_eq!(say(&mut g, "z"), "Time passes.");
    // One apology each, by default.
    say(&mut g, "hit baker");
    assert_says(&mut g, "apologise to baker", "The baker has heard enough of your apologies.");
    assert_says(&mut g, "z", "The baker attacks you.");
}

#[test]
fn an_apology_can_turn_a_village_back() {
    let mut g = game(compile_str(VILLAGE).story.unwrap());
    say(&mut g, "hit baker");
    say(&mut g, "hit smith");
    assert_says(&mut g, "z", "The miller attacks you.");
    // The miller never had a grudge of its own: the apology is for the village.
    let out = say(&mut g, "make peace with the miller");
    assert!(out.contains("The villagers seem a little less wary of you.") && !out.contains("miller attacks"), "{out}");
}

#[test]
fn an_apology_can_mend_a_blown_disguise() {
    let mut g = blackfang();
    for cmd in ["take spear", "take helmet", "wear helmet", "n", "hit grunt with spear", "hit chief with spear"] {
        say(&mut g, cmd);
    }
    // The goblins saw through it; apologising to the grunt (in disguise)
    // brings their attitude back to 1, and the grunt lets it go.
    let out = say(&mut g, "apologize to grunt");
    assert!(out.contains("The goblin grunt accepts your apology.") && out.contains("The goblins seem a little less wary of you."), "{out}");
    assert!(!out.contains("grunt attacks you"), "{out}");
    // The chief still holds its own grudge.
    assert!(out.contains("The goblin chief attacks you"), "{out}");
    // The wolves hate the player (and goblins) by nature.
    assert_eq!(say(&mut g, "apologize to the alpha wolf").lines().next(), Some("The alpha wolf isn't interested in apologies."));
    assert_says(&mut g, "apologize to spear", "There's nobody there to forgive you.");
}

#[test]
fn forgiveness_can_be_set_per_character() {
    let src = VILLAGE.replace("miller: { name: miller, in: green, faction: villagers, health: 9 }", "miller: { name: miller, in: green, faction: villagers, health: 9, patience: 0, forgiveness: 0 }");
    let mut g = game(compile_str(&src).story.unwrap());
    say(&mut g, "hit miller");
    assert_says(&mut g, "apologize to miller", "The miller has heard enough of your apologies.");
}

// ---------------------------------------------------------------- haggling

const SHOP: &str = "\
format: eifgb/0.1
game: { id: s, title: S, version: '1', start: shop }
player: { money: 20 }
rooms:
  shop: { name: Shop }
objects:
  lamp: { name: brass lamp, synonyms: [lamp], in: peddler, traits: [portable], price: 8 }
  rope: { name: rope, in: peddler, traits: [portable], price: 12 }
  map: { name: map, in: peddler, traits: [portable], price: 4 }
characters:
  peddler: { name: peddler, in: shop, behavior: merchant, wander: 0 }
";

#[test]
fn haggling_down_to_the_floor() {
    let mut g = game(compile_str(SHOP).story.unwrap());
    assert_eq!(say(&mut g, "haggle for lamp"), "How much? Try something like \"offer 5 for the brass lamp\".");
    // haggle defaults to 25% for merchants: the floor on 8 is 6.
    assert_eq!(say(&mut g, "offer 2 for the lamp"), "\"6 coins, and not a coin less,\" says the peddler.");
    assert_eq!(say(&mut g, "offer 6 coins for the lamp"), "The peddler grumbles, but sells you the brass lamp for 6 coins.");
    assert_says(&mut g, "i", "You have 14 coins.");
    // Offering the asking price or more is just buying.
    assert_eq!(say(&mut g, "offer 10 for the map"), "You buy the map from the peddler for 4 coins.");
    assert_eq!(say(&mut g, "offer 50 for the rope"), "You only have 10 coins.");
}

#[test]
fn too_many_lowball_offers_and_they_stop_trading_until_you_apologise() {
    let mut g = game(compile_str(SHOP).story.unwrap());
    say(&mut g, "offer 1 for the rope");
    say(&mut g, "offer 1 for the rope");
    assert_eq!(say(&mut g, "offer 1 for the rope"), "The peddler looks insulted, and won't trade with you any more.");
    for cmd in ["buy map", "wares", "offer 9 for the rope"] {
        assert_eq!(say(&mut g, cmd), "The peddler won't trade with you any more.", "{cmd}");
    }
    assert_eq!(say(&mut g, "apologize to the peddler"), "The peddler accepts your apology.");
    assert_says(&mut g, "offer 9 for the rope", "sells you the rope for 9 coins");
}

#[test]
fn fixed_prices() {
    let src = SHOP.replace("behavior: merchant, wander: 0", "behavior: merchant, wander: 0, haggle: 0");
    let mut g = game(compile_str(&src).story.unwrap());
    assert_eq!(say(&mut g, "offer 7 for the lamp"), "The peddler doesn't haggle. It's 8 coins.");
}

const SELLING: &str = "\
format: eifgb/0.1
game: { id: s, title: S, version: '1', start: shop }
player: { money: 20 }
rooms:
  shop: { name: Shop }
objects:
  lamp: { name: brass lamp, synonyms: [lamp], in: player, traits: [portable], price: 8 }
  ring: { name: gold ring, synonyms: [ring], in: player, traits: [portable], price: 40 }
  rope: { name: rope, in: peddler, traits: [portable], price: 12 }
characters:
  peddler: { name: peddler, in: shop, behavior: merchant, wander: 0 }
";

#[test]
fn selling_shows_the_offer_first_and_accept_takes_it() {
    let mut g = game(compile_str(SELLING).story.unwrap());
    assert_eq!(
        say(&mut g, "sell lamp"),
        "The peddler offers 4 coins for the brass lamp. SELL it again or ACCEPT to take that, or ask for more, like \"sell the brass lamp for 5\"."
    );
    assert_eq!(say(&mut g, "accept"), "You sell the brass lamp to the peddler for 4 coins.");
    assert_eq!(say(&mut g, "accept"), "There's no offer on the table.");
}

#[test]
fn haggling_when_selling() {
    let mut g = game(compile_str(SELLING).story.unwrap());
    // Offer 20 (half of 40); it'll go up to 25% more: 25.
    assert_eq!(say(&mut g, "sell ring for 24"), "The peddler grumbles, but pays you 24 coins for the gold ring.");
    let mut g = game(compile_str(SELLING).story.unwrap());
    assert_eq!(say(&mut g, "sell ring for 30"), "\"25 coins, and not a coin more,\" says the peddler.");
    assert_eq!(say(&mut g, "deal"), "You sell the gold ring to the peddler for 25 coins.");
    assert_says(&mut g, "i", "You have 45 coins.");
}

#[test]
fn greedy_sellers_wear_out_the_same_patience() {
    let mut g = game(compile_str(SELLING).story.unwrap());
    say(&mut g, "ask 99 for the ring");
    say(&mut g, "ask 99 for the ring");
    assert_eq!(say(&mut g, "ask 99 for the ring"), "The peddler looks insulted, and won't trade with you any more.");
    assert_eq!(say(&mut g, "sell lamp"), "The peddler won't trade with you any more.");
}

#[test]
fn a_buying_counter_offer_can_be_accepted() {
    let mut g = game(compile_str(SELLING).story.unwrap());
    assert_eq!(say(&mut g, "offer 2 for the rope"), "\"9 coins, and not a coin less,\" says the peddler.");
    assert_eq!(say(&mut g, "accept"), "You buy the rope from the peddler for 9 coins.");
}

#[test]
fn fixed_price_merchants_buy_straight_away() {
    let src = SELLING.replace("wander: 0 }", "wander: 0, haggle: 0 }");
    let mut g = game(compile_str(&src).story.unwrap());
    assert_eq!(say(&mut g, "sell lamp"), "You sell the brass lamp to the peddler for 4 coins.");
    assert_eq!(say(&mut g, "sell ring for 30"), "The peddler doesn't haggle. It'll pay 20 coins.");
}

// ------------------------------------------------------- taming and gifts

const MENAGERIE: &str = "\
format: eifgb/0.1
game: { id: m, title: M, version: '1', start: yard }
player: { health: 30, money: 10 }
factions:
  wolves: { hostile_to: [player] }
  village: {}
rooms:
  yard: { name: Yard, exits: { north: field } }
  field: { name: Field, exits: { south: yard } }
objects:
  bone: { name: bone, in: player, traits: [portable], eaten_by: [wolves] }
  flowers: { name: flowers, in: player, traits: [portable] }
  pebble: { name: pebble, in: player, traits: [portable] }
characters:
  dog: { name: wolf pup, synonyms: [pup], in: yard, faction: wolves, behavior: hunter, wander: 0, health: 20, damage: 2, tame_with: [bone] }
  rat: { name: rat, in: yard, behavior: hunter, wander: 0, health: 4, damage: 1 }
  guard: { name: guard, in: field, behavior: guard, health: 30, damage: 3, bribe: 5 }
  aunt: { name: aunt, in: yard, faction: village, health: 9, patience: 0, likes: [flowers] }
";

#[test]
fn a_tamed_animal_fights_for_the_player_and_follows() {
    let mut g = game(compile_str(MENAGERIE).story.unwrap());
    assert_eq!(say(&mut g, "give pebble to pup"), "The wolf pup doesn't want the pebble.\nThe wolf pup attacks you.\nThe rat attacks you.");
    let out = say(&mut g, "feed bone to pup");
    assert!(out.starts_with("The wolf pup wolfs down the bone, and seems to have decided you're a friend."), "{out}");
    // It turns on the rat, which is hostile to the player.
    assert!(out.contains("The wolf pup attacks the rat.") && !out.contains("pup attacks you"), "{out}");
    say(&mut g, "z");
    say(&mut g, "z");
    assert_says(&mut g, "n", "The wolf pup arrives.");
}

#[test]
fn hitting_a_tamed_animal_undoes_it() {
    let mut g = game(compile_str(MENAGERIE).story.unwrap());
    say(&mut g, "give bone to pup");
    assert_says(&mut g, "hit pup", "The wolf pup attacks you.");
}

#[test]
fn bribes_buy_a_character_off() {
    let mut g = game(compile_str(MENAGERIE).story.unwrap());
    say(&mut g, "n");
    // Asking how much is free.
    assert_eq!(say(&mut g, "bribe guard"), "How much? Try something like \"give 5 coins to the guard\".");
    assert_says(&mut g, "give 3 coins to guard", "The guard scoffs. It'll take more than 3 coins.");
    assert_says(&mut g, "give 50 coins to guard", "You only have 10 coins.");
    let out = say(&mut g, "pay 5 to the guard");
    assert!(out.starts_with("The guard pockets 5 coins and looks the other way.") && !out.contains("guard attacks"), "{out}");
    assert!(!say(&mut g, "z").contains("guard attacks"));
    assert_says(&mut g, "give 5 coins to guard", "The guard has already been paid.");
    assert_says(&mut g, "give 5 coins to rat", "won't be bought");
}

#[test]
fn a_welcome_gift_is_an_apology_without_the_limit() {
    let mut g = game(compile_str(MENAGERIE).story.unwrap());
    say(&mut g, "hit aunt");
    let out = say(&mut g, "give flowers to aunt");
    assert!(out.contains("The aunt gladly accepts the flowers.") && out.contains("The village seem a little less wary of you."), "{out}");
    assert!(!out.contains("aunt attacks"), "{out}");
    assert_says(&mut g, "i", "You're carrying a bone and a pebble.");
}

// ---------------------------------------------------------------- throwing

const THROWING: &str = r#"format: eifgb/0.1
game: { id: t, title: Throwing, version: "1", start: ledge }
player: { health: 30 }
vars: { bridge: false }
rooms:
  ledge:
    name: Ledge
    description: A narrow ledge. A chasm yawns to the north; the far ledge is just a stone's throw away.
    exits:
      north: [{ to: far_ledge, when: vars.bridge }, { blocked: "The chasm is far too wide to jump." }]
      east: meadow
  far_ledge: { name: Far Ledge, exits: { south: ledge } }
  meadow: { name: Meadow, exits: { west: ledge, east: den } }
  den: { name: Den, exits: { west: meadow } }
objects:
  key: { name: key, in: player, traits: [portable] }
  stick: { name: stick, in: player, traits: [portable] }
  spear: { name: spear, in: player, traits: [portable], damage: 4 }
  rock: { name: rock, in: player, traits: [portable] }
  boulder: { name: boulder, in: ledge, traits: [portable], size: 5 }
characters:
  dog: { name: dog, in: ledge, behavior: follower, fetches: true }
  wolf: { name: wolf, in: den, behavior: hunter, range: 0, wander: 0, health: 10, damage: 1 }
"#;

#[test]
fn throwing_over_a_chasm_and_into_the_next_room() {
    let mut g = game(compile_str(THROWING).story.unwrap());
    assert_says(&mut g, "n", "The chasm is far too wide to jump.");
    // Over an exit the player can't walk through.
    assert_eq!(say(&mut g, "throw key north"), "You throw the key north, and it lands somewhere out of sight.");
    assert!(!say(&mut g, "i").contains("key"));
    // Nowhere to go that way: no turn taken.
    assert_eq!(say(&mut g, "throw rock west"), "There's nowhere to throw anything that way.");
    assert_eq!(say(&mut g, "take boulder"), "Taken.");
    assert_eq!(say(&mut g, "throw boulder"), "The boulder is too heavy to throw.");
}

#[test]
fn a_thrown_weapon_hits_and_lands_in_the_room() {
    // No dog this time: it would fetch the spear.
    let src = THROWING
        .replace("in: den, behavior: hunter", "in: ledge, behavior: defender")
        .replace("  dog: { name: dog, in: ledge, behavior: follower, fetches: true }\n", "");
    let mut g = game(compile_str(&src).story.unwrap());
    assert_eq!(
        say(&mut g, "throw spear at wolf"),
        "You throw the spear at the wolf.\nYou hit the wolf with the spear.\nThe wolf attacks you."
    );
    assert_says(&mut g, "look", "a spear");
    // Things that can't be hurt: it bounces off.
    assert_eq!(
        say(&mut g, "throw stick at boulder"),
        "You throw the stick at the boulder.\nThe stick bounces off the boulder.\nThe wolf attacks you."
    );
}

#[test]
fn a_thrown_rock_draws_hunters_to_where_it_lands() {
    let mut g = game(compile_str(THROWING).story.unwrap());
    // The meadow is next to the den: the wolf hears the rock land.
    say(&mut g, "throw rock east");
    say(&mut g, "z");
    let out = say(&mut g, "e");
    assert!(out.contains("and a wolf here.") && out.contains("The wolf attacks you."), "{out}");
}

#[test]
fn followers_that_fetch_bring_things_back_but_only_what_they_can_reach() {
    let mut g = game(compile_str(THROWING).story.unwrap());
    // Across the chasm: the dog can't get there, so it doesn't try.
    assert!(!say(&mut g, "throw key north").contains("dog"));
    let out = say(&mut g, "throw stick east");
    assert!(out.contains("The dog bounds after the stick.") && out.contains("The dog goes east."), "{out}");
    say(&mut g, "z");
    assert_says(&mut g, "z", "The dog arrives.");
    assert_says(&mut g, "z", "The dog drops the stick at your feet.");
    assert_says(&mut g, "look", "You can see a stick");
}
