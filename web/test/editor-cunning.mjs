// The details panel's fields for the cunning tricks: moods, reaching,
// mirrors, taking things apart, voices, friends, memories and the
// characters the player can play as. Each is set by clicking and typing in
// the panel, and the game that comes out compiles. Run: node test/editor-cunning.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register({ url: "http://localhost/editor.html", width: 1200, height: 800 });
window.confirm = () => true;
const { Model } = await import("../src/editor/model.ts");
const { renderInspector } = await import("../src/editor/inspector.ts");
const { loadEngine } = await import("../src/engine.ts");

const GAME = `format: eifgb/0.1
game: { id: house, title: The House, version: "1", start: hall }
rooms:
  hall: { name: Hall, description: A cold hall., exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
objects:
  lantern: { name: lantern, in: hall, traits: [portable] }
  lens: { name: lens }
  oil: { name: oil }
  wine: { name: wine, in: hall, traits: [portable, drinkable] }
  pole: { name: pole, in: hall, traits: [portable] }
  hook: { name: hook, in: hall, traits: [portable] }
  rope: { name: rope, in: hall, traits: [portable] }
  grapnel: { name: grapnel }
  idol: { name: idol, in: hall, traits: [portable] }
  sandbag: { name: sandbag, in: hall, traits: [portable] }
  chalk: { name: chalk, in: hall, traits: [portable] }
  cloak: { name: cloak, in: hall, traits: [portable, wearable] }
  meat: { name: meat, in: hall, traits: [portable, edible] }
characters:
  mara: { name: Mara, in: hall }
  tobin: { name: Tobin, in: hall }
  guard: { name: guard, in: hall, faction: wolves }
factions:
  wolves: { hostile_to: [] }
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

// Moods: named in the game's panel, with a line for LOOK.
show(null);
panel.querySelector('input[aria-label="New mood"]').value = "Drunk";
button("Add a mood");
typeIn("While drunk, LOOK adds", "The floor sways.");
assert.equal(m.get(["moods", "drunk"]), "The floor sways.");

// A room and a character read differently in it.
show({ kind: "room", id: "hall" });
typeIn("While the player is drunk", "The hall spins.");
assert.equal(m.get(["rooms", "hall", "moods", "drunk"]), "The hall spins.");

// Things: reaching, a mirror, far off, coming apart, a mood from drinking.
show({ kind: "thing", id: "pole" });
tick("Reaches far and high things (a pole, a hook)");
assert.equal(m.get(["objects", "pole", "reaches"]), true);
tick("A mirror");
assert.equal(m.get(["objects", "pole", "reflects"]), true);
tick("A mirror");
assert.equal(m.get(["objects", "pole", "reflects"]), undefined);
tick("Far off, out of reach");
assert.equal(m.get(["objects", "pole", "far"]), true);
tick("Far off, out of reach");

show({ kind: "thing", id: "lantern" });
choose("Add a part", "lens");
choose("Add a part", "oil");
assert.deepEqual(m.list(["objects", "lantern", "splits_into"]), ["lens", "oil"]);
typeIn("When it comes apart, it says", "You unscrew the lantern.");
assert.deepEqual(m.list(["objects", "lantern", "splits_into", "parts"]), ["lens", "oil"]);
assert.equal(m.get(["objects", "lantern", "splits_into", "say"]), "You unscrew the lantern.");

show({ kind: "thing", id: "wine" });
choose("Mood", "drunk");
typeIn("For this many turns", "5");
assert.equal(m.get(["objects", "wine", "mood", "drunk"]), 5);

// The earlier tricks: combining, marks, decoys, putting out fires, traps, disguises, bait.
show({ kind: "thing", id: "hook" });
button("Add one");
choose("Combines with", "rope");
choose("Makes", "grapnel");
typeIn("It says", "You tie the hook to the rope.");
assert.deepEqual(m.get(["objects", "hook", "combines"]).toJSON(), { with: "rope", makes: "grapnel", say: "You tie the hook to the rope." });
tick("Keep the rope");
assert.deepEqual(m.list(["objects", "hook", "combines", "keep"]), ["rope"]);
show({ kind: "thing", id: "chalk" });
tick("Leaves a mark (chalk, a knife)");
tick("Puts out fires (water, sand)");
tick("A trap");
assert.equal(m.get(["objects", "chalk", "marks"]), true);
assert.equal(m.get(["objects", "chalk", "douses"]), true);
assert.equal(m.get(["objects", "chalk", "trap", "holds"]), 3);
tick("A trap");
assert.equal(m.get(["objects", "chalk", "trap"]), undefined);
show({ kind: "thing", id: "sandbag" });
choose("A fake of", "idol");
assert.equal(m.get(["objects", "sandbag", "decoy_for"]), "idol");
show({ kind: "thing", id: "cloak" });
choose("Disguise", "wolves");
assert.equal(m.get(["objects", "cloak", "disguise"]), "wolves");
show({ kind: "thing", id: "meat" });
tick("The wolves");
assert.deepEqual(m.list(["objects", "meat", "eaten_by"]), ["wolves"]);
show({ kind: "character", id: "guard" });
choose("Won't let the player take", "idol");
choose("Likes being given", "meat");
choose("Tamed by being given", "meat");
typeIn("Can be bribed with (money)", "5");
assert.deepEqual(m.list(["characters", "guard", "watches"]), ["idol"]);
assert.deepEqual(m.list(["characters", "guard", "likes"]), ["meat"]);
assert.deepEqual(m.list(["characters", "guard", "tame_with"]), ["meat"]);
assert.equal(m.get(["characters", "guard", "bribe"]), 5);

// Characters: playable (the first is who the player starts as), voices, friends, memories.
show({ kind: "character", id: "mara" });
tick("The player can play as them");
assert.equal(m.get(["player", "plays"]), "mara");
show({ kind: "character", id: "tobin" });
tick("The player can play as them");
assert.equal(m.get(["player", "plays"]), "mara", "still starts as the first");
choose("The player starts as", "tobin");
assert.equal(m.get(["player", "plays"]), "tobin");
show({ kind: "character", id: "guard" });
const voices = labelled("Comes running to the voice of").nextElementSibling;
const tobinBox = [...voices.querySelectorAll("label")].find((l) => l.textContent === "Tobin").querySelector("input");
tobinBox.checked = true;
tobinBox.dispatchEvent(new Event("change"));
assert.deepEqual(m.list(["characters", "guard", "answers_to"]), ["tobin"]);
const friends = labelled("Friends with").nextElementSibling;
const maraBox = [...friends.querySelectorAll("label")].find((l) => l.textContent === "Mara").querySelector("input");
maraBox.checked = true;
maraBox.dispatchEvent(new Event("change"));
assert.deepEqual(m.list(["characters", "guard", "friends"]), ["mara"]);
typeIn("You attacked them", "You again!");
assert.equal(m.get(["characters", "guard", "remembers", "attacked"]), "You again!");

// What the player learns and owes: facts, copying, places heard of, favours.
show(null);
panel.querySelector('input[aria-label="New fact"]').value = "meeting";
button("Add a fact");
typeIn("It says", "They meet at midnight.");
assert.equal(m.get(["facts", "meeting"]), "They meet at midnight.");
choose("It tells of a place", "hall");
assert.deepEqual(m.get(["facts", "meeting"]).toJSON(), { text: "They meet at midnight.", place: "hall" });
show({ kind: "thing", id: "sandbag" });
tick("They meet at midnight.");
assert.deepEqual(m.list(["objects", "sandbag", "teaches"]), ["meeting"]);
choose("COPY makes", "chalk");
tick("Something to copy onto (paper)");
tick("Something to write with (a pen)");
assert.equal(m.get(["objects", "sandbag", "copy"]), "chalk");
assert.equal(m.get(["objects", "sandbag", "blank"]), true);
assert.equal(m.get(["objects", "sandbag", "writes"]), true);
show({ kind: "room", id: "hall" });
tick("The player knows of it from the start");
assert.equal(m.get(["rooms", "hall", "heard_of"]), true);
show({ kind: "character", id: "mara" });
choose("They ask the player to bring", "cloak");
typeIn("The player promises to", "find her cloak");
const gives = labelled("And give the player").nextElementSibling;
const coin = [...gives.querySelectorAll("label")].find((l) => l.textContent === "hook").querySelector("input");
coin.checked = true;
coin.dispatchEvent(new Event("change"));
typeIn("Promised, the player has (turns)", "20");
assert.deepEqual(m.get(["characters", "mara", "asks"]).toJSON(), { for: "cloak", promise: "find her cloak", gives: ["hook"], within: 20 });
tick("Gives directions when asked the way");
assert.equal(m.get(["characters", "mara", "knows_the_way"]), true);

// The physical world: water, echoes, a gallery; plates, rafts, floats, scents; time.
show({ kind: "room", id: "yard" });
tick("Water rises here");
assert.deepEqual(m.get(["rooms", "yard", "floods"]).toJSON(), { every: 3, depth: 3 });
choose("Up to", "2");
choose("Drained by opening", "pole");
assert.equal(m.get(["rooms", "yard", "floods", "drains"]), undefined, "only openable things drain it");
tick("An echo");
button("Add answers");
assert.deepEqual(m.get(["rooms", "yard", "echo"]).toJSON(), { help: "…help… help…" });
typeIn("It answers", "hello: Hello yourself!");
assert.deepEqual(m.get(["rooms", "yard", "echo"]).toJSON(), { hello: "Hello yourself!" });
const gallery = labelled("What's said in these rooms is heard here").nextElementSibling;
const hallBox = [...gallery.querySelectorAll("label")].find((l) => l.textContent === "Hall").querySelector("input");
hallBox.checked = true;
hallBox.dispatchEvent(new Event("change"));
assert.deepEqual(m.list(["rooms", "yard", "hears"]), ["hall"]);
show(null);
typeIn("The player can hold their breath for (turns)", "6");
assert.equal(m.get(["player", "breath"]), 6);
show({ kind: "thing", id: "hook" });
tick("Floats: held, it keeps the player afloat");
choose("It smells of", "guard");
assert.equal(m.get(["objects", "hook", "floats"]), true);
assert.equal(m.get(["objects", "hook", "scent_of"]), "guard");
show({ kind: "character", id: "guard" });
tick("Follows a scent (a dog)");
assert.equal(m.get(["characters", "guard", "sniffs"]), true);
m.set(["objects", "rope", "traits"], ["portable", "supporter"]);
show({ kind: "thing", id: "rope" });
tick("A pressure plate");
typeIn("It takes this much weight", "4");
tick("Sinks when overloaded (a raft)");
assert.deepEqual(m.get(["objects", "rope", "pressure"]).toJSON(), { at_least: 4 });
assert.equal(m.get(["objects", "rope", "sinks"]), true);
// A way open only under a full moon, once there's a clock.
show({ kind: "exit", room: "hall", dir: "north" });
assert.ok(panel.textContent.includes("With a clock"));
m.set(["clock"], { start: "20:00" });
show({ kind: "exit", room: "hall", dir: "north" });
choose("When it's open", "full moon");
typeIn("When it isn't, going that way says", "Only a blank hillside.");
assert.deepEqual(m.get(["rooms", "hall", "exits", "north"]).toJSON(), { to: "yard", open_at: "full moon", blocked: "Only a blank hillside." });
choose("When it's open", "");
assert.equal(m.get(["rooms", "hall", "exits", "north"]), "yard");
show(null);
choose("The moon on the first night", "new");
assert.equal(m.get(["clock", "moon"]), "new");

// The game compiles.
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const errs = engine.check({ "game.yaml": m.toString() }).diagnostics.filter((d) => d.severity === "error");
assert.deepEqual(errs, [], m.toString());

// Renaming follows the new references; deleting takes them out.
m.renameCharacter("tobin", "toby");
assert.equal(m.get(["player", "plays"]), "toby");
assert.deepEqual(m.list(["characters", "guard", "answers_to"]), ["toby"]);
m.renameThing("oil", "lamp_oil");
assert.deepEqual(m.list(["objects", "lantern", "splits_into", "parts"]), ["lens", "lamp_oil"]);
m.deleteCharacter("toby");
assert.equal(m.get(["player", "plays"]), "mara");
assert.equal(m.get(["characters", "guard", "answers_to"]), undefined);
m.renameThing("idol", "golden_idol");
assert.equal(m.get(["objects", "sandbag", "decoy_for"]), "golden_idol");
assert.deepEqual(m.list(["characters", "guard", "watches"]), ["golden_idol"]);
m.renameThing("rope", "cord");
assert.equal(m.get(["objects", "hook", "combines", "with"]), "cord");
assert.deepEqual(m.list(["objects", "hook", "combines", "keep"]), ["cord"]);
m.deleteThing("golden_idol");
assert.equal(m.get(["objects", "sandbag", "decoy_for"]), undefined);
assert.equal(m.get(["characters", "guard", "watches"]), undefined);
m.deleteThing("grapnel");
assert.equal(m.get(["objects", "hook", "combines"]), undefined);
m.deleteThing("cloak");
assert.equal(m.get(["characters", "mara", "asks"]), undefined, "a favour for something gone, goes");
m.forgetFact("meeting");
assert.equal(m.get(["objects", "sandbag", "teaches"]), undefined);
m.deleteThing("lens");
m.deleteThing("lamp_oil");
assert.equal(m.get(["objects", "lantern", "splits_into"]), undefined);
assert.deepEqual(engine.check({ "game.yaml": m.toString() }).diagnostics.filter((d) => d.severity === "error"), []);

console.log("ok   editor tricks fields");
await GlobalRegistrator.unregister();
