// The editor's YAML model: edits keep comments and formatting, and the
// results compile. Run: node test/editor-model.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { Model, NEW_GAME, slug } from "../src/editor/model.ts";
import { loadEngine } from "../src/engine.ts";

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const errors = (text, extra = {}) => engine.check({ "game.yaml": text, ...extra }).diagnostics.filter((d) => d.severity === "error");
const helpers = { "scripts/helpers.lua": readFileSync(new URL("../../spec/examples/lighthouse/scripts/helpers.lua", import.meta.url), "utf8") };
const example = (id) => readFileSync(new URL(`../../spec/examples/${id}/game.yaml`, import.meta.url), "utf8");

// Untouched, every example comes back exactly as written.
for (const id of ["midnight_snack", "blackfang", "lighthouse", "campfire"]) {
  assert.equal(new Model(example(id)).toString(), example(id), `${id} round-trips`);
}

// An edit changes only the lines it touches: [door] stays [door] elsewhere.
const touched = new Model(example("midnight_snack"));
touched.set(["rooms", "pantry", "name"], "Larder");
const diff = touched.toString().split("\n").filter((l, i) => l !== example("midnight_snack").split("\n")[i]);
assert.deepEqual(diff, ["    name: Larder"]);

// Reading the map.
const snack = new Model(example("midnight_snack"));
assert.deepEqual(snack.roomIds(), ["kitchen", "pantry"]);
const kitchenExit = snack.exits("kitchen").find((e) => e.to === "pantry");
assert.equal(kitchenExit.kind, "door");
assert.equal(snack.door(kitchenExit.door).locked, true);

// A new game compiles, and grows.
const m = new Model(NEW_GAME);
assert.deepEqual(errors(m.toString()), []);
const hall = m.addRoom("Great Hall");
assert.equal(hall, "great_hall");
m.connect("first_room", "north", hall);
assert.equal(m.exit(hall, "south").to, "first_room", "two-way by default");
const cellar = m.addRoom("Cellar");
m.connect(hall, "down", cellar, false);
assert.equal(m.exit(cellar, "up"), undefined, "one-way when asked");
assert.deepEqual(errors(m.toString()), []);

// Doors go on both sides; locking one makes it lockable; it can be taken away.
const door = m.addDoor("first_room", "north", "oak door");
assert.equal(m.exit("first_room", "north").door, door);
assert.equal(m.exit(hall, "south").door, door);
m.setDoorState(door, "locked");
m.tidyState(["objects", door]);
assert.ok(m.door(door).lockable && m.door(door).locked);
assert.match(m.toString(), /state: \{locked: true\}/);
assert.match(m.toString(), /traits: \[door, openable, scenery, lockable\]/);
assert.deepEqual(errors(m.toString()), []);
m.setDoorState(door, "open");
assert.ok(!m.door(door).lockable && m.door(door).open);
m.removeDoor(door);
assert.equal(m.exit(hall, "south").kind, "plain");
assert.equal(m.door(door), null);

// Renaming a room's ID follows every reference; deleting cleans up.
assert.equal(m.renameRoom("first_room", "Entrance"), "entrance");
assert.equal(m.start(), "entrance");
assert.equal(m.exit(hall, "south").to, "entrance");
assert.equal(m.renameRoom(hall, "cellar"), null, "can't take an ID in use");
assert.deepEqual(errors(m.toString()), []);
m.deleteRoom(hall);
assert.deepEqual(m.exits("entrance"), []);
assert.deepEqual(errors(m.toString()), []);

// Editing an example keeps its comments, and it still compiles.
const light = new Model(example("lighthouse"));
const before = light.toString();
const extra = light.addRoom("Lamp Room");
light.connect(light.start(), "up", extra);
light.set(["rooms", extra, "description"], "Glass on every side.");
const after = light.toString();
for (const line of before.split("\n").filter((l) => l.trim().startsWith("#"))) assert.ok(after.includes(line), `comment kept: ${line}`);
assert.deepEqual(errors(after, helpers), []);
// Renaming a room with a door on it, in a real game.
const black = new Model(example("blackfang"));
const gateSide = black.doors()[0].in;
const renamed = black.renameRoom(gateSide, "gate yard");
assert.deepEqual(errors(black.toString()), [], `renamed ${gateSide} → ${renamed}`);

assert.equal(slug("Dark Cellar!"), "dark_cellar");
assert.equal(slug("1st floor"), "room_1st_floor");
console.log("ok   editor model");

