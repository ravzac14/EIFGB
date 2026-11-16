use eifgb_compiler::{compile, DirFiles};
use eifgb_engine::script::{Ctx, Effect, ScriptHost, ScriptValue};
use eifgb_engine::text::{reflow, render};
use eifgb_engine::World;
use eifgb_ir::{Code, EntityId, Phase, Story, Value, PLAYER};
use eifgb_lua::LuaHost;
use std::path::PathBuf;

fn lighthouse() -> Story {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/lighthouse");
    let out = compile(&DirFiles { root }, "game.yaml");
    out.story.unwrap_or_else(|| panic!("{:#?}", out.diagnostics))
}

struct T {
    w: World,
    h: LuaHost,
}

impl T {
    fn new() -> T {
        let story = lighthouse();
        let mut h = LuaHost::new().unwrap();
        h.load(&story).unwrap();
        T { w: World::new(story, 1), h }
    }

    fn id(&self, key: &str) -> EntityId {
        self.w.id(key).unwrap()
    }

    fn handler(&self, entity: &str, phase: Phase, event: &str) -> Code {
        let e = self.w.def(self.id(entity));
        e.handlers
            .iter()
            .find(|h| h.phase == phase && h.event == event)
            .unwrap_or_else(|| panic!("{entity} has no {phase:?} {event} handler"))
            .code
            .clone()
    }

    fn run(&mut self, code: &Code, ctx: &Ctx) -> (ScriptValue, Vec<Effect>) {
        let o = self.h.call(&mut self.w, code, ctx).unwrap_or_else(|e| panic!("{e}"));
        (o.value, o.effects)
    }

    fn eval(&mut self, src: &str) -> Result<ScriptValue, String> {
        let code = Code { src: src.into(), origin: "test".into() };
        self.h.call(&mut self.w, &code, &Ctx::new()).map(|o| o.value).map_err(|e| e.to_string())
    }

    fn describe(&mut self, key: &str) -> String {
        let id = self.id(key);
        let text = self.w.def(id).description.clone().unwrap();
        reflow(&render(&mut self.h, &mut self.w, &text, &Ctx::new().this(id)).unwrap().text)
    }

    fn message(&mut self, key: &str, ctx: &Ctx) -> String {
        let text = self.w.story().messages[key].clone();
        reflow(&render(&mut self.h, &mut self.w, &text, ctx).unwrap().text)
    }
}

