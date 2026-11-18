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

assert.deepEqual(errors, []);
console.log("ok   editor page");
await GlobalRegistrator.unregister();
process.exit(0);