// ------------------------------------------------------------- objects
{
  const s = new Model(example("midnight_snack"));
  assert.deepEqual(s.contents("kitchen").map((t) => t.id), ["mat", "small_key", "pie"], "doors aren't things");
  const key = s.thing("small_key");
  assert.ok(key.hidden && key.revealedBy[0] === "search mat" && key.traits.includes("portable"));
  assert.equal(s.roomOf("small_key"), "kitchen");

  const g = new Model(NEW_GAME);
  const box = g.addThing("container", "first_room", "wooden box");
  const coin = g.addThing("carry", box, "gold coin");
  const table = g.addThing("supporter", "first_room");
  assert.equal(g.roomOf(coin), "first_room");
  assert.match(g.toString(), /wooden_box:\n\s+name: wooden box\n\s+in: first_room\n\s+traits: \[container, openable\]/);
  // Moving: onto a table, to the player, nowhere; never into itself or what it holds, or into something that can't hold things.
  assert.ok(g.moveThing(box, table));
  assert.equal(g.moveThing(table, coin), false, "a coin can't hold things");
  assert.equal(g.moveThing(table, box), false, "the table is under the box");
  assert.ok(g.moveThing(coin, "player"));
  assert.ok(g.moveThing(coin, null));
  assert.equal(g.thing(coin).in, null);
  assert.ok(g.moveThing(coin, box));
  assert.deepEqual(errors(g.toString()), []);

  // Traits that go together come and go together.
  g.setTrait(box, "lockable", true);
  assert.ok(g.thing(box).traits.includes("openable"));
  g.setState(box, "locked", true);
  g.setTrait(box, "openable", false);
  assert.ok(!g.thing(box).traits.includes("lockable") && !g.thing(box).locked);
  g.setPortability(table, "portable");
  assert.deepEqual(g.thing(table).traits, ["supporter", "portable"]);
  g.setPortability(table, "fixed");
  assert.deepEqual(g.thing(table).traits, ["supporter"]);
  g.setList(["objects", coin, "synonyms"], ["coin", " money ", ""]);
  assert.match(g.toString(), /synonyms: \[coin, money\]/);
  assert.deepEqual(errors(g.toString()), []);

  // Renaming a key, a hidden thing's revealer, and deleting a holder.
  const k = new Model(example("midnight_snack"));
  assert.equal(k.renameThing("small_key", "brass key"), "brass_key");
  assert.equal(k.door("pantry_door").key, "brass_key");
  assert.equal(k.renameThing("mat", "rug"), "rug");
  assert.deepEqual(k.thing("brass_key").revealedBy, ["search rug"]);
  assert.deepEqual(errors(k.toString()), []);
  g.deleteThing(box);
  assert.equal(g.thing(coin).in, table, "the coin falls where the box was");
  assert.deepEqual(errors(g.toString()), []);
}
console.log("ok   editor objects");

// --------------------------------------------------- characters, factions
{
  const b = new Model(example("blackfang"));
  assert.deepEqual(b.factionIds(), ["wolves", "goblins", "undead"]);
  const wolves = b.faction("wolves");
  assert.ok(wolves.members.includes("alpha"));
  assert.ok(b.hostile(b.character("skeleton")), "a guard is hostile");

  const g = new Model(NEW_GAME);
  const shop = g.addCharacter("merchant", "first_room", "Old Meg");
  assert.equal(shop, "old_meg");
  const wolf = g.addCharacter("hunter", "first_room");
  assert.equal(g.get(["player", "health"]), 10, "enemies give the player health");
  assert.equal(g.behaviorOf(g.character(wolf)), "hunter");
  const coin = g.addThing("carry", shop, "brass coin");
  assert.equal(g.roomOf(coin), "first_room", "carried by someone in the room");
  g.set(["objects", coin, "price"], 3);
  assert.deepEqual(errors(g.toString()), []);

  // Sides: wolves hate goblins and the player.
  const w = g.addFaction("Wolves");
  const gob = g.addFaction("goblins");
  g.set(["characters", wolf, "faction"], w);
  g.setHostile(w, gob, true);
  g.setHostile(w, "player", true);
  assert.deepEqual(g.faction(w).hostileTo, [gob, "player"]);
  assert.match(g.toString(), /wolves:\n\s+hostile_to: \[goblins, player\]/);
  assert.deepEqual(errors(g.toString()), []);
  // Renaming a side follows its members and enemies; deleting removes them.
  assert.equal(g.renameFaction(gob, "orcs"), "orcs");
  assert.deepEqual(g.faction(w).hostileTo, ["orcs", "player"]);
  assert.equal(g.renameFaction("orcs", "player"), null);
  assert.equal(g.renameFaction(w, "pack"), "pack");
  assert.equal(g.character(wolf).faction, "pack");
  g.deleteFaction("orcs");
  assert.deepEqual(g.faction("pack").hostileTo, ["player"]);
  assert.deepEqual(errors(g.toString()), []);
  g.deleteFaction("pack");
  assert.equal(g.character(wolf).faction, null);
  assert.doesNotMatch(g.toString(), /factions:/);

  // Renaming a character keeps its belongings; deleting one leaves them in its room.
  assert.equal(g.renameCharacter(shop, "meg"), "meg");
  assert.equal(g.thing(coin).in, "meg");
  g.deleteCharacter("meg");
  assert.equal(g.thing(coin).in, "first_room");
  assert.deepEqual(errors(g.toString()), []);
}
console.log("ok   editor characters");

