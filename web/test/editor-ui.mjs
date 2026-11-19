// Drives the editor page in a simulated browser (happy-dom): makes a game,
// draws rooms and exits by clicking the map, adds a door from the details
// panel, undoes and redoes, edits the file text, and checks the YAML that
// comes out compiles. Run: node test/editor-ui.mjs
import "./vite-imports.mjs";
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

// Drag a selected room's corner to resize its box: kept in map.yaml with
// its position, through moves; a double-click puts back the usual size.
press($('#map [data-room="first_room"]'));
const grip = () => $("#map .grip");
pointer(grip(), "pointerdown", 0, 0);
pointer(grip(), "pointermove", 50, 20);
pointer(grip(), "pointerup", 50, 20);
assert.match(moved(), /first_room: \[0, 0, 250, 100\]/);
assert.equal($('#map [data-room="first_room"] rect').getAttribute("width"), "250");
pointer($('#map [data-room="first_room"]'), "pointerdown", 0, 0);
pointer($('#map [data-room="first_room"]'), "pointermove", 100, 0);
pointer($('#map [data-room="first_room"]'), "pointerup", 100, 0);
assert.match(moved(), /first_room: \[100, 0, 250, 100\]/);
$("#undo").click();
grip().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
assert.match(moved(), /first_room: \[0, 0\]/);
$("#undo").click();
assert.match(moved(), /first_room: \[0, 0, 250, 100\]/);
$("#undo").click();
assert.match(moved(), /first_room: \[0, 0\]/);

// The editor theme: the site's colours, or a blueprint; remembered in this browser.
const theme = $("#map-theme");
assert.equal(theme.value, "site");
assert.equal(theme.closest("label").textContent.trim().startsWith("Editor theme"), true);
theme.value = "blueprint";
theme.dispatchEvent(new Event("change"));
assert.ok($(".canvas").classList.contains("blueprint"));
assert.equal(localStorage.getItem("eifgb:map-theme"), '"blueprint"');
theme.value = "site";
theme.dispatchEvent(new Event("change"));
assert.ok(!$(".canvas").classList.contains("blueprint"));

// A colour for the room, from the swatches: kept in map.yaml, drawn on the map.
press($('#map [data-room="first_room"]'));
document.querySelector('#inspector .swatch[title="Green"]').click();
assert.match(moved(), /\ncolours:\n  first_room: green\n/);
assert.match($('#map [data-room="first_room"]').getAttribute("style"), /--tint/);
assert.equal(document.querySelector('#inspector .swatch[title="Green"]').getAttribute("aria-checked"), "true");
// Resizing back to the usual size keeps the colour.
grip().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
assert.match(moved(), /first_room: green/);
document.querySelector('#inspector .swatch[title="No colour"]').click();
assert.doesNotMatch(moved(), /\ncolours:/);

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
const where = [...document.querySelectorAll("#inspector select")].find((s) => s.getAttribute("aria-label") === "Where it starts");
assert.ok([...where.options].some((o) => o.value === "player"));
assert.ok([...where.options].some((o) => o.value === "old_chest"));
where.value = "player";
where.dispatchEvent(new Event("change"));
assert.match(yaml(), /gold_coin:\n\s+name: gold coin\n\s+in: player/);
$("#undo").click();
assert.match(yaml(), /in: old_chest/);

// The chest can be pushed between rooms, and put up high.
press($(`#map [data-room="${eastRoom}"]`));
rows().find((r) => r.textContent.includes("old chest")).click();
const box = (start) => [...document.querySelectorAll("#inspector label.check")].find((l) => l.textContent.startsWith(start)).querySelector("input");
box("Can be pushed").checked = true;
box("Can be pushed").dispatchEvent(new Event("change"));
assert.match(yaml(), /old_chest:[^]*?pushable/);
box("Up high").checked = true;
box("Up high").dispatchEvent(new Event("change"));
assert.match(yaml(), /old_chest:[^]*?high: true/);
box("Up high").checked = false;
box("Up high").dispatchEvent(new Event("change"));
assert.doesNotMatch(yaml(), /high: true/);

