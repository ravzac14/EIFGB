// The editor's start screen offers starter templates: checks they're listed
// after the blank game, and that starting from one opens it on the map,
// with its rooms placed and coloured. Run: node test/starters.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: pathToFileURL(`${web}public/editor.html`).href, width: 1200, height: 800 });
Element.prototype.setPointerCapture ??= () => {};
globalThis.CSS ??= { escape: (s) => s };
// The page's fetches, served from public/ (npm run prepare-site makes them).
globalThis.fetch = window.fetch = async (url) => {
  const path = String(url).replace(/^.*?(games\/|wasm\/|verbs\.json)/, (_, a) => a);
  const bytes = readFileSync(`${web}public/${path}`);
  const text = bytes.toString("utf8");
  return { ok: true, json: async () => JSON.parse(text), text: async () => text, arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.length) };
};
const downloads = [];
URL.createObjectURL = (blob) => (downloads.push(blob), "blob:test");
URL.revokeObjectURL = () => {};
HTMLAnchorElement.prototype.click = function () {
  downloads.at(-1).name = this.download;
};
HTMLDialogElement.prototype.showModal ??= function () { this.open = true; };

const html = readFileSync(`${web}editor.html`, "utf8");
document.body.innerHTML = html.match(/<body[^>]*>([\s\S]*)<\/body>/)[1].replace(/<script[\s\S]*?<\/script>/g, "");
document.body.className = "editor";
await import("../src/editor/main.ts");
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
await tick(50);

// The blank game first, then each template.
const rows = [...document.querySelectorAll("#starters li")];
assert.deepEqual(
  rows.map((r) => r.querySelector(".name").textContent),
  ["Blank game", "A Small House", "The Locked Door", "Market Day"],
);
assert.ok(document.querySelector("#starters #new-game"), "the blank game's button is still #new-game");

// Start from the dungeon: it opens, with its four rooms where its map.yaml puts them.
rows[2].querySelector("button").click();
await tick(50);
assert.equal(document.querySelector("#workspace").hidden, false);
assert.equal(document.querySelectorAll("#map .room").length, 4);
const key = new URLSearchParams(location.search).get("project");
const saved = JSON.parse(localStorage.getItem(`eifgb:local:${key}`));
assert.equal(saved.title, "The Locked Door");
assert.deepEqual(Object.keys(saved.files).sort(), ["game.yaml", "map.yaml", "walkthrough.txt"]);
assert.match(saved.files["map.yaml"], /guard_room: red/);
assert.match(document.querySelector('#map [data-room="stairs"]').getAttribute("style") ?? "", /--tint/);

// Download: the files as a zip, or the game as one web page.
document.querySelector("#download").click();
const dialog = document.querySelector("#download-dialog");
const choice = (text) => [...dialog.querySelectorAll("button")].find((b) => b.textContent.startsWith(text));
choice("Download the files").click();
assert.equal(downloads.at(-1).name, "locked_dungeon.zip");
document.querySelector("#download").click();
choice("Download a web page").click();
for (let i = 0; i < 100 && downloads.length < 2; i++) await tick(20);
const page = downloads.at(-1);
assert.equal(page.name, "locked_dungeon.html");
const pageHtml = await page.text();
assert.match(pageHtml, /<title>The Locked Door<\/title>/);
assert.match(pageHtml, /id="eifgb-wasm"/);
assert.ok(pageHtml.length > 1_000_000, "the engine is inside");

console.log("starters: ok");
process.exit(0);
