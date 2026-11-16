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
    assert!(played >= 2, "expected at least the lighthouse and midnight_snack walkthroughs");
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
    assert!(out.contains("You can see a wooden box (locked), a key and a scarf here."), "{out}");
    // The hatch is scenery, so it isn't listed, but its state is still mentioned.
    assert!(out.contains("The hatch is closed."), "{out}");

    assert_says(&mut g, "x box", "It's locked.");
    say(&mut g, "take key");
    say(&mut g, "unlock box");
    assert_says(&mut g, "x box", "It's closed.");
    say(&mut g, "open box");
    assert_says(&mut g, "x box", "It's open.");
    assert_says(&mut g, "look", "a wooden box (open)");

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
    assert_says(&mut g, "go through statue", "That's not something you can go through.");
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
    assert!(out.contains("The radio is broken."), "{out}");
    assert!(out.contains("You can see a lamp here."), "{out}");

    say(&mut g, "light lamp");
    assert_eq!(say(&mut g, "x lamp"), "A brass lamp. Its fuel is 2. It's lit.");
    assert_says(&mut g, "look", "You can see a lamp (lit) here.");
    assert_eq!(say(&mut g, "x radio"), "It's broken. Its channel is sixteen.");
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
