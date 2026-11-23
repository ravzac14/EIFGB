// Games from links, and the gallery's checks. Run: node test/remote.mjs
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { zipSync, strToU8 } from "fflate";
import { resolve, fetchGame } from "../src/remote.ts";
import { loadEngine } from "../src/engine.ts";
import { check } from "../scripts/bundle-gallery.mjs";

// ------------------------------------------------------- resolving links
const raw = "https://raw.githubusercontent.com";
assert.deepEqual(resolve("https://github.com/ann/cave"), { kind: "yaml", url: `${raw}/ann/cave/HEAD/game.yaml` });
assert.deepEqual(resolve("https://github.com/ann/cave/tree/main/games/one"), { kind: "yaml", url: `${raw}/ann/cave/main/games/one/game.yaml` });
assert.deepEqual(resolve("https://github.com/ann/cave/blob/main/game.yaml"), { kind: "yaml", url: `${raw}/ann/cave/main/game.yaml` });
assert.deepEqual(resolve("https://github.com/ann/cave/blob/v1/cave.zip"), { kind: "zip", url: `${raw}/ann/cave/v1/cave.zip` });
assert.deepEqual(resolve("https://gist.github.com/ann/0123456789abcdef0123"), { kind: "gist", id: "0123456789abcdef0123" });
assert.deepEqual(resolve("https://example.com/games/cave/"), { kind: "yaml", url: "https://example.com/games/cave/game.yaml" });
assert.deepEqual(resolve("https://example.com/games/cave"), { kind: "yaml", url: "https://example.com/games/cave/game.yaml" });
assert.deepEqual(resolve("https://example.com/cave.yml"), { kind: "yaml", url: "https://example.com/cave.yml" });
assert.throws(() => resolve("http://example.com/game.yaml"), /https/);
assert.throws(() => resolve("not a link"), /web address/);
assert.throws(() => resolve("https://github.com/ann/cave/issues/3"), /doesn't point at a game/);

// --------------------------------------------------- fetching for real
const examples = fileURLToPath(new URL("../../spec/examples/", import.meta.url));
const campfire = {};
const walk = (d) => {
  for (const n of readdirSync(d)) {
    const p = join(d, n);
    if (statSync(p).isDirectory()) walk(p);
    else campfire[relative(join(examples, "campfire"), p).split("\\").join("/")] = readFileSync(p, "utf8");
  }
};
walk(join(examples, "campfire"));
const zip = zipSync(Object.fromEntries(Object.entries(campfire).map(([p, t]) => [`campfire/${p}`, strToU8(t)])));
const server = createServer((req, res) => {
  if (req.url === "/campfire.zip") return res.end(Buffer.from(zip));
  const p = join(examples, decodeURIComponent(req.url ?? "/"));
  if (!p.startsWith(examples) || !statSync(p, { throwIfNoEntry: false })?.isFile()) return (res.statusCode = 404), res.end();
  res.end(readFileSync(p));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const base = `http://127.0.0.1:${server.address().port}`;
try {
  const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
  // A folder link: game.yaml, and the scripts it lists, from beside it.
  const files = await fetchGame(`${base}/campfire/`);
  assert.deepEqual(Object.keys(files).sort(), ["game.yaml", "scripts/camp.lua"]);
  assert.equal(engine.check(files).diagnostics.filter((d) => d.severity === "error").length, 0);
  // A zip.
  // A zip: its walkthrough comes too.
  const zipped = await fetchGame(`${base}/campfire.zip`);
  assert.deepEqual(Object.keys(zipped).sort(), ["game.yaml", "scripts/camp.lua", "walkthrough.txt"]);
  assert.equal(engine.load(zipped).meta.title, "Campfire");
  // Nothing there.
  await assert.rejects(fetchGame(`${base}/nowhere/`), /Nothing was found/);
  console.log("ok   remote games");
} finally {
  server.close();
}

// --------------------------------------------------------- the gallery
const good = `games:\n  - { title: Cave, author: Ann, url: "https://github.com/ann/cave", about: A cave., tags: [short] }\n`;
assert.deepEqual(check(good).errors, []);
const { errors } = check(
  `games:\n  - { title: Cave, author: Ann, url: "http://x.com", about: A cave., tags: [scary], colour: red }\n  - { title: Two, author: B, about: x }\n  - { title: Cave2, author: Ann, url: "https://github.com/ann/cave", about: A cave. }\n  - { title: Cave3, author: Ann, url: "https://github.com/ann/cave", about: Again. }\n`,
);
const all = errors.join("\n");
for (const m of [/unknown field `colour`/, /must start with https/, /unknown tag `scary`/, /`url` is missing/, /already in the gallery/]) assert.match(all, m);
assert.deepEqual(check(readFileSync(new URL("../gallery.yaml", import.meta.url), "utf8")).errors, []);
console.log("ok   gallery");

// --------------------------- links into the docs' safety section exist
const sharing = readFileSync(new URL("../public/docs/sharing.html", import.meta.url), "utf8");
for (const f of ["../gallery.html", "../src/play.ts"]) {
  const src = readFileSync(new URL(f, import.meta.url), "utf8");
  const anchor = src.match(/docs\/sharing\.html#([\w-]+)/)?.[1];
  assert.ok(anchor, `${f} links to the safety section`);
  assert.ok(sharing.includes(`id="${anchor}"`), `${f} links to #${anchor}, which exists`);
}
assert.match(sharing, /class="callout warning"[^>]*><p class="callout-label">Warning</);
console.log("ok   safety links");
