// Two players, one screen: taking turns as a game's playable characters,
// in a simulated browser. Run: node test/player-together.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: `${pathToFileURL(`${web}public/play.html`).href}?local=two`, width: 1200, height: 800 });
window.scrollTo = () => {};
Element.prototype.scrollIntoView ??= () => {};
window.fetch = async (url) => new Response(readFileSync(new URL(url, location.href)));
HTMLDialogElement.prototype.showModal ??= function () {
  this.open = true;
};
HTMLDialogElement.prototype.close ??= function () {
  this.open = false;
};

const game = `format: eifgb/0.1
game: { id: two, title: Two, version: "1", start: hall }
player: { plays: mara }
rooms:
  hall: { name: Hall, exits: { east: kitchen } }
  kitchen: { name: Kitchen, exits: { west: hall } }
objects:
  pebble: { name: pebble, in: hall, traits: [portable] }
  pan: { name: pan, in: kitchen, traits: [portable] }
characters:
  mara: { name: Mara, in: hall, playable: true, description: A wiry thief. }
  tobin: { name: Tobin, in: kitchen, playable: true, description: A big cook. }
`;
localStorage.setItem("eifgb:local", JSON.stringify(["two"]));
localStorage.setItem("eifgb:local:two", JSON.stringify({ key: "two", title: "Two", files: { "game.yaml": game }, added: 1 }));

const html = readFileSync(`${web}play.html`, "utf8");
document.body.innerHTML = html.match(/<body[^>]*>([\s\S]*)<\/body>/)[1].replace(/<script[\s\S]*?<\/script>/g, "");
await import("../src/play.ts");

const $ = (s) => document.querySelector(s);
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
for (let i = 0; i < 100 && $("#prompt").hidden; i++) await tick(50);
assert.equal($("#prompt").hidden, false, "the game started");
const type = (text) => {
  $("#line").value = text;
  $("#prompt").requestSubmit();
};
const lastLines = (n) => [...$("#log").children].slice(-n).map((e) => e.textContent).join("\n");

// The button shows for a game with characters to play.
assert.equal($("#together-open").hidden, false);
$("#together-open").click();
const d = $("#together");
assert.ok(d.open);
const inputs = d.querySelectorAll("input[type=text]");
const picks = d.querySelectorAll("select");
inputs[0].value = "Sam";
inputs[1].value = "Jo";
assert.equal(picks[0].value, "mara");
assert.equal(picks[1].value, "tobin");
// The same character twice won't do.
picks[1].value = "mara";
[...d.querySelectorAll("button")].find((b) => b.textContent === "Start taking turns").click();
assert.match(d.textContent, /a different character for each/);
picks[1].value = "tobin";
[...d.querySelectorAll("button")].find((b) => b.textContent === "Start taking turns").click();
assert.equal(d.open, false);
assert.equal($("#turn-bar").hidden, false);
assert.match($("#turn-bar").textContent, /Sam's turn, as Mara\./);

// Sam moves: then it's Jo's turn, as Tobin, in the kitchen; no extra turn passes.
type("take pebble");
assert.match($("#log").textContent, /Sam: take pebble/);
assert.match(lastLines(6), /Over to Jo, as Tobin\./);
assert.match(lastLines(6), /You're Tobin now\./);
assert.match($("#status").textContent, /Kitchen · Turn 1/);
assert.match($("#turn-bar").textContent, /Jo's turn, as Tobin\./);
type("take pan");
assert.match($("#status").textContent, /Hall · Turn 2/);
assert.match($("#turn-bar").textContent, /Sam's turn/);
type("notes");
// NOTES takes no turn: still Sam's.
assert.match($("#turn-bar").textContent, /Sam's turn/);
type("i");
assert.match($("#log").textContent, /Sam: iYou.re carrying a pebble/);
assert.match($("#turn-bar").textContent, /Jo's turn/, "INVENTORY takes a turn, like most things");
// UNDO: Sam's move taken back, and Sam's turn again.
type("undo");
assert.match($("#turn-bar").textContent, /Sam's turn, as Mara/);
// Kept with the game: reloading carries on taking turns.
const kept = JSON.parse(localStorage.getItem("eifgb:together:local:two"));
assert.deepEqual(kept.players.map((p) => p.name), ["Sam", "Jo"]);
// Stopping.
[...$("#turn-bar").querySelectorAll("button")].find((b) => b.textContent === "Stop taking turns").click();
assert.equal($("#turn-bar").hidden, true);
assert.equal(localStorage.getItem("eifgb:together:local:two"), null);
type("look");
assert.doesNotMatch(lastLines(4), /Over to/);

console.log("player together: ok");
process.exit(0);
