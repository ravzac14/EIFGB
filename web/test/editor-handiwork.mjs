// The details panel's fields for handiwork: ropes and anchors, sheer
// drops, hollow hiding places, passwords, codes that jam, mending, food
// that goes bad, hands and the cold. Each is set by clicking and typing in
// the panel, and the game that comes out compiles and plays.
// Run: node test/editor-handiwork.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register({ url: "http://localhost/editor.html", width: 1200, height: 800 });
window.confirm = () => true;
const { Model } = await import("../src/editor/model.ts");
const { renderInspector } = await import("../src/editor/inspector.ts");
const { loadEngine } = await import("../src/engine.ts");

const GAME = `format: eifgb/0.1
game: { id: cliff, title: The Cliff, version: "1", start: top }
rooms:
  top: { name: Cliff Top, exits: { down: ledge, east: gate } }
  ledge: { name: Ledge, exits: { up: top } }
  gate: { name: Gate, exits: { west: top, north: keep } }
  keep: { name: Keep, exits: { south: gate } }
objects:
  rope: { name: rope, in: top, traits: [portable] }
  post: { name: post, in: top, traits: [scenery] }
  panel: { name: loose panel, in: keep, traits: [portable], state: { hidden: true } }
  key: { name: iron key, in: top, traits: [portable], uses: 2 }
  bits: { name: broken key }
  glue: { name: glue, in: top, traits: [portable] }
  bread: { name: bread, in: top, traits: [portable, edible] }
  chest: { name: chest, in: top, traits: [portable, container] }
  coat: { name: coat, in: top, traits: [portable, wearable] }
  safe: { name: safe, in: keep, traits: [container, openable, lockable], state: { locked: true }, code: 471 }
characters:
  porter: { name: porter, in: gate }
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


// A rope, something to tie it to, and a sheer drop.
show({ kind: "thing", id: "rope" });
tick("A rope");
assert.equal(m.get(["objects", "rope", "rope"]), true);
show({ kind: "thing", id: "post" });
tick("Something to tie a rope to (a tree, a post, a hook)");
assert.equal(m.get(["objects", "post", "anchor"]), true);
show({ kind: "exit", room: "top", dir: "down" });
tick("Only with a rope tied at the top (or the bottom)");
assert.deepEqual(m.get(["rooms", "top", "exits", "down"]).toJSON(), { to: "ledge", rope: true });
assert.equal(m.exit("top", "down").rope, true);
// Kept when the way changes.
m.setVia("top", "down", ["post"]);
assert.deepEqual(m.get(["rooms", "top", "exits", "down"]).toJSON(), { to: "ledge", via: "post", rope: true });
m.setVia("top", "down", []);

// Hollow, two hands, warm, mending, going bad, what's left, a jamming code.
show({ kind: "thing", id: "panel" });
tick("Sounds hollow when knocked on");
show({ kind: "thing", id: "chest" });
tick("Takes both hands to carry");
show({ kind: "thing", id: "coat" });
tick("Keeps the cold out, worn (a coat)");
show({ kind: "thing", id: "glue" });
tick("Mends things (glue, needle and thread)");
show({ kind: "thing", id: "bread" });
typeIn("Goes bad after (turns)", "30");
show({ kind: "thing", id: "key" });
choose("When it breaks, it leaves", "bits");
show({ kind: "thing", id: "safe" });
typeIn("Jams after this many wrong codes", "3");
tick("Says how close a wrong code was");
assert.equal(m.get(["objects", "panel", "hollow"]), true);
assert.equal(m.get(["objects", "chest", "two_handed"]), true);
assert.equal(m.get(["objects", "coat", "warm"]), true);
assert.equal(m.get(["objects", "glue", "mends"]), true);
assert.equal(m.get(["objects", "bread", "spoils"]), 30);
assert.equal(m.get(["objects", "key", "breaks_into"]), "bits");
assert.equal(m.get(["objects", "safe", "tries"]), 3);
assert.equal(m.get(["objects", "safe", "feedback"]), true);

// A cold room, and then the player's warmth; hands.
show({ kind: "room", id: "ledge" });
tick("Bitterly cold");
assert.equal(m.get(["rooms", "ledge", "cold"]), true);
show(null);
tick("Hands count: only so much can be held at once");
assert.equal(m.get(["player", "hands"]), 2);
typeIn("The player lasts in the cold for (turns)", "6");
assert.equal(m.get(["player", "warmth"]), 6);

// A password, barring the way north.
show({ kind: "character", id: "porter" });
tick("Won't let the player by without a password");
typeIn("The password", "mellon");
assert.equal(m.get(["characters", "porter", "password", "word"]), "mellon");
const bars = labelled("The ways they bar").nextElementSibling;
const box = (d) => [...bars.querySelectorAll("label")].find((l) => l.textContent === d).querySelector("input");
box("west").checked = true;
box("west").dispatchEvent(new Event("change"));
assert.deepEqual(m.list(["characters", "porter", "password", "bars"]), ["north", "west"]);
box("west").checked = false;
box("west").dispatchEvent(new Event("change"));
assert.equal(m.get(["characters", "porter", "password", "bars"]), "north");

// Renaming and deleting what breaks into what.
m.renameThing("bits", "broken_key");
assert.equal(m.get(["objects", "key", "breaks_into"]), "broken_key");

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const out = engine.check({ "game.yaml": m.toString() });
assert.deepEqual(out.diagnostics, [], m.toString());
engine.load({ "game.yaml": m.toString() }, "game.yaml", 1);
const says = (c) => engine.input(c).lines.map((l) => l.text).join("\n");
assert.match(says("d"), /You'd need a rope/);
says("take rope");
assert.match(says("tie rope to post"), /drop the other end over the edge/);
assert.match(says("d"), /Ledge/);
m.deleteThing("broken_key");
assert.equal(m.get(["objects", "key", "breaks_into"]), undefined);

console.log("ok   editor handiwork fields");
await GlobalRegistrator.unregister();
