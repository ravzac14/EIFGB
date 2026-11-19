// The player's map: what the engine reports as visited, how rooms are
// placed, and what gets drawn. Run: node test/player-map.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";
import { authorPositions, drawMap, placeRooms } from "../src/player-map.ts";

const game = `
format: eifgb/0.1
game: { id: maptest, title: Map test, version: "1", start: hall }
rooms:
  hall:
    name: Hall
    description: A hall.
    exits: { north: study, east: { to: cellar, through: trapdoor }, west: garden }
  study: { name: Study, description: A study., exits: { south: hall, up: attic } }
  attic: { name: Attic, description: An attic., exits: { down: study } }
  cellar: { name: Cellar, description: A cellar., exits: { west: { to: hall, through: trapdoor } } }
  garden: { name: Garden, description: A garden., exits: { east: hall } }
objects:
  trapdoor: { name: trapdoor, in: hall, traits: [door, openable, scenery] }
`;
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const loaded = engine.load({ "game.yaml": game }, "game.yaml", 1);
assert.ok(loaded.output, JSON.stringify(loaded.diagnostics));

// At the start: one room, with unexplored ways out (the closed door shown as such).
let m = engine.map();
assert.equal(m.here, "hall");
assert.deepEqual(m.rooms.map((r) => r.id), ["hall"]);
assert.ok(m.exits.every((e) => !e.to), "nothing beyond the hall is known yet");
assert.deepEqual(m.exits.find((e) => e.dir === "east"), { from: "hall", dir: "east", door: "trapdoor", closed: true });

// After walking north and up: three rooms, joined.
engine.input("north");
engine.input("up");
m = engine.map();
assert.equal(m.here, "attic");
assert.deepEqual(new Set(m.rooms.map((r) => r.id)), new Set(["hall", "study", "attic"]));
assert.ok(m.exits.some((e) => e.from === "study" && e.dir === "up" && e.to === "attic"));
assert.ok(m.exits.some((e) => e.from === "hall" && e.dir === "west" && !e.to), "the garden isn't known yet");

// Placed by compass: the study north of the hall; nothing overlaps.
const pos = placeRooms(m, new Map());
assert.ok(pos.get("study").y < pos.get("hall").y && pos.get("study").x === pos.get("hall").x);
assert.equal(new Set([...pos.values()].map((p) => `${p.x},${p.y}`)).size, pos.size);
// Author positions (map.yaml from the editor) win.
const author = authorPositions("# comment\nrooms:\n  hall: [420, 260]\n  study: [0, 0]\n");
assert.deepEqual(placeRooms(m, author).get("hall"), { x: 320, y: 180 });
// A room resized in the editor is drawn bigger: 300 wide there is 248 here.
const big = authorPositions("rooms:\n  hall: [0, 0, 300, 112]\n");
assert.deepEqual(big.get("hall"), { x: 0, y: 0, w: 248, h: 88 });
// Room colours from the editor.
const tinted = authorPositions("rooms:\n  hall: [0, 0]\n  study: [0, -130]\ncolours:\n  hall: green\n  study: nonsense\n");
assert.equal(tinted.get("hall").colour, "green");
assert.equal(tinted.get("study").colour, undefined);

// Drawn: a box per room, the current one marked. Nothing on the map moves
// the player: every action is typed.
GlobalRegistrator.register();
const box = document.createElement("div");
drawMap(box, m, authorPositions("rooms:\n  hall: [0, 0]\ncolours:\n  hall: green\n"));
assert.equal(box.querySelectorAll(".room").length, 3);
assert.match(box.querySelector(".room.coloured").getAttribute("style"), /--tint: hsl\(125/);
assert.equal(box.querySelector(".room.here title").textContent, "Attic (you are here)");
assert.equal(box.querySelectorAll("[role=button], [tabindex]").length, 0, "nothing on the map is clickable");
assert.ok(box.querySelector(".stub"), "unexplored ways are drawn");
console.log("ok   player map");
