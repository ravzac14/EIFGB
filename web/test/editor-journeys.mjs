// The details panel's fields for getting about and looking back: boats,
// horses and the ways only they go, crowds, characters who read notes,
// flashbacks and the epilogue. Each is set by clicking and typing in the
// panel, and the game that comes out compiles and plays.
// Run: node test/editor-journeys.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register({ url: "http://localhost/editor.html", width: 1200, height: 800 });
window.confirm = () => true;
const { Model } = await import("../src/editor/model.ts");
const { renderInspector } = await import("../src/editor/inspector.ts");
const { loadEngine } = await import("../src/engine.ts");

const GAME = `format: eifgb/0.1
game: { id: river, title: The River, version: "1", start: quay }
clock: { start: "9:00" }
rooms:
  quay: { name: Quay, exits: { east: isle, west: market } }
  isle: { name: Isle, exits: { west: quay } }
  market: { name: Market, exits: { east: quay } }
  garden_then: { name: Old Garden }
objects:
  boat: { name: boat, in: quay, traits: [container, enterable] }
  locket: { name: locket, in: quay, traits: [portable] }
  ring_then: { name: little ring }
  ring: { name: ring }
characters:
  horse: { name: horse, in: quay }
  clerk: { name: clerk, in: market, behavior: merchant }
endings:
  home: { kind: win, text: Home. }
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


// A boat, and a way only it goes.
show({ kind: "thing", id: "boat" });
tick("A boat or a cart: it carries the player");
assert.equal(m.get(["objects", "boat", "vehicle"]), true);
show({ kind: "character", id: "horse" });
tick("Can be ridden (a horse)");
typeIn("Tires after (moves)", "6");
assert.equal(m.get(["characters", "horse", "mount"]), true);
assert.equal(m.get(["characters", "horse", "stamina"]), 6);
tick("Reads notes");
assert.equal(m.get(["characters", "horse", "reads"]), true);
tick("Reads notes");
assert.equal(m.get(["characters", "horse", "reads"]), undefined);
show({ kind: "character", id: "clerk" });
tick("Reads notes");
assert.equal(m.get(["characters", "clerk", "reads"]), false, "a merchant reads unless told not to");
show({ kind: "exit", room: "quay", dir: "east" });
const byBox = [...panel.querySelectorAll("label.check")].find((l) => l.textContent === "boat").querySelector("input");
byBox.checked = true;
byBox.dispatchEvent(new Event("change"));
assert.deepEqual(m.get(["rooms", "quay", "exits", "east"]).toJSON(), { to: "isle", by: "boat" });
assert.deepEqual(m.exit("quay", "east").by, ["boat"]);

// Crowds: always, then at certain hours.
show({ kind: "room", id: "market" });
choose("Crowded", "always");
assert.equal(m.get(["rooms", "market", "crowd"]), true);
choose("Crowded", "hours");
typeIn("Crowded between", "9:00-17:00");
assert.equal(m.get(["rooms", "market", "crowd"]), "9:00-17:00");

// A flashback, what brings it back, and what the past is now.
show(null);
panel.querySelector('input[aria-label="New flashback"]').value = "Childhood";
button("Add a flashback");
choose("It starts in", "garden_then");
typeIn("It begins with", "You're small again.");
const carries = labelled("In it, the player has").nextElementSibling;
const ringBox = [...carries.querySelectorAll("label")].find((l) => l.textContent === "little ring").querySelector("input");
ringBox.checked = true;
ringBox.dispatchEvent(new Event("change"));
assert.deepEqual(m.get(["flashbacks", "childhood"]).toJSON(), { at: "garden_then", turns: 5, begins: "You're small again.", carries: ["ring_then"] });
show({ kind: "thing", id: "locket" });
choose("Looking at it brings back", "childhood");
show({ kind: "thing", id: "ring_then" });
choose("In a memory, this is what's now", "ring");
show({ kind: "room", id: "garden_then" });
choose("In a memory, this room is now", "quay");
assert.equal(m.get(["objects", "locket", "flashback"]), "childhood");
assert.equal(m.get(["objects", "ring_then", "now"]), "ring");
assert.equal(m.get(["rooms", "garden_then", "now"]), "quay");

// The epilogue: a line, a condition, an ending.
show(null);
button("Add a line");
typeIn("It says", "The ring was found, years later.");
typeIn("When (Lua)", "world.ring.location == world.quay");
const after = labelled("Only after").nextElementSibling;
const home = [...after.querySelectorAll("label")].find((l) => l.textContent === "home").querySelector("input");
home.checked = true;
home.dispatchEvent(new Event("change"));
assert.deepEqual(m.get(["epilogue"]).toJSON(), [{ text: "The ring was found, years later.", when: "world.ring.location == world.quay", endings: ["home"] }]);
button("Add a line");
assert.equal(m.get(["epilogue"]).toJSON().length, 2);
const del = [...panel.querySelectorAll("h4")].find((h) => h.textContent.startsWith("Line 2")).querySelector("button");
del.click();
assert.equal(m.get(["epilogue"]).toJSON().length, 1);

// Renames follow; deletes are forgotten.
m.renameThing("ring_then", "small_ring");
assert.deepEqual(m.list(["flashbacks", "childhood", "carries"]), ["small_ring"]);
m.renameThing("boat", "skiff");
assert.deepEqual(m.exit("quay", "east").by, ["skiff"]);

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const out = engine.check({ "game.yaml": m.toString() });
assert.deepEqual(out.diagnostics, [], m.toString());
engine.load({ "game.yaml": m.toString() }, "game.yaml", 1);
const says = (c) => engine.input(c).lines.map((l) => l.text).join("\n");
assert.match(says("e"), /without a boat/);
assert.match(says("x locket"), /You're small again/);
m.deleteThing("skiff");
assert.equal(m.exit("quay", "east").by, undefined);
assert.deepEqual(m.get(["rooms", "quay", "exits", "east"]), "isle");

console.log("ok   editor journeys fields");
await GlobalRegistrator.unregister();
