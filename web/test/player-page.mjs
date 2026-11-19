// The play page in a simulated browser: plays an example to its end by
// typing, then checks the endings found and the transcript download.
// Run: node test/player-page.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: `${pathToFileURL(`${web}public/play.html`).href}?example=midnight_snack`, width: 1200, height: 800 });
// Things a test DOM leaves out.
window.scrollTo = () => {};
window.fetch = async (url) => new Response(readFileSync(new URL(url, location.href)));
const downloads = [];
URL.createObjectURL = (blob) => (downloads.push(blob), "blob:test");
URL.revokeObjectURL = () => {};
HTMLAnchorElement.prototype.click = function () {
  if (this.download) downloads.at(-1).name = this.download;
};
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
const steps = readFileSync(`${web}../spec/examples/midnight_snack/walkthrough.txt`, "utf8")
  .split("\n")
  .map((l) => l.trim())
  .filter((l) => l && !l.startsWith("#"));
for (const s of steps) type(s);

// The end: what kind, and the endings found so far.
assert.match($("#after").textContent, /You won\. You've found 1 of 2 endings\./);
// The game says UNDO, RESTORE or RESTART still work, so the prompt stays.
assert.match($("#log").textContent, /Type UNDO to take back your last move/);
assert.equal($("#prompt").hidden, false);
type("undo");
assert.equal($("#after").textContent, "", "back in the story: the ending's buttons go");
assert.match($("#status").textContent, /Pantry/);
type("eat it");
assert.match($("#after").textContent, /You won\./);

// The transcript, from the button after the ending.
[...$("#after").querySelectorAll("button")].find((b) => b.textContent === "Download the transcript").click();
const file = downloads.at(-1);
assert.equal(file.name, "midnight_snack-transcript.txt");
const text = await file.text();
assert.match(text, /^Midnight Snack/);
assert.match(text, new RegExp(`\\n> ${steps[0]}\\n`));
assert.match(text, /\n> .+\n[^>\n]/, "each command is followed by its reply");

// Feedback from the play page: where they are in the game, and its last lines.
HTMLDialogElement.prototype.showModal ??= function () { this.open = true; };
$("#feedback-open").click();
const sentAlong = $("#feedback .feedback-context").textContent;
assert.match(sentAlong, /game: Midnight Snack/);
assert.match(sentAlong, /where: Pantry, turn \d+/);
assert.match(sentAlong, /state: the game has ended/);
assert.match(sentAlong, /last lines of the game:[^]*> eat it/);
$("#feedback").close?.();

// Typewriter reveal: new lines start see-through and are typed out, but
// their whole text is there from the start.
localStorage.setItem("eifgb:settings", JSON.stringify({ reveal: "typewriter" }));
type("undo");
type("look");
const typing = [...$("#log").querySelectorAll(".untyped")];
assert.ok(typing.length >= 2, "the room's lines are being typed");
assert.match($("#log").textContent.slice(-200), /Shelves of jars[^]*Known exits/, "the whole text is on the page");
for (let i = 0; i < 100 && $("#log .untyped"); i++) await tick(20);
assert.equal($("#log .untyped"), null, "typed out in the end");
// Any key shows everything at once.
type("look");
assert.ok($("#log .untyped"));
document.dispatchEvent(new KeyboardEvent("keydown", { key: "x" }));
assert.equal($("#log .untyped"), null);
assert.match($("#log").textContent.slice(-200), /Shelves of jars[^]*Known exits/);
// The settings dialog offers it here, on the play page.
$("#settings-open").click();
assert.ok($("#setting-reveal"), "Text appears is offered while playing");
localStorage.removeItem("eifgb:settings");
console.log("ok   player page");
