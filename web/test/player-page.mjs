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
// Bookmarks: mark a moment, read it again, show it, and go back to it.
window.confirm = () => true;
Element.prototype.scrollIntoView ??= () => {};
const clickIn = (box, text) => {
  const b = [...box.querySelectorAll("button")].find((x) => x.textContent === text);
  assert.ok(b, `a "${text}" button in ${box.textContent}`);
  b.click();
};
type("look");
$("#bookmarks-open").click();
const bm = $("#bookmarks");
assert.ok(bm.open);
assert.match(bm.textContent, /No bookmarks yet/);
bm.querySelector("input[type=text]").value = "The kitchen at midnight";
clickIn(bm, "Bookmark this moment");
assert.equal(bm.querySelectorAll(".bookmarks li strong").length, 1);
assert.match(bm.querySelector(".bookmarks li").textContent, /The kitchen at midnight.*Kitchen, turn/);
assert.ok(document.querySelectorAll("#log .bookmarked").length >= 2, "the command and its reply are marked");
const kept = JSON.parse(localStorage.getItem("eifgb:bookmarks:example:midnight_snack"));
assert.equal(kept[0].text[0], "> look");
assert.ok(kept[0].text.length > 1 && kept[0].save, "the reply, and the game then");
clickIn(bm, "Read");
assert.match(bm.querySelector(".passage").textContent, /> look/);
clickIn(bm, "Show it in the story");
assert.equal(bm.open, false);
assert.ok($("#log .flash"), "shown, and flashed");
// Somewhere else, then back to the bookmark.
const then = $("#status").textContent;
type("x mat");
type("look under the mat");
assert.notEqual($("#status").textContent, then);
$("#bookmarks-open").click();
clickIn(bm, "Go back to this moment");
assert.equal($("#status").textContent, then, "back where it was bookmarked");

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
$("#settings-dialog")?.close?.();
localStorage.removeItem("eifgb:settings");

// Read aloud: new text is spoken with the browser's voices, a reply at a
// time; Escape stops it. Without a voice, the setting isn't offered.
$("#settings-open").click();
assert.equal($("#setting-speech"), null, "no speech in this browser: not offered");
const spoken = [];
let cancels = 0;
window.SpeechSynthesisUtterance = globalThis.SpeechSynthesisUtterance = class {
  constructor(text) {
    this.text = text;
  }
};
window.speechSynthesis = globalThis.speechSynthesis = { speak: (u) => spoken.push(u), cancel: () => cancels++ };
$("#settings-open").click();
assert.ok($("#setting-speech"), "Read aloud is offered while playing");
type("look");
assert.equal(spoken.length, 0, "off unless chosen");
localStorage.setItem("eifgb:settings", JSON.stringify({ speech: "slow" }));
type("look");
assert.ok(spoken.some((u) => /Shelves of jars/.test(u.text)), spoken.map((u) => u.text).join(" | "));
assert.ok(!spoken.some((u) => u.text === "look"), "the command itself isn't read");
assert.equal(spoken[0].rate, 0.8);
assert.equal(spoken[0].lang, "en");
assert.equal($("#log").lang, "en", "the story is marked with its language");
const before = cancels;
document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
assert.equal(cancels, before + 1, "Escape stops reading");
localStorage.removeItem("eifgb:settings");
console.log("ok   player page");
