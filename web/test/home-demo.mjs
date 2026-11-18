// The home page's demo, in a simulated browser: its opening (written into
// the HTML, so it shows before the engine loads) matches what the engine
// says, and it can be played to its ending. Run: node test/home-demo.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";

const web = fileURLToPath(new URL("..", import.meta.url));


// The opening on the page is the engine's own.
const yaml = readFileSync(`${web}src/demo/game.yaml`, "utf8");
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const opening = engine.load({ "game.yaml": yaml }).output.lines.filter((l) => l.kind !== "banner").map((l) => l.text);
const html = readFileSync(`${web}index.html`, "utf8");
const shown = [...html.match(/<div id="demo-log"[^>]*>([\s\S]*?)<\/div>/)[1].matchAll(/<p[^>]*>([^<]*)<\/p>/g)].map((m) => m[1]);
// The engine also lists exits; the page leaves that line for the engine's first reply.
assert.deepEqual(shown, opening.filter((l) => !l.startsWith("Exits:")), "index.html's demo opening matches the game");

// Played on the page.
GlobalRegistrator.register({ url: pathToFileURL(`${web}public/index.html`).href });
window.fetch = async () => new Response("[]");
document.body.innerHTML = html.match(/<body[^>]*>([\s\S]*)<\/body>/)[1].replace(/<script[\s\S]*?<\/script>/g, "");
await import("../src/home.ts");
const line = document.getElementById("demo-line");
const say = async (text) => {
  line.value = text;
  document.getElementById("demo-form").dispatchEvent(new Event("submit", { cancelable: true }));
  await new Promise((r) => setTimeout(r, 50));
};
line.dispatchEvent(new Event("focus"));
await new Promise((r) => setTimeout(r, 300));
for (const c of ["open drawer", "take key", "unlock door", "open door", "north"]) await say(c);
const log = document.getElementById("demo-log").textContent;
assert.match(log, /In the desk drawer you can see a brass key/);
assert.match(log, /You're out\. That's interactive fiction/);
assert.equal(document.getElementById("demo-form").hidden, true);
assert.ok(document.querySelector(".demo .after a[href='./editor.html']"));
console.log("ok   home demo");
await GlobalRegistrator.unregister();
process.exit(0);
