//! The puzzle chain: what the player does, in order, and what can't happen.

use eifgb_compiler::puzzles::puzzles;
use eifgb_compiler::{compile, compile_str, DirFiles};
use std::path::PathBuf;

fn texts(p: &eifgb_compiler::puzzles::Puzzles) -> Vec<String> {
    p.steps.iter().map(|s| s.text.clone()).collect()
}

#[test]
fn the_examples_chain_from_start_to_end() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/midnight_snack");
    let p = puzzles(&compile(&DirFiles { root }, "game.yaml").story.unwrap());
    assert_eq!(
        texts(&p),
        [
            "Start in Kitchen.",
            "Search the worn mat: a small key.",
            "Unlock the pantry door with the small key: Pantry opens up.",
            "The fed ending: from Eat the red apple (objects.apple.on.after_eat).",
            "The feast ending: from Eat the apple pie (objects.pie.on.after_eat).",
        ]
    );
    assert!(p.problems.is_empty(), "{:#?}", p.problems);
    assert!(p.endings.iter().all(|e| e.reachable));
    for (dir, game) in [("examples", "lighthouse"), ("examples", "blackfang"), ("examples", "campfire"), ("templates", "small_house"), ("templates", "locked_dungeon"), ("templates", "market")] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec").join(dir).join(game);
        let p = puzzles(&compile(&DirFiles { root }, "game.yaml").story.unwrap());
        assert!(p.problems.is_empty(), "{game}: {:#?}", p.problems);
    }
}

#[test]
fn what_can_never_happen() {
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
rooms:
  hall: { name: Hall, exits: { north: { to: vault, through: vault_door }, east: garden } }
  garden: { name: Garden, exits: { west: hall } }
  vault: { name: Vault, on: { enter: 'end_game("rich")' } }
  tower: { name: Tower, on: { enter: 'end_game("high")' } }
objects:
  vault_door: { name: vault door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: gold_key }
  gold_key: { name: gold key, in: chest, traits: [portable] }
  chest: { name: chest, in: garden, traits: [container, openable, lockable], state: { locked: true } }
  spare_key: { name: spare key, in: garden, traits: [portable] }
  gem: { name: gem, in: garden, traits: [portable], state: { hidden: true } }
endings:
  rich: { kind: win, text: Rich. }
  high: { kind: win, text: High. }
  lost: { kind: loss, text: Lost. }
"#,
        )
        .story
        .unwrap(),
    );
    let msgs: Vec<String> = p.problems.iter().map(|x| format!("{} {}: {}", x.what, x.id, x.message)).collect();
    let has = |s: &str| msgs.iter().any(|m| m.contains(s));
    assert!(has("room vault: The player can't get to Vault."), "{msgs:#?}");
    assert!(has("room tower: The player can't get to Tower."), "{msgs:#?}");
    assert!(has("thing chest: The chest is locked, and there's no key for it."), "{msgs:#?}");
    assert!(has("thing gold_key: The player can't get to the gold key."), "{msgs:#?}");
    assert!(has("thing spare_key: The spare key opens nothing"), "{msgs:#?}");
    assert!(has("thing gem: The player can't get to the gem (it's hidden, and nothing reveals it)."), "{msgs:#?}");
    assert!(has("ending rich: The 'rich' ending can't happen: only rooms.vault.on.enter end the game that way"), "{msgs:#?}");
    assert!(has("ending lost: Nothing ends the game with 'lost'"), "{msgs:#?}");
    assert!(!p.endings.iter().any(|e| e.reachable));
}

/// The step whose text starts with `start`.
fn step<'a>(p: &'a eifgb_compiler::puzzles::Puzzles, start: &str) -> (usize, &'a eifgb_compiler::puzzles::Step) {
    p.steps.iter().enumerate().find(|(_, s)| s.text.starts_with(start)).unwrap_or_else(|| panic!("no step {start:?} in {:#?}", texts(p)))
}

/// Two puzzles side by side, one inside the other, and two ways to one thing.
#[test]
fn steps_know_what_they_need() {
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
rooms:
  hall: { name: Hall, exits: { north: { to: study, through: study_door }, east: { to: shed, through: shed_door } } }
  study: { name: Study, exits: { south: hall } }
  shed: { name: Shed, exits: { west: hall }, on: { enter: 'end_game("out")' } }
objects:
  study_door: { name: study door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: brass_key }
  shed_door: { name: shed door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: iron_key }
  rug: { name: rug, in: hall }
  brass_key: { name: brass key, in: hall, traits: [portable], state: { hidden: true }, revealed_by: search rug }
  desk: { name: desk, in: study, traits: [container, openable, lockable], state: { locked: true }, unlocks_with: brass_key, code: "4512" }
  iron_key: { name: iron key, in: desk, traits: [portable] }
  lamp: { name: lamp, in: hall, traits: [portable], state: { hidden: true }, revealed_by: search rug }
endings:
  out: { kind: win, text: Out. }
"#,
        )
        .story
        .unwrap(),
    );
    let (rug, _) = step(&p, "Search the rug: a brass key.");
    let (study, s) = step(&p, "Unlock the study door with the brass key");
    assert_eq!(s.needs, [rug]);
    let (desk, s) = step(&p, "Unlock the desk with the brass key: an iron key.");
    assert_eq!(s.needs, [rug, study], "the key, and getting to the desk");
    assert_eq!(s.or, ["Unlock the desk with the code 4512: an iron key."]);
    let (shed, s) = step(&p, "Unlock the shed door with the iron key");
    assert_eq!(s.needs, [desk]);
    // The win takes all four; the lamp is a side puzzle nothing needs.
    let out = p.endings.iter().find(|e| e.key == "out").unwrap();
    assert_eq!(out.needs, [rug, study, desk, shed]);
    let (lamp, _) = step(&p, "Search the rug: a lamp.");
    assert!(!out.needs.contains(&lamp));
    assert!(p.stuck.is_empty(), "{:#?}", p.stuck);
}

