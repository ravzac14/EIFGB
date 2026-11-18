// Drives the editor page in a simulated browser (happy-dom): makes a game,
// draws rooms and exits by clicking the map, adds a door from the details
// panel, undoes and redoes, edits the file text, and checks the YAML that
// comes out compiles. Run: node test/editor-ui.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: pathToFileURL(`${web}public/editor.html`).href, width: 1200, height: 800 });
// Things a test DOM leaves out.
Element.prototype.setPointerCapture ??= () => {};
window.confirm = () => true;
window.prompt = () => "wolves";
window.open = () => null;
globalThis.CSS ??= { escape: (s) => s };
const errors = [];
window.addEventListener("error", (e) => errors.push(e.error ?? e.message));

const html = readFileSync(`${web}editor.html`, "utf8");
document.body.innerHTML = html.match(/<body[^>]*>([\s\S]*)<\/body>/)[1].replace(/<script[\s\S]*?<\/script>/g, "");
document.body.className = "editor";
await import("../src/editor/main.ts");

const $ = (s) => document.querySelector(s);
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
const yaml = () => JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files["game.yaml"];
const press = (el) => {
  for (const type of ["pointerdown", "pointerup"]) el.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: 0, clientY: 0, pointerId: 1 }));
};
const clickButton = (text) => {
  const b = [...document.querySelectorAll("#inspector button")].find((x) => x.textContent === text);
  assert.ok(b, `a "${text}" button`);
  b.click();
};

// The start screen shows only itself: no map, and no game buttons yet.
const css = readFileSync(`${web}src/style.css`, "utf8");
assert.match(css, /\[hidden\] \{ display: none !important; \}/);
assert.equal($("#workspace").hidden, true);
assert.equal($("#tools").hidden, true);

// A new game: one room, on the map.
$("#new-game").click();
await tick();
assert.equal($("#workspace").hidden, false);
assert.equal(document.querySelectorAll("#map .room").length, 1);

// Select the room; its + handles appear. Click the north one: a new room, joined both ways.
press($('#map [data-room="first_room"]'));
const handles = document.querySelectorAll("#map .handle");
assert.equal(handles.length, 8);
const north = [...handles].find((h) => h.querySelector("title").textContent.includes("north"));
press(north);
assert.equal(document.querySelectorAll("#map .room").length, 2);
assert.match(yaml(), /exits:\n\s+north: new_room/);
assert.match(yaml(), /new_room:\n\s+name: New room\n\s+exits:\n\s+south: first_room/);
assert.equal(document.querySelectorAll("#map .link").length, 1, "one line for a two-way route");

// The new room is selected, with its name ready to type over.
const name = document.activeElement;
assert.equal(name.tagName, "INPUT");
name.value = "Attic";
name.dispatchEvent(new Event("input"));
assert.match(yaml(), /name: Attic/);

// Select the line, and put a door on it.
press($("#map .link .hit"));
assert.match($("#inspector h2").textContent, /First Room ⇄ Attic/);
clickButton("Put a door here");
assert.match(yaml(), /through: first_room_door/);
assert.match(yaml(), /first_room_door:\n\s+name: door\n\s+in: first_room\n\s+traits: \[door, openable, scenery\]/);
// Lock it.
const locked = [...document.querySelectorAll("#inspector input[type=radio]")].find((r) => r.value === "locked");
locked.checked = true;
locked.dispatchEvent(new Event("change"));
assert.match(yaml(), /state: \{locked: true\}/);
assert.equal(document.querySelectorAll("#map .door.locked").length, 1);

// Undo the lock and the door; redo them.
$("#undo").click();
assert.doesNotMatch(yaml(), /locked: true/);
$("#undo").click();
assert.doesNotMatch(yaml(), /first_room_door/);
$("#redo").click();
$("#redo").click();
assert.match(yaml(), /locked: true/);

