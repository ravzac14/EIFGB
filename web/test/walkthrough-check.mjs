// Recorded walkthroughs: the editor's check replays walkthrough.txt on a
// game of its own and warns when it no longer wins; test play records
// what's typed and saves it. Run: node test/walkthrough-check.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";

const root = fileURLToPath(new URL("../../", import.meta.url));
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const dir = `${root}spec/templates/small_house/`;
const game = readFileSync(`${dir}game.yaml`, "utf8");
const steps = readFileSync(`${dir}walkthrough.txt`, "utf8");
// The starter's picture, as the site keeps it.
const pics = { "pictures/fridge_note.svg": `data:image/svg+xml;base64,${readFileSync(`${dir}pictures/fridge_note.svg`).toString("base64")}` };

// No walkthrough: nothing to say.
assert.equal(engine.check({ ...pics, "game.yaml": game }).walkthrough, undefined);
// One that wins.
let c = engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": steps });
assert.deepEqual({ result: c.walkthrough.result, steps: c.walkthrough.steps, line: c.walkthrough.line, ending: c.walkthrough.ending }, { result: "win", steps: 8, line: 9, ending: "out_the_door" });
assert.equal(c.walkthrough.problem, undefined);
assert.equal(c.diagnostics.length, 0);
// What it never reaches: the kitchen, and what's in it.
assert.deepEqual(c.walkthrough.never.rooms, ["kitchen"]);
assert.ok(c.walkthrough.never.things.includes("toast"), JSON.stringify(c.walkthrough.never));
// Expected replies: one that's said, and one that isn't.
c = engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": steps.replace("search sofa", "search sofa => keys") });
assert.equal(c.walkthrough.result, "win");
c = engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": steps.replace("search sofa", "search sofa => a golden crown") });
assert.equal(c.walkthrough.result, "reply");
assert.match(c.diagnostics[0].message, /^the walkthrough expected “a golden crown” at line \d+ \(search sofa\), but the game said: /);
// A walkthrough for an ending: checked too, each under its own name.
c = engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": steps, "walkthrough-out_the_door.txt": steps, "walkthrough-nowhere.txt": steps });
assert.deepEqual(c.walkthroughs.map((w) => [w.file, w.goal, w.result]), [["walkthrough-nowhere.txt", "nowhere", "win"], ["walkthrough-out_the_door.txt", "out_the_door", "win"], ["walkthrough.txt", null, "win"]]);
assert.deepEqual(c.diagnostics.map((d) => d.path), ["walkthrough-nowhere.txt"]);
assert.match(c.diagnostics[0].message, /^walkthrough-nowhere\.txt no longer reaches the nowhere ending: at line 9 \(s\) the game ends with the out_the_door ending/);
// The wanderer: random moves on a game of its own.
const w = engine.wander({ ...pics, "game.yaml": game }, 3, 300);
assert.equal(w.moves, 300);
assert.deepEqual(w.errors, []);
assert.ok(w.rooms[0] >= 2 && w.rooms[1] >= 5, JSON.stringify(w.rooms));
assert.deepEqual(engine.wander({ ...pics, "game.yaml": game }, 3, 300), w, "the same seed wanders the same way");
assert.match(engine.wander({ "game.yaml": "nope: [" }, 1, 10).error, /mistakes/);
// The keys no longer open the door: still going at the end, and a warning under Problems.
c = engine.check({ ...pics, "game.yaml": game.replace("unlocks_with: keys", "unlocks_with: toast"), "walkthrough.txt": steps });
assert.equal(c.walkthrough.result, "unfinished");
assert.deepEqual(
  c.diagnostics.map((d) => [d.severity, d.path]),
  [["warning", "walkthrough.txt"]],
);
assert.match(c.diagnostics[0].message, /no longer wins: after its 8 commands the game is still going/);
// An ending that isn't a win, with where it happened.
c = engine.check({ ...pics, "game.yaml": game.replace("kind: win", "kind: loss"), "walkthrough.txt": steps });
assert.equal(c.walkthrough.result, "loss");
assert.match(c.walkthrough.problem, /at line 9 \(s\) the game ends in a loss/);
// A broken game isn't played at all.
c = engine.check({ ...pics, "game.yaml": game.replace("start: bedroom", "start: nowhere"), "walkthrough.txt": steps });
assert.equal(c.walkthrough, undefined);
// The game in progress isn't disturbed by a check.
engine.load({ "game.yaml": game }, "game.yaml", 1);
engine.input("s");
engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": steps });
assert.match(engine.input("look").status.room, /Hallway/);

// Test play records what's typed, from the start only.
GlobalRegistrator.register();
window.confirm = () => true;
const { TestPlay } = await import("../src/editor/testplay.ts");
const saved = [];
const panel = document.createElement("div");
const selected = { id: "kitchen", name: "Kitchen" };
const tp = new TestPlay(panel, {
  engine: async () => engine,
  files: () => ({ "game.yaml": game }),
  selectedRoom: () => selected,
  markHere: () => {},
  showProblems: () => {},
  saveWalkthrough: (t, file) => saved.push(t) && files.push(file ?? "walkthrough.txt"),
  nameOf: (id) => id,
  show: () => {},
});
const files = [];
const button = (text) => [...panel.querySelectorAll("button")].find((b) => b.textContent === text);
const type = (text) => {
  panel.querySelector(".tp-prompt input").value = text;
  panel.querySelector(".tp-prompt").requestSubmit();
};
assert.match(panel.querySelector(".tp-walk").textContent, /No walkthrough yet/);
await tp.start();
assert.equal(button("Save as walkthrough").disabled, true, "nothing typed yet");
for (const s of steps.split("\n").filter((l) => l && !l.startsWith("#"))) type(s);
assert.equal(button("Save as walkthrough").disabled, false);
button("Save as walkthrough").click();
assert.equal(saved.length, 1);
assert.match(saved[0], /^# Recorded in test play, \d{4}-\d\d-\d\d\./);
assert.equal(saved[0].split("\n").slice(2).join("\n"), "s\ne\nsearch sofa\ntake keys\nw\nunlock door\nopen door\ns\n");
// It reached an ending: that can be saved as the ending's walkthrough.
assert.equal(button("Save for the out_the_door ending").hidden, false);
button("Save for the out_the_door ending").click();
assert.equal(files[1], "walkthrough-out_the_door.txt");
assert.match(saved[1], /It should reach the out_the_door ending/);
// What's saved wins, as the check sees it.
assert.equal(engine.check({ ...pics, "game.yaml": game, "walkthrough.txt": saved[0] }).walkthrough.result, "win");
// Started somewhere else: not a walkthrough.
await tp.start("kitchen");
type("e");
assert.equal(button("Save as walkthrough").disabled, true);
// The check's verdict, shown in the panel.
tp.walkthrough({ result: "win", steps: 8, line: 9, problem: null });
assert.match(panel.querySelector(".tp-walk").textContent, /✓ The walkthrough still wins, in 8 commands/);
tp.walkthrough({ result: "unfinished", steps: 8, problem: "the walkthrough no longer wins: after its 8 commands the game is still going" });
assert.match(panel.querySelector(".tp-walk").textContent, /✗ The walkthrough no longer wins/);
console.log("ok   walkthrough check");