// ------------------------------------------- scripts, timers, endings, choices
{
  // Handlers: existing ones read; one-liners plain, several lines as a block.
  const s = new Model(example("midnight_snack"));
  assert.deepEqual(s.handlers(["objects", "pie"]).map((h) => h.event), ["before_take", "before_eat", "after_eat"]);
  s.setHandler(["objects", "apple"], "after_eat", 'end_game("fed")\nscore(1)');
  assert.match(s.toString(), /after_eat: \|\n\s+end_game\("fed"\)\n\s+score\(1\)\n/);
  s.setHandler(["rooms", "pantry"], "enter", 'say("It smells of apples.")');
  assert.match(s.toString(), /pantry:[\s\S]*on:\n\s+enter: say\("It smells of apples."\)/);
  s.setHandler([], "start", 'say("Hello.")');
  assert.match(s.toString(), /\non:\n  start: say\("Hello."\)/);
  assert.deepEqual(errors(s.toString()), []);
  assert.ok(s.renameHandler(["rooms", "pantry"], "enter", "leave"));
  s.setHandler([], "start", "");
  assert.doesNotMatch(s.toString(), /\non:\n/, "an empty game-wide on: goes away");
  assert.deepEqual(errors(s.toString()), []);

  // Endings and timers: renaming an ending follows its timers.
  const g = new Model(NEW_GAME);
  const won = g.addEnding("Escaped");
  const t = g.addTimer("storm");
  g.set(["timers", t, "end"], won);
  assert.deepEqual(errors(g.toString()), []);
  assert.equal(g.renameEntry("endings", won, "free"), "free");
  assert.equal(g.get(["timers", t, "end"]), "free");
  g.addVar("gold", 0);
  g.setHandler(["rooms", "first_room"], "enter", "vars.gold = vars.gold + 1");
  assert.deepEqual(errors(g.toString()), []);

  // A conversation: nodes, options, where each goes; renaming and deleting nodes.
  const c = g.addChoice("Chat with Meg");
  assert.equal(c, "chat_with_meg");
  const meg = g.addCharacter("talker", "first_room", "Meg");
  g.set(["characters", meg, "talk"], c);
  const n = g.addNode(c);
  g.addOption(c, "start");
  g.set(["choices", c, "nodes", "start", "options", 1, "text"], "Ask about the weather.");
  g.setOptionNext(c, "start", 1, { goto: n });
  g.set(["choices", c, "nodes", n, "text"], '"Stormy," she says.');
  assert.deepEqual(errors(g.toString()), [], g.toString());
  assert.equal(g.renameNode(c, n, "weather"), "weather");
  assert.equal(g.choice(c).nodes[0].options[1].goto, "weather");
  g.deleteNode(c, "weather");
  assert.ok(g.choice(c).nodes[0].options[1].end, "a link into a deleted node ends the talk");
  assert.equal(g.renameEntry("choices", c, "meg_talk"), "meg_talk");
  assert.equal(g.character(meg).talk, "meg_talk");
  assert.deepEqual(errors(g.toString()), []);
  g.deleteEntry("choices", "meg_talk");
  assert.equal(g.character(meg).talk, null);

  // A custom verb.
  const v = g.addVerb("ring");
  assert.match(g.toString(), /verbs:\n\s+ring:\n\s+patterns: \["ring \{target\}"\]\n\s+default: Nothing happens./);
  assert.deepEqual(errors(g.toString()).filter((e) => !/talk/.test(e.message)), []);
  void v;

  // Hints: steps as a list, a `when`, and an order that can change.
  const h1 = g.addHint("Find the key");
  const h2 = g.addHint("Open the gate");
  assert.deepEqual([h1, h2], ["find_the_key", "open_the_gate"]);
  assert.deepEqual(g.list(["hints", h1, "steps"]), ["A nudge: what should the player notice?", "The answer: what to type."]);
  g.setHintSteps(h1, ["Look around.", "Something's under the ${'mat'}: what is it?", "SEARCH MAT."]);
  g.set(["hints", h1, "when"], "vars.gold == 0");
  assert.match(g.toString(), /hints:\n  find_the_key:\n    steps:\n      - Look around\.\n/);
  assert.deepEqual(errors(g.toString()).filter((e) => !/talk/.test(e.message)), []);
  assert.ok(g.moveEntry("hints", h2, -1));
  assert.deepEqual(g.ids("hints"), [h2, h1]);
  assert.equal(g.moveEntry("hints", h2, -1), false, "already first");
  assert.equal(g.renameEntry("hints", h1, "key"), "key");
  g.deleteEntry("hints", "key");
  g.deleteEntry("hints", h2);
  assert.doesNotMatch(g.toString(), /hints:/);

  // Goals, named from their first words, and the time of day.
  const g1 = g.addGoal("Find the brass key.");
  const g2 = g.addGoal("Find the brass key, again");
  assert.deepEqual([g1, g2], ["find_the_brass", "find_the_brass_2"]);
  g.set(["goals", g1, "done"], "vars.gold > 1");
  g.set(["goals", g1, "points"], 2);
  assert.match(g.toString(), /goals:\n  find_the_brass:\n    text: Find the brass key\.\n    done: vars.gold > 1\n    points: 2\n/);
  g.addClock();
  g.addClock();
  assert.match(g.toString(), /clock:\n  start: "?8:00"?\n  minutes_per_turn: 1\n/);
  g.addTimer();
  g.set(["timers", g.ids("timers").at(-1), "after"], undefined);
  g.set(["timers", g.ids("timers").at(-1), "at"], "dusk");
  assert.deepEqual(errors(g.toString()).filter((e) => !/talk/.test(e.message)), []);
  g.set(["clock"], undefined);
  assert.ok(errors(g.toString()).some((e) => /clock:/.test(e.message)), "a timer at a time needs the clock");
}
console.log("ok   editor scripts");

