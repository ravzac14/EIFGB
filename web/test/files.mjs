// Checks how dropped or picked files become a project. Run: node test/files.mjs
import assert from "node:assert/strict";
import { zipSync, strToU8 } from "fflate";
import { normalize, fromFileList } from "../src/files.ts";
import { dataBytes, isPicture, pictureSrc, picturesOf } from "../src/pictures.ts";

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
// Pictures come along as data: URLs, and go back out as the same bytes.
const png = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3]);
const withPic = zipSync({ "g/game.yaml": strToU8("z"), "g/pictures/hall.png": png, "g/notes.txt": strToU8("skip") });
const loaded = await fromFileList([new File([withPic], "g.zip")]);
assert.deepEqual(Object.keys(loaded).sort(), ["game.yaml", "pictures/hall.png"]);
assert.match(loaded["pictures/hall.png"], /^data:image\/png;base64,/);
assert.ok(isPicture("pictures/hall.png", loaded["pictures/hall.png"]));
assert.deepEqual(dataBytes(loaded["pictures/hall.png"]), png);
// Where a picture is shown from: the project's copy, or https.
assert.equal(pictureSrc("pictures/hall.png", loaded), loaded["pictures/hall.png"]);
assert.equal(pictureSrc("https://example.com/a.jpg", {}), "https://example.com/a.jpg");
assert.equal(pictureSrc("http://example.com/a.jpg", {}), null);
assert.equal(pictureSrc("pictures/gone.png", loaded), null);
// The pictures a game names, for fetching beside a linked game.
assert.deepEqual(
  await picturesOf("rooms:\n  a: { name: A, picture: a.jpg }\nobjects:\n  p: { name: P, picture: { file: pics/p.png, alt: x } }\n  q: { name: Q, picture: https://x.org/q.png }\n"),
  ["a.jpg", "pics/p.png"],
);
console.log("ok   files");
