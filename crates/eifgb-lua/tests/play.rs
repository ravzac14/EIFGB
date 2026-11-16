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

#[test]
fn walkthrough_wins() {
    let mut g = game(lighthouse());
    let walkthrough = std::fs::read_to_string(lighthouse_dir().join("walkthrough.txt")).unwrap();
    let mut last = Output::default();
    for line in walkthrough.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        last = g.input(line);
        assert!(
            !last.lines.iter().any(|l| l.kind == LineKind::Error),
            "> {line}\n{}",
            last.text()
        );
    }
    assert_eq!(last.ended, Some(EndingKind::Win), "{}", last.text());
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
