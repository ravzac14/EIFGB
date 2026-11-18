// Checks how dropped or picked files become a project. Run: node test/files.mjs
import assert from "node:assert/strict";
import { zipSync, strToU8 } from "fflate";
import { normalize, fromFileList } from "../src/files.ts";

const m = (o) => new Map(Object.entries(o));

// A folder: game.yaml at the root of the project, other files relative to it.
assert.deepEqual(normalize(m({ "mygame/game.yaml": "g", "mygame/scripts/a.lua": "l", "mygame/notes.txt": "x", "mygame/.git/config.yaml": "no" })), {
  "game.yaml": "g",
  "scripts/a.lua": "l",
});
// The shallowest game.yaml wins; a lone .yaml file is the game.
assert.deepEqual(normalize(m({ "a/game.yaml": "top", "a/b/game.yaml": "deep" })), { "game.yaml": "top", "b/game.yaml": "deep" });
assert.deepEqual(normalize(m({ "story.yml": "s" })), { "game.yaml": "s" });
// Mistakes say what's wrong.
assert.match(normalize(m({ "a.lua": "x" })), /no game\.yaml/);
assert.match(normalize(m({ "a.yaml": "x", "b.yaml": "y" })), /more than one/);
// Windows paths and zip junk.
assert.deepEqual(normalize(m({ "g\\game.yaml": "g", "__MACOSX/g/game.yaml": "junk" })), { "game.yaml": "g" });

// A zip.
const zip = zipSync({ "campfire/game.yaml": strToU8("z"), "campfire/scripts/fire.lua": strToU8("f") });
const file = new File([zip], "campfire.zip");
assert.deepEqual(await fromFileList([file]), { "game.yaml": "z", "scripts/fire.lua": "f" });
console.log("ok   files");
