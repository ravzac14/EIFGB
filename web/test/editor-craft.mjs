// Kinds in the editor, the prose check, the pacing chart, and threads in
// the player. Run: node test/editor-craft.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register({ url: "http://localhost/editor.html", width: 1200, height: 800 });
window.confirm = () => true;
const { Model } = await import("../src/editor/model.ts");
const { renderInspector } = await import("../src/editor/inspector.ts");
const { loadEngine } = await import("../src/engine.ts");
const { prose, grade } = await import("../src/editor/prose.ts");
const { pacingChart, dryStretches, brought } = await import("../src/editor/pacing.ts");
const { follow, unfollow, turnsOf, mentions } = await import("../src/threads.ts");

const GAME = `format: eifgb/0.1
game: { id: castle, title: The Castle, version: "1", start: gate }
rooms:
  gate: { name: Gate, description: "You are standing at an ancient gate. Ancient stones rise around you.", exits: { north: hall } }
  hall: { name: Hall, description: "You are standing in an ancient hall, cold and dim.", exits: { south: gate, north: tower } }
  tower: { name: Tower, description: "You are standing in the ancient tower. Wind whistles through the ancient stones.", exits: { south: hall } }
objects:
  lamp: { name: brass lamp, in: gate, traits: [portable, light_source], fuel: 20, size: 1 }
characters:
  guard: { name: guard, in: hall, behavior: guard, health: 5 }
`;

const m = new Model(GAME);
const panel = document.createElement("div");
document.body.append(panel);
const base = {
  model: m,
  selection: null,
  problems: [],
  verbs: [],
  change(fn) {
    fn(m);
    draw();
  },
  select(sel) {
    ctx.selection = sel;
    draw();
  },
  addRoomBeside: () => "",
  deleteRoom: () => {},
  showInFile: () => {},
  focusName: () => {},
  notice: () => {},
  openFile: () => {},
  addScriptFile: () => null,
  addPicture: async () => null,
  pictureUrl: () => null,
  addSound: async () => null,
  soundUrl: () => null,
  sounds: () => [],
  pictures: () => [],
  hasWalkthrough: () => false,
  roomColour: () => null,
};
// Anything else the panels ask of the editor: nothing.
const ctx = new Proxy(base, { get: (t, k) => (k in t ? t[k] : () => null), set: (t, k, v) => ((t[k] = v), true) });
const draw = () => renderInspector(panel, ctx);
const show = (sel) => ctx.select(sel);

const labelled = (text) => {
  const l = [...panel.querySelectorAll("label")].find((x) => x.textContent.trim() === text);
  assert.ok(l, `a "${text}" field`);
  return l;
};
const tick = (text) => {
  const box = labelled(text).querySelector("input") ?? labelled(text).nextElementSibling;
  box.checked = !box.checked;
  box.dispatchEvent(new Event("change"));
};
const typeIn = (text, value) => {
  const l = labelled(text);
  let i = l.nextElementSibling;
  while (i && !/^(INPUT|TEXTAREA)$/.test(i.tagName)) i = i.querySelector?.("input, textarea") ?? i.nextElementSibling;
  i.value = value;
  i.dispatchEvent(new Event("input"));
};
const choose = (label, value) => {
  const s = [...panel.querySelectorAll("select")].find((x) => x.getAttribute("aria-label") === label);
  assert.ok(s, `a "${label}" list`);
  s.value = value;
  s.dispatchEvent(new Event("change"));
};
const button = (text) => {
  const b = [...panel.querySelectorAll("button")].find((x) => x.textContent === text);
  assert.ok(b, `a "${text}" button`);
  b.click();
};


// Kinds: made from a thing, used by another, and deleted written out.
show({ kind: "thing", id: "lamp" });
[...panel.querySelectorAll("input")].find((i) => i.getAttribute("aria-label") === "New kind's name").value = "Lamp";
button("Make a kind of this");
assert.deepEqual(m.kindFields("lamp"), { traits: ["portable", "light_source"], fuel: 20, size: 1 });
assert.equal(m.get(["objects", "lamp", "like"]), "lamp");
assert.equal(m.get(["objects", "lamp", "fuel"]), undefined);
assert.deepEqual(m.thing("lamp").traits, ["portable", "light_source"], "traits from its kind");
// A trait from the kind can't be unticked here.
const portable = [...panel.querySelectorAll("label.check")].find((l) => l.textContent === "Gives light").querySelector("input");
assert.ok(portable.disabled, "from its kind");
m.set(["objects", "torch"], { name: "torch", in: "hall", like: "lamp", traits: ["wearable"] });
assert.deepEqual(m.thing("torch").traits, ["wearable", "portable", "light_source"]);
assert.deepEqual(m.usersOfKind("lamp"), [["objects", "lamp"], ["objects", "torch"]]);
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
assert.deepEqual(engine.check({ "game.yaml": m.toString() }).diagnostics.filter((d) => d.severity === "error"), [], m.toString());
show(null);
assert.match(panel.textContent, /Like it: brass lamp, torch\./);
m.deleteKind("lamp");
assert.equal(m.get(["kinds"]), undefined);
assert.deepEqual(m.get(["objects", "torch"]).toJSON(), { traits: ["portable", "light_source", "wearable"], fuel: 20, size: 1, name: "torch", in: "hall" });

