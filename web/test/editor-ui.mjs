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
// The steps to a first game, none done yet.
const firstSteps = [...document.querySelectorAll("#inspector .started li")];
assert.deepEqual(firstSteps.map((li) => li.querySelector("strong").textContent), ["Give it a title", "Add a second room", "Put something in a room", "Add an ending, and something that leads to it", "Win it in Test play, and save the walkthrough"]);
assert.equal(document.querySelectorAll("#inspector .started li.done").length, 0);
// Not wanted: hidden, for every game in this browser, until brought back.
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Don't show this again").click();
assert.equal($("#inspector .started"), null);
assert.equal(localStorage.getItem("eifgb:getting-started-hidden"), "true");
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Show the Getting started checklist again").click();
assert.equal(document.querySelectorAll("#inspector .started li").length, 5);

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
// The add row under "For sale".
let sale = [...document.querySelectorAll("#inspector h3")].find((h) => h.textContent === "For sale");
while (sale && !sale.matches(".add.two")) sale = sale.nextElementSibling;
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

// Renaming something reaches the script files too, and UNDO takes it all back.
deselect();
[...document.querySelectorAll("#inspector .thing")].find((r) => r.textContent.includes("scripts/helpers.lua")).click();
assert.equal($("#file").value, "scripts/helpers.lua");
$("#yaml").value = "function shine()\n  say(world.treasure.name)\nend\n";
$("#yaml").dispatchEvent(new Event("input"));
await tick(700);
// The coin is in the chest: up to the room the chest is in.
let coinRoom = "treasure";
for (let i = 0; i < 5 && !$(`#map [data-room="${coinRoom}"]`); i++) coinRoom = projectFiles()["game.yaml"].match(new RegExp(`\\n  ${coinRoom}:\\n(?:    .*\\n)*?    in: (\\w+)`))[1];
press($(`#map [data-room="${coinRoom}"]`));
rows().find((r) => r.textContent.includes("silver coin")).click();
inputIn("Thing ID").value = "loot";
buttonIn("Rename").click();
assert.match(projectFiles()["game.yaml"], /\n  loot:\n\s+name: silver coin/);
assert.equal(projectFiles()["scripts/helpers.lua"], "function shine()\n  say(world.loot.name)\nend\n");
$("#undo").click();
assert.equal(projectFiles()["scripts/helpers.lua"], "function shine()\n  say(world.treasure.name)\nend\n");
$("#redo").click();

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
// A goal, from Add component; then when it's done.
addComponent("goal");
form("Goal").value = "Light the lamp.";
addButton("Add the goal").click();
assert.match(yaml(), /goals:\n  light_the_lamp:\n    text: Light the lamp\.\n/);
const doneWhen = $('#inspector textarea[aria-label="Done when (Lua)"]');
doneWhen.value = "vars.lamp_oil > 5";
doneWhen.dispatchEvent(new Event("input"));
assert.match(yaml(), /done: vars.lamp_oil > 5/);
// The time of day, and a timer that goes off at dusk.
addComponent("clock");
assert.match(yaml(), /clock:\n  start: "?8:00"?/);
clickButton("Add a timer");
const how = $('#inspector select[aria-label="How often"]');
how.value = "at";
how.dispatchEvent(new Event("change"));
assert.match(yaml(), /at: dusk/);
assert.equal($('#inspector input[aria-label="Time of day"]').value, "dusk");
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
// What wasn't understood is kept, with a fix for each.
const tpSay = (text) => {
  tpLine.value = text;
  tp.querySelector(".tp-prompt").requestSubmit();
};
assert.ok(tp.querySelector(".tp-misses").hidden, "nothing yet");
tpSay("yoink everything");
tpSay("yoink everything");
tpSay("x zorkmid");
const misses = tp.querySelector(".tp-misses");
assert.equal(misses.hidden, false);
assert.match(misses.querySelector("summary").textContent, /Not understood \(2\)/, "the same miss is kept once");
assert.match(misses.textContent, /“yoink” isn't a verb this game knows/);
assert.match(misses.textContent, /“zorkmid” isn't a name for anything here/);
const yoink = [...misses.querySelectorAll("li")].find((li) => li.textContent.includes("yoink"));
yoink.querySelector("select").value = "take";
[...yoink.querySelectorAll("button")].find((b) => b.textContent === "Make it mean that").click();
assert.match(yaml(), /aliases:\n\s+yoink: take/);
assert.match(misses.querySelector("summary").textContent, /Not understood \(1\)/);
const zork = [...misses.querySelectorAll("li")].find((li) => li.textContent.includes("zorkmid"));
[...zork.querySelectorAll("button")].find((b) => b.textContent === "Add it as something here").click();
assert.match(yaml(), /\n  zorkmid:\n\s+name: zorkmid/);
assert.ok(misses.hidden, "all fixed");
$("#tab-play").click();
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

// The puzzle chain: from the start, and what can't happen.
await tick(1500);
$("#tab-puzzles").click();
assert.equal($("#panel-puzzles").hidden, false);
const chain = [...document.querySelectorAll("#panel-puzzles .chain .step")].map((li) => li.textContent);
assert.match(chain[0], /^Start in /, chain.join(" | "));
assert.ok($("#panel-puzzles").textContent.includes("Endings"), $("#panel-puzzles").textContent);
document.querySelector("#panel-puzzles .chain .step").click();
assert.equal($("#inspector .kind").textContent, "Room", "a step selects what it's about");

// History: each change says what it was, and going back several at once works.
$("#history").click();
const hist = $("#history-dialog");
assert.ok(hist.open);
const steps = [...hist.querySelectorAll(".history li:not(.now):not(.undone)")];
assert.ok(steps.length >= 5, hist.textContent);
assert.match(steps[0].querySelector(".what").textContent, /Renamed the room New room to Changed room/, "the newest first, by name");
assert.ok(steps.some((li) => /Added the room/.test(li.textContent)), hist.textContent);
const beforeBack = yaml();
[...steps[1].querySelectorAll("button")].find((b) => /Undo back to here \(2 changes\)/.test(b.textContent)).click();
assert.equal(hist.open, false);
assert.notEqual(yaml(), beforeBack);
$("#history").click();
assert.equal(hist.querySelectorAll(".history li.undone").length, 2, "both can be redone");
[...hist.querySelectorAll(".history li.undone button")][0].click();
assert.equal(yaml(), beforeBack, "redone, both");
assert.match($("#undo").title, /^Undo: Renamed the room New room to Changed room/);

// Find in game: a word anywhere, and going to it.
$("#tab-find").click();
assert.equal($("#panel-find").hidden, false);
const findText = $("#find-text");
findText.value = "changed room";
findText.dispatchEvent(new Event("input"));
await tick(250);
const results = [...document.querySelectorAll("#find-results li")];
assert.ok(results.length >= 1, $("#panel-find").textContent);
assert.match(results[0].textContent, /^Room Changed room/);
assert.equal(results[0].querySelector("mark").textContent.toLowerCase(), "changed room");
results[0].click();
assert.equal($("#inspector .kind").textContent, "Room");
assert.equal($("#panel-find").hidden, false, "the results stay open");
findText.value = "no such words anywhere";
findText.dispatchEvent(new Event("input"));
await tick(250);
assert.match($("#find-count").textContent, /Not found/);

// Tidy up: one change, which undo takes back.
$("#tidy").click();
assert.match($("#undo").title, /^Undo: Tidied up the map/);
$("#undo").click();

// A game with timers, a key to find and a walkthrough: the Timers tab, hint
// drafts, and test play from a step of the walkthrough.
const snack = readFileSync(`${web}../spec/examples/midnight_snack/game.yaml`, "utf8");
$("#tab-yaml").click();
$("#file").value = "game.yaml";
$("#file").dispatchEvent(new Event("change"));
$("#yaml").value = snack;
$("#yaml").dispatchEvent(new Event("input"));
await tick(700);
assert.equal($("#tab-timers").hidden, false);
$("#tab-timers").click();
const tl = $("#panel-timers");
assert.match(tl.textContent, /pie_cools/);
assert.match(tl.textContent, /once, after 6 actions/);
assert.equal(tl.querySelectorAll(".tl-mark").length, 1);
tl.querySelector(".tl-label button").click();
assert.match($("#inspector").textContent, /pie_cools/, "a timer's name selects it");
await tick(500);
$("#tab-puzzles").click();
// For each ending, only the steps it needs, each after what it waits on.
const heads = [...document.querySelectorAll("#panel-puzzles h4")].map((h) => h.textContent);
assert.ok(heads.includes("The fed ending (a win): 2 steps"), heads.join(" | "));
const fed = document.querySelectorAll("#panel-puzzles .chain")[0];
const fedSteps = [...fed.querySelectorAll(".step .what")].map((x) => x.textContent);
assert.deepEqual(fedSteps.slice(0, 3), ["Start in Kitchen.", "Search the worn mat: a small key.", "Unlock the pantry door with the small key: Pantry opens up."]);
assert.equal(fed.querySelectorAll(".step")[2].querySelector(".after").textContent, " after 2");
const draft = [...document.querySelectorAll("#panel-puzzles .draft-hints button")][0];
assert.match(draft.textContent, /Draft 2 hints from these steps/);
draft.click();
assert.match(yaml(), /hints:\n\s+find_small_key:\n\s+when: not world\.small_key\.seen\n\s+steps:\n\s+- A small key might be useful/);
assert.match($("#undo").title, /^Undo: Drafted 2 hints from the puzzle chain/);
await tick(500);
assert.equal(document.querySelectorAll("#panel-puzzles .draft-hints button").length, 0, "not drafted twice");
// Record the first three moves as the walkthrough, then start at step 2.
$("#tab-play").click();
await tick(50);
[...tp.querySelectorAll(".tp-bar button")].find((b) => b.textContent === "Start over").click();
await tick(50);
for (const c of ["n", "x mat", "look under the mat"]) tpSay(c);
[...tp.querySelectorAll(".tp-bar button")].find((b) => b.textContent === "Save as walkthrough").click();
await tick(500);
const jump = tp.querySelector(".tp-jump");
assert.equal(jump.hidden, false);
assert.equal(jump.options.length, 4);
jump.value = "2";
jump.dispatchEvent(new Event("change"));
await tick(50);
assert.match(tp.querySelector(".tp-log").textContent, /Played the walkthrough up to step 2 \(x mat\)/);
assert.doesNotMatch(tp.querySelector(".tp-log").textContent, /> n\b/, "the moves before aren't shown");
// After an edit: the moves again, on the new version.
typeName([...document.querySelectorAll("#inspector input[type=text]")][0], "Renamed again");
const replay = [...tp.querySelectorAll(".tp-stale button")].find((b) => /new version up to here/.test(b.textContent));
replay.click();
await tick(50);
assert.match(tp.querySelector(".tp-log").textContent, /Played your 2 moves again on the new version/);

// Either-or choices, and what the chain takes on trust, in the Puzzles tab.
$("#tab-yaml").click();
$("#file").value = "game.yaml";
$("#file").dispatchEvent(new Event("change"));
$("#yaml").value = `format: eifgb/0.1
game: { id: t, title: T, version: "1", start: forge }
vars: { armed: false }
rooms:
  forge: { name: Forge, exits: { north: lair, east: { to: yard, when: clock.night } } }
  lair: { name: Lair, exits: { south: forge, north: { to: hoard, through: hoard_door } } }
  hoard: { name: Hoard, on: { enter: 'end_game("rich")' } }
  yard: { name: Yard }
clock: { start: "8:00" }
objects:
  hoard_door: { name: hoard door, in: lair, traits: [door, openable, lockable], state: { locked: true }, unlocks_with: sword }
  sword: { name: sword, traits: [portable] }
  shield: { name: shield, traits: [portable] }
characters:
  smith: { name: smith, in: forge, talk: armoury }
choices:
  armoury:
    start: pick
    nodes:
      pick:
        text: Take one.
        options:
          - { text: Take the sword, when: not vars.armed, do: 'vars.armed = true; world.sword:move_to(player)', end: true }
          - { text: Take the shield, when: not vars.armed, do: 'vars.armed = true; world.shield:move_to(player)', end: true }
endings:
  rich: { kind: win, text: Rich. }
`;
$("#yaml").dispatchEvent(new Event("input"));
await tick(1200);
$("#tab-puzzles").click();
const pz = $("#panel-puzzles");
assert.match(pz.textContent, /Where a player could get stuck/);
assert.match(pz.textContent, /If the player chooses “Take the shield”, they can't win: it rules out “Take the sword”\./);
assert.match(pz.querySelector(".only-if").textContent, /^Only if the player chooses “Take the sword”\.$/);
const chooseStep = pz.querySelector(".chain .step.choose");
assert.match(chooseStep.querySelector(".either").textContent, /Only one of these: it rules out Choose “Take the shield” in the conversation armoury\./);
chooseStep.click();
assert.match($("#inspector").textContent, /armoury/, "a choice selects its conversation");
assert.match(pz.querySelector(".assumes-box summary").textContent, /What the chain takes on trust/);
assert.ok(!/clock\.night/.test(pz.querySelector(".assumes-box").textContent), "the yard isn't on the way to an ending");

// A conversation as a graph: on the canvas while it's selected.
assert.equal($("#talk").hasAttribute("hidden"), false, "the conversation's graph");
assert.equal($("#map").hasAttribute("hidden"), true);
assert.equal(document.querySelectorAll("#talk .talk-node").length, 1);
assert.equal(document.querySelectorAll("#talk .talk-port").length, 2, "a dot for each reply");
assert.ok(document.querySelector("#talk .talk-end"), "replies that end it lead to End");
// Drag a reply's dot into empty space: a new part, and the reply leads there.
const port = document.querySelector("#talk .talk-port");
port.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 270, clientY: 70, pointerId: 1 }));
window.dispatchEvent(new PointerEvent("pointermove", { clientX: 600, clientY: 300, pointerId: 1 }));
window.dispatchEvent(new PointerEvent("pointerup", { clientX: 600, clientY: 300, pointerId: 1 }));
assert.equal(document.querySelectorAll("#talk .talk-node").length, 2);
assert.match(yaml(), /- \{ ?text: Take the sword, when: not vars\.armed, do: 'vars\.armed = true; world\.sword:move_to\(player\)', goto: node ?\}|goto: node/);
assert.match($("#undo").title, /Added a part to the conversation/);
const mapFile = () => JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files["map.yaml"];
assert.match(mapFile(), /talk:\n {2}armoury:\n {4}pick: \[0, 0\]\n {4}node: \[560, 260\]/, "where the parts are, in map.yaml");
// Clicking a part shows it in the details panel.
document.querySelectorAll("#talk .talk-node")[1].dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 600, clientY: 300, pointerId: 1 }));
window.dispatchEvent(new PointerEvent("pointerup", { clientX: 600, clientY: 300, pointerId: 1 }));
assert.ok(document.querySelector("#talk .talk-node.picked"));
$("#talk-back").click();
assert.equal($("#talk").hasAttribute("hidden"), true);
assert.equal($("#map").hasAttribute("hidden"), false);

