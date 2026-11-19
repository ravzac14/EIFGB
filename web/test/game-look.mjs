// A game's own look (`presentation:`): passed on by the engine, and used
// by the player unless the player picked their own colours.
// Run: node test/game-look.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { loadEngine } from "../src/engine.ts";

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
const game = `
format: eifgb/0.1
game: { id: look, title: Look, version: "1", start: hall }
presentation: { theme: terminal, accent: "#c33" }
rooms:
  hall: { name: Hall }
`;
assert.deepEqual(engine.load({ "game.yaml": game }).meta.presentation, { theme: "terminal", accent: "#c33" });
assert.equal(engine.load({ "game.yaml": game.replace(/presentation.*\n/, "") }).meta.presentation, null);
assert.equal(engine.load({ "game.yaml": game.replace("terminal", "parchment") }).meta.presentation.theme, "parchment");

GlobalRegistrator.register();
const { applySettings, setGameLook, settings } = await import("../src/store.ts");
const html = document.documentElement;
settings.set({ ...settings.get(), theme: "auto" });
setGameLook({ theme: "terminal", accent: "#c33" });
assert.equal(html.dataset.theme, "terminal");
assert.equal(html.style.getPropertyValue("--accent"), "#c33");
// The player's own choice wins, accent and all.
settings.set({ ...settings.get(), theme: "sepia" });
applySettings();
assert.equal(html.dataset.theme, "sepia");
assert.equal(html.style.getPropertyValue("--accent"), "");
// Back on auto, and then a game with no look of its own: the device decides.
settings.set({ ...settings.get(), theme: "auto" });
setGameLook(null);
assert.equal(html.dataset.theme, undefined);
console.log("ok   game look");