// Edit the file directly: the map follows.
const ta = $("#yaml");
ta.value = yaml().replace("name: Attic", "name: Loft");
ta.dispatchEvent(new Event("input"));
await tick(700);
assert.equal([...document.querySelectorAll("#map .room text")].some((t) => t.textContent === "Loft"), true);
// A YAML mistake isn't applied, and says why.
ta.value = "rooms: [unclosed";
ta.dispatchEvent(new Event("input"));
await tick(700);
assert.match($("#yaml-msg").textContent, /Not applied yet/);
assert.match(yaml(), /name: Loft/);

// The checker spots the locked door with no key; clicking it goes to the door.
await tick(1500);
assert.match($("#panel-problems").textContent, /first_room_door.*starts locked but has no 'unlocks_with'/);
$("#panel-problems li").click();
assert.match($("#inspector h2").textContent, /First Room ⇄ Loft/);
assert.match($("#inspector").textContent, /Unlocked with/);

// Map positions are kept beside the game.
const files = JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files;
assert.match(files["map.yaml"], /first_room: \[0, 0\]/);

// The test page has no size, so screen and map coordinates are the same.
const pointer = (el, type, x, y) => el.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: y, pointerId: 1 }));

// Drag a room to move it: saved to map.yaml, as one undo step.
const room = $('#map [data-room="first_room"]');
pointer(room, "pointerdown", 0, 0);
pointer(room, "pointermove", 200, 20);
pointer(room, "pointermove", 400, 40);
pointer(room, "pointerup", 400, 40);
const moved = () => JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files["map.yaml"];
assert.match(moved(), /first_room: \[400, 40\]/);
$("#undo").click();
assert.match(moved(), /first_room: \[0, 0\]/);

// Drag a + onto another room: they're joined that way (and back, if free).
press($('#map [data-room="first_room"]'));
const east = [...document.querySelectorAll("#map .handle")].find((h) => h.querySelector("title").textContent.includes("to the east"));
press(east); // a new room to the east
const eastRoom = yaml().match(/east: (\w+)/)[1];
press($('#map [data-room="new_room"]')); // the loft
const se = [...document.querySelectorAll("#map .handle")].find((h) => h.querySelector("title").textContent.includes("southeast"));
const target = [...document.querySelectorAll("#map .room")].find((r) => r.dataset.room === eastRoom);
const [, tx, ty] = target.getAttribute("transform").match(/translate\((-?[\d.]+) (-?[\d.]+)\)/).map(Number);
pointer(se, "pointerdown", 0, 0);
pointer(se, "pointermove", tx, ty);
pointer(se, "pointerup", tx, ty);
assert.match(yaml(), new RegExp(`new_room:[\\s\\S]*southeast: ${eastRoom}`));
assert.match(yaml(), new RegExp(`${eastRoom}:[\\s\\S]*northwest: new_room`));
assert.equal($("#inspector .kind").textContent, "Way out", "the new way is selected");

// ------------------------------------------------------------- things
const transfer = () => {
  const data = {};
  return { types: [], setData(k, v) { data[k] = v; this.types.push(k); }, getData: (k) => data[k] ?? "", effectAllowed: "", dropEffect: "" };
};
const drag = (from, to, x = 0, y = 0) => {
  const dt = transfer();
  const ev = (type) => {
    const e = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y });
    Object.defineProperty(e, "dataTransfer", { value: dt });
    return e;
  };
  from.dispatchEvent(ev("dragstart"));
  to.dispatchEvent(ev("dragover"));
  to.dispatchEvent(ev("drop"));
};
// The Add row that offers this kind (things and people have separate rows).
const addThing = (kind) => {
  const row = [...document.querySelectorAll("#inspector .add.two")].find((r) => [...r.querySelectorAll("option")].some((o) => o.value === kind));
  const sel = row.querySelector("select");
  sel.value = kind;
  sel.dispatchEvent(new Event("change"));
  row.querySelector("button").click();
  const name = document.activeElement;
  return name;
};
// Typing, then leaving the field (which is when an automatic ID follows the name).
const typeName = (input, text) => {
  input.value = text;
  input.dispatchEvent(new Event("input"));
  input.dispatchEvent(new Event("change"));
};