// Words in a description: what players can use, and what isn't anything yet.
press($('#map [data-room="forge"]'));
const desc = [...document.querySelectorAll("#inspector textarea")].find((t) => t.placeholder === "What the player sees here.");
desc.value = "Sparks fly from the anvil. The smith hums. A hammer, a hammer, and a hamer hang on the wall.";
desc.dispatchEvent(new Event("input"));
await tick(400);
const words = document.querySelector("#inspector .desc-words");
assert.deepEqual([...words.querySelectorAll(".kw")].map((k) => k.textContent), ["smith"], "a word for the smith");
const adds = [...words.querySelectorAll(".desc-new button")].map((b) => b.textContent);
assert.ok(adds.includes("+ anvil") && adds.includes("+ wall"), adds.join(" "));
[...words.querySelectorAll(".desc-new button")].find((b) => b.textContent === "+ anvil").click();
assert.match(yaml(), /\n  anvil:\n\s+name: anvil\n\s+in: forge\n\s+traits: \[scenery\]/);
// The typo, under Problems.
await tick(500);
assert.match($("#panel-problems .typos-box").textContent, /“hamer”, perhaps “hammer”/);

// The wanderer, in test play.
$("#tab-play").click();
await tick(50);
[...tp.querySelectorAll(".tp-bar button")].find((b) => b.textContent === "Wander").click();
await tick(100);
assert.match(tp.querySelector(".tp-wander").textContent, /2000 random moves over \d+ games?, apart from yours/);
assert.match(tp.querySelector(".tp-wander summary").textContent, /no script errors/);

