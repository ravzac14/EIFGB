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
            "The fed ending: from objects.apple.on.after_eat.",
            "The feast ending: from objects.pie.on.after_eat.",
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
