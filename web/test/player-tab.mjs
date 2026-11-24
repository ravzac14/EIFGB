// Tab in the player: finishing a word from what the player can see, going
// through several matches, Esc, and Tab moving on when there's nothing to
// finish. Run: node test/player-tab.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";

const game = `
format: eifgb/0.1
game: { id: tabtest, title: Tab test, version: "1", start: hall }
rooms:
  hall: { name: Hall, description: A hall. }
objects:
  lantern: { name: brass lantern, synonyms: [lamp], in: hall, traits: [portable] }
  ladder: { name: ladder, in: hall }
  ledge: { name: ledge, in: hall, traits: [scenery] }
`;
const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
assert.ok(engine.load({ "game.yaml": game }, "game.yaml", 1).output);
assert.deepEqual(engine.complete("take la"), ["ladder", "lamp", "lantern"]);
assert.deepEqual(engine.complete("take br"), ["brass"]);

GlobalRegistrator.register();
await import("../src/tab.js");
document.body.innerHTML = `<form id="f"><input id="line"></form>`;
const line = document.getElementById("line");
globalThis.eifgbTab(line, (t) => engine.complete(t));
const hint = document.querySelector(".tab-options");
const tab = (shiftKey = false) => {
  const e = new KeyboardEvent("keydown", { key: "Tab", shiftKey, cancelable: true, bubbles: true });
  line.dispatchEvent(e);
  return e.defaultPrevented;
};
const type = (v) => {
  line.value = v;
  line.dispatchEvent(new Event("input", { bubbles: true }));
};

// One match: finished, with a space after it.
type("take br");
assert.equal(tab(), true);
assert.equal(line.value, "take brass ");
assert.equal(hint.hidden, true);

// Several: the first, all shown, Tab and Shift+Tab go through them.
type("take la");
tab();
assert.equal(line.value, "take ladder");
assert.equal(hint.hidden, false);
assert.equal(hint.textContent.trim().replace(/\s+/g, " "), "ladder lamp lantern");
assert.equal(hint.querySelector(".on").textContent, "ladder");
tab();
assert.equal(line.value, "take lamp");
tab();
tab();
assert.equal(line.value, "take ladder", "round again");
tab(true);
assert.equal(line.value, "take lantern", "Shift+Tab goes back");
// Typing on keeps the choice; the options go.
type("take lantern ");
assert.equal(hint.hidden, true);

// Esc puts back what was typed.
type("take la");
tab();
line.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
assert.equal(line.value, "take la");

// Capitals kept.
type("TAKE BR");
tab();
assert.equal(line.value, "TAKE BRASS ");

// Nothing to finish (or nothing typed): Tab moves on, as usual.
type("take zz");
assert.equal(tab(), false);
type("");
assert.equal(tab(), false);

await GlobalRegistrator.unregister();
console.log("ok   player tab");