// Panels: the essentials first, the rest folded, saying what's set.
press($('#map [data-room="forge"]'));
[...document.querySelectorAll("#inspector .thing")].find((r) => r.textContent.includes("smith")).click();
assert.equal($("#inspector .kind").textContent, "Character", $("#inspector").textContent.slice(0, 200));
const sections = [...document.querySelectorAll("#inspector details.section > summary")].map((x) => x.textContent);
assert.ok(sections.some((x) => /^What they do · Stays put, to talk to/.test(x)), sections.join(" | "));
assert.ok(sections.includes("Talking · armoury"), sections.join(" | "));
// A daily routine: it needs the time of day first.
const doing = [...document.querySelectorAll("#inspector details.section")].find((d) => d.querySelector("summary").textContent.startsWith("What they do"));
doing.open = true;
doing.dispatchEvent(new Event("toggle"));
// (This game keeps the time already; one that didn't would offer "Keep the time of day".)
assert.ok(![...doing.querySelectorAll("button")].some((b) => b.textContent === "Keep the time of day"));
const doing2 = () => [...document.querySelectorAll("#inspector details.section")].find((d) => d.querySelector("summary").textContent.startsWith("What they do"));
[...doing2().querySelectorAll("button")].find((b) => b.textContent === "+ Add a time").click();
assert.equal(doing2().open, true, "an opened section stays open after a change");
assert.match(yaml(), /routine: \{ ?"?8:00"?: forge ?\}/);
const time = doing2().querySelector(".routine input.time");
time.value = "13:00";
time.dispatchEvent(new Event("change"));
assert.match(yaml(), /routine: \{ ?"?13:00"?: forge ?\}/);