// The prose check.
const r = prose(m.toString());
assert.equal(r.passages.length, 3);
assert.deepEqual(r.overused.map((o) => o.word), ["ancient"]);
assert.equal(r.overused[0].times, 5);
assert.deepEqual(r.openings.map((o) => [o.start, o.rooms.length]), [["You are standing", 3]]);
assert.ok(grade("The cat sat on the mat.") < grade("Notwithstanding considerable architectural ornamentation, the edifice communicates unmistakable institutional austerity."));

// Pacing: a starter's walkthrough, its chart, and a long dry stretch.
const dir = new URL("../../spec/examples/midnight_snack/", import.meta.url);
const files = Object.fromEntries(readdirSync(dir).filter((f) => /\.(yaml|lua|txt)$/.test(f)).map((f) => [f, readFileSync(new URL(f, dir), "utf8")]));
for (const s of JSON.parse(JSON.stringify(engine.check(files).diagnostics))) assert.notEqual(s.severity, "error");
const beats = engine.pacing(files, files["walkthrough.txt"]);
assert.ok(Array.isArray(beats) && beats.length > 5, JSON.stringify(beats));
assert.ok(beats.some((b) => b.things.length) && beats.some((b) => b.rooms.length));
assert.ok(beats.at(-1).ending, "it ends");
const dull = [...beats, ...Array.from({ length: 9 }, () => ({ command: "wait", room: "Beach", rooms: [], things: [], score: 0, learned: [], goals: [] }))];
assert.deepEqual(dryStretches(dull).at(-1), [beats.length, beats.length + 8]);
assert.deepEqual(brought({ command: "n", room: "Hall", rooms: ["Hall"], things: ["key"], score: 2, learned: [], goals: [] }), ["reached Hall", "got the key", "+2 points"]);
let clicked = -1;
const chart = pacingChart(dull, (i) => (clicked = i));
document.body.append(chart);
assert.equal(chart.querySelectorAll("circle.pace-mark").length, beats.reduce((n, b) => n + [b.rooms, b.things, b.score ? [1] : [], [...b.learned, ...b.goals]].filter((x) => x.length).length, 0));
assert.match(chart.textContent, /9 steps, nothing new/);
const hit = chart.querySelectorAll("rect.pace-hit")[2];
hit.dispatchEvent(new Event("mouseenter"));
assert.match(chart.querySelector(".pace-tip").textContent, /^3\. /);
hit.dispatchEvent(new Event("click"));
assert.equal(clicked, 2);
assert.equal(chart.querySelectorAll("table tbody tr").length, dull.length);

// Threads: only the turns that mention the baker.
const log = document.createElement("div");
const line = (t, cls = "") => Object.assign(document.createElement("p"), { textContent: t, className: cls });
log.append(line("A village."), line("look", "cmd"), line("The baker waves."), line("north", "cmd"), line("A field."), line("give bread to baker", "cmd"), line("The baker's delighted."));
assert.equal(turnsOf(log).length, 4);
assert.ok(mentions("The bakers argue.", ["baker"]) && !mentions("The bakery.", ["baker"]));
assert.deepEqual(follow(log, ["baker"]), { shown: 2, total: 4 });
assert.deepEqual([...log.children].filter((e) => !e.classList.contains("thread-out")).map((e) => e.textContent), ["look", "The baker waves.", "give bread to baker", "The baker's delighted."]);
unfollow(log);
assert.equal(log.querySelectorAll(".thread-out").length, 0);
engine.load({ "game.yaml": GAME }, "game.yaml", 1);
engine.input("n");
assert.deepEqual(engine.met().map((x) => x.name), ["guard", "brass lamp"]);

console.log("ok   editor kinds, prose, pacing and threads");
await GlobalRegistrator.unregister();