/// Where a player could get stuck: what they need can be lost, with no other way.
#[test]
fn where_a_player_could_get_stuck() {
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
rooms:
  hall: { name: Hall, exits: { north: { to: a, through: a_door }, east: { to: b, through: b_door }, west: { to: c, through: c_door } } }
  a: { name: A, exits: { south: hall } }
  b: { name: B, exits: { west: hall }, on: { enter: 'world.pass:move_to(nil)' } }
  c: { name: C, exits: { east: hall, north: { to: d, through: d_door } } }
  d: { name: D, on: { enter: 'end_game("won")' } }
objects:
  a_door: { name: red door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: candy_key }
  candy_key: { name: candy key, in: hall, traits: [portable, edible] }
  b_door: { name: blue door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: glass_key }
  glass_key: { name: glass key, in: a, traits: [portable], uses: 1 }
  c_door: { name: green door, in: hall, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: glass_key }
  d_door: { name: gold door, in: c, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: pass }
  pass: { name: pass, in: a, traits: [portable] }
endings:
  won: { kind: win, text: Won. }
"#,
        )
        .story
        .unwrap(),
    );
    let msgs: Vec<String> = p.stuck.iter().map(|x| format!("{}: {}", x.id, x.message)).collect();
    let has = |s: &str| msgs.iter().any(|m| m.contains(s));
    assert!(has("candy_key: The player can eat the candy key, which they need (unlock the red door with the candy key"), "{msgs:#?}");
    assert!(has("glass_key: The glass key breaks after 1 use, exactly as many as the player needs"), "{msgs:#?}");
    assert!(has("pass: A script (rooms.b.on.enter) can take the pass away"), "{msgs:#?}");
}

/// A way that waits on a variable: what sets it, or that nothing does.
#[test]
fn ways_that_wait_on_a_variable() {
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
vars: { bridge_down: false, never: false }
rooms:
  hall: { name: Hall, exits: { north: { to: tower, when: vars.bridge_down }, east: { to: pit, when: vars.never } } }
  tower: { name: Tower, exits: { south: hall } }
  pit: { name: Pit, exits: { west: hall } }
objects:
  lever: { name: lever, in: tower, on: { pull: 'vars.never = vars.never' } }
  winch: { name: winch, in: hall, on: { push: 'vars.bridge_down = true' } }
"#,
        )
        .story
        .unwrap(),
    );
    let (_, s) = step(&p, "Go north from Hall to Tower");
    assert_eq!(s.text, "Go north from Hall to Tower, once PUSH THE WINCH sets vars.bridge_down.");
    // Set, but only once the Tower is reached: so the Pit comes after it.
    let (tower, _) = step(&p, "Go north from Hall to Tower");
    let (_, pit) = step(&p, "Go east from Hall to Pit");
    assert_eq!(pit.needs, [tower]);
    let nothing = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
vars: { open_sesame: false }
rooms:
  hall: { name: Hall, exits: { north: { to: cave, when: vars.open_sesame } } }
  cave: { name: Cave }
"#,
        )
        .story
        .unwrap(),
    );
    assert!(nothing.problems.iter().any(|x| x.message == "The way north from Hall to Cave waits on vars.open_sesame, but nothing ever sets it."), "{:#?}", nothing.problems);
}

/// Either-or: choosing one thing rules out another.
#[test]
fn either_or_choices() {
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: forge }
vars: { armed: false, side: false }
rooms:
  forge: { name: Forge, exits: { north: lair, east: { to: camp, when: vars.side == "rebels" }, west: { to: palace, when: vars.side == "empire" } } }
  lair: { name: Lair, exits: { south: forge, north: { to: hoard, through: hoard_door } } }
  hoard: { name: Hoard, exits: { south: lair, north: { to: vault, through: vault_door } }, on: { enter: 'end_game("rich")' } }
  vault: { name: Vault, on: { enter: 'end_game("legend")' } }
  camp: { name: Camp, on: { enter: 'end_game("free")' } }
  palace: { name: Palace, on: { enter: 'end_game("crowned")' } }