// The game's look: a theme and a highlight colour, and back to none.
pointer($("#map"), "pointerdown", 5, 5);
pointer($("#map"), "pointerup", 5, 5);
const looks = () => [...document.querySelectorAll("#inspector select")].find((x) => x.getAttribute("aria-label") === "Colours");
looks().value = "terminal";
looks().dispatchEvent(new Event("change"));
assert.match(yaml(), /presentation:\n\s+theme: terminal/);
const colour = $('#inspector input[type=color]');
colour.value = "#cc3333";
colour.dispatchEvent(new Event("change"));
assert.match(yaml(), /presentation:\n\s+theme: terminal\n\s+accent: "#cc3333"/);
clickButton("Use the usual colour");
assert.doesNotMatch(yaml(), /accent/);
looks().value = "";
looks().dispatchEvent(new Event("change"));
assert.doesNotMatch(yaml(), /presentation/);
looks().value = "sepia";
looks().dispatchEvent(new Event("change"));
// Listing exits: on by default; off for the game, and back on for one room.
const checkbox = (start) => [...document.querySelectorAll("#inspector label.check")].find((l) => l.textContent.startsWith(start)).querySelector("input");
checkbox("List each room's exits").checked = false;
checkbox("List each room's exits").dispatchEvent(new Event("change"));
assert.match(yaml(), /settings:\n\s+list_exits: false/);
press($('#map [data-room="first_room"]'));
assert.equal(checkbox("Lists its exits").checked, false, "a room follows the game's setting");
checkbox("Lists its exits").checked = true;
checkbox("Lists its exits").dispatchEvent(new Event("change"));
assert.match(yaml(), /first_room:[^]*?list_exits: true/);
pointer($("#map"), "pointerdown", 5, 5);
pointer($("#map"), "pointerup", 5, 5);
checkbox("List each room's exits").checked = true;
checkbox("List each room's exits").dispatchEvent(new Event("change"));
assert.doesNotMatch(yaml(), /settings:/, "back to the default: no settings section left");

// It all compiles.
await tick(1500);
assert.doesNotMatch($("#panel-problems").textContent, /Error/);

// An ID set by hand stays put when the name changes.
coinRow.click();
const named = [...document.querySelectorAll("#inspector input[type=text]")];
const idBox = named.find((i) => i.getAttribute("aria-label") === "Thing ID");
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
const sideSel = [...document.querySelectorAll("#inspector select")].find((s) => s.getAttribute("aria-label") === "Side");
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

// ------------------------------------------------------------ story
const projectFiles = () => JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files;
const buttonIn = (text, root = document.querySelector("#inspector")) => [...root.querySelectorAll("button")].find((b) => b.textContent === text);
const inputIn = (label) => [...document.querySelectorAll("#inspector input")].find((i) => i.getAttribute("aria-label") === label);
const selectIn = (label) => [...document.querySelectorAll("#inspector select")].find((s) => s.getAttribute("aria-label") === label);
const choose = (sel, value) => {
  sel.value = value;
  sel.dispatchEvent(new Event("change"));
};
const deselect = () => {
  const svg = $("#map");
  svg.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerId: 1 }));
  svg.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerId: 1 }));
};

// An ending, and a timer that ends the game with it.
deselect();
inputIn("New ending's name").value = "escaped";
buttonIn("Add an ending").click();
assert.equal($("#inspector .kind").textContent, "Ending");
assert.match(yaml(), /endings:\n\s+escaped:\n\s+kind: win\n\s+text: You did it./);
deselect();
buttonIn("Add a timer").click();
assert.equal($("#inspector .kind").textContent, "Timer");
choose(selectIn("Ending"), "escaped");
assert.match(yaml(), /timers:\n\s+timer:\n\s+after: 5\n\s+say: Something happens.\n\s+end: escaped/);

// A room script; a syntax error shows under it; fixing it clears that.
press($('#map [data-room="first_room"]'));
buttonIn("Add a script").click();
assert.match(yaml(), /first_room:[\s\S]*?on:\n\s+enter: say\("Something happens."\)/);
const code = () => [...document.querySelectorAll("#inspector textarea.code")][0];
code().value = 'say("hi"';
code().dispatchEvent(new Event("input"));
await tick(1500);
assert.match($("#inspector .script").textContent, /Lua syntax error/);
code().value = 'say("Welcome.")\nscore(1)';
code().dispatchEvent(new Event("input"));
assert.match(yaml(), /enter: \|\n\s+say\("Welcome."\)\n\s+score\(1\)/);
await tick(1500);
assert.doesNotMatch($("#inspector .script").textContent, /Lua syntax error/);

// A conversation, from a character's Talking list.
[...document.querySelectorAll("#inspector .thing")].find((r) => r.textContent.includes("Old Meg")).click();
choose(selectIn("Conversation"), "__new");
assert.equal($("#inspector .kind").textContent, "Conversation or menu");
assert.match(yaml(), /old_meg:[\s\S]*talk: talk_with_old_meg/);
assert.match(yaml(), /choices:\n\s+talk_with_old_meg:\n\s+start: start\n\s+nodes:\n\s+start:\n\s+text: What do you say\?\n\s+options:\n\s+- text: Goodbye.\n\s+end: true/);
buttonIn("+ Add a part").click();
buttonIn("+ Add a reply").click();
const reply = [...document.querySelectorAll("#inspector .option input")].at(1);
reply.value = "Ask about the oil.";
reply.dispatchEvent(new Event("input"));
choose([...document.querySelectorAll("#inspector .option select")].at(1), "node");
assert.match(yaml(), /- text: Ask about the oil.\n\s+goto: node/);