// ------------------------------------------------------------- layout
const { layout, readMap, writeMap, directionBetween, CELL } = await import("../src/editor/layout.ts");
const fang = new Model(example("blackfang"));
const auto = layout(fang, new Map());
assert.equal(auto.size, fang.roomIds().length, "every room placed");
const cells = new Set([...auto.values()].map((p) => `${p.x},${p.y}`));
assert.equal(cells.size, auto.size, "no two rooms on one spot");
const start = auto.get(fang.start());
assert.deepEqual(start, { x: 0, y: 0 });
const north = fang.exits(fang.start()).find((e) => e.dir === "north");
if (north) assert.ok(auto.get(north.to).y < 0, "north is up");
// Saved positions are kept, and survive the file.
const saved = new Map([[fang.start(), { x: 500, y: -40 }]]);
const text = writeMap(layout(fang, saved), fang.roomIds());
assert.match(text, new RegExp(`${fang.start()}: \\[500, -40\\]`));
assert.deepEqual(readMap(text).get(fang.start()), { x: 500, y: -40 });
assert.equal(readMap("not: [valid").size, 0);
// A resized room keeps its size too; the usual size isn't written.
const sized = writeMap(new Map([["hall", { x: 10, y: 20, w: 300, h: 90 }], ["cupboard", { x: 0, y: 0, w: 150, h: 56 }]]), ["hall", "cupboard"]);
assert.match(sized, /hall: \[10, 20, 300, 90\]\n\s+cupboard: \[0, 0\]/);
assert.deepEqual(readMap(sized).get("hall"), { x: 10, y: 20, w: 300, h: 90 });
assert.equal(directionBetween({ x: 0, y: 0 }, { x: 0, y: -100 }), "north");
assert.equal(directionBetween({ x: 0, y: 0 }, { x: 100, y: 90 }), "southeast");
void CELL;
console.log("ok   editor layout");
