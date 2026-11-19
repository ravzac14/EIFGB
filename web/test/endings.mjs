// Endings found: the engine says which ending was reached and lists them
// all; the site keeps the ones a player has found, per game.
// Run: node test/endings.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const game = `
format: eifgb/0.1
game: { id: two, title: Two ways, version: "1", start: hall }
rooms:
  hall: { name: Hall, exits: { north: garden, south: pit } }
  garden: { name: Garden, on: { enter: end_game("free") } }
  pit: { name: Pit, on: { enter: end_game("fell") } }
endings:
  free: { kind: win, text: Free at last. }
  fell: { kind: loss, text: Down you go. }
  hidden: { kind: neutral, text: A secret way. }
`;
const loaded = engine.load({ "game.yaml": game }, "game.yaml", 1);
assert.deepEqual(loaded.meta.endings, ["free", "fell", "hidden"]);
assert.equal(loaded.output.ending, null);
const out = engine.input("north");
assert.equal(out.ended, "win");
assert.equal(out.ending, "free");

GlobalRegistrator.register();
const { endingsFor, endingsText } = await import("../src/store.ts");
const all = loaded.meta.endings;
assert.equal(endingsText("local:two"), null, "nothing before the first");
endingsFor("local:two").record("free", all);
assert.equal(endingsText("local:two"), "You've found 1 of 3 endings.");
endingsFor("local:two").record("free", all);
endingsFor("local:two").record("fell", all);
assert.equal(endingsText("local:two"), "You've found 2 of 3 endings.");
endingsFor("local:two").record("hidden", all);
assert.equal(endingsText("local:two"), "You've found all 3 endings.");
// A new version of the game without one of them: it's forgotten.
endingsFor("local:two").record("free", ["free", "fell"]);
assert.equal(endingsText("local:two"), "You've found both endings.");
// A game with one ending.
endingsFor("local:one").record("end", ["end"]);
assert.equal(endingsText("local:one"), "You've found the ending.");
console.log("ok   endings found");