// A variable, and a verb.
deselect();
const [vname, vvalue] = [...document.querySelectorAll("#inspector .add.three input")];
vname.value = "gold";
vvalue.value = "5";
[...document.querySelectorAll("#inspector .add.three")].find((r) => r.contains(vname)).querySelector("button").click();
assert.match(yaml(), /vars:\n  gold: 5\n/);
inputIn("New verb").value = "ring";
buttonIn("Add a verb").click();
assert.equal($("#inspector .kind").textContent, "Verb");
assert.match(yaml(), /verbs:\n\s+ring:\n\s+patterns: \["ring \{target\}"\]/);

// A script file: listed in scripts:, opened in the Files tab; a mistake in it leads back to it.
deselect();
inputIn("New script file's name").value = "helpers";
buttonIn("Add a file").click();
assert.ok(projectFiles()["scripts/helpers.lua"].includes("function greet"));
assert.match(yaml(), /version: "1.0"[\s\S]*?\n\nscripts:\n  - scripts\/helpers.lua\n/);
assert.equal($("#file").value, "scripts/helpers.lua");
$("#yaml").value = "function greet( end";
$("#yaml").dispatchEvent(new Event("input"));
await tick(700);
assert.equal(projectFiles()["scripts/helpers.lua"], "function greet( end");
await tick(1500);
const lua = [...document.querySelectorAll("#panel-problems li")].find((li) => li.textContent.includes("helpers.lua"));
assert.ok(lua, $("#panel-problems").textContent);
$("#file").value = "game.yaml";
$("#file").dispatchEvent(new Event("change"));
lua.click();
assert.equal($("#file").value, "scripts/helpers.lua", "the problem opens the file");
// Undo takes the edit back, then the file away.
$("#undo").click();
assert.ok(projectFiles()["scripts/helpers.lua"].includes("function greet"));
$("#undo").click();
assert.equal(projectFiles()["scripts/helpers.lua"], undefined);
assert.doesNotMatch(yaml(), /scripts:/);
$("#redo").click();
assert.ok(projectFiles()["scripts/helpers.lua"]);

await tick(1500);
assert.doesNotMatch($("#panel-problems").textContent, /Error/, $("#panel-problems").textContent);