objects:
  hoard_door: { name: hoard door, in: lair, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: sword }
  vault_door: { name: vault door, in: hoard, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: shield }
  sword: { name: sword, traits: [portable] }
  shield: { name: shield, traits: [portable] }
characters:
  smith: { name: smith, in: forge, talk: armoury }
choices:
  armoury:
    start: pick
    nodes:
      pick:
        text: Take one.
        options:
          - { text: Take the sword, when: not vars.armed, do: 'vars.armed = true; world.sword:move_to(player)', end: true }
          - { text: Take the shield, when: not vars.armed, do: 'vars.armed = true; world.shield:move_to(player)', end: true }
          - { text: Join the rebels, when: vars.side == false, do: 'vars.side = "rebels"', end: true }
          - { text: Serve the empire, when: vars.side == false, do: 'vars.side = "empire"', end: true }
endings:
  rich: { kind: win, text: Rich. }
  legend: { kind: win, text: Legend. }
  free: { kind: neutral, text: Free. }
  crowned: { kind: neutral, text: Crowned. }
"#,
        )
        .story
        .unwrap(),
    );
    // The choices are steps, each saying what it rules out.
    let (sword, s) = step(&p, "Choose “Take the sword”");
    assert_eq!(s.text, "Choose “Take the sword” in the conversation armoury.");
    assert_eq!(s.either, ["Choose “Take the shield” in the conversation armoury."]);
    let (found, s) = step(&p, "Choose “Take the sword”: a sword.");
    assert_eq!(s.needs, [sword]);
    let (hoard, s) = step(&p, "Unlock the hoard door with the sword");
    assert_eq!(s.needs, [found]);
    step(&p, "Go east from Forge to Camp, once the player chooses “Join the rebels”.");
    let ending = |k: &str| p.endings.iter().find(|e| e.key == k).unwrap();
    assert!(ending("rich").needs.contains(&hoard));
    // Which ending needs which choice: by the value set, too.
    assert_eq!(ending("rich").only_if, ["only if the player chooses “Take the sword”"]);
    assert_eq!(ending("free").only_if, ["only if the player chooses “Join the rebels”"]);
    assert_eq!(ending("crowned").only_if, ["only if the player chooses “Serve the empire”"]);
    // Needing both the sword and the shield: it can't happen.
    assert!(!ending("legend").reachable);
    let has = |list: &[eifgb_compiler::puzzles::PuzzleProblem], s: &str| list.iter().any(|x| x.message.contains(s));
    assert!(has(&p.problems, "The 'legend' ending can't happen: it needs more than one of “Take the sword” and “Take the shield”, and only one can happen (each sets vars.armed)."), "{:#?}", p.problems);
    // The shield leaves no way to win; the sides don't matter to winning.
    assert!(has(&p.stuck, "If the player chooses “Take the shield”, they can't win: it rules out “Take the sword”."), "{:#?}", p.stuck);
    assert_eq!(p.stuck.len(), 1, "{:#?}", p.stuck);
}

/// Steps solved in Lua: what the chain can read of them, and what it can't.
#[test]
fn what_the_chain_reads_of_scripts_and_assumes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/templates/market");
    let p = puzzles(&compile(&DirFiles { root }, "game.yaml").story.unwrap());
    // Bought from a merchant, then given: read from the handler's `args.tool == world.child`.
    assert_eq!(texts(&p)[1..], ["Buy the loaf of bread from the baker.", "The kindness ending: from Give the loaf of bread to the hungry child (objects.bread.on.after_give)."]);
    assert!(p.assumes.is_empty(), "{:#?}", p.assumes);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/templates/locked_dungeon");
    let p = puzzles(&compile(&DirFiles { root }, "game.yaml").story.unwrap());
    assert_eq!(p.assumes[0].message, "Assumes the player can get the iron key from the guard, by asking, buying, stealing or fighting.");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/lighthouse");
    let p = puzzles(&compile(&DirFiles { root }, "game.yaml").story.unwrap());
    let (_, s) = step(&p, "Search the tide pool: a brass key.");
    assert_eq!(s.script.as_deref(), Some("objects.tide_pool.on.search"));
    assert!(p.assumes.iter().any(|a| a.message.starts_with("Leans on a script (objects.tide_pool.on.search)")), "{:#?}", p.assumes);
    // A condition it can't read is taken on trust, and says so.
    let p = puzzles(
        &compile_str(
            r#"
format: eifgb/0.1
game: { id: t, title: T, version: "1", start: hall }
clock: { start: "8:00" }
rooms:
  hall: { name: Hall, exits: { north: { to: garden, when: clock.night } } }
  garden: { name: Garden, on: { enter: 'end_game("out")' } }
endings:
  out: { kind: win, text: Out. }
"#,
        )
        .story
        .unwrap(),
    );
    assert_eq!(p.assumes[0].message, "Assumes the way north from Hall to Garden opens (when: clock.night).");
}
