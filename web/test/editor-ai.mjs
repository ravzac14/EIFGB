// Making a game with an AI assistant: copying the instructions, opening a
// game from its reply, sending the problems back, and pasting a fixed
// reply over the game. Run: node test/editor-ai.mjs
import "./vite-imports.mjs";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

const web = fileURLToPath(new URL("..", import.meta.url));
GlobalRegistrator.register({ url: pathToFileURL(`${web}public/editor.html`).href, width: 1200, height: 800 });
Element.prototype.setPointerCapture ??= () => {};
window.confirm = () => true;
globalThis.CSS ??= { escape: (s) => s };
// The guide, as the site serves it; and a clipboard to read back.
window.fetch = async (url) => new Response(readFileSync(new URL(url, location.href)));
let clipboard = "";
Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => void (clipboard = t) }, configurable: true });
const errors = [];
window.addEventListener("error", (e) => errors.push(e.error ?? e.message));

const html = readFileSync(`${web}editor.html`, "utf8");
document.body.innerHTML = html.match(/<body[^>]*>([\s\S]*)<\/body>/)[1].replace(/<script[\s\S]*?<\/script>/g, "");
await import("../src/editor/main.ts");
const $ = (s) => document.querySelector(s);
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));
const project = () => JSON.parse(localStorage.getItem(`eifgb:local:${new URLSearchParams(location.search).get("project")}`)).files;

// 1. The instructions, with the guide in them.
$("#ai-copy").click();
await tick(50);
assert.match(clipboard, /^I'd like your help writing a text adventure/);
assert.match(clipboard, /Here is the guide to the format:\n\n# Writing games with an AI assistant/);
assert.match($("#ai-msg").textContent, /^Copied/);

// 2. Its reply, chat and all: opened as a game, with its walkthrough.
$("#ai-open").click();
assert.match($("#ai-msg").textContent, /There's no game in that/);
const game = "format: eifgb/0.1\ngame: { id: t, title: The Shed, version: '1', start: shed }\nrooms:\n  shed: { name: Shed, exits: { out: yard } }\n  yard: { name: Yard, on: { enter: 'end_game(\"out\")' } }\ncharacters:\n  cat: { name: cat, in: shed, talk: cat_chat }\nendings:\n  out: { kind: win, text: Out. }\n";
$("#ai-reply").value = `Here it is!\n\n\`\`\`yaml\n${game}\`\`\`\n\nWalkthrough:\n\n\`\`\`text\nout\n\`\`\`\n`;
$("#ai-open").click();
await tick(600);
assert.equal($("#workspace").hidden, false, "opened");
assert.equal($("#title").textContent, "The Shed");
assert.equal(project()["game.yaml"], game);
assert.equal(project()["walkthrough.txt"], "out\n");

// 3. The problems, as a message to send back.
const forAi = [...document.querySelectorAll("#panel-problems button")].find((b) => b.textContent === "Copy for your AI");
assert.ok(forAi, $("#panel-problems").textContent);
forAi.click();
await tick(20);
assert.match(clipboard, /^The EIFGB checker found 1 error in the game:\n\n- Error at characters\.cat\.talk: /);
assert.match(clipboard, /send the whole game\.yaml again/);

// 4. The fixed reply, pasted over game.yaml: the fences come off.
$("#tab-yaml").click();
const fixed = game.replace(", talk: cat_chat", "");
const paste = new Event("paste", { bubbles: true, cancelable: true });
paste.clipboardData = { getData: () => `Fixed:\n\`\`\`yaml\n${fixed}\`\`\`` };
$("#yaml").dispatchEvent(paste);
assert.equal(paste.defaultPrevented, true);
assert.equal($("#yaml").value, fixed);
await tick(1000);
assert.equal(project()["game.yaml"], fixed);
assert.match($("#tab-problems").textContent, /^Problems$/, "no problems now");

assert.deepEqual(errors, []);
console.log("ok   editor and AI assistants");
await GlobalRegistrator.unregister();
process.exit(0);