// Add component: one button for everything, a form where it needs one.
const addComponent = (which) => {
  $("#add-component").click();
  assert.ok($("#add-dialog").open, "the window opens");
  $(`#add-dialog [data-component="${which}"]`).click();
};
const form = (label) => [...document.querySelectorAll("#add-dialog select, #add-dialog input")].find((x) => x.getAttribute("aria-label") === label);
const addButton = (text) => [...document.querySelectorAll("#add-dialog button")].find((b) => b.textContent === text);
// A room: added straight away, in the middle of the map.
const roomsBefore = (yaml().match(/^  \w+:\n    name: New room/gm) ?? []).length;
addComponent("room");
assert.equal($("#add-dialog").open, false);
assert.equal((yaml().match(/^  \w+:\n    name: New room/gm) ?? []).length, roomsBefore + 1);
assert.equal($("#inspector .kind").textContent, "Room");
// An object: it asks which room (suggesting the selected one) before it's made.
press($(`#map [data-room="${eastRoom}"]`));
const before = yaml();
addComponent("object");
assert.equal(yaml(), before, "nothing is made until the room is chosen");
assert.equal(form("Room to add it to").value, eastRoom);
form("Kind of object").value = "container";
form("Kind of object").dispatchEvent(new Event("change"));
form("Room to add it to").value = "first_room";
form("Room to add it to").dispatchEvent(new Event("change"));
addButton("Add the object").click();
assert.equal($("#add-dialog").open, false);
assert.match(yaml(), /\n  box:\n    name: box\n    in: first_room\n    traits: \[container, openable\]/);
assert.equal($("#inspector .kind").textContent, "Thing");
// A character, an ending, a variable and a door.
addComponent("character");
form("Room to add them to").value = "first_room";
form("Room to add them to").dispatchEvent(new Event("change"));
addButton("Add the character").click();
assert.match(yaml(), /characters:[^]*?in: first_room/);
addComponent("ending");
form("Ending's name").value = "Escaped";
form("Kind of ending").value = "loss";
form("Kind of ending").dispatchEvent(new Event("change"));
addButton("Add the ending").click();
assert.match(yaml(), /endings:[^]*?escaped(_\d+)?:\n\s+kind: loss/);
addComponent("variable");
form("Variable's name").value = "lamp_oil";
form("Its starting value").value = "3";
addButton("Add the variable").click();
assert.match(yaml(), /vars:[^]*?lamp_oil: 3/);
addComponent("door");
form("Room the door is in").value = "first_room";
form("Room the door is in").dispatchEvent(new Event("change"));
assert.ok(form("Which way"), $("#add-dialog").textContent);
form("Door's name").value = "oak door";
addButton("Add the door").click();
assert.match(yaml(), /name: oak door\n\s+in: first_room\n\s+traits: \[door, openable, scenery\]/);
// Back to the list from a form, and Cancel closes it.
addComponent("verb");
[...document.querySelectorAll("#add-dialog button")].find((b) => b.textContent.startsWith("←")).click();
assert.ok($('#add-dialog [data-component="room"]'));
addButton("Cancel").click();
assert.equal($("#add-dialog").open, false);
// A hint, from Add component; then its steps and condition in the details panel.
addComponent("hint");
form("Puzzle's name").value = "Light the lamp";
addButton("Add the hint").click();
assert.match(yaml(), /hints:\n  light_the_lamp:\n    steps:\n      - "A nudge: what should the player notice\?"/);
const stepBox = (n) => $(`#inspector textarea[aria-label="Step ${n}"]`);
stepBox(1).value = "The lamp needs oil.";
stepBox(1).dispatchEvent(new Event("input"));
clickButton("Add a step");
stepBox(3).value = "FILL LAMP.";
stepBox(3).dispatchEvent(new Event("input"));
const offered = $('#inspector textarea[aria-label="Offered while"]');
offered.value = "vars.lamp_oil > 0";
offered.dispatchEvent(new Event("input"));
assert.match(yaml(), /light_the_lamp:\n    steps:\n      - The lamp needs oil\.\n      - "The answer: what to type\."\n      - FILL LAMP\.\n    when: vars.lamp_oil > 0/);
[...document.querySelectorAll("#inspector .hint-step")][1].querySelector("button").click();
assert.equal(document.querySelectorAll("#inspector .hint-step").length, 2);
await tick(1500);
assert.doesNotMatch($("#panel-problems").textContent, /Error/, $("#panel-problems").textContent);

// Test play, in the drawer: the game as it is, from the start room or a chosen one.
$("#tab-play").click();
for (let i = 0; i < 40 && !$("#panel-play .tp-log .title"); i++) await tick(50);
const tp = $("#panel-play");
const firstRoom = yaml().match(/start: (\w+)/)[1];
assert.ok(tp.querySelector(".tp-log .title"), tp.textContent);
assert.equal($("#map .room.here")?.dataset.room, firstRoom, "the map marks where the player is");
const tpLine = tp.querySelector(".tp-prompt input");
tpLine.value = "look";
tp.querySelector(".tp-prompt").requestSubmit();
assert.match(tp.querySelector(".tp-log").textContent, /> ?look|look/);
// Start in another room: select it on the map first.
press($(`#map [data-room="${eastRoom}"]`));
const startIn = [...tp.querySelectorAll(".tp-bar button")].find((b) => b.textContent.startsWith("Start in"));
assert.equal(startIn.disabled, false);
startIn.click();
await tick(50);
assert.match(tp.querySelector(".tp-log").textContent, /as if the game began there/);
assert.equal($("#map .room.here")?.dataset.room, eastRoom);
assert.match(yaml(), new RegExp(`start: ${firstRoom}`), "the game file itself is unchanged");
// Changing the game marks the game in progress as out of date; moving a room doesn't.
assert.ok(tp.querySelector(".tp-stale").hidden);
pointer($(`#map [data-room="${eastRoom}"]`), "pointerdown", 0, 0);
pointer($(`#map [data-room="${eastRoom}"]`), "pointermove", 60, 0);
pointer($(`#map [data-room="${eastRoom}"]`), "pointerup", 60, 0);
assert.ok(tp.querySelector(".tp-stale").hidden, "moving a room isn't a change to the game");
typeName([...document.querySelectorAll("#inspector input[type=text]")][0], "Changed room");
assert.equal(tp.querySelector(".tp-stale").hidden, false);
// Leaving the tab takes the mark off the map.
$("#tab-problems").click();
assert.equal($("#map .room.here"), null);

assert.deepEqual(errors, []);
console.log("ok   editor page");
await GlobalRegistrator.unregister();
process.exit(0);
