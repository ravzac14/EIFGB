// The editor's tools: what the history calls each change, Find in game,
// tidying the map and spotting ways that are hard to read, hints drafted
// from the puzzle chain, and the timer timeline. Run: node test/editor-tools.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { Model } from "../src/editor/model.ts";
import { describeChange, when } from "../src/editor/history.ts";
import { findInGame } from "../src/editor/find.ts";
import { mapProblems, tidy } from "../src/editor/tidy.ts";
import { layout } from "../src/editor/layout.ts";
import { draftHints } from "../src/editor/hint-drafts.ts";
import { parseTime, timeline, timeText } from "../src/editor/timeline.ts";
import { walkthroughCommands } from "../src/editor/testplay.ts";
import { fileBytes, sizeText } from "../src/pictures.ts";
import { readDescription, wordsHere } from "../src/editor/describe-words.ts";
import { typos } from "../src/editor/spelling.ts";
import { talkLayout } from "../src/editor/talk-graph.ts";
import { readTalk, writeMap } from "../src/editor/layout.ts";
import { loadEngine } from "../src/engine.ts";

const spec = (p) => readFileSync(new URL(`../../spec/${p}`, import.meta.url), "utf8");
const snack = spec("examples/midnight_snack/game.yaml");

// ------------------------------------------------------------- history

