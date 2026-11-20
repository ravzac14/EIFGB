// A game downloaded as a single web page: builds one for a starter
// template, runs its scripts the way a browser would (in a simulated
// one), and plays it to a win; then checks progress carries on after a
// reload. Run: node test/standalone.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
const dir = `${web}../spec/templates/locked_dungeon/`;
// With a picture of the cell, kept inside the page.
const cellPicture = "data:image/svg+xml;base64,PHN2Zy8+";
const files = {
  "game.yaml": readFileSync(`${dir}game.yaml`, "utf8").replace(/\n  cell:\n/, '\n  cell:\n    picture: { file: pictures/cell.svg, alt: "Straw on a stone floor." }\n'),
  "pictures/cell.svg": cellPicture,
};
const steps = readFileSync(`${dir}walkthrough.txt`, "utf8").split("\n").filter((l) => l.trim() && !l.startsWith("#"));
const { standalonePage, classicLoader } = await import("../src/standalone.ts");
const engine = { loader: readFileSync(`${web}public/wasm/eifgb.mjs`, "utf8"), wasm: new Uint8Array(readFileSync(`${web}public/wasm/eifgb_web.wasm`)) };
const page = standalonePage(files, "The <Locked> Door", engine, "https://example.org/EIFGB/");

// One file, with everything in it.
assert.ok(page.length < 4_000_000, `${(page.length / 1e6).toFixed(1)} MB`);
assert.match(page, /<title>The &lt;Locked&gt; Door<\/title>/);
assert.match(page, /Made with <a href="https:\/\/example.org\/EIFGB\/">EIFGB<\/a>/);
assert.doesNotMatch(page, /<script[^>]+src=|<link[^>]+href=|import\.meta/, "nothing loaded from elsewhere");
assert.equal(page.match(/<\/script>/g).length, 4, "four scripts, none cut short");
assert.match(page, /:root\[data-theme="parchment"\]/, "the site's themes come along");
assert.throws(() => classicLoader("console.log(1)"), /expected form/);

// Run it: the page's body, then its two scripts, in order.
GlobalRegistrator.register({ url: "file:///C:/games/locked_dungeon.html", width: 1000, height: 800 });
window.scrollTo = () => {};
const body = page.match(/<body[^>]*>([\s\S]*)<\/body>/)[1];
const scripts = [...body.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
assert.equal(scripts.length, 2);
const run = () => {
  document.body.innerHTML = body.replace(/<script>[\s\S]*?<\/script>/g, "");
  for (const s of scripts) (0, eval)(s);
};
const $ = (s) => document.querySelector(s);
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
const started = async () => {
  for (let i = 0; i < 200 && $("#prompt").hidden; i++) await tick(25);
  assert.equal($("#prompt").hidden, false, $("#log").textContent);
};
const type = (text) => {
  $("#line").value = text;
  $("#prompt").requestSubmit();
};
run();
await started();
assert.equal(document.title, "The Locked Door");
assert.match($("#log").textContent, /You wake on cold straw/);
assert.match($("#status").textContent, /Your Cell · Turn 0/);
const img = $("#log figure.picture img");
assert.equal(img?.src, cellPicture, "the room's picture, from inside the page");
assert.equal(img.alt, "Straw on a stone floor.");

// Part way, then the page is opened again: it carries on.
type(steps[0]);
type(steps[1]);
assert.match($("#status").textContent, /Guard Room/);
run();
await started();
assert.match($("#log").textContent, /Carrying on where you left off/);
assert.match($("#status").textContent, /Guard Room · Turn 2/);
for (const s of steps.slice(2)) type(s);
assert.match($("#log").textContent, /Free!/);
assert.match($("#log").textContent, /You have won/);
// SAVE and RESTORE, kept in the browser.
type("undo");
type("save");
assert.match($("#log").lastElementChild.textContent, /Saved/);
type("s");
type("restore");
assert.match($("#status").textContent, /Corridor/);
console.log("ok   single web page");
process.exit(0);
