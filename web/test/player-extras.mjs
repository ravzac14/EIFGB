// The play page's notes panel, sounds and embedded mode, in a simulated
// browser, on a small game kept in this browser.
// Run: node test/player-extras.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: `${pathToFileURL(`${web}public/play.html`).href}?local=sounds&embed=1`, width: 1200, height: 800 });
window.scrollTo = () => {};
Element.prototype.scrollIntoView ??= () => {};
window.fetch = async (url) => new Response(readFileSync(new URL(url, location.href)));
globalThis.requestAnimationFrame = (f) => setTimeout(() => f(performance.now()), 0);

// Sounds: what was played, and what's still playing.
const played = [];
class FakeAudio {
  constructor(src) {
    this.src = src;
    this.volume = 1;
    this.loop = false;
    this.paused = true;
  }
  play() {
    this.paused = false;
    played.push(this);
    return Promise.resolve();
  }
  pause() {
    this.paused = true;
  }
}
globalThis.Audio = window.Audio = FakeAudio;

const game = `format: eifgb/0.1
game: { id: sounds, title: Sounds, version: "1", start: hall }
rooms:
  hall: { name: Hall, description: A fire crackles., sound_file: fire.ogg, exits: { north: yard } }
  yard: { name: Yard, exits: { south: hall } }
goals:
  out: { text: Go outside, done: 'player:is_in(world.yard)' }
timers:
  bell: { every: 2, say: A bell rings., sound_file: bell.mp3 }
`;
const files = { "game.yaml": game, "fire.ogg": "data:audio/ogg;base64,AAAA", "bell.mp3": "data:audio/mpeg;base64,AAAA" };
localStorage.setItem("eifgb:local", JSON.stringify(["sounds"]));
localStorage.setItem("eifgb:local:sounds", JSON.stringify({ key: "sounds", title: "Sounds", files, added: 1 }));

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

// Embedded: a link to a full page, not back to the site; no map to start with.
assert.ok(document.body.classList.contains("embedded"));
const back = $("header .back");
assert.equal(back.textContent, "Full page ↗");
assert.equal(back.target, "_blank");
assert.ok(!back.href.includes("embed"), back.href);
assert.ok(back.href.includes("local=sounds"), back.href);
assert.equal($("#map").hidden, true, "the map starts closed in a frame");

// Sound is off until the player turns it on, and they're told the game has some.
assert.equal(played.length, 0);
assert.match($("#log").textContent, /This game has sounds\. To hear them, turn on Sound/);
type("wait");
assert.equal(played.length, 0, "nothing plays with sound off");
localStorage.setItem("eifgb:settings", JSON.stringify({ sound: "on" }));
dispatchEvent(new Event("eifgb-sound"));
await tick(10);
const fire = played.find((a) => a.src.startsWith("data:audio/ogg"));
assert.ok(fire && fire.loop, "the hall's own sound, over and over");
type("wait");
type("wait");
assert.ok(played.some((a) => a.src.startsWith("data:audio/mpeg") && !a.loop), "the bell, once");
// Out of the hall, its sound stops.
type("north");
await tick(700);
assert.equal(fire.paused, true, "the fire's sound fades out in the yard");
assert.equal(played.filter((a) => a.loop).length, 1, "the yard has no sound of its own");

// The notes panel: the journal beside the story, kept up to date.
assert.equal($("#notes").hidden, true);
$("#notes-open").click();
assert.equal($("#notes").hidden, false);
assert.equal($("#notes-open").getAttribute("aria-pressed"), "true");
assert.match($("#notes-body").textContent, /Go outside/);
type("note the code is 4512");
assert.match($("#notes-body").textContent, /the code is 4512/, "a note shows straight away");
const turns = $("#status").textContent;
$("#notes-close").click();
$("#notes-open").click();
assert.equal($("#status").textContent, turns, "reading the notes takes no turn");
assert.ok(!localStorage.getItem("eifgb:notes-open"), "in a frame, panels aren't remembered for the site");
type("erase notes");
assert.doesNotMatch($("#notes-body").textContent, /4512/);

// Narrow screens: one panel at a time.
window.matchMedia = (q) => ({ matches: !q.includes("min-width"), addEventListener() {}, removeEventListener() {} });
$("#map-open").click();
assert.equal($("#map").hidden, false);
assert.equal($("#notes").hidden, true, "opening the map on a narrow screen closes the notes");

// The embed snippet.
const { embedCode, embedUrl } = await import("../src/embed.ts");
assert.equal(embedUrl("https://x.example/play.html?game=a%20b"), "https://x.example/play.html?game=a+b&embed=1");
const code = embedCode("https://x.example/play.html?example=cave", 'The "Cave" <1>');
assert.match(code, /^<iframe src="https:\/\/x\.example\/play\.html\?example=cave&amp;embed=1" title="The &quot;Cave&quot; &lt;1>"/);

console.log("player extras: ok");
process.exit(0);