fn says(effects: &[Effect]) -> String {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Say(s) => Some(s.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------- lighthouse handlers

#[test]
fn searching_the_tide_pool_finds_the_key_once() {
    let mut t = T::new();
    let (pool, key) = (t.id("tide_pool"), t.id("brass_key"));
    let code = t.handler("tide_pool", Phase::On, "search");

    let (_, fx) = t.run(&code, &Ctx::new().this(pool));
    assert!(says(&fx).contains("pull out a small brass key"), "{fx:?}");
    assert!(fx.contains(&Effect::Score(5)));
    assert_eq!(t.w.location(key), Some(PLAYER));
    assert!(!t.w.flag(key, "hidden"));
    assert_eq!(t.w.state.score, 5);

    let (_, fx) = t.run(&code, &Ctx::new().this(pool));
    assert_eq!(says(&fx), "Nothing but seaweed.");
    assert_eq!(t.w.state.score, 5);
}

#[test]
fn templates_react_to_state() {
    let mut t = T::new();
    assert!(t.describe("tide_pool").contains("Something glints"));
    let key = t.id("brass_key");
    t.w.set(key, "hidden", Value::Bool(false));
    assert_eq!(t.describe("tide_pool"), "Cold water collects between the rocks.");
}

#[test]
fn shore_description_reflows_and_uses_vars() {
    let mut t = T::new();
    let text = t.describe("shore");
    assert!(text.contains("climbs north to the lighthouse, its lamp dark."), "{text}");
    t.w.state.vars.insert("lamp_lit".into(), Value::Bool(true));
    assert!(t.describe("shore").ends_with("climbs north to the lighthouse."));
}

#[test]
fn lamp_needs_oil_before_it_lights() {
    let mut t = T::new();
    let (lamp, can, key) = (t.id("great_lamp"), t.id("oil_can"), t.id("brass_key"));
    let before_light = t.handler("great_lamp", Phase::Before, "light");
    let fill = t.handler("great_lamp", Phase::On, "fill");

    let (v, fx) = t.run(&before_light, &Ctx::new().this(lamp).entity("target", lamp));
    assert_eq!(v, ScriptValue::Block);
    assert!(says(&fx).contains("It needs oil"));

    // Entity proxies compare by identity, so `args.tool ~= world.oil_can` works.
    let (v, fx) = t.run(&fill, &Ctx::new().this(lamp).entity("target", lamp).entity("tool", key));
    assert_eq!(v, ScriptValue::Block);
    assert_eq!(says(&fx), "That won't fuel a lamp.");

    let (v, _) = t.run(&fill, &Ctx::new().this(lamp).entity("target", lamp).entity("tool", can));
    assert_ne!(v, ScriptValue::Block);
    assert!(t.w.flag(lamp, "fueled"));
    assert_eq!(t.w.get(can, "full"), Some(&Value::Bool(false)));

    let (v, _) = t.run(&before_light, &Ctx::new().this(lamp).entity("target", lamp));
    assert_ne!(v, ScriptValue::Block);
}

#[test]
fn radio_ends_the_game_only_once_the_lamp_is_lit() {
    let mut t = T::new();
    let radio = t.w.story().choices.iter().find(|c| c.key == "radio_dial").unwrap();
    let code = radio.nodes[0].options[1].run.clone().unwrap();

    let (_, fx) = t.run(&code, &Ctx::new());
    assert!(says(&fx).contains("We need a light!"));
    assert!(!fx.iter().any(|e| matches!(e, Effect::EndGame(_))));

    t.w.state.vars.insert("lamp_lit".into(), Value::Bool(true));
    let (_, fx) = t.run(&code, &Ctx::new());
    assert!(fx.contains(&Effect::EndGame("saved".into())));
    assert_eq!(t.w.state.vars["ship_contacted"], Value::Bool(true));
}

#[test]
fn turn_handler_ends_the_game_at_turn_60() {
    let mut t = T::new();
    let code = t.w.story().handlers.iter().find(|h| h.event == "turn").unwrap().code.clone();
    t.w.state.turn = 40;
    let (_, fx) = t.run(&code, &Ctx::new());
    assert!(says(&fx).contains("dangerously close"));
    t.w.state.turn = 60;
    let (_, fx) = t.run(&code, &Ctx::new());
    assert_eq!(fx, vec![Effect::EndGame("lost".into())]);
}

#[test]
fn helper_functions_from_script_files_are_callable() {
    let mut t = T::new();
    assert_eq!(t.eval("trust_level()").unwrap(), ScriptValue::Str("cold".into()));
    t.w.state.vars.insert("keeper_trust".into(), Value::Int(2));
    assert_eq!(t.eval("trust_level()").unwrap(), ScriptValue::Str("warm".into()));
}

#[test]
fn stdlib_messages_render_with_article_helpers() {
    let mut t = T::new();
    let (lamp, oil) = (t.id("great_lamp"), t.id("oil_can"));
    assert_eq!(t.message("take.fixed", &Ctx::new().entity("target", lamp)), "The great lamp won't budge.");
    assert_eq!(t.message("examine.default", &Ctx::new().entity("target", oil)), "You see nothing special about the oil can.");
    assert_eq!(t.message("take.already", &Ctx::new().entity("target", PLAYER)), "You already have yourself.");
    t.w.state.score = 15;
    t.w.state.turn = 7;
    assert_eq!(
        t.message("score.report", &Ctx::new()),
        "You have scored 15 out of a possible 30, in 7 turns."
    );
}

#[test]
fn a_and_an() {
    let mut t = T::new();
    assert_eq!(t.eval("a(world.oil_can)").unwrap(), ScriptValue::Str("an oil can".into()));
    assert_eq!(t.eval("A(world.radio)").unwrap(), ScriptValue::Str("A radio".into()));
}

// --------------------------------------------------------- the script API

#[test]
fn here_world_and_entity_fields() {
    let mut t = T::new();
    assert_eq!(t.eval("here == world.shore").unwrap(), ScriptValue::Bool(true));
    assert_eq!(t.eval("world.brass_key.location == world.tide_pool").unwrap(), ScriptValue::Bool(true));
    assert_eq!(t.eval("world.oil_can.name").unwrap(), ScriptValue::Str("oil can".into()));
    assert_eq!(t.eval("world.keeper.kind").unwrap(), ScriptValue::Str("character".into()));
    assert_eq!(t.eval("world.brass_key:is_in(world.shore)").unwrap(), ScriptValue::Bool(true));
    assert_eq!(t.eval("world.brass_key:has('portable')").unwrap(), ScriptValue::Bool(true));
    assert_eq!(t.eval("#world.tide_pool:contents()").unwrap(), ScriptValue::Int(1));
    assert_eq!(t.eval("player").unwrap(), ScriptValue::Entity(PLAYER));
    assert_eq!(t.eval("game.max_score").unwrap(), ScriptValue::Int(30));
}

#[test]
fn moving_the_player_changes_here() {
    let mut t = T::new();
    t.eval("player:move_to(world.lamp_room)").unwrap();
    assert_eq!(t.w.here(), t.id("lamp_room"));
    assert_eq!(t.eval("here.key").unwrap(), ScriptValue::Str("lamp_room".into()));
}

#[test]
fn state_values_round_trip() {
    let mut t = T::new();
    t.eval("world.radio.channel = 16").unwrap();
    t.eval("world.radio.log = {'a', 'b'}").unwrap();
    t.eval("world.radio.meta = {owner = 'keeper', age = 40}").unwrap();
    let radio = t.id("radio");
    assert_eq!(t.w.get(radio, "channel"), Some(&Value::Int(16)));
    assert_eq!(t.w.get(radio, "log"), Some(&Value::List(vec![Value::Str("a".into()), Value::Str("b".into())])));
    assert_eq!(t.eval("world.radio.meta.owner").unwrap(), ScriptValue::Str("keeper".into()));
    t.eval("world.radio.channel = nil").unwrap();
    assert_eq!(t.w.get(radio, "channel"), None);
}

#[test]
fn random_is_deterministic_and_rewinds_with_undo() {
    let mut a = T::new();
    let mut b = T::new();
    let roll = |t: &mut T| t.eval("random(1, 1000000)").unwrap();
    assert_eq!(roll(&mut a), roll(&mut b));

    a.w.checkpoint();
    let first = roll(&mut a);
    a.w.undo();
    assert_eq!(roll(&mut a), first);
}

// ------------------------------------------------------------- mistakes

fn assert_err(t: &mut T, src: &str, needle: &str) {
    let e = t.eval(src).expect_err(&format!("{src} should fail"));
    assert!(e.contains(needle), "error for {src:?} was {e:?}, expected {needle:?}");
}

#[test]
fn helpful_errors_for_common_mistakes() {
    let mut t = T::new();
    assert_err(&mut t, "world.brass_kye", "no object called 'brass_kye'");
    assert_err(&mut t, "vars.lamp_lt = true", "vars.lamp_lt isn't declared");
    assert_err(&mut t, "vars.lamp_lt", "isn't declared");
    assert_err(&mut t, "counter = 1", "can't create global 'counter'");
    assert_err(&mut t, "world.radio.owner = world.keeper", "store its key instead");
    assert_err(&mut t, "world.radio.location = world.shore", "use :move_to");
    assert_err(&mut t, "world.shore:move_to(world.radio)", "can't be moved");
    assert_err(&mut t, "world.tide_pool:move_to(world.brass_key)", "into itself");
    assert_err(&mut t, "end_game('nope')", "no ending called 'nope'");
    assert_err(&mut t, "undefined_function()", "unknown name 'undefined_function'");
}

#[test]
fn errors_name_their_origin() {
    let mut t = T::new();
    let code = Code { src: "local x = nil; return x.y".into(), origin: "objects.radio.on.use".into() };
    let e = t.h.call(&mut t.w, &code, &Ctx::new()).unwrap_err();
    assert_eq!(e.origin, "objects.radio.on.use");
    assert!(e.message.starts_with("objects.radio.on.use:1:"), "{}", e.message);
}

// --------------------------------------------------------------- sandbox

#[test]
fn dangerous_libraries_are_absent() {
    let mut t = T::new();
    for name in ["os", "io", "require", "load", "loadstring", "dofile", "loadfile", "debug", "package",
                 "getmetatable", "setmetatable", "rawset", "rawget", "collectgarbage", "_G"] {
        assert_err(&mut t, name, &format!("unknown name '{name}'"));
    }
    assert_eq!(t.eval("string.dump").unwrap(), ScriptValue::Nil);
    assert_eq!(t.eval("('x'):rep(3)").unwrap(), ScriptValue::Str("xxx".into()));
    assert_eq!(t.eval("math.random").unwrap(), ScriptValue::Nil);
    assert_eq!(t.eval("math.floor(2.7)").unwrap(), ScriptValue::Int(2));
}

#[test]
fn endless_loops_are_stopped_and_the_host_keeps_working() {
    let mut t = T::new();
    assert_err(&mut t, "while true do end", "ran too long");
    assert_err(&mut t, "while true do pcall(function() while true do end end) end", "ran too long");
    assert_eq!(t.eval("1 + 1").unwrap(), ScriptValue::Int(2));
}

#[test]
fn memory_bombs_are_stopped() {
    let mut t = T::new();
    let e = t.eval("local s = 'x' for i = 1, 40 do s = s .. s end return #s").unwrap_err();
    assert!(e.contains("memory") || e.contains("too long"), "{e}");
    assert_eq!(t.eval("1 + 1").unwrap(), ScriptValue::Int(2));
}

#[test]
fn proxies_cannot_be_tampered_with() {
    let mut t = T::new();
    assert_err(&mut t, "world.oil_can.contents = 1", "is a method name");
    assert_err(&mut t, "world.oil_can.move_to = nil", "is a method name");
    assert_err(&mut t, "world.oil_can.name = 'jug'", "'name' can't be changed");
    assert_eq!(t.eval("#world.tide_pool:contents()").unwrap(), ScriptValue::Int(1));
}

#[test]
fn script_files_may_only_define_functions() {
    let mut story = lighthouse();
    story.scripts[0].src.push_str("\ncounter = 0\n");
    let mut h = LuaHost::new().unwrap();
    let e = h.load(&story).unwrap_err();
    assert!(e.message.contains("may only define functions"), "{e}");
}
