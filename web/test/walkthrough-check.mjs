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

// No walkthrough: nothing to say.
assert.equal(engine.check({ "game.yaml": game }).walkthrough, undefined);
// One that wins.
let c = engine.check({ "game.yaml": game, "walkthrough.txt": steps });
assert.deepEqual(c.walkthrough, { result: "win", steps: 8, line: 9, problem: null });
assert.equal(c.diagnostics.length, 0);
// The keys no longer open the door: still going at the end, and a warning under Problems.
c = engine.check({ "game.yaml": game.replace("unlocks_with: keys", "unlocks_with: toast"), "walkthrough.txt": steps });
assert.equal(c.walkthrough.result, "unfinished");
assert.deepEqual(
  c.diagnostics.map((d) => [d.severity, d.path]),
  [["warning", "walkthrough.txt"]],
);
assert.match(c.diagnostics[0].message, /no longer wins: after its 8 commands the game is still going/);
// An ending that isn't a win, with where it happened.
c = engine.check({ "game.yaml": game.replace("kind: win", "kind: loss"), "walkthrough.txt": steps });
assert.equal(c.walkthrough.result, "loss");
assert.match(c.walkthrough.problem, /at line 9 \(s\) the game ends in a loss/);
// A broken game isn't played at all.
c = engine.check({ "game.yaml": game.replace("start: bedroom", "start: nowhere"), "walkthrough.txt": steps });
assert.equal(c.walkthrough, undefined);
// The game in progress isn't disturbed by a check.
engine.load({ "game.yaml": game }, "game.yaml", 1);
engine.input("s");
engine.check({ "game.yaml": game, "walkthrough.txt": steps });
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
  saveWalkthrough: (t) => saved.push(t),
});
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
assert.equal(saved[0].split("\n").slice(1).join("\n"), "s\ne\nsearch sofa\ntake keys\nw\nunlock door\nopen door\ns\n");
// What's saved wins, as the check sees it.
assert.equal(engine.check({ "game.yaml": game, "walkthrough.txt": saved[0] }).walkthrough.result, "win");
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
