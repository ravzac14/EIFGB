use eifgb_compiler::{compile, DirFiles};
use eifgb_engine::{MoveError, SaveError, World};
use eifgb_ir::{Story, Value, PLAYER};
use std::path::PathBuf;

fn lighthouse() -> Story {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/lighthouse");
    let out = compile(&DirFiles { root }, "game.yaml");
    out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics))
}

fn world() -> World {
    World::new(lighthouse(), 1)
}

fn id(w: &World, key: &str) -> eifgb_ir::EntityId {
    w.id(key).unwrap_or_else(|| panic!("no entity {key}"))
}

#[test]
fn player_starts_in_start_room() {
    let w = world();
    assert_eq!(w.here(), id(&w, "shore"));
    assert!(w.state.visited.contains(&id(&w, "shore")));
}

#[test]
fn hidden_things_are_out_of_scope_until_revealed() {
    let mut w = world();
    let (key, pool) = (id(&w, "brass_key"), id(&w, "tide_pool"));
    assert!(w.in_scope(pool));
    assert!(!w.in_scope(key), "the key starts hidden in the pool");

    w.set(key, "hidden", Value::Bool(false));
    assert!(w.in_scope(key), "the pool is an open container, so its contents are visible");
}

#[test]
fn closed_containers_hide_their_contents() {
    let mut w = world();
    let (key, pool) = (id(&w, "brass_key"), id(&w, "tide_pool"));
    w.set(key, "hidden", Value::Bool(false));
    w.set(pool, "open", Value::Bool(false));
    // The tide pool isn't `openable`, so it's always open whatever its state says.
    assert!(w.in_scope(key));
}

#[test]
fn carried_things_follow_the_player() {
    let mut w = world();
    let key = id(&w, "brass_key");
    w.move_to(key, Some(PLAYER)).unwrap();
    assert_eq!(w.inventory(), vec![key]);
    w.move_to(PLAYER, Some(id(&w, "lighthouse_base"))).unwrap();
    assert!(w.in_scope(key));
    assert_eq!(w.room_of(key), Some(id(&w, "lighthouse_base")));
    assert!(w.state.visited.contains(&id(&w, "lighthouse_base")));
}

#[test]
fn things_in_other_rooms_are_out_of_scope() {
    let w = world();
    assert!(!w.in_scope(id(&w, "oil_can")));
    assert!(!w.in_scope(id(&w, "keeper")));
}

#[test]
fn moves_that_would_create_loops_are_refused() {
    let mut w = world();
    let (key, pool) = (id(&w, "brass_key"), id(&w, "tide_pool"));
    assert_eq!(w.move_to(pool, Some(key)), Err(MoveError::WouldContainItself));
    assert_eq!(w.move_to(pool, Some(pool)), Err(MoveError::WouldContainItself));
    assert_eq!(w.move_to(id(&w, "shore"), Some(pool)), Err(MoveError::Immovable));
    assert_eq!(w.move_to(PLAYER, None), Err(MoveError::Immovable));
}

#[test]
fn undo_restores_the_previous_checkpoint() {
    let mut w = world();
    let key = id(&w, "brass_key");
    assert!(!w.undo());

    w.checkpoint();
    w.move_to(key, Some(PLAYER)).unwrap();
    w.state.vars.insert("lamp_lit".into(), Value::Bool(true));
    w.state.turn += 1;

    assert!(w.undo());
    assert_eq!(w.location(key), Some(id(&w, "tide_pool")));
    assert_eq!(w.state.vars["lamp_lit"], Value::Bool(false));
    assert_eq!(w.state.turn, 0);
}

#[test]
fn undo_also_rewinds_randomness() {
    let mut w = world();
    w.checkpoint();
    let a = w.state.rng.range(1, 1_000_000);
    w.undo();
    let b = w.state.rng.range(1, 1_000_000);
    assert_eq!(a, b);
}

#[test]
fn save_and_restore_round_trip() {
    let mut w = world();
    let key = id(&w, "brass_key");
    w.move_to(key, Some(PLAYER)).unwrap();
    w.state.score = 5;
    let saved = w.save();

    let mut fresh = world();
    fresh.restore(&saved).unwrap();
    assert_eq!(fresh.state, w.state);
}

#[test]
fn restore_refuses_saves_from_other_games() {
    let w = world();
    let saved = w.save().replace("\"story_id\":\"lighthouse\"", "\"story_id\":\"other\"");
    let mut fresh = world();
    assert!(matches!(fresh.restore(&saved), Err(SaveError::WrongStory { .. })));
    assert!(matches!(fresh.restore("not json"), Err(SaveError::Malformed(_))));
}