// In the first room: a chest and a coin.
press($('#map [data-room="first_room"]'));
typeName(addThing("container"), "old chest");
assert.equal($("#inspector .kind").textContent, "Thing");
assert.match(yaml(), /\n  old_chest:\n\s+name: old chest/, "the ID followed the name");
assert.equal($("#inspector h2").textContent, "old chest");
press($('#map [data-room="first_room"]'));
typeName(addThing("carry"), "gold coin");
press($('#map [data-room="first_room"]'));
const rows = () => [...document.querySelectorAll("#inspector .thing")];
assert.deepEqual(rows().map((r) => r.querySelector(".name").textContent), ["old chest", "gold coin"]);
assert.match([...document.querySelectorAll('#map [data-room="first_room"] text')].map((t) => t.textContent).join(" "), /2 things/);

// Drag the coin into the chest: nested under it.
drag(rows()[1], rows()[0]);
assert.match(yaml(), /gold_coin:\n\s+name: gold coin\n\s+in: old_chest/);
assert.equal(document.querySelectorAll("#inspector .things.nested .thing").length, 1);
// Dropping the chest on the coin would put it inside itself: refused, with a message.
drag(rows()[0], rows()[1]);
assert.match(yaml(), /old_chest:\n\s+name: old chest\n\s+in: first_room/);
assert.match($("#saved").textContent, /can't go there/);

// Drop the chest on another room on the map: it moves, with the coin inside.
const [, ex, ey] = $(`#map [data-room="${eastRoom}"]`).getAttribute("transform").match(/translate\((-?[\d.]+) (-?[\d.]+)\)/).map(Number);
drag(rows()[0], $("#map"), ex, ey);
assert.match(yaml(), new RegExp(`old_chest:\\n\\s+name: old chest\\n\\s+in: ${eastRoom}`));
assert.match(yaml(), /gold_coin:\n\s+name: gold coin\n\s+in: old_chest/);
assert.equal($("#inspector .kind").textContent, "Thing", "the dropped thing is selected");

// Edit the coin: hidden until the chest is searched; the chest locks with a key.
press($(`#map [data-room="${eastRoom}"]`));
const coinRow = rows().find((r) => r.textContent.includes("gold coin"));
coinRow.click();
const hiddenBox = [...document.querySelectorAll("#inspector label.check")].find((l) => l.textContent.startsWith("Hidden until")).querySelector("input");
hiddenBox.checked = true;
hiddenBox.dispatchEvent(new Event("change"));
const [verbSel, whatSel] = [...document.querySelectorAll("#inspector .add.two select")];
whatSel.value = "old_chest";
whatSel.dispatchEvent(new Event("change"));
assert.match(yaml(), /state: \{hidden: true\}\n\s+revealed_by: search old_chest/);
void verbSel;
// Where-it-starts offers rooms, the player, and holders not inside it.
const where = [...document.querySelectorAll("#inspector select")].find((s) => s.ariaLabel === "Where it starts");
assert.ok([...where.options].some((o) => o.value === "player"));
assert.ok([...where.options].some((o) => o.value === "old_chest"));
where.value = "player";
where.dispatchEvent(new Event("change"));
assert.match(yaml(), /gold_coin:\n\s+name: gold coin\n\s+in: player/);
$("#undo").click();
assert.match(yaml(), /in: old_chest/);

// It all compiles.
await tick(1500);
assert.doesNotMatch($("#panel-problems").textContent, /Error/);

// An ID set by hand stays put when the name changes.
coinRow.click();
const named = [...document.querySelectorAll("#inspector input[type=text]")];
const idBox = named.find((i) => i.ariaLabel === "Thing ID");
idBox.value = "treasure";
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Rename").click();
assert.match(yaml(), /\n  treasure:\n\s+name: gold coin/);
typeName(document.querySelector("#inspector input[type=text]"), "silver coin");
assert.match(yaml(), /\n  treasure:\n\s+name: silver coin/);

// ------------------------------------------------------- characters
const addPerson = (kind) => {
  const rows = [...document.querySelectorAll("#inspector .add.two")];
  const row = rows.find((r) => [...r.querySelectorAll("option")].some((o) => o.value === kind));
  const sel = row.querySelector("select");
  sel.value = kind;
  sel.dispatchEvent(new Event("change"));
  row.querySelector("button").click();
  return document.activeElement;
};
press($(`#map [data-room="${eastRoom}"]`));
typeName(addPerson("hunter"), "grey wolf");
assert.equal($("#inspector .kind").textContent, "Character");
assert.match(yaml(), /\n  grey_wolf:\n\s+name: grey wolf\n\s+in: \w+\n\s+behavior: hunter\n\s+health: 5\n\s+damage: 1/);
assert.match(yaml(), /intro: Your story begins here.\n\nplayer:\n  health: 10\n\nrooms:/, "the player can fight back; the section goes after game");
const pill = () => [...document.querySelectorAll("#map .person")].find((g) => g.textContent.includes("grey wolf"));
assert.ok(pill().classList.contains("hostile"));

// A side, from the character's Side list ("A new side…" asks for its name).
const sideSel = [...document.querySelectorAll("#inspector select")].find((s) => s.ariaLabel === "Side");
sideSel.value = "__new";
sideSel.dispatchEvent(new Event("change"));
assert.match(yaml(), /factions:\n\s+wolves: \{\}/);
assert.match(yaml(), /grey_wolf:[\s\S]*faction: wolves/);
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Edit the wolves").click();
assert.equal($("#inspector .kind").textContent, "Side");
const playerBox = [...document.querySelectorAll("#inspector label.check")].find((l) => l.textContent === "The player").querySelector("input");
playerBox.checked = true;
playerBox.dispatchEvent(new Event("change"));
assert.match(yaml(), /wolves:\n\s+hostile_to: \[player\]/);
assert.match($("#inspector").textContent, /grey wolf/, "members are listed");

// Drag the wolf's pill to the first room.
const [, fx, fy] = $('#map [data-room="first_room"]').getAttribute("transform").match(/translate\((-?[\d.]+) (-?[\d.]+)\)/).map(Number);
pointer(pill(), "pointerdown", ex, ey + 40);
pointer(pill(), "pointermove", fx, fy);
pointer(pill(), "pointerup", fx, fy);
assert.match(yaml(), /grey_wolf:\n\s+name: grey wolf\n\s+in: first_room/);
assert.equal($("#inspector .kind").textContent, "Character");

// A shopkeeper, with something for sale.
press($('#map [data-room="first_room"]'));
typeName(addPerson("merchant"), "Old Meg");
const sale = [...document.querySelectorAll("#inspector .add.two")].at(-1);
sale.querySelector("button").click();
typeName(document.activeElement, "lantern oil");
const price = [...document.querySelectorAll("#inspector label")].find((l) => l.textContent.startsWith("Price")).nextElementSibling;
price.value = "5";
price.dispatchEvent(new Event("input"));
assert.match(yaml(), /lantern_oil:\n\s+name: lantern oil\n\s+in: old_meg\n\s+traits: \[portable\]\n\s+price: 5/);

// Fighting settings with defaults: unticking "Fights back" writes false; ticking it again removes it.
press($('#map [data-room="first_room"]'));
[...document.querySelectorAll("#inspector .thing")].find((r) => r.textContent.includes("grey wolf")).click();
const fights = () => [...document.querySelectorAll("#inspector label.check")].find((l) => l.textContent === "Fights back when attacked").querySelector("input");
fights().checked = false;
fights().dispatchEvent(new Event("change"));
assert.match(yaml(), /fights_back: false/);
fights().checked = true;
fights().dispatchEvent(new Event("change"));
assert.doesNotMatch(yaml(), /fights_back/);

await tick(1500);
assert.doesNotMatch($("#panel-problems").textContent, /Error/, $("#panel-problems").textContent);

assert.deepEqual(errors, []);
console.log("ok   editor page");
await GlobalRegistrator.unregister();
process.exit(0);