// Text that varies, in the panel: say something different each time.
press($('#map [data-room="forge"]'));
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Say something different each time…").click();
assert.match(yaml(), /description: ?(\[|\n\s+- )"?Sparks fly/);
assert.equal(document.querySelectorAll("#inspector .vary-lines textarea").length, 2);
const varies = [...document.querySelectorAll("#inspector select")].find((x) => x.getAttribute("aria-label") === "How it varies");
varies.value = "random";
varies.dispatchEvent(new Event("change"));
assert.match(yaml(), /description: ?\{ ?random: ?\[ ?"?Sparks fly|description:\n\s+random:\n\s+- "?Sparks fly/);
[...document.querySelectorAll("#inspector button")].find((b) => b.textContent === "Make it plain again").click();
assert.match(yaml(), /description: "?Sparks fly/);

// The game's panel: scenes beside goals, and the steps to a first game.
$("#map").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
assert.equal($("#inspector .kind").textContent, "Your game");
assert.equal($("#inspector .started"), null, "every step done: no checklist");
const scenesGroup = [...document.querySelectorAll("#inspector details.group")].find((d) => d.querySelector("summary").textContent.startsWith("Scenes"));
assert.equal(scenesGroup.open, false, "folded until opened");
[...scenesGroup.querySelectorAll("button")].find((b) => b.textContent === "Add a scene").click();
assert.match(yaml(), /scenes:\n\s+a_new_scene:\n\s+title: A new scene\n\s+when: "?false"?\n\s+done: "?false"?/);
const scenesGroup2 = [...document.querySelectorAll("#inspector details.group")].find((d) => d.querySelector("summary").textContent.startsWith("Scenes"));
assert.match(scenesGroup2.querySelector("summary").textContent, /Scenes · 1/);

assert.deepEqual(errors, []);
console.log("ok   editor page");
await GlobalRegistrator.unregister();
process.exit(0);