{
  const m = new Model(snack);
  const before = { "game.yaml": snack };
  m.set(["rooms", "pantry", "name"], "Larder");
  assert.equal(describeChange(before, { "game.yaml": m.toString() }), "Changed the room Larder");
  const id = m.addRoom("Attic");
  assert.equal(describeChange(before, { "game.yaml": m.toString() }), "Added the room Attic, and changed the room Larder");
  const added = new Model(snack);
  added.addRoom("Attic");
  assert.equal(describeChange(before, { "game.yaml": added.toString() }), "Added the room Attic");
  const gone = new Model(snack);
  gone.deleteThing("apple");
  assert.match(describeChange(before, { "game.yaml": gone.toString() }), /^Deleted the thing red apple/);
  const renamed = new Model(snack);
  renamed.renameRoom("pantry", "larder");
  assert.match(describeChange(before, { "game.yaml": renamed.toString() }), /^Renamed the room Pantry's ID to larder$/);
  // Only the map: rooms moved.
  assert.equal(describeChange({ ...before, "map.yaml": "a" }, { ...before, "map.yaml": "b" }), "Moved rooms on the map");
  // A new room is placed on the map too, but that's not worth saying.
  assert.equal(describeChange({ ...before, "map.yaml": "a" }, { "game.yaml": added.toString(), "map.yaml": "b" }), "Added the room Attic");
  assert.equal(describeChange(before, { ...before, "scripts/x.lua": "-- hi" }), "Added scripts/x.lua");
  assert.equal(describeChange(before, { ...before, "walkthrough.txt": "look" }), "Saved the walkthrough");
  assert.equal(describeChange(before, { ...before, "pictures/attic.jpg": "data:image/jpeg;base64,AAAA" }), "Added the picture attic.jpg");
  // Several: the first, and how many more.
  const many = new Model(snack);
  for (const n of ["A", "B", "C"]) many.addRoom(n);
  assert.equal(describeChange(before, { "game.yaml": many.toString() }), "Added the room A, and 2 more changes");
  assert.equal(describeChange(before, before), "A change");
  assert.ok(id);
  assert.equal(when(Date.now() - 5000), "just now");
  assert.equal(when(Date.now() - 5 * 60_000), "5 minutes ago");
  assert.match(when(Date.now() - 3 * 3600_000), /^at \d\d:\d\d$/);
}

// ---------------------------------------------------------------- find

{
  const files = { "game.yaml": spec("examples/lighthouse/game.yaml"), "scripts/helpers.lua": spec("examples/lighthouse/scripts/helpers.lua"), "walkthrough.txt": "search pool\nn\n" };
  const found = findInGame(files, "brass");
  assert.ok(found.length >= 2, JSON.stringify(found));
  const key = found.find((f) => f.path === "objects.brass_key.name");
  assert.ok(key, "the key's name");
  assert.equal(key.where, "Thing brass key");
  assert.equal(key.text.slice(key.at, key.at + key.length).toLowerCase(), "brass");
  // IDs too, and the parts of a script, each once.
  assert.ok(findInGame(files, "brass_key").some((f) => f.field === "ID"));
  // Case doesn't matter.
  assert.deepEqual(findInGame(files, "BRASS").map((f) => f.path), found.map((f) => f.path));
  // Scripts and the walkthrough, by line.
  const fn = findInGame(files, "function")[0];
  assert.equal(fn.path, "scripts/helpers.lua");
  assert.ok(fn.line >= 1);
  assert.match(fn.where, /^scripts\/helpers\.lua, line \d+$/);
  assert.equal(findInGame(files, "search pool").find((f) => f.path === "walkthrough.txt").line, 1);
  assert.deepEqual(findInGame(files, "   "), []);
  assert.deepEqual(findInGame(files, "xyzzy-not-there"), []);
  // Long text is cut around the match.
  const long = findInGame({ "game.yaml": `rooms:\n  r:\n    name: R\n    description: ${"a ".repeat(100)}needle ${"b ".repeat(100)}\n` }, "needle")[0];
  assert.ok(long.text.length < 120 && long.text.startsWith("…") && long.text.endsWith("…"), long.text);
  assert.equal(long.text.slice(long.at, long.at + 6), "needle");
  // A file with a mistake is searched as text.
  assert.equal(findInGame({ "game.yaml": "rooms: [\nneedle" }, "needle")[0].line, 2);
}

// ------------------------------------------------------------ the map

{
  // Four rooms in a ring, drawn so that two ways cross, and one drawn backwards.
  const m = new Model(`format: eifgb/0.1
game: { id: t, title: T, version: '1', start: a }
rooms:
  a: { name: A, exits: { east: b, south: c } }
  b: { name: B, exits: { west: a, south: d } }
  c: { name: C, exits: { north: a, east: d } }
  d: { name: D, exits: { north: b, west: c } }
`);
  const neat = layout(m, new Map());
  assert.deepEqual(mapProblems(m, neat), [], "laid out by direction, nothing to say");
  // Swap B and D: A–B and C–D now cross... and B's south leads up.
  const tangled = new Map(neat);
  tangled.set("b", neat.get("d"));
  tangled.set("d", { ...neat.get("b"), colour: "green", w: 200 });
  const hard = mapProblems(m, tangled);
  assert.ok(hard.some((p) => p.kind === "cross"), JSON.stringify(hard));
  assert.ok(hard.some((p) => p.kind === "backwards" && p.a.room === "b" && p.a.dir === "south"), JSON.stringify(hard));
  // Tidy lays it out again, keeping each room's colour and size.
  const tidied = tidy(m, tangled);
  assert.deepEqual(mapProblems(m, tidied), []);
  assert.equal(tidied.get("d").colour, "green");
  assert.equal(tidied.get("d").w, 200);
  assert.deepEqual({ x: tidied.get("a").x, y: tidied.get("a").y }, { x: neat.get("a").x, y: neat.get("a").y });
  // A way drawn straight through another room.
  const line = new Model(`format: eifgb/0.1
game: { id: t, title: T, version: '1', start: a }
rooms:
  a: { name: A, exits: { east: c } }
  b: { name: B }
  c: { name: C, exits: { west: a } }
`);
  const through = mapProblems(line, new Map([["a", { x: 0, y: 0 }], ["b", { x: 210, y: 0 }], ["c", { x: 420, y: 0 }]]));
  assert.deepEqual(through.map((p) => [p.kind, p.room]), [["through", "b"]]);
}

// ------------------------------------------------------- hint drafts

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
{
  const chain = engine.check({ "game.yaml": snack }).puzzles.steps;
  const drafts = draftHints(new Model(snack), chain);
  assert.deepEqual(drafts.map((d) => d.when), ["not world.small_key.seen", "world.pantry_door.locked"]);
  assert.deepEqual(drafts[0].steps, ["A small key might be useful. Where could it be?", "Take a closer look at the worn mat, in the Kitchen.", "SEARCH THE WORN MAT."]);
  assert.deepEqual(drafts[1].steps, ["The pantry door is locked. What might open it?", "Try the small key.", "UNLOCK THE PANTRY DOOR WITH THE SMALL KEY."]);
  // Taken from someone: a different wording.
  const dungeon = spec("templates/locked_dungeon/game.yaml");
  const d = draftHints(new Model(dungeon), engine.check({ "game.yaml": dungeon }).puzzles.steps);
  assert.equal(d[0].steps[2], "TAKE THE IRON KEY FROM THE GUARD.");
  // Added to the game, they compile, and HINT shows the first.
  const m = new Model(snack);
  for (const h of drafts) {
    const id = m.addHint(h.name);
    m.set(["hints", id, "when"], h.when);
    m.setHintSteps(id, h.steps);
  }
  const files = { "game.yaml": m.toString() };
  assert.deepEqual(engine.check(files).diagnostics.filter((x) => x.severity === "error"), []);
  engine.load(files, "game.yaml", 1);
  assert.match(engine.input("hint").lines.map((l) => l.text).join(" "), /small key might be useful/);
}

// ---------------------------------------------------------- timeline

{
  assert.equal(parseTime("7:30 pm"), 19 * 60 + 30);
  assert.equal(parseTime("7pm"), 19 * 60);
  assert.equal(parseTime("12:00 am"), 0);
  assert.equal(parseTime("noon"), 720);
  assert.equal(parseTime("dusk"), 1080);
  assert.equal(parseTime(7), 420);
  assert.equal(parseTime("half past"), null);
  assert.equal(timeText(19 * 60 + 5), "7:05 pm");
  assert.equal(timeText(0), "12:00 am");

  const t = timeline(new Model(spec("examples/lighthouse/game.yaml")));
  assert.deepEqual(t.timers.map((x) => [x.id, x.schedule, x.marks]), [
    ["ship_closer", "once, after 40 actions", [{ from: 40, to: 40 }]],
    ["shipwreck", "once, after 60 actions", [{ from: 60, to: 60 }]],
  ]);
  assert.deepEqual(t.timers[1].does, ["ends the game (lost)"]);
  assert.deepEqual(t.timers[1].unless, ["only if not vars.ship_contacted"]);
  assert.ok(t.span >= 60 && t.span <= 80, String(t.span));
  assert.equal(t.clock, undefined);

  // A clock: times of day, a random wait, a repeating timer, and one started later.
  const c = timeline(new Model(`format: eifgb/0.1
game: { id: t, title: T, version: '1', start: a }
clock: { start: "8:00", minutes_per_turn: 5 }
rooms:
  a: { name: A }
timers:
  bell: { at: "9:00", say: Ding. }
  storm: { after: 5-10, say: Thunder., end: wet }
  drip: { every: 4, say: Drip. }
  fuse: { after: 3, started_by: { verb: light }, noise: { loudness: 3 } }
endings:
  wet: { kind: loss, text: Soaked. }
`));
  const by = Object.fromEntries(c.timers.map((x) => [x.id, x]));
  assert.deepEqual(c.clock, { start: 480, perTurn: 5 });
  assert.equal(by.bell.schedule, "each day at 9:00 am");
  assert.deepEqual(by.bell.marks[0], { from: 12, to: 12 }, "an hour at five minutes an action");
  assert.equal(by.storm.schedule, "once, after 5 to 10 actions");
  assert.deepEqual(by.storm.marks, [{ from: 5, to: 10 }]);
  assert.deepEqual(by.storm.does, ["says “Thunder.”", "ends the game (wet)"]);
  assert.equal(by.drip.schedule, "every 4 actions");
  assert.deepEqual(by.drip.marks.slice(0, 3), [{ from: 4, to: 4 }, { from: 8, to: 8 }, { from: 12, to: 12 }]);
  assert.equal(by.fuse.fromStart, false);
  assert.equal(by.fuse.starts, "when the player does: verb light");
  assert.deepEqual(by.fuse.does, ["makes a noise"]);
}

// ------------------------------------------- walkthroughs and pictures

assert.deepEqual(walkthroughCommands("# a comment\nsearch pool\n\n  n  \n"), ["search pool", "n"]);
assert.deepEqual(walkthroughCommands(undefined), []);
assert.equal(fileBytes("data:image/png;base64,AAAA"), 3);
assert.equal(fileBytes("hello"), 5);
assert.equal(sizeText(512), "512 bytes");
assert.equal(sizeText(42_000), "42 KB");
assert.equal(sizeText(1_400_000), "1.4 MB");

// ------------------------------------------------- words in descriptions

{
  const m = new Model(spec("examples/lighthouse/game.yaml"));
  const { known } = wordsHere(m, { room: "shore" });
  assert.equal(known.get("pool"), "tide pool", "a word for something here");
  const { parts, nouns } = readDescription("Waves break against black rocks. A tide pool shivers in a hollow nearby. A narrow path climbs north to the lighthouse, its lamp dark.", known, new Set(["rocky", "shore"]));
  assert.deepEqual(parts.filter((p) => p.kind === "known").map((p) => p.text), ["tide", "pool"]);
  assert.deepEqual(nouns, ["hollow", "path", "lighthouse", "lamp"]);
  // Plurals name the same thing; `${…}` is left alone; vague words aren't things.
  const k = new Map([["key", "brass key"]]);
  assert.deepEqual(readDescription("Two keys hang by the door.", k).parts.filter((p) => p.kind === "known").map((p) => p.text), ["keys"]);
  // The words shown inside ${if} count; the Lua in ${…} doesn't.
  assert.deepEqual(readDescription("${if the vars.x}The candle${end} burns at the end of the table.", new Map()).nouns, ["candle", "table"]);
}

// ------------------------------------------------------------ spelling

{
  const game = `format: eifgb/0.1
game: { id: t, title: T, version: '1', start: hall }
rooms:
  hall: { name: Hall, description: "A lantern hangs here. The lantern is lit. Beside the lantren, a door." }
objects:
  lamp: { name: brass lamp, in: hall, description: "A brass lamp. Its brass is dull." }
  door: { name: door, in: hall, description: "The doors are shut, the door is red." }
`;
  assert.deepEqual(typos(game).map((t) => [t.word, t.like, t.path]), [["lantren", "lantern", "rooms.hall.description"]]);
  // The game's own names are always allowed, and code isn't prose.
  assert.deepEqual(typos(game.replace("lantren", "lanterns")), []);
  assert.deepEqual(typos(`${game.replace("lantren", "lantern")}  wick: { name: lantrn, in: hall, on: { take: 'say("lantren")' } }\n`), []);
}

// ------------------------------------------------- conversations as a graph

{
  const m = new Model(spec("examples/lighthouse/game.yaml"));
  const c = m.choice("keeper_talk");
  const places = talkLayout(c, undefined);
  assert.equal(places.size, c.nodes.length);
  assert.deepEqual(places.get(c.start), { x: 0, y: 0 }, "the first part first");
  for (const n of c.nodes.filter((n) => n.id !== c.start)) assert.ok(places.get(n.id).x > 0, `${n.id} after the start`);
  // Placed by hand: kept in map.yaml, beside the rooms, for parts that still exist.
  const talk = new Map([["keeper_talk", new Map([[c.start, { x: 40, y: 50 }], ["gone", { x: 1, y: 1 }]])]]);
  const text = writeMap(new Map(), [], talk, { keeper_talk: c.nodes.map((n) => n.id) });
  assert.match(text, /talk:\n {2}keeper_talk:\n {4}\w+: \[40, 50\]\n$/);
  assert.deepEqual(readTalk(text).get("keeper_talk").get(c.start), { x: 40, y: 50 });
  assert.equal(talkLayout(c, readTalk(text).get("keeper_talk")).get(c.start).x, 40);
}

console.log("ok   editor tools");
