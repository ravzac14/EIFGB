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
assert.equal(directionBetween({ x: 0, y: 0 }, { x: 0, y: -100 }), "north");
assert.equal(directionBetween({ x: 0, y: 0 }, { x: 100, y: 90 }), "southeast");
void CELL;
console.log("ok   editor layout");
